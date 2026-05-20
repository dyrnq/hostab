/// A single hosts entry
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    pub id: usize,
    pub ip: String,
    pub hostnames: Vec<String>,
    pub comment: Option<String>,
    pub disabled: bool,
    #[serde(skip)]
    pub raw: Option<String>,
}

/// A row for display/rendering
#[derive(Debug, Clone, serde::Serialize)]
pub struct Row {
    pub ip: String,
    pub host: String,
    pub comment: Option<String>,
}

/// Search match info
#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchMatch {
    pub entry_id: Option<usize>,
    pub field: String,
    pub matched_text: String,
    pub line: String,
}
