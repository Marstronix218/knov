use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

use crate::{
    error::{AppError, AppResult},
    models::{
        ActivityEvent, ActivitySource, Dashboard, DashboardRequest, HistoryRequest,
        ProfileDocument, Recommendation, Settings, UsageItem, UserCorrection,
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
        let applications = aggregate(
            "app_name",
            "source IN ('app_focus','chrome_extension')",
        )?;
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

        let domain_expression =
            "replace(substr(url,instr(url,'//')+2,
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

        Ok(serde_json::json!({
            "startAt":start_at,
            "endAt":end_at,
            "trackedSeconds":tracked_seconds,
            "sustainedSeconds":sustained_seconds,
            "liveBrowserSeconds":live_browser_seconds,
            "applicationTime":applications,
            "liveWebsiteTime":live_sites,
            "historicalWebsiteVisits":historical_sites
        }))
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
            "extension_state",
            "settings",
        ] {
            tx.execute(&format!("DELETE FROM {table}"), [])?;
        }
        tx.commit()?;
        drop(conn);
        self.ensure_defaults()
    }

    pub fn mark_extension_seen(&self, token: &str, at: i64) -> AppResult<bool> {
        let changed = self.conn().execute(
            "UPDATE extension_state SET last_seen_at=?2 WHERE singleton=1 AND pairing_token=?1",
            params![token, at],
        )?;
        Ok(changed == 1)
    }

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
