use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub session_id: String,
    pub source: String,
    pub user_input: String,
    pub generated_command: Option<String>,
    pub executed_command: Option<String>,
    pub linked_plan_id: Option<String>,
    pub planner_request_id: Option<String>,
    pub status: String,
    pub exit_code: Option<i32>,
    pub created_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub cwd: Option<String>,
    pub planner_source: Option<String>,
    pub workflow_run_id: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRow {
    pub id: String,
    pub session_id: String,
    pub user_intent: String,
    pub command: String,
    pub risk: String,
    pub explanation: String,
    pub generated_at: String,
}

pub fn append(conn: &Connection, item: &HistoryItem) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO history_items (id, session_id, source, user_input, generated_command, executed_command, linked_plan_id, planner_request_id, status, exit_code, created_at, finished_at, duration_ms, cwd, planner_source, workflow_run_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        rusqlite::params![
            item.id, item.session_id, item.source, item.user_input,
            item.generated_command, item.executed_command, item.linked_plan_id,
            item.planner_request_id, item.status, item.exit_code, item.created_at,
            item.finished_at, item.duration_ms, item.cwd, item.planner_source,
            item.workflow_run_id,
        ],
    ).map_err(|e| format!("history append: {e}"))?;
    Ok(())
}

pub fn list(
    conn: &Connection,
    session_id: Option<&str>,
    limit: u32,
) -> Result<Vec<HistoryItem>, String> {
    let items = if let Some(sid) = session_id {
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, source, user_input, generated_command, executed_command, linked_plan_id, planner_request_id, status, exit_code, created_at, finished_at, duration_ms, cwd, planner_source, workflow_run_id FROM history_items WHERE session_id = ?1 ORDER BY created_at DESC LIMIT ?2",
            )
            .map_err(|e| format!("history list: {e}"))?;

        let rows = stmt
            .query_map(rusqlite::params![sid, limit], map_row)
            .map_err(|e| format!("history list: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("history list: {e}"))?
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, source, user_input, generated_command, executed_command, linked_plan_id, planner_request_id, status, exit_code, created_at, finished_at, duration_ms, cwd, planner_source, workflow_run_id FROM history_items ORDER BY created_at DESC LIMIT ?1",
            )
            .map_err(|e| format!("history list: {e}"))?;

        let rows = stmt
            .query_map(rusqlite::params![limit], map_row)
            .map_err(|e| format!("history list: {e}"))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("history list: {e}"))?
    };

    Ok(items)
}

pub fn update(
    conn: &Connection,
    history_id: &str,
    status: Option<&str>,
    exit_code: Option<i32>,
    executed_command: Option<&str>,
    finished_at: Option<&str>,
    duration_ms: Option<i64>,
) -> Result<(), String> {
    let changed = conn.execute(
        "UPDATE history_items SET status = COALESCE(?1, status), exit_code = COALESCE(?2, exit_code), executed_command = COALESCE(?3, executed_command), finished_at = COALESCE(?4, finished_at), duration_ms = COALESCE(?5, duration_ms) WHERE id = ?6",
        rusqlite::params![status, exit_code, executed_command, finished_at, duration_ms, history_id],
    )
    .map_err(|e| format!("history update: {e}"))?;
    if changed == 0 {
        return Err(format!("history update: no history item with id {history_id}"));
    }
    Ok(())
}

