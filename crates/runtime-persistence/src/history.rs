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
        "INSERT OR REPLACE INTO history_items (id, session_id, source, user_input, generated_command, executed_command, linked_plan_id, planner_request_id, status, exit_code, created_at, finished_at, duration_ms, cwd, planner_source) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        rusqlite::params![
            item.id, item.session_id, item.source, item.user_input,
            item.generated_command, item.executed_command, item.linked_plan_id,
            item.planner_request_id, item.status, item.exit_code, item.created_at,
            item.finished_at, item.duration_ms, item.cwd, item.planner_source,
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
                "SELECT id, session_id, source, user_input, generated_command, executed_command, linked_plan_id, planner_request_id, status, exit_code, created_at, finished_at, duration_ms, cwd, planner_source FROM history_items WHERE session_id = ?1 ORDER BY created_at DESC LIMIT ?2",
            )
            .map_err(|e| format!("history list: {e}"))?;

        let rows = stmt
            .query_map(rusqlite::params![sid, limit], map_row)
            .map_err(|e| format!("history list: {e}"))?;
        rows.filter_map(|r| r.ok()).collect()
    } else {
        let mut stmt = conn
            .prepare(
                "SELECT id, session_id, source, user_input, generated_command, executed_command, linked_plan_id, planner_request_id, status, exit_code, created_at, finished_at, duration_ms, cwd, planner_source FROM history_items ORDER BY created_at DESC LIMIT ?1",
            )
            .map_err(|e| format!("history list: {e}"))?;

        let rows = stmt
            .query_map(rusqlite::params![limit], map_row)
            .map_err(|e| format!("history list: {e}"))?;
        rows.filter_map(|r| r.ok()).collect()
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
    conn.execute(
        "UPDATE history_items SET status = COALESCE(?1, status), exit_code = COALESCE(?2, exit_code), executed_command = COALESCE(?3, executed_command), finished_at = COALESCE(?4, finished_at), duration_ms = COALESCE(?5, duration_ms) WHERE id = ?6",
        rusqlite::params![status, exit_code, executed_command, finished_at, duration_ms, history_id],
    )
    .map_err(|e| format!("history update: {e}"))?;
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
    })
}
