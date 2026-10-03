use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    #[serde(default)]
    pub product_mode: Option<String>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub font_size: Option<String>,
    #[serde(default)]
    pub density: Option<String>,
    #[serde(default)]
    pub default_input_mode: Option<String>,
    #[serde(default)]
    pub auto_open_plan_panel: Option<bool>,
    #[serde(default)]
    pub confirm_medium_risk: Option<bool>,
    #[serde(default)]
    pub explanation_verbosity: Option<String>,
    #[serde(default)]
    pub reduced_clutter: Option<bool>,
    #[serde(default)]
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

fn fill_defaults(settings: SettingsSnapshot) -> SettingsSnapshot {
    let defaults = default_settings();
    SettingsSnapshot {
        product_mode: settings.product_mode.or(defaults.product_mode),
        theme: settings.theme.or(defaults.theme),
        font_size: settings.font_size.or(defaults.font_size),
        density: settings.density.or(defaults.density),
        default_input_mode: settings.default_input_mode.or(defaults.default_input_mode),
        auto_open_plan_panel: settings.auto_open_plan_panel.or(defaults.auto_open_plan_panel),
        confirm_medium_risk: settings.confirm_medium_risk.or(defaults.confirm_medium_risk),
        explanation_verbosity: settings
            .explanation_verbosity
            .or(defaults.explanation_verbosity),
        reduced_clutter: settings.reduced_clutter.or(defaults.reduced_clutter),
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
    let current = get(conn)?;
    let current_value =
        serde_json::to_value(&current).map_err(|e| format!("settings update: {e}"))?;
    let patch_value =
        serde_json::to_value(patch).map_err(|e| format!("settings update: {e}"))?;

    let merged = merge_json(current_value, patch_value);
    let parsed: SettingsSnapshot = serde_json::from_value(merged).unwrap_or_else(|_| default_settings());
    let filled = fill_defaults(parsed);
    let merged_str =
        serde_json::to_string(&filled).map_err(|e| format!("settings update: {e}"))?;

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
    fn missing_row_loads_confirm_medium_risk_true() {
        let conn = open();
        let settings = get(&conn).unwrap();
        assert_eq!(settings.confirm_medium_risk, Some(true));
        assert_eq!(settings.theme.as_deref(), Some("dark"));
    }

    #[test]
    fn older_object_missing_confirm_medium_risk_stays_true() {
        let conn = open();
        conn.execute(
            "INSERT INTO settings (key, value_json) VALUES ('app', ?1)",
            [r#"{"theme":"light"}"#],
        )
        .unwrap();
        let settings = get(&conn).unwrap();
        assert_eq!(settings.theme.as_deref(), Some("light"));
        assert_eq!(settings.confirm_medium_risk, Some(true));
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("\"confirmMediumRisk\":true"), "{json}");
        assert!(!json.contains("null"), "{json}");
    }

    #[test]
    fn null_confirm_medium_risk_is_filled_true() {
        let conn = open();
        conn.execute(
            "INSERT INTO settings (key, value_json) VALUES ('app', ?1)",
            [r#"{"theme":"light","confirmMediumRisk":null}"#],
        )
        .unwrap();
        let settings = get(&conn).unwrap();
        assert_eq!(settings.confirm_medium_risk, Some(true));
        assert_eq!(settings.theme.as_deref(), Some("light"));
    }

    #[test]
    fn explicit_false_confirm_medium_risk_is_kept() {
        let conn = open();
        conn.execute(
            "INSERT INTO settings (key, value_json) VALUES ('app', ?1)",
            [r#"{"confirmMediumRisk":false}"#],
        )
        .unwrap();
        let settings = get(&conn).unwrap();
        assert_eq!(settings.confirm_medium_risk, Some(false));
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
        assert_eq!(settings.confirm_medium_risk, Some(true));
        assert_eq!(settings.theme.as_deref(), Some("dark"));
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
            ..SettingsSnapshot {
                product_mode: None,
                theme: None,
                font_size: None,
                density: None,
                default_input_mode: None,
                auto_open_plan_panel: None,
                confirm_medium_risk: None,
                explanation_verbosity: None,
                reduced_clutter: None,
                simplified_summaries: None,
            }
        };
        update(&conn, &patch).unwrap();
        let stored = raw_json(&conn);
        assert!(!stored.contains("null"), "{stored}");
        assert!(stored.contains("\"confirmMediumRisk\":true"), "{stored}");
        assert!(stored.contains("\"theme\":\"light\""), "{stored}");
        assert!(stored.contains("\"fontSize\":\"lg\""), "{stored}");
        assert_eq!(get(&conn).unwrap().confirm_medium_risk, Some(true));
    }

    #[test]
    fn update_keeps_an_explicit_false_and_fails_without_the_table() {
        let conn = open();
        let patch = SettingsSnapshot {
            confirm_medium_risk: Some(false),
            theme: Some("light".into()),
            ..default_settings()
        };
        // Fill every other field so the patch is a complete object, then
        // clear the ones the merge must leave to the stored defaults.
        let patch = SettingsSnapshot {
            product_mode: None,
            theme: patch.theme,
            font_size: None,
            density: None,
            default_input_mode: None,
            auto_open_plan_panel: None,
            confirm_medium_risk: Some(false),
            explanation_verbosity: None,
            reduced_clutter: None,
            simplified_summaries: None,
        };
        update(&conn, &patch).unwrap();
        let stored = get(&conn).unwrap();
        assert_eq!(stored.confirm_medium_risk, Some(false));
        assert_eq!(stored.theme.as_deref(), Some("light"));
        assert_eq!(stored.font_size.as_deref(), Some("md"));

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
