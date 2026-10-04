use rusqlite::{params, OptionalExtension};

use crate::{
    db::Database,
    error::{AppError, AppResult},
};

use super::models::{
    CalibrationBin, PredictionHistoryItem, PredictionResource, PredictionStats, WorkPrediction,
};

impl Database {
    pub(super) fn insert_prediction(
        &self,
        value: &WorkPrediction,
        batch_id: &str,
        rank: i64,
        state: &str,
    ) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO predictions (id,batch_id,rank,created_at,prediction_source,predicted_intent,
             predicted_action,predicted_resource_type,predicted_resource_label,predicted_resource_locator,
             predicted_thread_id,confidence,horizon_minutes,reasoning_summary,evidence_json,
             sanitized_state_summary,evaluation_status,predicted_goal,predicted_workflow_id) VALUES
             (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,'pending',?17,?18)",
            params![value.id,batch_id,rank,value.created_at,value.source,value.intent,value.next_action,
                value.next_resource.as_ref().map(|r| &r.resource_type),value.next_resource.as_ref().map(|r| &r.label),
                value.next_resource.as_ref().and_then(|r| r.safe_locator.as_deref()),value.thread_id,value.confidence,
                value.horizon_minutes,value.reasoning_summary,serde_json::to_string(&value.evidence)?,state,
                value.goal,value.workflow_id])?;
        Ok(())
    }

    pub fn prediction_history(&self, limit: u32) -> AppResult<Vec<PredictionHistoryItem>> {
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT id,created_at,prediction_source,predicted_intent,predicted_action,
            predicted_resource_type,predicted_resource_label,predicted_resource_locator,predicted_thread_id,
            confidence,horizon_minutes,reasoning_summary,evidence_json,evaluation_status,match_score,user_feedback,
            observed_outcome,feedback_reason,evaluated_at,predicted_goal,predicted_workflow_id FROM predictions ORDER BY created_at DESC,rank ASC LIMIT ?1")?;
        let rows = statement.query_map([limit.min(500)], map_prediction_history)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub(super) fn visible_predictions(
        &self,
        now: i64,
        threshold: f64,
    ) -> AppResult<Vec<WorkPrediction>> {
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT id,created_at,prediction_source,predicted_intent,predicted_action,
            predicted_resource_type,predicted_resource_label,predicted_resource_locator,predicted_thread_id,
            confidence,horizon_minutes,reasoning_summary,evidence_json,evaluation_status,match_score,user_feedback,
            observed_outcome,feedback_reason,evaluated_at,predicted_goal,predicted_workflow_id FROM predictions WHERE prediction_source IN ('provider','workflow')
            AND evaluation_status='pending' AND confidence>=?1 AND created_at+horizon_minutes*60>?2
            AND (user_feedback IS NULL OR user_feedback NOT IN ('dismissed','incorrect'))
            ORDER BY created_at DESC,rank ASC LIMIT 3")?;
        let rows = statement.query_map(
            params![threshold.clamp(0.0, 1.0), now],
            map_prediction_history,
        )?;
        Ok(rows
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(|item| item.prediction)
            .collect())
    }

    pub(super) fn pending_predictions(&self, now: i64) -> AppResult<Vec<PredictionHistoryItem>> {
        let conn = self.conn();
        let mut statement = conn.prepare("SELECT id,created_at,prediction_source,predicted_intent,predicted_action,
            predicted_resource_type,predicted_resource_label,predicted_resource_locator,predicted_thread_id,
            confidence,horizon_minutes,reasoning_summary,evidence_json,evaluation_status,match_score,user_feedback,
            observed_outcome,feedback_reason,evaluated_at,predicted_goal,predicted_workflow_id FROM predictions WHERE evaluation_status='pending'
            AND created_at+horizon_minutes*60<=?1 ORDER BY created_at")?;
        let rows = statement.query_map([now], map_prediction_history)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub(super) fn finish_prediction_evaluation(
        &self,
        id: &str,
        status: &str,
        score: f64,
        outcome: &str,
        now: i64,
    ) -> AppResult<()> {
        self.conn().execute("UPDATE predictions SET evaluation_status=?2,match_score=?3,observed_outcome=?4,evaluated_at=?5 WHERE id=?1",
            params![id,status,score.clamp(0.0,1.0),outcome,now])?;
        Ok(())
    }

    pub fn record_prediction_feedback(
        &self,
        id: &str,
        feedback: &str,
        reason: Option<&str>,
    ) -> AppResult<bool> {
        let changed = self.conn().execute(
            "UPDATE predictions SET user_feedback=?2,feedback_reason=?3 WHERE id=?1",
            params![id, feedback, reason],
        )?;
        Ok(changed == 1)
    }

    pub fn last_prediction_at(&self) -> AppResult<Option<i64>> {
        Ok(self
            .conn()
            .query_row("SELECT MAX(created_at) FROM predictions", [], |row| {
                row.get(0)
            })
            .optional()?
            .flatten())
    }

    pub fn latest_activity_at(&self) -> AppResult<Option<i64>> {
        Ok(self
            .conn()
            .query_row("SELECT MAX(occurred_at) FROM activity_events", [], |row| {
                row.get(0)
            })
            .optional()?
            .flatten())
    }

    pub(super) fn prediction_stats(&self) -> AppResult<PredictionStats> {
        let conn = self.conn();
        let counts = |where_sql: &str| -> AppResult<i64> {
            Ok(conn.query_row(
                &format!("SELECT COUNT(*) FROM predictions WHERE {where_sql}"),
                [],
                |row| row.get(0),
            )?)
        };
        let accuracy = |where_sql: &str| -> AppResult<Option<f64>> {
            Ok(conn.query_row(&format!("SELECT AVG(CASE WHEN evaluation_status='matched' THEN 1.0 ELSE 0.0 END) FROM predictions WHERE evaluation_status IN ('matched','partial','missed') AND {where_sql}"),[],|row|row.get(0))?)
        };
        Ok(PredictionStats{
            total_predictions:counts("1=1")?, evaluated_predictions:counts("evaluation_status IN ('matched','partial','missed')")?,
            matched:counts("evaluation_status='matched'")?,partial:counts("evaluation_status='partial'")?,missed:counts("evaluation_status='missed'")?,
            provider_top1_accuracy:accuracy("prediction_source='provider' AND rank=1")?,baseline_top1_accuracy:accuracy("prediction_source='heuristic' AND rank=1")?,
            workflow_top1_accuracy:accuracy("prediction_source='workflow' AND rank=1")?,
            high_confidence_accuracy:accuracy("confidence>=0.75")?,
            user_positive_feedback_rate:conn.query_row("SELECT AVG(CASE WHEN user_feedback='correct' THEN 1.0 ELSE 0.0 END) FROM predictions WHERE user_feedback IN ('correct','incorrect')",[],|row|row.get(0))?,
            calibration:CALIBRATION_BINS.iter().map(|(label,min,max)| {
                let (count,mean,observed):(i64,Option<f64>,Option<f64>)=conn.query_row(
                    "SELECT COUNT(*),AVG(confidence),AVG(CASE WHEN evaluation_status='matched' THEN 1.0 ELSE 0.0 END)
                     FROM predictions WHERE evaluation_status IN ('matched','partial','missed')
                       AND confidence>=?1 AND (confidence<?2 OR ?2>=1.0)",
                    params![min,max],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?)))?;
                Ok(CalibrationBin{label:(*label).into(),min_confidence:*min,max_confidence:*max,count,
                    mean_confidence:mean.map(round3),observed_accuracy:observed.map(round3)})
            }).collect::<AppResult<Vec<_>>>()?,
        })
    }
}

