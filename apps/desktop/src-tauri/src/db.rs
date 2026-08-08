use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    analytics::InferenceRun,
    error::{AppError, AppResult},
    models::{
        ActivityEvent, ActivitySource, Dashboard, DashboardRequest, HistoryRequest,
        ProfileDocument, QueryActivityFacts, Recommendation, Settings, UsageItem, UserCorrection,
    },
};

const MIGRATIONS: &[&str] = &[
    r#"
CREATE TABLE activity_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  occurred_at INTEGER NOT NULL,
  ended_at INTEGER,
  duration_seconds INTEGER NOT NULL DEFAULT 0,
  app_name TEXT NOT NULL,
  window_title TEXT,
  url TEXT,
  page_title TEXT,
  search_query TEXT,
  browser_profile_id TEXT,
  source TEXT NOT NULL,
  is_bootstrap INTEGER NOT NULL DEFAULT 0,
  fingerprint TEXT UNIQUE
);
CREATE INDEX activity_time_idx ON activity_events(occurred_at DESC);
CREATE INDEX activity_source_idx ON activity_events(source, occurred_at DESC);
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE chrome_profiles (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL, selected INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE profile_versions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  created_at INTEGER NOT NULL,
  run_day TEXT NOT NULL,
  run_kind TEXT NOT NULL,
  document TEXT NOT NULL,
  UNIQUE(run_day, run_kind)
);
CREATE TABLE user_corrections (
  id TEXT PRIMARY KEY, subject TEXT NOT NULL UNIQUE, value TEXT NOT NULL,
  created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
);
CREATE TABLE recommendations (
  id TEXT PRIMARY KEY, kind TEXT NOT NULL, text TEXT NOT NULL, evidence TEXT NOT NULL,
  dismissed INTEGER NOT NULL DEFAULT 0, feedback TEXT, created_at INTEGER NOT NULL
);
CREATE TABLE refresh_runs (
  run_day TEXT PRIMARY KEY, started_at INTEGER NOT NULL, completed_at INTEGER, status TEXT NOT NULL
);
CREATE TABLE extension_state (
  singleton INTEGER PRIMARY KEY CHECK(singleton=1), pairing_token TEXT NOT NULL, last_seen_at INTEGER
);
"#,
    r#"ALTER TABLE extension_state ADD COLUMN extension_id TEXT;"#,
    r#"
CREATE TABLE inference_runs (
  id TEXT PRIMARY KEY,
  timestamp TEXT NOT NULL,
  model TEXT NOT NULL,
  baseline_input_tokens INTEGER NOT NULL,
  optimized_input_tokens INTEGER NOT NULL,
  tokens_saved INTEGER NOT NULL,
  reduction_percent REAL NOT NULL,
  actual_input_tokens INTEGER,
  output_tokens INTEGER,
  latency_ms INTEGER NOT NULL,
  estimated_cost_usd REAL,
  memory_count INTEGER NOT NULL,
  mode TEXT NOT NULL,
  memory_provider TEXT NOT NULL,
  measurement_method TEXT NOT NULL,
  stored_locally INTEGER NOT NULL DEFAULT 1,
  persistence_error TEXT
);
CREATE INDEX inference_runs_time_idx ON inference_runs(timestamp DESC);
"#,
    r#"
ALTER TABLE inference_runs ADD COLUMN context_budget_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE inference_runs ADD COLUMN context_estimated_tokens INTEGER NOT NULL DEFAULT 0;
ALTER TABLE inference_runs ADD COLUMN context_units_considered INTEGER NOT NULL DEFAULT 0;
ALTER TABLE inference_runs ADD COLUMN context_units_sent INTEGER NOT NULL DEFAULT 0;
ALTER TABLE inference_runs ADD COLUMN context_units_omitted INTEGER NOT NULL DEFAULT 0;
ALTER TABLE inference_runs ADD COLUMN context_detail_level TEXT NOT NULL DEFAULT 'legacy';
ALTER TABLE inference_runs ADD COLUMN provider_preflight_input_tokens INTEGER;
ALTER TABLE inference_runs ADD COLUMN cache_read_input_tokens INTEGER;
ALTER TABLE inference_runs ADD COLUMN cache_write_input_tokens INTEGER;
"#,
    r#"
CREATE TABLE product_events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  event_type TEXT NOT NULL,
  thread_id TEXT,
  occurred_at INTEGER NOT NULL
);
CREATE INDEX product_events_time_idx ON product_events(occurred_at DESC);
CREATE INDEX product_events_type_idx ON product_events(event_type, occurred_at DESC);
"#,
];