pub fn store_plan(conn: &Connection, plan: &PlanRow) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO plans (id, session_id, user_intent, command, risk, explanation, generated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![plan.id, plan.session_id, plan.user_intent, plan.command, plan.risk, plan.explanation, plan.generated_at],
    ).map_err(|e| format!("plan store: {e}"))?;
    Ok(())
}

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<HistoryItem> {
    Ok(HistoryItem {
        id: row.get(0)?,
        session_id: row.get(1)?,
        source: row.get(2)?,
        user_input: row.get(3)?,
        generated_command: row.get(4)?,
        executed_command: row.get(5)?,
        linked_plan_id: row.get(6)?,
        planner_request_id: row.get(7)?,
        status: row.get(8)?,
        exit_code: row.get(9)?,
        created_at: row.get(10)?,
        finished_at: row.get(11)?,
        duration_ms: row.get(12)?,
        cwd: row.get(13)?,
        planner_source: row.get(14)?,
        workflow_run_id: row.get(15)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::init_schema;
    use rusqlite::Connection;

    fn open() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    fn item(id: &str, session: &str, created: &str) -> HistoryItem {
        HistoryItem {
            id: id.into(),
            session_id: session.into(),
            source: "shell".into(),
            user_input: "list the directory".into(),
            generated_command: Some("Get-ChildItem".into()),
            executed_command: None,
            linked_plan_id: Some("plan-1".into()),
            planner_request_id: Some("req-1".into()),
            status: "running".into(),
            exit_code: None,
            created_at: created.into(),
            finished_at: None,
            duration_ms: None,
            cwd: Some("/work".into()),
            planner_source: Some("mock".into()),
            workflow_run_id: Some("run-1".into()),
        }
    }

    #[test]
    fn append_list_update_and_plan_round_trip() {
        let conn = open();
        let mut first = item("h1", "s1", "2026-01-01T00:00:00Z");
        first.generated_command = None;
        first.linked_plan_id = None;
        first.planner_request_id = None;
        first.cwd = None;
        first.planner_source = None;
        append(&conn, &first).unwrap();
        append(&conn, &item("h2", "s1", "2026-01-02T00:00:00Z")).unwrap();
        append(&conn, &item("h3", "s2", "2026-01-03T00:00:00Z")).unwrap();

        let all = list(&conn, None, 2).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, "h3");
        assert_eq!(all[1].id, "h2");
        assert_eq!(all[0].cwd.as_deref(), Some("/work"));
        assert_eq!(all[0].planner_source.as_deref(), Some("mock"));
        assert_eq!(all[0].workflow_run_id.as_deref(), Some("run-1"));

        let session = list(&conn, Some("s1"), 10).unwrap();
        assert_eq!(session.len(), 2);
        assert!(session.iter().all(|row| row.session_id == "s1"));
        let bare = session.iter().find(|row| row.id == "h1").unwrap();
        assert!(bare.generated_command.is_none());
        assert!(bare.cwd.is_none());

        assert!(list(&conn, Some("missing"), 10).unwrap().is_empty());

        update(&conn, "h2", Some("done"), None, None, None, None).unwrap();
        let kept = list(&conn, Some("s1"), 10)
            .unwrap()
            .into_iter()
            .find(|row| row.id == "h2")
            .unwrap();
        assert_eq!(kept.status, "done");
        assert!(kept.exit_code.is_none());
        assert!(kept.executed_command.is_none());

        update(
            &conn,
            "h2",
            Some("finished"),
            Some(0),
            Some("Get-ChildItem"),
            Some("2026-01-02T00:00:01Z"),
            Some(12),
        )
        .unwrap();
        let done = list(&conn, Some("s1"), 10)
            .unwrap()
            .into_iter()
            .find(|row| row.id == "h2")
            .unwrap();
        assert_eq!(done.status, "finished");
        assert_eq!(done.exit_code, Some(0));
        assert_eq!(done.executed_command.as_deref(), Some("Get-ChildItem"));
        assert_eq!(done.finished_at.as_deref(), Some("2026-01-02T00:00:01Z"));
        assert_eq!(done.duration_ms, Some(12));

        expect_err(
            update(&conn, "missing", Some("done"), Some(1), None, None, None),
            "no history item",
        );

        let plan = PlanRow {
            id: "plan-1".into(),
            session_id: "s1".into(),
            user_intent: "list the directory".into(),
            command: "Get-ChildItem".into(),
            risk: "low".into(),
            explanation: "lists the working directory".into(),
            generated_at: "2026-01-02T00:00:00Z".into(),
        };
        store_plan(&conn, &plan).unwrap();
        store_plan(
            &conn,
            &PlanRow {
                command: "Get-ChildItem -Force".into(),
                ..plan
            },
        )
        .unwrap();
        let command: String = conn
            .query_row("SELECT command FROM plans WHERE id = 'plan-1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(command, "Get-ChildItem -Force");
    }

    #[test]
    fn a_row_that_cannot_map_is_an_error() {
        let conn = open();
        append(&conn, &item("good", "s1", "2026-01-01T00:00:00Z")).unwrap();
        append(&conn, &item("bad", "s1", "2026-01-01T00:00:01Z")).unwrap();
        conn.execute(
            "UPDATE history_items SET status = ?1 WHERE id = 'bad'",
            [rusqlite::types::Value::Blob(vec![0xff])],
        )
        .unwrap();
        expect_err(list(&conn, None, 10), "history list");
        expect_err(list(&conn, Some("s1"), 10), "history list");
    }

    fn expect_err<T>(result: Result<T, String>, needle: &str) {
        match result {
            Err(err) => assert!(err.contains(needle), "{err}"),
            Ok(_) => panic!("expected an error containing {needle}"),
        }
    }

    #[test]
    fn missing_tables_are_errors() {
        let conn = Connection::open_in_memory().unwrap();
        let sample = item("h1", "s1", "2026-01-01T00:00:00Z");
        expect_err(append(&conn, &sample), "history append");
        expect_err(list(&conn, None, 10), "history list");
        expect_err(list(&conn, Some("s1"), 10), "history list");
        expect_err(
            update(&conn, "h1", Some("done"), None, None, None, None),
            "history update",
        );
        let plan = PlanRow {
            id: "p".into(),
            session_id: "s".into(),
            user_intent: "i".into(),
            command: "c".into(),
            risk: "low".into(),
            explanation: "e".into(),
            generated_at: "t".into(),
        };
        expect_err(store_plan(&conn, &plan), "plan store");
    }
}
