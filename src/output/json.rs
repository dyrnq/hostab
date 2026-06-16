use crate::core::model::Row;
/// Format rows as JSON array
pub fn format_json_rows(rows: &[Row]) -> String {
    serde_json::to_string_pretty(rows).unwrap_or_else(|e| format!("JSON error: {}", e))
}
