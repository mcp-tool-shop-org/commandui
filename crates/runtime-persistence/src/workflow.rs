use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    pub id: String,
    pub label: String,
    pub source: String,
    pub original_intent: Option<String>,
    pub command: String,
    pub steps_json: Option<String>,
    pub project_root: Option<String>,
    pub created_at: String,
}

pub fn add(conn: &Connection, wf: &Workflow) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO workflows (id, label, source, original_intent, command, steps_json, project_root, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![wf.id, wf.label, wf.source, wf.original_intent, wf.command, wf.steps_json, wf.project_root, wf.created_at],
    ).map_err(|e| format!("workflow add: {e}"))?;
    Ok(())
}

pub fn list(conn: &Connection) -> Result<Vec<Workflow>, String> {
    let mut stmt = conn
        .prepare("SELECT id, label, source, original_intent, command, steps_json, project_root, created_at FROM workflows ORDER BY created_at DESC")
        .map_err(|e| format!("workflow list: {e}"))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Workflow {
                id: row.get(0)?,
                label: row.get(1)?,
                source: row.get(2)?,
                original_intent: row.get(3)?,
                command: row.get(4)?,
                steps_json: row.get(5)?,
                project_root: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| format!("workflow list: {e}"))?;
    let workflows = rows.filter_map(|r| r.ok()).collect();

    Ok(workflows)
}

pub fn delete(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM workflows WHERE id = ?1",
        rusqlite::params![id],
    )
    .map_err(|e| format!("workflow delete: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::init_schema;
    use rusqlite::Connection;

    fn sample(id: &str, created: &str) -> Workflow {
        Workflow {
            id: id.into(),
            label: format!("label-{id}"),
            source: "ask".into(),
            original_intent: Some("ship the patch".into()),
            command: "git status".into(),
            steps_json: Some("[\"git status\"]".into()),
            project_root: Some("/work".into()),
            created_at: created.into(),
        }
    }

    #[test]
    fn add_lists_newest_first_and_delete_removes_one() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        let mut older = sample("w1", "2026-01-01T00:00:00Z");
        older.original_intent = None;
        older.steps_json = None;
        older.project_root = None;
        add(&conn, &older).unwrap();
        add(&conn, &sample("w2", "2026-01-02T00:00:00Z")).unwrap();

        let listed = list(&conn).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, "w2");
        assert_eq!(listed[0].steps_json.as_deref(), Some("[\"git status\"]"));
        assert!(listed[1].original_intent.is_none());
        assert!(listed[1].steps_json.is_none());
        assert!(listed[1].project_root.is_none());

        delete(&conn, "w2").unwrap();
        delete(&conn, "missing").unwrap();
        let listed = list(&conn).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "w1");
    }

    #[test]
    fn a_row_that_cannot_map_is_skipped() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        add(&conn, &sample("good", "2026-01-01T00:00:00Z")).unwrap();
        add(&conn, &sample("bad", "2026-01-02T00:00:00Z")).unwrap();
        conn.execute(
            "UPDATE workflows SET label = ?1 WHERE id = 'bad'",
            [rusqlite::types::Value::Blob(vec![0xff])],
        )
        .unwrap();
        let listed = list(&conn).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "good");
    }

    fn expect_err<T>(result: Result<T, String>, needle: &str) {
        match result {
            Err(err) => assert!(err.contains(needle), "{err}"),
            Ok(_) => panic!("expected an error containing {needle}"),
        }
    }

    #[test]
    fn missing_table_is_an_error() {
        let conn = Connection::open_in_memory().unwrap();
        let wf = sample("w", "t");
        expect_err(add(&conn, &wf), "workflow add");
        expect_err(list(&conn), "workflow list");
        expect_err(delete(&conn, "w"), "workflow delete");
    }
}
