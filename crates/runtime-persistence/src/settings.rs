use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    #[serde(default)]
    pub product_mode: Option<String>,
    #[serde(default)]
    pub font_size: Option<String>,
    #[serde(default)]
    pub default_input_mode: Option<String>,
    #[serde(default)]
    pub planner_model: Option<String>,
    #[serde(default)]
    pub planner_endpoint: Option<String>,
    #[serde(default)]
    pub simplified_summaries: Option<bool>,
}

pub fn default_settings() -> SettingsSnapshot {
    SettingsSnapshot {
        product_mode: Some("classic".to_string()),
        font_size: Some("md".to_string()),
        default_input_mode: Some("command".to_string()),
        planner_model: Some("qwen2.5:14b".to_string()),
        planner_endpoint: Some("http://localhost:11434".to_string()),
        simplified_summaries: Some(false),
    }
}

fn fill_defaults(settings: SettingsSnapshot) -> SettingsSnapshot {
    let defaults = default_settings();
    SettingsSnapshot {
        product_mode: settings.product_mode.or(defaults.product_mode),
        font_size: settings.font_size.or(defaults.font_size),
        default_input_mode: settings.default_input_mode.or(defaults.default_input_mode),
        planner_model: settings.planner_model.or(defaults.planner_model),
        planner_endpoint: settings.planner_endpoint.or(defaults.planner_endpoint),
        simplified_summaries: settings.simplified_summaries.or(defaults.simplified_summaries),
    }
}

pub fn get(conn: &Connection) -> Result<SettingsSnapshot, String> {
    let result: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT value_json FROM settings WHERE key = 'app'",
        [],
        |row| row.get(0),
    );

    // Only a missing row means "never saved". Any other read error (locked,
    // busy, corrupt) must surface so update() cannot persist defaults over it.
    let settings = match result {
        Ok(json) => serde_json::from_str(&json)
            .map(fill_defaults)
            .unwrap_or_else(|_| default_settings()),
        Err(rusqlite::Error::QueryReturnedNoRows) => default_settings(),
        Err(e) => return Err(format!("settings get: {e}")),
    };

    Ok(settings)
}

