use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub product_mode: Option<String>,
    pub theme: Option<String>,
    pub font_size: Option<String>,
    pub density: Option<String>,
    pub default_input_mode: Option<String>,
    pub auto_open_plan_panel: Option<bool>,
    pub confirm_medium_risk: Option<bool>,
    pub explanation_verbosity: Option<String>,
    pub reduced_clutter: Option<bool>,
    pub simplified_summaries: Option<bool>,
}

pub fn default_settings() -> SettingsSnapshot {
    SettingsSnapshot {
        product_mode: Some("classic".to_string()),
        theme: Some("dark".to_string()),
        font_size: Some("md".to_string()),
        density: Some("comfortable".to_string()),
        default_input_mode: Some("command".to_string()),
        auto_open_plan_panel: Some(true),
        confirm_medium_risk: Some(true),
        explanation_verbosity: Some("normal".to_string()),
        reduced_clutter: Some(false),
        simplified_summaries: Some(false),
    }
}

pub fn get(conn: &Connection) -> Result<SettingsSnapshot, String> {
    let result: Result<String, _> = conn.query_row(
        "SELECT value_json FROM settings WHERE key = 'app'",
        [],
        |row| row.get(0),
    );

    let settings = match result {
        Ok(json) => serde_json::from_str(&json).unwrap_or_else(|_| default_settings()),
        Err(_) => default_settings(),
    };

    Ok(settings)
}

pub fn update(conn: &Connection, patch: &SettingsSnapshot) -> Result<(), String> {
    // Read current
    let current_json: String = conn
        .query_row(
            "SELECT value_json FROM settings WHERE key = 'app'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| {
            serde_json::to_string(&default_settings()).unwrap_or_else(|_| "{}".to_string())
        });

    let current: serde_json::Value =
        serde_json::from_str(&current_json).unwrap_or(serde_json::Value::Object(Default::default()));
    let patch_value: serde_json::Value =
        serde_json::to_value(patch).unwrap_or(serde_json::Value::Object(Default::default()));

    let merged = merge_json(current, patch_value);
    let merged_str =
        serde_json::to_string(&merged).map_err(|e| format!("settings update: {e}"))?;

    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value_json) VALUES ('app', ?1)",
        rusqlite::params![merged_str],
    )
    .map_err(|e| format!("settings update: {e}"))?;

    Ok(())
}

fn merge_json(base: serde_json::Value, patch: serde_json::Value) -> serde_json::Value {
    match (base, patch) {
        (serde_json::Value::Object(mut base_map), serde_json::Value::Object(patch_map)) => {
            for (key, value) in patch_map {
                if !value.is_null() {
                    let existing = base_map.remove(&key).unwrap_or(serde_json::Value::Null);
                    base_map.insert(key, merge_json(existing, value));
                }
            }
            serde_json::Value::Object(base_map)
        }
        (_, patch) => patch,
    }
}
