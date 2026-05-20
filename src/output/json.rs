use crate::core::model::Row;
use serde::Serialize;

/// Format rows as JSON
pub fn format_json<T: Serialize>(data: &T) -> String {
    serde_json::to_string_pretty(data).unwrap_or_else(|e| format!("JSON error: {}", e))
}

/// Format rows as JSON array
pub fn format_json_rows(rows: &[Row]) -> String {
    serde_json::to_string_pretty(rows).unwrap_or_else(|e| format!("JSON error: {}", e))
}
