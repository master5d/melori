use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct QuestionRow {
    pub id: i64,
    pub source: String,
    pub kind: String,
    pub question: String,
    pub options: Option<String>,
    pub answer: Option<String>,
    pub status: String,
    pub asked_at: i64,
    pub answered_at: Option<i64>,
}

pub struct BridgeStore {
    conn: Mutex<Connection>,
}

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS agent_questions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source TEXT NOT NULL,
    kind TEXT NOT NULL,
    question TEXT NOT NULL,
    options TEXT,
    answer TEXT,
    status TEXT NOT NULL DEFAULT 'pending',
    asked_at INTEGER NOT NULL,
    answered_at INTEGER
);";

impl BridgeStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn insert_question(
        &self,
        source: &str,
        kind: &str,
        question: &str,
        options_json: Option<&str>,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO agent_questions (source, kind, question, options, asked_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![source, kind, question, options_json, now_ms()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn mark_answered(&self, id: i64, answer: &str) -> Result<()> {
        self.set_status(id, "answered", Some(answer))
    }
    pub fn mark_timeout(&self, id: i64) -> Result<()> {
        self.set_status(id, "timeout", None)
    }
    pub fn mark_dismissed(&self, id: i64) -> Result<()> {
        self.set_status(id, "dismissed", None)
    }

    fn set_status(&self, id: i64, status: &str, answer: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let affected = conn.execute(
            "UPDATE agent_questions
             SET status = ?2, answer = ?3, answered_at = ?4 WHERE id = ?1",
            rusqlite::params![id, status, answer, now_ms()],
        )?;
        if affected == 0 {
            anyhow::bail!("question {id} not found");
        }
        Ok(())
    }

    pub fn list_since(&self, since_ms: i64) -> Result<Vec<QuestionRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, source, kind, question, options, answer, status, asked_at, answered_at
             FROM agent_questions WHERE asked_at >= ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map([since_ms], |r| {
                Ok(QuestionRow {
                    id: r.get(0)?,
                    source: r.get(1)?,
                    kind: r.get(2)?,
                    question: r.get(3)?,
                    options: r.get(4)?,
                    answer: r.get(5)?,
                    status: r.get(6)?,
                    asked_at: r.get(7)?,
                    answered_at: r.get(8)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

pub(crate) fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> BridgeStore {
        BridgeStore::open_in_memory().unwrap()
    }

    #[test]
    fn insert_then_answer_roundtrip() {
        let s = mem();
        let id = s
            .insert_question("claude", "text", "Deploy?", None)
            .unwrap();
        s.mark_answered(id, "yes").unwrap();
        let rows = s.list_since(0).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, "answered");
        assert_eq!(rows[0].answer.as_deref(), Some("yes"));
        assert_eq!(rows[0].question, "Deploy?");
    }

    #[test]
    fn set_status_unknown_id_errors() {
        let s = mem();
        assert!(s.mark_answered(999, "x").is_err());
    }

    #[test]
    fn timeout_and_dismiss_statuses() {
        let s = mem();
        let a = s.insert_question("cron", "confirm", "Meds?", None).unwrap();
        let b = s
            .insert_question("cron", "choice", "Mood?", Some(r#"["good","bad"]"#))
            .unwrap();
        s.mark_timeout(a).unwrap();
        s.mark_dismissed(b).unwrap();
        let rows = s.list_since(0).unwrap();
        assert_eq!(rows[0].status, "timeout");
        assert_eq!(rows[1].status, "dismissed");
        assert_eq!(rows[1].options.as_deref(), Some(r#"["good","bad"]"#));
    }
}