pub struct Database {
    connection: Mutex<Connection>,
    path: PathBuf,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700))?;
            }
        }
        let connection = Connection::open(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        let db = Self {
            connection: Mutex::new(connection),
            path,
        };
        db.migrate()?;
        db.ensure_defaults()?;
        Ok(db)
    }

    #[cfg(test)]
    pub fn in_memory() -> AppResult<Self> {
        let connection = Connection::open_in_memory()?;
        let db = Self {
            connection: Mutex::new(connection),
            path: PathBuf::from(":memory:"),
        };
        db.migrate()?;
        db.ensure_defaults()?;
        Ok(db)
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.connection.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn migrate(&self) -> AppResult<()> {
        let mut conn = self.conn();
        let current: usize = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        for (index, sql) in MIGRATIONS.iter().enumerate().skip(current) {
            let transaction = conn.transaction()?;
            transaction.execute_batch(sql)?;
            transaction.pragma_update(None, "user_version", index + 1)?;
            transaction.commit()?;
        }
        Ok(())
    }

    fn ensure_defaults(&self) -> AppResult<()> {
        if self.get_setting::<Settings>("settings")?.is_none() {
            self.set_setting("settings", &Settings::default())?;
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn set_setting<T: serde::Serialize>(&self, key: &str, value: &T) -> AppResult<()> {
        let value = serde_json::to_string(value)?;
        self.conn().execute(
            "INSERT INTO settings(key,value) VALUES(?1,?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting<T: serde::de::DeserializeOwned>(&self, key: &str) -> AppResult<Option<T>> {
        let raw: Option<String> = self
            .conn()
            .query_row("SELECT value FROM settings WHERE key=?1", [key], |r| {
                r.get(0)
            })
            .optional()?;
        raw.map(|value| serde_json::from_str(&value).map_err(AppError::from))
            .transpose()
    }

    pub fn settings(&self) -> AppResult<Settings> {
        Ok(self.get_setting("settings")?.unwrap_or_default())
    }

    pub fn save_settings(&self, settings: &Settings) -> AppResult<()> {
        self.set_setting("settings", settings)
    }

    pub fn insert_event(&self, event: &ActivityEvent, fingerprint: &str) -> AppResult<bool> {
        let conn = self.conn();
        upsert_event(&conn, event, fingerprint)
    }

    pub fn insert_events(&self, events: &[(ActivityEvent, String)]) -> AppResult<usize> {
        if events.is_empty() {
            return Ok(0);
        }

        let mut conn = self.conn();
        let transaction = conn.transaction()?;
        let mut inserted = 0;
        for (event, fingerprint) in events {
            inserted += upsert_event(&transaction, event, fingerprint)? as usize;
        }
        transaction.commit()?;
        Ok(inserted)
    }

    pub fn history(&self, request: &HistoryRequest) -> AppResult<Vec<ActivityEvent>> {
        let search = request.search.as_deref().unwrap_or("");
        let pattern = format!("%{search}%");
        let source = request.source.map(ActivitySource::as_str);
        let limit = request.limit.unwrap_or(250).min(1000);
        let offset = request.offset.unwrap_or(0);
        let conn = self.conn();
        let mut statement = conn.prepare(
            "SELECT id,occurred_at,ended_at,duration_seconds,app_name,window_title,url,page_title,
                    search_query,browser_profile_id,source,is_bootstrap
             FROM activity_events
             WHERE occurred_at BETWEEN ?1 AND ?2
               AND (?3='' OR app_name LIKE ?4 OR COALESCE(window_title,'') LIKE ?4
                    OR COALESCE(url,'') LIKE ?4 OR COALESCE(page_title,'') LIKE ?4)
               AND (?5 IS NULL OR source=?5)
             ORDER BY occurred_at DESC LIMIT ?6 OFFSET ?7",
        )?;
        let rows = statement.query_map(
            params![
                request.start_at,
                request.end_at,
                search,
                pattern,
                source,
                limit,
                offset
            ],
            map_activity,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::from)
    }

    pub fn dashboard(&self, request: &DashboardRequest) -> AppResult<Dashboard> {
        let conn = self.conn();
        let total: i64 = conn.query_row(
            "SELECT COALESCE(SUM(duration_seconds),0) FROM activity_events
             WHERE occurred_at BETWEEN ?1 AND ?2
               AND source IN ('app_focus','chrome_extension')",
            params![request.start_at, request.end_at],
            |r| r.get(0),
        )?;
        let focused: i64 = conn.query_row(
            "SELECT COALESCE(SUM(duration_seconds),0) FROM activity_events
             WHERE occurred_at BETWEEN ?1 AND ?2
               AND source IN ('app_focus','chrome_extension')
               AND duration_seconds>=300",
            params![request.start_at, request.end_at],
            |r| r.get(0),
        )?;
        let aggregate = |expression: &str, source_filter: &str| -> AppResult<Vec<UsageItem>> {
            let denominator_sql = format!(
                "SELECT COALESCE(SUM(duration_seconds),0) FROM activity_events
                 WHERE occurred_at BETWEEN ?1 AND ?2
                   AND {source_filter}
                   AND {expression} IS NOT NULL"
            );
            let denominator: i64 = conn.query_row(
                &denominator_sql,
                params![request.start_at, request.end_at],
                |row| row.get(0),
            )?;
            let sql = format!(
                "SELECT {expression},SUM(duration_seconds) AS seconds FROM activity_events
                 WHERE occurred_at BETWEEN ?1 AND ?2
                   AND {source_filter}
                   AND {expression} IS NOT NULL
                 GROUP BY {expression} ORDER BY seconds DESC LIMIT 25"
            );
            let mut statement = conn.prepare(&sql)?;
            let rows = statement.query_map(params![request.start_at, request.end_at], |r| {
                let seconds: i64 = r.get(1)?;
                Ok(UsageItem {
                    key: r.get(0)?,
                    seconds,
                    percentage: if denominator == 0 {
                        0.0
                    } else {
                        seconds as f64 * 100.0 / denominator as f64
                    },
                })
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        };
        let applications = aggregate("app_name", "source IN ('app_focus','chrome_extension')")?;
        let websites = aggregate(
            "CASE WHEN url LIKE 'http%' THEN
               replace(substr(url,instr(url,'//')+2,
                 CASE WHEN instr(substr(url,instr(url,'//')+2),'/')=0
                   THEN length(url)
                   ELSE instr(substr(url,instr(url,'//')+2),'/')-1 END),'www.','')
             END",
            "source IN ('chrome_history','chrome_extension')",
        )?;
        drop(conn);
        Ok(Dashboard {
            total_seconds: total,
            focused_seconds: focused,
            applications,
            websites,
            recommendations: self.recommendations(false)?,
        })
    }

    pub fn purge_expired(&self, now: i64, include_bootstrap: bool) -> AppResult<usize> {
        let cutoff = now - 30 * 86_400;
        let sql = if include_bootstrap {
            "DELETE FROM activity_events WHERE occurred_at < ?1"
        } else {
            "DELETE FROM activity_events WHERE occurred_at < ?1 AND is_bootstrap=0"
        };
        Ok(self.conn().execute(sql, [cutoff])?)
    }

    pub fn profile(&self) -> AppResult<ProfileDocument> {
        let raw: Option<String> = self
            .conn()
            .query_row(
                "SELECT document FROM profile_versions ORDER BY created_at DESC,id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(raw
            .map(|v| serde_json::from_str(&v))
            .transpose()?
            .unwrap_or_default())
    }

    pub fn update_latest_profile(&self, profile: &ProfileDocument) -> AppResult<()> {
        let changed = self.conn().execute(
            "UPDATE profile_versions SET document=?1
             WHERE id=(SELECT id FROM profile_versions ORDER BY created_at DESC,id DESC LIMIT 1)",
            [serde_json::to_string(profile)?],
        )?;
        if changed == 0 {
            return Err(AppError::InvalidInput(
                "Generate a profile before editing its summary.".into(),
            ));
        }
        Ok(())
    }

    pub fn upsert_correction(&self, correction: &UserCorrection) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO user_corrections(id,subject,value,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(subject) DO UPDATE SET value=excluded.value,updated_at=excluded.updated_at",
            params![
                correction.id,
                correction.subject,
                correction.value,
                correction.created_at,
                correction.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn corrections(&self) -> AppResult<Vec<UserCorrection>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id,subject,value,created_at,updated_at FROM user_corrections ORDER BY subject",
        )?;
        let values = stmt
            .query_map([], |r| {
                Ok(UserCorrection {
                    id: r.get(0)?,
                    subject: r.get(1)?,
                    value: r.get(2)?,
                    created_at: r.get(3)?,
                    updated_at: r.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(values)
    }

    pub fn remove_correction(&self, id: &str) -> AppResult<()> {
        self.conn()
            .execute("DELETE FROM user_corrections WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn commit_profile_refresh(
        &self,
        profile: &ProfileDocument,
        recommendations: &[Recommendation],
        settings: &Settings,
        run_day: &str,
        run_kind: &str,
        bootstrap_cutoff: Option<i64>,
    ) -> AppResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO profile_versions(created_at,run_day,run_kind,document)
             VALUES(?1,?2,?3,?4)
             ON CONFLICT(run_day,run_kind) DO UPDATE SET
               created_at=excluded.created_at, document=excluded.document",
            params![
                Utc::now().timestamp(),
                run_day,
                run_kind,
                serde_json::to_string(profile)?
            ],
        )?;
        tx.execute("DELETE FROM recommendations WHERE dismissed=0", [])?;
        for item in recommendations {
            tx.execute(
                "INSERT OR REPLACE INTO recommendations
                 (id,kind,text,evidence,dismissed,feedback,created_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    item.id,
                    item.kind,
                    item.text,
                    item.evidence,
                    item.dismissed,
                    item.feedback,
                    item.created_at
                ],
            )?;
        }
        if let Some(cutoff) = bootstrap_cutoff {
            tx.execute(
                "DELETE FROM activity_events WHERE is_bootstrap=1 AND occurred_at < ?1",
                [cutoff],
            )?;
        }
        tx.execute(
            "INSERT INTO settings(key,value) VALUES('settings',?1)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            [serde_json::to_string(settings)?],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn recommendations(&self, include_dismissed: bool) -> AppResult<Vec<Recommendation>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id,kind,text,evidence,dismissed,feedback,created_at FROM recommendations
             WHERE ?1 OR dismissed=0 ORDER BY created_at DESC",
        )?;
        let values = stmt
            .query_map([include_dismissed], |r| {
                Ok(Recommendation {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    text: r.get(2)?,
                    evidence: r.get(3)?,
                    dismissed: r.get(4)?,
                    feedback: r.get(5)?,
                    created_at: r.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(values)
    }

    pub fn dismiss_recommendation(&self, id: &str, feedback: Option<&str>) -> AppResult<()> {
        self.conn().execute(
            "UPDATE recommendations SET dismissed=1,feedback=?2 WHERE id=?1",
            params![id, feedback],
        )?;
        Ok(())
    }

    pub fn profile_digest(&self, since: i64) -> AppResult<serde_json::Value> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT app_name,COALESCE(window_title,''),COALESCE(url,''),SUM(duration_seconds),COUNT(*)
             FROM activity_events WHERE occurred_at>=?1
             GROUP BY app_name,window_title,url ORDER BY SUM(duration_seconds) DESC LIMIT 200",
        )?;
        let entries = stmt
            .query_map([since], |r| {
                Ok(serde_json::json!({
                    "app": r.get::<_,String>(0)?,
                    "title": redact(&r.get::<_,String>(1)?),
                    "site": domain_only(&r.get::<_,String>(2)?),
                    "seconds": r.get::<_,i64>(3)?,
                    "occurrences": r.get::<_,i64>(4)?
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(serde_json::json!({ "activitySummary": entries }))
    }

    pub fn chat_activity_summary(
        &self,
        start_at: i64,
        end_at: i64,
    ) -> AppResult<serde_json::Value> {
        let conn = self.conn();
        let tracked_seconds: i64 = conn.query_row(
            "SELECT COALESCE(SUM(duration_seconds),0) FROM activity_events
             WHERE occurred_at BETWEEN ?1 AND ?2
               AND source IN ('app_focus','chrome_extension')",
            params![start_at, end_at],
            |row| row.get(0),
        )?;
        let sustained_seconds: i64 = conn.query_row(
            "SELECT COALESCE(SUM(duration_seconds),0) FROM activity_events
             WHERE occurred_at BETWEEN ?1 AND ?2
               AND source IN ('app_focus','chrome_extension')
               AND duration_seconds>=300",
            params![start_at, end_at],
            |row| row.get(0),
        )?;

        let mut app_statement = conn.prepare(
            "SELECT app_name,SUM(duration_seconds),COUNT(*) FROM activity_events
             WHERE occurred_at BETWEEN ?1 AND ?2
               AND source IN ('app_focus','chrome_extension')
             GROUP BY app_name ORDER BY SUM(duration_seconds) DESC LIMIT 15",
        )?;
        let applications = app_statement
            .query_map(params![start_at, end_at], |row| {
                Ok(serde_json::json!({
                    "name":row.get::<_,String>(0)?,
                    "seconds":row.get::<_,i64>(1)?,
                    "sessions":row.get::<_,i64>(2)?
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let domain_expression = "replace(substr(url,instr(url,'//')+2,
               CASE WHEN instr(substr(url,instr(url,'//')+2),'/')=0
                 THEN length(url)
                 ELSE instr(substr(url,instr(url,'//')+2),'/')-1 END),'www.','')";
        let mut live_site_statement = conn.prepare(&format!(
            "SELECT {domain_expression},SUM(duration_seconds),COUNT(*) FROM activity_events
             WHERE occurred_at BETWEEN ?1 AND ?2
               AND source='chrome_extension' AND url LIKE 'http%'
             GROUP BY {domain_expression} ORDER BY SUM(duration_seconds) DESC LIMIT 15"
        ))?;
        let live_sites = live_site_statement
            .query_map(params![start_at, end_at], |row| {
                Ok(serde_json::json!({
                    "domain":row.get::<_,String>(0)?,
                    "seconds":row.get::<_,i64>(1)?,
                    "sessions":row.get::<_,i64>(2)?
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let live_browser_seconds = live_sites
            .iter()
            .filter_map(|site| site.get("seconds").and_then(serde_json::Value::as_i64))
            .sum::<i64>();

        let mut history_site_statement = conn.prepare(&format!(
            "SELECT {domain_expression},COUNT(*) FROM activity_events
             WHERE occurred_at BETWEEN ?1 AND ?2
               AND source='chrome_history' AND url LIKE 'http%'
             GROUP BY {domain_expression} ORDER BY COUNT(*) DESC LIMIT 15"
        ))?;
        let historical_sites = history_site_statement
            .query_map(params![start_at, end_at], |row| {
                Ok(serde_json::json!({
                    "domain":row.get::<_,String>(0)?,
                    "visits":row.get::<_,i64>(1)?
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut editor_statement = conn.prepare(
            "SELECT app_name,COALESCE(window_title,''),MAX(occurred_at),COUNT(*)
             FROM activity_events
             WHERE occurred_at BETWEEN ?1 AND ?2 AND source='editor_history'
             GROUP BY app_name,window_title
             ORDER BY MAX(occurred_at) DESC LIMIT 20",
        )?;
        let recent_editor_changes = editor_statement
            .query_map(params![start_at, end_at], |row| {
                Ok(serde_json::json!({
                    "editor":row.get::<_,String>(0)?,
                    "resource":row.get::<_,String>(1)?,
                    "lastChangedAt":row.get::<_,i64>(2)?,
                    "changes":row.get::<_,i64>(3)?
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(serde_json::json!({
            "startAt":start_at,
            "endAt":end_at,
            "trackedSeconds":tracked_seconds,
            "sustainedSeconds":sustained_seconds,
            "liveBrowserSeconds":live_browser_seconds,
            "applicationTime":applications,
            "liveWebsiteTime":live_sites,
            "historicalWebsiteVisits":historical_sites,
            "recentEditorChanges":recent_editor_changes
        }))
    }

    pub fn query_activity_facts(
        &self,
        query: &str,
        start_at: i64,
        end_at: i64,
    ) -> AppResult<Vec<QueryActivityFacts>> {
        let terms = meaningful_query_terms(query);
        let searches = if terms.is_empty() && activity_metric_intent(query) {
            vec![None]
        } else {
            terms.into_iter().map(Some).collect::<Vec<_>>()
        };
        if searches.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.conn();
        let mut facts = Vec::new();
        for term in searches {
            let mut sql = String::from(
                "SELECT occurred_at,ended_at,duration_seconds,source,
                        lower(COALESCE(app_name,'') || ' ' || COALESCE(window_title,'') || ' ' ||
                              COALESCE(url,'') || ' ' || COALESCE(page_title,'') || ' ' ||
                              COALESCE(search_query,''))
                 FROM activity_events
                 WHERE (
                   (source IN ('app_focus','chrome_extension')
                    AND occurred_at<=?2
                    AND MAX(COALESCE(ended_at,occurred_at),
                            occurred_at + MAX(duration_seconds,0))>=?1)
                   OR
                   (source NOT IN ('app_focus','chrome_extension')
                    AND occurred_at BETWEEN ?1 AND ?2)
                 )",
            );
            if term.is_some() {
                sql.push_str(
                    " AND lower(COALESCE(app_name,'') || ' ' || COALESCE(window_title,'') || ' ' ||
                     COALESCE(url,'') || ' ' || COALESCE(page_title,'') || ' ' ||
                     COALESCE(search_query,'')) LIKE ?3",
                );
            }
            let mut statement = conn.prepare(&sql)?;
            let pattern = term.as_ref().map(|value| format!("%{value}%"));
            let rows = if let Some(pattern) = pattern.as_deref() {
                statement.query_map(params![start_at, end_at, pattern], map_fact_event)?
            } else {
                statement.query_map(params![start_at, end_at], map_fact_event)?
            };

            let mut matched_events = 0_i64;
            let mut first_seen_at = i64::MAX;
            let mut last_seen_at = i64::MIN;
            let mut active_intervals = Vec::new();
            let mut app_focus_seconds = 0_i64;
            let mut live_browser_seconds = 0_i64;
            let mut historical_visits = 0_i64;
            let mut historical_reported_seconds = 0_i64;
            let mut editor_changes = 0_i64;

            for row in rows {
                let (occurred_at, ended_at, duration_seconds, source, searchable_text) = row?;
                if term
                    .as_deref()
                    .is_some_and(|term| !metadata_contains_query_term(&searchable_text, term))
                {
                    continue;
                }
                matched_events += 1;
                let is_live = matches!(
                    source,
                    ActivitySource::AppFocus | ActivitySource::ChromeExtension
                );
                let observed_start = if is_live {
                    occurred_at.max(start_at)
                } else {
                    occurred_at
                };
                first_seen_at = first_seen_at.min(observed_start);
                let reliable_end = if is_live {
                    ended_at
                        .unwrap_or(occurred_at)
                        .max(occurred_at.saturating_add(duration_seconds.max(0)))
                        .min(end_at)
                } else {
                    occurred_at
                };
                last_seen_at = last_seen_at.max(reliable_end);

                match source {
                    ActivitySource::AppFocus => {
                        app_focus_seconds = app_focus_seconds
                            .saturating_add(reliable_end.saturating_sub(observed_start));
                        active_intervals.push((observed_start, reliable_end));
                    }
                    ActivitySource::ChromeExtension => {
                        live_browser_seconds = live_browser_seconds
                            .saturating_add(reliable_end.saturating_sub(observed_start));
                        active_intervals.push((observed_start, reliable_end));
                    }
                    ActivitySource::ChromeHistory => {
                        historical_visits += 1;
                        historical_reported_seconds =
                            historical_reported_seconds.saturating_add(duration_seconds.max(0));
                    }
                    ActivitySource::EditorHistory => editor_changes += 1,
                }
            }

            if matched_events == 0 {
                continue;
            }
            let subject = term
                .as_deref()
                .map(display_query_subject)
                .unwrap_or_else(|| "All tracked activity".into());
            facts.push(QueryActivityFacts {
                subject,
                match_basis: if term.is_some() {
                    "exact query-term metadata match".into()
                } else {
                    "general activity-time request".into()
                },
                matched_events,
                first_seen_at,
                last_seen_at,
                observed_span_seconds: last_seen_at.saturating_sub(first_seen_at),
                observed_active_seconds: merged_interval_seconds(&mut active_intervals),
                app_focus_seconds,
                live_browser_seconds,
                historical_visits,
                historical_reported_seconds,
                editor_changes,
                modified_files: Vec::new(),
                coverage_start_at: start_at,
                coverage_end_at: end_at,
            });
        }
        facts.sort_by(|left, right| {
            query_subject_priority(&right.subject)
                .cmp(&query_subject_priority(&left.subject))
                .then_with(|| right.matched_events.cmp(&left.matched_events))
                .then_with(|| right.subject.len().cmp(&left.subject.len()))
        });
        facts.truncate(3);
        Ok(facts)
    }

    pub fn record_inference_run(&self, run: &InferenceRun) -> AppResult<()> {
        self.conn().execute(
            "INSERT OR REPLACE INTO inference_runs
             (id,timestamp,model,baseline_input_tokens,optimized_input_tokens,tokens_saved,
              reduction_percent,actual_input_tokens,output_tokens,context_budget_tokens,
              context_estimated_tokens,context_units_considered,context_units_sent,
              context_units_omitted,context_detail_level,provider_preflight_input_tokens,
              cache_read_input_tokens,cache_write_input_tokens,latency_ms,estimated_cost_usd,
              memory_count,mode,memory_provider,measurement_method,stored_locally,persistence_error)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,
                    ?19,?20,?21,?22,?23,?24,?25,?26)",
            params![
                run.id,
                run.timestamp,
                run.model,
                run.baseline_input_tokens,
                run.optimized_input_tokens,
                run.tokens_saved,
                run.reduction_percent,
                run.actual_input_tokens,
                run.output_tokens,
                run.context_budget_tokens,
                run.context_estimated_tokens,
                run.context_units_considered,
                run.context_units_sent,
                run.context_units_omitted,
                run.context_detail_level,
                run.provider_preflight_input_tokens,
                run.cache_read_input_tokens,
                run.cache_write_input_tokens,
                run.latency_ms,
                run.estimated_cost_usd,
                run.memory_count,
                run.mode,
                run.memory_provider,
                run.measurement_method,
                true,
                Option::<&str>::None,
            ],
        )?;
        Ok(())
    }

    pub fn record_product_event(
        &self,
        event_type: &str,
        thread_id: Option<&str>,
        occurred_at: i64,
    ) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO product_events(event_type,thread_id,occurred_at) VALUES(?1,?2,?3)",
            params![event_type, thread_id, occurred_at],
        )?;
        Ok(())
    }

    pub fn delete_all_local_data(&self) -> AppResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        for table in [
            "activity_events",
            "chrome_profiles",
            "profile_versions",
            "user_corrections",
            "recommendations",
            "refresh_runs",
            "inference_runs",
            "product_events",
            "extension_state",
            "settings",
        ] {
            tx.execute(&format!("DELETE FROM {table}"), [])?;
        }
        tx.commit()?;
        drop(conn);
        self.ensure_defaults()
    }

    #[cfg(feature = "chrome-extension")]
    pub fn mark_extension_seen(&self, token: &str, at: i64) -> AppResult<bool> {
        let changed = self.conn().execute(
            "UPDATE extension_state SET last_seen_at=?2 WHERE singleton=1 AND pairing_token=?1",
            params![token, at],
        )?;
        Ok(changed == 1)
    }

    #[cfg(any(feature = "chrome-extension", test))]
    pub fn authenticate_native_extension(
        &self,
        token: &str,
        extension_id: &str,
        at: i64,
    ) -> AppResult<bool> {
        if extension_id.len() != 32
            || !extension_id
                .chars()
                .all(|character| ('a'..='p').contains(&character))
        {
            return Ok(false);
        }
        let conn = self.conn();
        let accepted = conn.execute(
            "UPDATE extension_state SET extension_id=COALESCE(extension_id,?2),last_seen_at=?3
             WHERE singleton=1 AND pairing_token=?1
               AND (extension_id IS NULL OR extension_id=?2)",
            params![token, extension_id, at],
        )?;
        Ok(accepted == 1)
    }

    pub fn extension_state(&self) -> AppResult<Option<(String, Option<i64>)>> {
        Ok(self
            .conn()
            .query_row(
                "SELECT pairing_token,last_seen_at FROM extension_state WHERE singleton=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    pub fn set_pairing_token(&self, token: &str) -> AppResult<()> {
        self.conn().execute(
            "INSERT INTO extension_state(singleton,pairing_token,extension_id,last_seen_at) VALUES(1,?1,NULL,NULL)
             ON CONFLICT(singleton) DO UPDATE SET pairing_token=excluded.pairing_token,
               extension_id=NULL,last_seen_at=NULL",
            [token],
        )?;
        Ok(())
    }

    pub fn selected_profile_paths(&self) -> AppResult<HashMap<String, PathBuf>> {
        let settings = self.settings()?;
        let mut result = HashMap::new();
        let conn = self.conn();
        let mut stmt = conn.prepare("SELECT id,path FROM chrome_profiles WHERE id=?1")?;
        for id in settings.selected_chrome_profiles {
            if let Some(path) = stmt
                .query_row([&id], |r| Ok(PathBuf::from(r.get::<_, String>(1)?)))
                .optional()?
            {
                result.insert(id, path);
            }
        }
        Ok(result)
    }

    pub fn save_chrome_profiles(&self, profiles: &[(String, String, String)]) -> AppResult<()> {
        let mut conn = self.conn();
        let tx = conn.transaction()?;
        for (id, name, path) in profiles {
            tx.execute(
                "INSERT INTO chrome_profiles(id,name,path,selected) VALUES(?1,?2,?3,0)
                 ON CONFLICT(id) DO UPDATE SET name=excluded.name,path=excluded.path",
                params![id, name, path],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

fn upsert_event(conn: &Connection, event: &ActivityEvent, fingerprint: &str) -> AppResult<bool> {
    let existed: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM activity_events WHERE fingerprint=?1)",
        [fingerprint],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO activity_events
         (occurred_at,ended_at,duration_seconds,app_name,window_title,url,page_title,
          search_query,browser_profile_id,source,is_bootstrap,fingerprint)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
         ON CONFLICT(fingerprint) DO UPDATE SET
           ended_at=excluded.ended_at,
           duration_seconds=CASE
             WHEN excluded.source='chrome_history' THEN excluded.duration_seconds
             ELSE MAX(activity_events.duration_seconds,excluded.duration_seconds)
           END,
           page_title=COALESCE(excluded.page_title,activity_events.page_title),
           search_query=COALESCE(excluded.search_query,activity_events.search_query)",
        params![
            event.occurred_at,
            event.ended_at,
            event.duration_seconds.max(0),
            event.app_name,
            event.window_title,
            event.url,
            event.page_title,
            event.search_query,
            event.browser_profile_id,
            event.source.as_str(),
            event.is_bootstrap,
            fingerprint
        ],
    )?;
    Ok(!existed)
}

fn map_activity(row: &rusqlite::Row<'_>) -> rusqlite::Result<ActivityEvent> {
    let source: String = row.get(10)?;
    Ok(ActivityEvent {
        id: row.get(0)?,
        occurred_at: row.get(1)?,
        ended_at: row.get(2)?,
        duration_seconds: row.get(3)?,
        app_name: row.get(4)?,
        window_title: row.get(5)?,
        url: row.get(6)?,
        page_title: row.get(7)?,
        search_query: row.get(8)?,
        browser_profile_id: row.get(9)?,
        source: ActivitySource::try_from(source.as_str()).unwrap_or(ActivitySource::AppFocus),
        is_bootstrap: row.get(11)?,
    })
}

fn map_fact_event(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<(i64, Option<i64>, i64, ActivitySource, String)> {
    let source = row.get::<_, String>(3)?;
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        ActivitySource::try_from(source.as_str()).unwrap_or(ActivitySource::AppFocus),
        row.get(4)?,
    ))
}

fn metadata_contains_query_term(text: &str, term: &str) -> bool {
    let tokens = text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect::<HashSet<_>>();
    if !tokens.contains(term) {
        return false;
    }
    !(term == "snowflake" && crate::threading::is_natural_snowflake_context(text))
}

fn query_subject_priority(subject: &str) -> usize {
    matches!(
        subject,
        "BigQuery" | "Databricks" | "OpenAI" | "PostgreSQL" | "Snowflake"
    ) as usize
}

fn meaningful_query_terms(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut seen = HashSet::new();
    for term in query
        .split(|character: char| !character.is_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|term| {
            term.len() >= 4
                && !term.chars().all(|character| character.is_numeric())
                && !is_activity_query_stopword(term)
        })
    {
        if seen.insert(term.clone()) {
            terms.push(term);
        }
    }
    terms
}

pub(crate) fn has_meaningful_activity_subject(query: &str) -> bool {
    !meaningful_query_terms(query).is_empty()
}

fn is_activity_query_stopword(value: &str) -> bool {
    matches!(
        value,
        "about"
            | "answer"
            | "activity"
            | "architecture"
            | "been"
            | "browser"
            | "chrome"
            | "could"
            | "days"
            | "does"
            | "doing"
            | "done"
            | "during"
            | "first"
            | "file"
            | "files"
            | "filename"
            | "filenames"
            | "from"
            | "have"
            | "hours"
            | "information"
            | "last"
            | "long"
            | "many"
            | "minutes"
            | "month"
            | "most"
            | "much"
            | "overall"
            | "please"
            | "project"
            | "recent"
            | "recently"
            | "change"
            | "changed"
            | "edit"
            | "edited"
            | "modify"
            | "modified"
            | "save"
            | "saved"
            | "show"
            | "should"
            | "since"
            | "spend"
            | "spent"
            | "start"
            | "started"
            | "tell"
            | "that"
            | "these"
            | "this"
            | "time"
            | "today"
            | "total"
            | "tracked"
            | "touch"
            | "touched"
            | "using"
            | "week"
            | "what"
            | "when"
            | "where"
            | "which"
            | "while"
            | "with"
            | "work"
            | "worked"
            | "working"
            | "would"
            | "your"
            | "yesterday"
    )
}

fn activity_metric_intent(query: &str) -> bool {
    let query = query.to_ascii_lowercase();
    let words = query
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<HashSet<_>>();
    let time_intent = [
        "work", "worked", "working", "spend", "spent", "activity", "tracked",
    ]
    .iter()
    .any(|term| words.contains(term))
        && ["how long", "how much", "time", "hours", "minutes"]
            .iter()
            .any(|term| query.contains(term));
    time_intent || recent_file_activity_intent(&query)
}

pub(crate) fn recent_file_activity_intent(query: &str) -> bool {
    let words = query
        .split(|character: char| !character.is_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|word| !word.is_empty())
        .collect::<HashSet<_>>();
    ["file", "files", "filename", "filenames"]
        .iter()
        .any(|term| words.contains(*term))
        && [
            "work", "worked", "working", "change", "changed", "edit", "edited", "modify",
            "modified", "save", "saved", "touch", "touched", "recent", "recently",
        ]
        .iter()
        .any(|term| words.contains(*term))
}

fn display_query_subject(value: &str) -> String {
    match value {
        "bigquery" => "BigQuery".into(),
        "databricks" => "Databricks".into(),
        "openai" => "OpenAI".into(),
        "postgresql" => "PostgreSQL".into(),
        "snowflake" => "Snowflake".into(),
        _ => {
            let mut characters = value.chars();
            characters
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
                .unwrap_or_default()
        }
    }
}

fn merged_interval_seconds(intervals: &mut [(i64, i64)]) -> i64 {
    intervals.sort_unstable_by_key(|(start, _)| *start);
    let mut total = 0_i64;
    let mut current: Option<(i64, i64)> = None;
    for &(start, end) in intervals.iter() {
        if end <= start {
            continue;
        }
        current = match current {
            None => Some((start, end)),
            Some((current_start, current_end)) if start <= current_end => {
                Some((current_start, current_end.max(end)))
            }
            Some((current_start, current_end)) => {
                total = total.saturating_add(current_end.saturating_sub(current_start));
                Some((start, end))
            }
        };
    }
    if let Some((start, end)) = current {
        total = total.saturating_add(end.saturating_sub(start));
    }
    total
}

fn domain_only(value: &str) -> String {
    url::Url::parse(value)
        .ok()
        .and_then(|u| u.host_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}

fn redact(value: &str) -> String {
    value
        .split_whitespace()
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            let credential_marker = [
                "token=",
                "key=",
                "password=",
                "secret=",
                "bearer",
                "sk-",
                "api_key",
            ]
            .iter()
            .any(|marker| lower.contains(marker));
            let email = word.contains('@') && word.contains('.');
            let local_path = word.starts_with("/Users/") || word.starts_with("~/");
            let long_identifier = word.len() > 36
                && word
                    .chars()
                    .filter(|character| character.is_ascii_alphanumeric())
                    .count()
                    > 30;
            if credential_marker || email || local_path || long_identifier {
                "[redacted]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(180)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(at: i64, bootstrap: bool) -> ActivityEvent {
        ActivityEvent {
            id: None,
            occurred_at: at,
            ended_at: Some(at + 60),
            duration_seconds: 60,
            app_name: "Code".into(),
            window_title: Some("Knov".into()),
            url: None,
            page_title: None,
            search_query: None,
            browser_profile_id: None,
            source: ActivitySource::AppFocus,
            is_bootstrap: bootstrap,
        }
    }

    #[test]
    fn inference_run_persists_context_economics_telemetry() {
        let db = Database::in_memory().unwrap();
        let run = InferenceRun {
            id: "run-context-1".into(),
            timestamp: "2026-08-07T12:00:00Z".into(),
            model: "test-model".into(),
            baseline_input_tokens: 200,
            optimized_input_tokens: 120,
            tokens_saved: 80,
            reduction_percent: 40.0,
            actual_input_tokens: Some(125),
            output_tokens: Some(20),
            context_budget_tokens: 160,
            context_estimated_tokens: 110,
            context_units_considered: 30,
            context_units_sent: 18,
            context_units_omitted: 12,
            context_detail_level: "detailed".into(),
            provider_preflight_input_tokens: Some(128),
            cache_read_input_tokens: Some(48),
            cache_write_input_tokens: Some(16),
            latency_ms: 40,
            estimated_cost_usd: Some(0.02),
            memory_count: 6,
            mode: "optimized".into(),
            memory_provider: "local".into(),
            measurement_method: "test".into(),
        };

        db.record_inference_run(&run).unwrap();

        let persisted = db
            .conn()
            .query_row(
                "SELECT context_budget_tokens, context_estimated_tokens,
                        context_units_considered, context_units_sent, context_units_omitted,
                        context_detail_level, provider_preflight_input_tokens,
                        cache_read_input_tokens, cache_write_input_tokens
                 FROM inference_runs WHERE id=?1",
                [&run.id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(
            persisted,
            (
                160,
                110,
                30,
                18,
                12,
                "detailed".into(),
                Some(128),
                Some(48),
                Some(16)
            )
        );
    }

    #[test]
    fn product_events_are_local_and_removed_by_delete_everything() {
        let db = Database::in_memory().unwrap();
        db.record_product_event("thread_feedback_helpful", Some("knov-implementation"), 42)
            .unwrap();

        let persisted: (String, Option<String>, i64) = db
            .conn()
            .query_row(
                "SELECT event_type,thread_id,occurred_at FROM product_events",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(
            persisted,
            (
                "thread_feedback_helpful".into(),
                Some("knov-implementation".into()),
                42,
            )
        );

        db.delete_all_local_data().unwrap();
        let remaining: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM product_events", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 0);
    }

    #[test]
    fn inference_run_migration_backfills_legacy_context_telemetry() {
        let connection = Connection::open_in_memory().unwrap();
        for (index, sql) in MIGRATIONS.iter().take(3).enumerate() {
            connection.execute_batch(sql).unwrap();
            connection
                .pragma_update(None, "user_version", index + 1)
                .unwrap();
        }
        connection
            .execute(
                "INSERT INTO inference_runs
                 (id,timestamp,model,baseline_input_tokens,optimized_input_tokens,tokens_saved,
                  reduction_percent,latency_ms,memory_count,mode,memory_provider,
                  measurement_method)
                 VALUES('legacy-run','2026-08-06T12:00:00Z','legacy-model',100,50,50,
                        50.0,10,2,'optimized','local','legacy')",
                [],
            )
            .unwrap();
        let db = Database {
            connection: Mutex::new(connection),
            path: PathBuf::from(":memory:"),
        };

        db.migrate().unwrap();

        let migrated = db
            .conn()
            .query_row(
                "SELECT context_budget_tokens, context_estimated_tokens,
                        context_units_considered, context_units_sent, context_units_omitted,
                        context_detail_level, provider_preflight_input_tokens,
                        cache_read_input_tokens, cache_write_input_tokens
                 FROM inference_runs WHERE id='legacy-run'",
                [],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(migrated, (0, 0, 0, 0, 0, "legacy".into(), None, None, None));
    }

    #[test]
    fn migration_defaults_and_deduplication_work() {
        let db = Database::in_memory().unwrap();
        assert!(!db.settings().unwrap().collection_enabled);
        assert!(db.insert_event(&event(100, false), "same").unwrap());
        assert!(!db.insert_event(&event(100, false), "same").unwrap());
    }

    #[test]
    fn reimport_can_correct_an_overstated_chrome_history_duration() {
        let db = Database::in_memory().unwrap();
        let mut imported = event(100, false);
        imported.source = ActivitySource::ChromeHistory;
        imported.duration_seconds = 50_000;
        db.insert_event(&imported, "history-correction").unwrap();

        imported.duration_seconds = 120;
        db.insert_event(&imported, "history-correction").unwrap();

        let corrected = db
            .history(&HistoryRequest {
                start_at: 0,
                end_at: 1_000,
                search: None,
                source: Some(ActivitySource::ChromeHistory),
                limit: None,
                offset: None,
            })
            .unwrap();
        assert_eq!(corrected[0].duration_seconds, 120);
    }

    #[test]
    fn dashboard_separates_imported_duration_from_live_foreground_focus() {
        let db = Database::in_memory().unwrap();
        let mut collector = event(100, false);
        collector.duration_seconds = 600;
        db.insert_event(&collector, "collector").unwrap();

        let mut imported = event(200, false);
        imported.duration_seconds = 900;
        imported.source = ActivitySource::ChromeHistory;
        db.insert_event(&imported, "history").unwrap();

        let mut chrome = event(300, false);
        chrome.duration_seconds = 300;
        chrome.source = ActivitySource::ChromeExtension;
        db.insert_event(&chrome, "extension").unwrap();

        db.insert_event(&event(400, false), "short-collector")
            .unwrap();

        let dashboard = db
            .dashboard(&DashboardRequest {
                start_at: 0,
                end_at: 1_000,
            })
            .unwrap();

        assert_eq!(dashboard.total_seconds, 960);
        assert_eq!(dashboard.focused_seconds, 900);
        assert_eq!(
            dashboard
                .applications
                .iter()
                .map(|item| item.seconds)
                .sum::<i64>(),
            960
        );
    }

    #[test]
    fn chat_summary_separates_live_youtube_time_from_historical_visits() {
        let db = Database::in_memory().unwrap();
        let mut live_youtube = event(100, false);
        live_youtube.app_name = "Google Chrome".into();
        live_youtube.url = Some("https://www.youtube.com/watch?v=live".into());
        live_youtube.duration_seconds = 600;
        live_youtube.source = ActivitySource::ChromeExtension;
        db.insert_event(&live_youtube, "live-youtube").unwrap();

        let mut historical_youtube = event(200, false);
        historical_youtube.app_name = "Google Chrome".into();
        historical_youtube.url = Some("https://www.youtube.com/watch?v=history".into());
        historical_youtube.duration_seconds = 50_000;
        historical_youtube.source = ActivitySource::ChromeHistory;
        db.insert_event(&historical_youtube, "historical-youtube")
            .unwrap();

        let summary = db.chat_activity_summary(0, 1_000).unwrap();

        assert_eq!(summary["trackedSeconds"], 600);
        assert_eq!(summary["liveBrowserSeconds"], 600);
        assert_eq!(summary["liveWebsiteTime"][0]["domain"], "youtube.com");
        assert_eq!(summary["liveWebsiteTime"][0]["seconds"], 600);
        assert_eq!(
            summary["historicalWebsiteVisits"][0]["domain"],
            "youtube.com"
        );
        assert_eq!(summary["historicalWebsiteVisits"][0]["visits"], 1);
    }

    #[test]
    fn chat_summary_includes_recent_editor_metadata_without_counting_it_as_focus_time() {
        let db = Database::in_memory().unwrap();
        let mut editor_change = event(300, false);
        editor_change.app_name = "Visual Studio Code".into();
        editor_change.window_title = Some("Knov — src/platform.rs".into());
        editor_change.page_title = Some("src/platform.rs".into());
        editor_change.duration_seconds = 0;
        editor_change.ended_at = None;
        editor_change.source = ActivitySource::EditorHistory;
        db.insert_event(&editor_change, "editor-change").unwrap();

        let summary = db.chat_activity_summary(0, 1_000).unwrap();

        assert_eq!(summary["trackedSeconds"], 0);
        assert_eq!(
            summary["recentEditorChanges"][0]["editor"],
            "Visual Studio Code"
        );
        assert_eq!(
            summary["recentEditorChanges"][0]["resource"],
            "Knov — src/platform.rs"
        );
        assert_eq!(summary["recentEditorChanges"][0]["changes"], 1);
    }

    #[test]
    fn query_activity_facts_are_compact_and_separate_span_from_observed_time() {
        let db = Database::in_memory().unwrap();

        let mut historical = event(100, false);
        historical.app_name = "Google Chrome".into();
        historical.page_title = Some("Snowflake architecture".into());
        historical.url = Some("https://app.snowflake.com/example/secret".into());
        historical.duration_seconds = 50_000;
        historical.source = ActivitySource::ChromeHistory;
        db.insert_event(&historical, "snowflake-history").unwrap();

        let mut live_browser = event(200, false);
        live_browser.app_name = "Google Chrome".into();
        live_browser.search_query = Some("Snowflake architecture".into());
        live_browser.duration_seconds = 600;
        live_browser.source = ActivitySource::ChromeExtension;
        db.insert_event(&live_browser, "snowflake-live").unwrap();

        let mut editor = event(300, false);
        editor.app_name = "Cursor".into();
        editor.window_title = Some("Knov — src/snowflake_client.rs".into());
        editor.duration_seconds = 0;
        editor.ended_at = None;
        editor.source = ActivitySource::EditorHistory;
        db.insert_event(&editor, "snowflake-editor").unwrap();

        let mut app = event(400, false);
        app.window_title = Some("Snowflake hackathon notes".into());
        app.duration_seconds = 300;
        db.insert_event(&app, "snowflake-app").unwrap();

        let mut unrelated = event(500, false);
        unrelated.window_title = Some("Slack".into());
        db.insert_event(&unrelated, "unrelated").unwrap();

        let facts = db
            .query_activity_facts("How long have I been working on snowflake?", 0, 1_000)
            .unwrap();

        assert_eq!(facts.len(), 1);
        let facts = &facts[0];
        assert_eq!(facts.subject, "Snowflake");
        assert_eq!(facts.matched_events, 4);
        assert_eq!(facts.first_seen_at, 100);
        assert_eq!(facts.last_seen_at, 800);
        assert_eq!(facts.observed_span_seconds, 700);
        assert_eq!(facts.observed_active_seconds, 600);
        assert_eq!(facts.app_focus_seconds, 300);
        assert_eq!(facts.live_browser_seconds, 600);
        assert_eq!(facts.historical_visits, 1);
        assert_eq!(facts.historical_reported_seconds, 50_000);
        assert_eq!(facts.editor_changes, 1);

        let serialized = serde_json::to_string(facts).unwrap();
        for forbidden in [
            "example/secret",
            "windowTitle",
            "pageTitle",
            "searchQuery",
            "browserProfileId",
        ] {
            assert!(!serialized.contains(forbidden));
        }
    }

    #[test]
    fn query_activity_facts_support_general_time_questions_without_dumping_rows() {
        let db = Database::in_memory().unwrap();
        db.insert_event(&event(100, false), "activity").unwrap();

        let facts = db
            .query_activity_facts("How long have I been working today?", 0, 1_000)
            .unwrap();

        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].subject, "All tracked activity");
        assert_eq!(facts[0].matched_events, 1);
    }

    #[test]
    fn query_activity_facts_support_recent_file_questions() {
        let db = Database::in_memory().unwrap();
        let mut code = event(100, false);
        code.app_name = "Code".into();
        db.insert_event(&code, "code-activity").unwrap();

        let facts = db
            .query_activity_facts("Which files did I work on most recently?", 0, 1_000)
            .unwrap();

        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].subject, "All tracked activity");
        assert_eq!(facts[0].matched_events, 1);
        assert!(recent_file_activity_intent(
            "Which files did I work on most recently?"
        ));
        assert!(!recent_file_activity_intent(
            "Explain what a configuration file is."
        ));
    }

    #[test]
    fn query_activity_facts_do_not_mix_snowflake_weather_with_the_company() {
        let db = Database::in_memory().unwrap();
        let mut company = event(100, false);
        company.page_title = Some("Snowflake data warehouse".into());
        db.insert_event(&company, "company").unwrap();

        let mut weather = event(200, false);
        weather.page_title = Some("How a snowflake forms in winter weather".into());
        db.insert_event(&weather, "weather").unwrap();

        let mut photography = event(300, false);
        photography.page_title = Some("Snowflake macro photography".into());
        db.insert_event(&photography, "photography").unwrap();

        let mut storm = event(400, false);
        storm.page_title = Some("Snowflake forecast during a snow storm".into());
        db.insert_event(&storm, "storm").unwrap();

        let facts = db
            .query_activity_facts("How long did I use Snowflake?", 0, 1_000)
            .unwrap();

        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].matched_events, 1);
        assert_eq!(facts[0].first_seen_at, 100);
    }

    #[test]
    fn query_activity_facts_include_live_sessions_overlapping_the_range_start() {
        let db = Database::in_memory().unwrap();
        let mut session = event(50, false);
        session.window_title = Some("Snowflake worksheet".into());
        session.ended_at = Some(150);
        session.duration_seconds = 100;
        db.insert_event(&session, "cross-boundary").unwrap();

        let facts = db
            .query_activity_facts("Snowflake time today", 100, 200)
            .unwrap();

        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].first_seen_at, 100);
        assert_eq!(facts[0].last_seen_at, 150);
        assert_eq!(facts[0].observed_active_seconds, 50);
    }

    #[test]
    fn query_activity_facts_match_metadata_tokens_not_substrings() {
        let db = Database::in_memory().unwrap();
        let mut javascript = event(100, false);
        javascript.window_title = Some("JavaScript documentation".into());
        db.insert_event(&javascript, "javascript").unwrap();

        let facts = db
            .query_activity_facts("How much Java time?", 0, 1_000)
            .unwrap();

        assert!(facts.is_empty());
    }

    #[test]
    fn retention_preserves_temporary_bootstrap_until_profile_success() {
        let db = Database::in_memory().unwrap();
        let now = 100 * 86_400;
        db.insert_event(&event(now - 60 * 86_400, true), "old-bootstrap")
            .unwrap();
        db.insert_event(&event(now - 60 * 86_400, false), "old-normal")
            .unwrap();
        db.purge_expired(now, false).unwrap();
        let all = db
            .history(&HistoryRequest {
                start_at: 0,
                end_at: now,
                search: None,
                source: None,
                limit: None,
                offset: None,
            })
            .unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].is_bootstrap);
        db.commit_profile_refresh(
            &ProfileDocument::default(),
            &[],
            &db.settings().unwrap(),
            "1970-04-11",
            "bootstrap",
            Some(now - 30 * 86_400),
        )
        .unwrap();
        assert!(db
            .history(&HistoryRequest {
                start_at: 0,
                end_at: now,
                search: None,
                source: None,
                limit: None,
                offset: None,
            })
            .unwrap()
            .is_empty());
    }

    #[test]
    fn native_extension_identity_is_bound_on_first_authenticated_request() {
        let db = Database::in_memory().unwrap();
        db.set_pairing_token("secret").unwrap();
        let first = "abcdefghijklmnopabcdefghijklmnop";
        let other = "ponmlkjihgfedcbaponmlkjihgfedcba";
        assert!(db
            .authenticate_native_extension("secret", first, 10)
            .unwrap());
        assert!(db
            .authenticate_native_extension("secret", first, 11)
            .unwrap());
        assert!(!db
            .authenticate_native_extension("secret", other, 12)
            .unwrap());
        assert!(!db
            .authenticate_native_extension("wrong", first, 13)
            .unwrap());
    }
}
