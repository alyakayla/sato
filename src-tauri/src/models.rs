use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub name: String,
    /// Free-form practice area, e.g. "Corporate", "Family", "IP".
    pub practice_area: String,
    pub color: String,
    pub description: Option<String>,
    pub created_at: String,
    pub case_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Case {
    pub id: String,
    pub reference: String,
    pub title: String,
    pub category_id: Option<String>,
    /// Open, Closed, On Hold, Pending.
    pub status: String,
    pub description: Option<String>,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub document_count: i64,
    pub people_count: i64,
}

/// A party, witness, judge, or the other side's lawyer attached to a case.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub id: String,
    pub full_name: String,
    /// Client, Opposing Party, Witness, Judge, Expert, Other.
    pub role: String,
    pub organization: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub case_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CasePerson {
    pub person_id: String,
    pub case_id: String,
    /// Per-case nuance of the person's role, e.g. "Lead Defendant".
    pub role_in_case: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub case_id: Option<String>,
    pub file_name: String,
    /// Absolute path to the original file chosen by the user.
    pub source_path: String,
    /// Copy kept inside the app data directory.
    pub stored_path: String,
    pub mime: String,
    pub size_bytes: i64,
    /// SHA-256 of the file, used to skip re-ingesting duplicates.
    pub checksum: String,
    pub page_count: Option<i64>,
    pub word_count: Option<i64>,
    /// Pending, Indexing, Ready, Failed.
    pub index_status: String,
    pub index_error: Option<String>,
    pub created_at: String,
    pub chunk_count: i64,
    pub case_title: Option<String>,
    pub case_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    pub id: String,
    pub document_id: String,
    pub ordinal: i64,
    pub start_char: i64,
    pub end_char: i64,
    pub page: Option<i64>,
    pub text: String,
    pub token_estimate: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    /// Global or scoped to one case.
    pub case_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    /// user, assistant, system.
    pub role: String,
    pub content: String,
    /// Citations backing an assistant reply.
    pub citations: Vec<Citation>,
    /// Present on assistant messages that failed.
    pub error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Citation {
    pub chunk_id: String,
    pub document_id: String,
    pub document_name: String,
    pub case_reference: Option<String>,
    pub page: Option<i64>,
    pub score: f64,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spreadsheet {
    pub id: String,
    pub name: String,
    pub case_id: Option<String>,
    /// Rows x cols grid stored as a JSON blob of cells.
    pub rows: i64,
    pub cols: i64,
    /// Serialized cell map: { "r,c": { "v": "...", "f": "=..." } }.
    pub data: String,
    pub created_at: String,
    pub updated_at: String,
    pub case_title: Option<String>,
    /// Reference of the linked case, shown next to the title in the UI.
    pub case_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexProgress {
    pub document_id: String,
    pub file_name: String,
    pub stage: String,
    pub chunks_done: i64,
    pub chunks_total: i64,
    pub status: String,
}

/// A chunk plus its similarity score, as returned by retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub chunk: Chunk,
    pub score: f64,
    pub document_name: String,
    pub case_reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub name: String,
    pub configured: bool,
    pub detail: String,
    /// Dimension of the embedding vector, if already known.
    pub embedding_dim: Option<i64>,
    /// Why it is not usable, when it is not.
    pub problem: Option<crate::providers::Problem>,
    /// Configured models the server does not have.
    pub missing_models: Vec<String>,
    pub base_url: String,
    pub chat_model: String,
    pub embedding_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub open_cases: i64,
    pub total_cases: i64,
    pub documents: i64,
    pub indexed_documents: i64,
    pub chunks: i64,
    pub people: i64,
    pub categories: i64,
    pub spreadsheets: i64,
    pub recent_documents: Vec<Document>,
    pub cases_by_status: Vec<StatusCount>,
    pub category_breakdown: Vec<CategoryCount>,
}

/// A dated item on the calendar: a hearing, filing deadline, or meeting.
///
/// Unlike documents and spreadsheets, events cascade on case delete. They are
/// pure scheduling metadata with no meaning once the case they belong to is
/// gone, so leaving orphans in the tree would be worse than removing them.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: String,
    pub case_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    /// Hearing, Filing, Meeting, Deadline, Other.
    pub kind: String,
    /// RFC 3339 start. Null `ends_at` means a point in time.
    pub starts_at: String,
    pub ends_at: Option<String>,
    pub all_day: bool,
    pub location: Option<String>,
    pub color: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub case_title: Option<String>,
    pub case_reference: Option<String>,
}

/// One row of the case tree, returned flat and ordered.
///
/// Flat rather than nested on purpose: the grid view virtualises a long list,
/// and a flat array can be filtered, searched and paged without walking a
/// nested structure. The hierarchy is recovered from `parent_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeNode {
    /// Unique node id. Prefixed by kind so it can never collide.
    pub id: String,
    /// Stable handle used in chat mentions, e.g. `case-01/document-abc.pdf`.
    pub node_key: String,
    /// case, document, person, sheet, event.
    pub kind: String,
    pub label: String,
    /// Null for a root node.
    pub parent_id: Option<String>,
    pub depth: i64,
    /// Sort position among siblings.
    pub ordinal: i64,
    /// Case status, index status, or event kind depending on `kind`.
    pub status: Option<String>,
    /// Secondary line: case title, mime type, organisation, location.
    pub detail: Option<String>,
    pub color: Option<String>,
    pub case_id: Option<String>,
    pub document_id: Option<String>,
    pub size_bytes: Option<i64>,
    pub updated_at: String,
    /// Set on event nodes; drives the calendar view.
    pub starts_at: Option<String>,
}

/// Everything needed to draw a case tree in one round trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseTree {
    pub nodes: Vec<TreeNode>,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusCount {
    pub status: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryCount {
    pub name: String,
    pub color: String,
    pub count: i64,
}