pub fn update(conn: &Connection, patch: &SettingsSnapshot) -> Result<(), String> {
    // Read, merge and write under one write lock, or two concurrent partial
    // updates would each merge into the same stale snapshot.
    let tx = rusqlite::Transaction::new_unchecked(conn, rusqlite::TransactionBehavior::Immediate)
        .map_err(|e| format!("settings update: {e}"))?;
    let current = get(&tx)?;
    let current_value =
        serde_json::to_value(&current).map_err(|e| format!("settings update: {e}"))?;
    let patch_value =
        serde_json::to_value(patch).map_err(|e| format!("settings update: {e}"))?;

    let merged = merge_json(current_value, patch_value);
    let parsed: SettingsSnapshot = serde_json::from_value(merged).unwrap_or_else(|_| default_settings());
    let filled = fill_defaults(parsed);
    let merged_str =
        serde_json::to_string(&filled).map_err(|e| format!("settings update: {e}"))?;

    tx.execute(
        "INSERT OR REPLACE INTO settings (key, value_json) VALUES ('app', ?1)",
        rusqlite::params![merged_str],
    )
    .map_err(|e| format!("settings update: {e}"))?;
    tx.commit().map_err(|e| format!("settings update: {e}"))?;

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

    fn raw_json(conn: &Connection) -> String {
        conn.query_row(
            "SELECT value_json FROM settings WHERE key = 'app'",
            [],
            |row| row.get(0),
        )
        .unwrap()
    }

    #[test]
    fn missing_row_loads_the_planner_defaults() {
        let conn = open();
        let settings = get(&conn).unwrap();
        assert_eq!(settings.planner_model.as_deref(), Some("qwen2.5:14b"));
        assert_eq!(settings.planner_endpoint.as_deref(), Some("http://localhost:11434"));
        assert_eq!(settings.font_size.as_deref(), Some("md"));
    }

    #[test]
    fn older_object_drops_settings_that_do_nothing() {
        let conn = open();
        conn.execute(
            "INSERT INTO settings (key, value_json) VALUES ('app', ?1)",
            [r#"{"theme":"light","density":"compact","autoOpenPlanPanel":false,"explanationVerbosity":"brief","reducedClutter":true,"confirmMediumRisk":true,"fontSize":"lg"}"#],
        )
        .unwrap();
        let settings = get(&conn).unwrap();
        assert_eq!(settings.font_size.as_deref(), Some("lg"));
        assert_eq!(settings.planner_model.as_deref(), Some("qwen2.5:14b"));
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"plannerModel\":\"qwen2.5:14b\""), "{json}");
        assert!(json.contains("\"fontSize\":\"lg\""), "{json}");
        for key in [
            "theme",
            "density",
            "autoOpenPlanPanel",
            "explanationVerbosity",
            "reducedClutter",
            "confirmMediumRisk",
        ] {
            assert!(!json.contains(key), "{key} still in {json}");
        }
        assert!(!json.contains("null"), "{json}");
    }

    #[test]
    fn null_planner_model_is_filled() {
        let conn = open();
        conn.execute(
            "INSERT INTO settings (key, value_json) VALUES ('app', ?1)",
            [r#"{"theme":"light","plannerModel":null}"#],
        )
        .unwrap();
        let settings = get(&conn).unwrap();
        assert_eq!(settings.planner_model.as_deref(), Some("qwen2.5:14b"));
        let json = serde_json::to_string(&settings).unwrap();
        assert!(!json.contains("theme"), "{json}");
    }

    #[test]
    fn an_explicit_planner_model_is_kept() {
        let conn = open();
        conn.execute(
            "INSERT INTO settings (key, value_json) VALUES ('app', ?1)",
            [r#"{"plannerModel":"qwen2.5:7b"}"#],
        )
        .unwrap();
        let settings = get(&conn).unwrap();
        assert_eq!(settings.planner_model.as_deref(), Some("qwen2.5:7b"));
        assert_eq!(settings.planner_endpoint.as_deref(), Some("http://localhost:11434"));
    }

    #[test]
    fn invalid_json_falls_back_to_defaults() {
        let conn = open();
        conn.execute(
            "INSERT INTO settings (key, value_json) VALUES ('app', ?1)",
            ["{"],
        )
        .unwrap();
        let settings = get(&conn).unwrap();
        assert_eq!(settings.planner_model.as_deref(), Some("qwen2.5:14b"));
        assert_eq!(settings.font_size.as_deref(), Some("md"));
    }

    #[test]
    fn update_backfills_partial_object_without_nulls() {
        let conn = open();
        conn.execute(
            "INSERT INTO settings (key, value_json) VALUES ('app', ?1)",
            [r#"{"theme":"light"}"#],
        )
        .unwrap();
        let patch = SettingsSnapshot {
            font_size: Some("lg".to_string()),
            product_mode: None,
            default_input_mode: None,
            planner_model: None,
            planner_endpoint: None,
            simplified_summaries: None,
        };
        update(&conn, &patch).unwrap();
        let stored = raw_json(&conn);
        assert!(!stored.contains("null"), "{stored}");
        assert!(stored.contains("\"plannerModel\":\"qwen2.5:14b\""), "{stored}");
        assert!(stored.contains("\"plannerEndpoint\":\"http://localhost:11434\""), "{stored}");
        assert!(!stored.contains("theme"), "{stored}");
        assert!(stored.contains("\"fontSize\":\"lg\""), "{stored}");
        assert_eq!(get(&conn).unwrap().planner_model.as_deref(), Some("qwen2.5:14b"));
    }

    #[test]
    fn update_keeps_an_explicit_false_and_fails_without_the_table() {
        let conn = open();
        // A partial patch must not wipe the fields it leaves out.
        let patch = SettingsSnapshot {
            product_mode: None,
            font_size: None,
            default_input_mode: None,
            planner_model: Some("qwen2.5:7b".into()),
            planner_endpoint: None,
            simplified_summaries: Some(false),
        };
        update(&conn, &patch).unwrap();
        let stored = get(&conn).unwrap();
        assert_eq!(stored.planner_model.as_deref(), Some("qwen2.5:7b"));
        assert_eq!(stored.planner_endpoint.as_deref(), Some("http://localhost:11434"));
        assert_eq!(stored.simplified_summaries, Some(false));
        assert_eq!(stored.font_size.as_deref(), Some("md"));
        let json = serde_json::to_string(&stored).unwrap();
        assert!(!json.contains("theme"), "{json}");

        let bare = Connection::open_in_memory().unwrap();
        let err = get(&bare).err().unwrap();
        assert!(err.contains("settings get"), "{err}");
        let err = update(&bare, &default_settings()).unwrap_err();
        assert!(err.contains("settings"), "{err}");
    }

    #[test]
    fn a_read_error_is_not_treated_as_missing_and_leaves_the_row_alone() {
        let conn = open();
        conn.execute(
            "INSERT INTO settings (key, value_json) VALUES ('app', ?1)",
            [r#"{"theme":"light"}"#],
        )
        .unwrap();
        // A non-text value makes query_row fail with a type error, not NoRows.
        conn.execute(
            "UPDATE settings SET value_json = ?1 WHERE key = 'app'",
            [rusqlite::types::Value::Blob(vec![0xff])],
        )
        .unwrap();
        assert!(get(&conn).is_err());
        let patch = SettingsSnapshot {
            font_size: Some("lg".to_string()),
            ..default_settings()
        };
        assert!(update(&conn, &patch).is_err());
        let blob: Vec<u8> = conn
            .query_row(
                "SELECT value_json FROM settings WHERE key = 'app'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(blob, vec![0xff]);
    }
}