/// Confidence bands used to compare stated confidence with observed outcomes.
const CALIBRATION_BINS: &[(&str, f64, f64)] = &[
    ("Below 50%", 0.0, 0.5),
    ("50–65%", 0.5, 0.65),
    ("65–80%", 0.65, 0.8),
    ("80% and above", 0.8, 1.0),
];

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

fn map_prediction_history(row: &rusqlite::Row<'_>) -> rusqlite::Result<PredictionHistoryItem> {
    let resource_type: Option<String> = row.get(5)?;
    let label: Option<String> = row.get(6)?;
    let created_at: i64 = row.get(1)?;
    let horizon: i64 = row.get(10)?;
    Ok(PredictionHistoryItem {
        prediction: WorkPrediction {
            id: row.get(0)?,
            created_at,
            source: row.get(2)?,
            intent: row.get(3)?,
            next_action: row.get(4)?,
            next_resource: resource_type.zip(label).map(|(resource_type, label)| {
                PredictionResource {
                    resource_type,
                    label,
                    safe_locator: row.get(7).ok().flatten(),
                }
            }),
            thread_id: row.get(8)?,
            confidence: row.get(9)?,
            horizon_minutes: horizon,
            reasoning_summary: row.get(11)?,
            evidence: serde_json::from_str(&row.get::<_, String>(12)?).unwrap_or_default(),
            evaluation_status: row.get(13)?,
            expires_at: created_at + horizon * 60,
            match_score: row.get(14)?,
            user_feedback: row.get(15)?,
            goal: row.get(19)?,
            workflow_id: row.get(20)?,
        },
        observed_outcome: row.get(16)?,
        feedback_reason: row.get(17)?,
        evaluated_at: row.get(18)?,
    })
}
