/// A single hosts entry
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    pub id: usize,
    pub ip: String,
    pub canonical: String,
    pub aliases: Vec<String>,
    pub comment: Option<String>,
    pub disabled: bool,
    #[serde(skip)]
    pub raw: Option<String>,
}

/// A row for display/rendering
#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct Row {
    pub ip: String,
    pub host: String,
    pub comment: Option<String>,
    /// The canonical hostname for this IP
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canonical: Option<String>,
    /// Alias hostnames (empty if none)
    pub aliases: Vec<String>,
}

/// Search match info
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchMatch {
    pub entry_id: Option<usize>,
    pub field: String,
    pub matched_text: String,
    pub line: String,
}
