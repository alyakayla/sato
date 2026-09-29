use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::chunk;
use crate::db;
use crate::error::{Error, Result};
use crate::extract;
use crate::models::*;
use crate::providers::{Provider, ProviderConfig};
use crate::vectordb::VectorDb;

/// Shared application state.
///
/// Tauri requires managed state to be `Send + Sync`, but `rusqlite::Connection`
/// and `reqwest` internals are not. The connection is therefore held behind a
/// `Mutex` and the provider behind an `RwLock`.
///
/// Every command is `async`, including the ones that never await. Tauri runs
/// synchronous commands on the main thread, so a sync command waiting for this
/// lock — say, behind a long retrieval — froze the entire window. Async
/// commands run on the runtime instead, and the UI stays live.
///
/// An important consequence: a lock guard must never be held across an `.await`.
/// The accessors below hand out short-lived guards so that stays easy to see —
/// `db()` for a scoped read/write, and `provider()` which returns a cheap clone
/// so no guard is needed during network calls.
pub struct AppState {
    conn: Mutex<Connection>,
    /// Read-only connection used by retrieval (see `db::open_reader`).
    reader: Mutex<Connection>,
    /// The embedded vector database: every indexed chunk and its embedding,
    /// in memory, persisted as one segment file per document.
    pub vectors: Arc<VectorDb>,
    provider: RwLock<Provider>,
    pub data_dir: PathBuf,
    /// Serialises document indexing: the pipeline is already batched, and
    /// running two at once would only contend for the same connection. Held
    /// behind an `Arc` so a background task can take it without borrowing.
    /// Uses tokio's mutex because the guard is held across `.await` points and
    /// therefore has to be `Send`.
    index_lock: Arc<tokio::sync::Mutex<()>>,
}

impl AppState {
    pub fn db(&self) -> MutexGuard<'_, Connection> {
        // A poisoned lock means a previous command panicked mid-statement. The
        // SQLite handle is still usable for reads, so recover rather than
        // taking the whole app down.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn reader(&self) -> MutexGuard<'_, Connection> {
        self.reader.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn provider(&self) -> Provider {
        self.provider
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set_provider(&self, p: Provider) {
        *self.provider.write().unwrap_or_else(|e| e.into_inner()) = p;
    }

    pub fn index_lock(&self) -> Arc<tokio::sync::Mutex<()>> {
        Arc::clone(&self.index_lock)
    }
}

type CmdResult<T> = std::result::Result<T, Error>;

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn load_provider_config(conn: &Connection) -> Result<ProviderConfig> {
    let raw = db::get_setting(conn, "provider")?;
    Ok(match raw {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => ProviderConfig::default(),
    })
}

// ---------------------------------------------------------------------------
// Settings and providers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub provider: ProviderConfig,
    pub chunk_target_chars: usize,
    pub retrieval_limit: usize,
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> CmdResult<Settings> {
    let retrieval = db::get_setting(&state.db(), "retrievalLimit")?
        .and_then(|v| v.parse().ok())
        .unwrap_or(8);
    let target = db::get_setting(&state.db(), "chunkTargetChars")?
        .and_then(|v| v.parse().ok())
        .unwrap_or(chunk::CHUNK_TARGET_CHARS);
    Ok(Settings {
        provider: load_provider_config(&state.db())?,
        chunk_target_chars: target,
        retrieval_limit: retrieval,
    })
}

#[tauri::command]
pub async fn save_provider(
    state: State<'_, AppState>,
    config: ProviderConfig,
) -> CmdResult<ProviderStatus> {
    db::set_setting(&state.db(), "provider", &serde_json::to_string(&config)?)?;
    // Rebuild the client so the next call uses the new base URL and key.
    let client = Provider::new(config.clone())?;
    let diagnosis = client.diagnose().await;
    state.set_provider(client);
    provider_status(&state, &config, diagnosis)
}

fn provider_status(
    state: &State<'_, AppState>,
    cfg: &ProviderConfig,
    d: crate::providers::Diagnosis,
) -> CmdResult<ProviderStatus> {
    let embedding_dim = db::get_setting(&state.db(), "embeddingDim")?.and_then(|v| v.parse().ok());
    Ok(ProviderStatus {
        name: cfg.kind.clone(),
        configured: d.ok,
        detail: if d.ok { format!("connected to {} ({})", cfg.base_url, cfg.chat_model) } else { d.detail },
        embedding_dim,
        problem: d.problem,
        missing_models: d.missing_models,
        base_url: cfg.base_url.clone(),
        chat_model: cfg.chat_model.clone(),
        embedding_model: cfg.embedding_model.clone(),
    })
}

#[tauri::command]
pub async fn test_provider(state: State<'_, AppState>) -> CmdResult<ProviderStatus> {
    // Clone the provider so the network call holds no lock.
    let provider = state.provider();
    let diagnosis = provider.diagnose().await;
    let cfg = provider.config().clone();
    provider_status(&state, &cfg, diagnosis)
}

#[tauri::command]
pub async fn set_retrieval_limit(state: State<'_, AppState>, limit: i64) -> CmdResult<()> {
    let limit = limit.clamp(1, 40);
    db::set_setting(&state.db(), "retrievalLimit", &limit.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn dashboard(state: State<'_, AppState>) -> CmdResult<DashboardStats> {
    let conn = &state.db();
    let count = |sql: &str| -> Result<i64> {
        Ok(conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap_or(0))
    };

    let open_cases = count("SELECT COUNT(*) FROM cases WHERE status = 'Open'")?;
    let total_cases = count("SELECT COUNT(*) FROM cases")?;
    let documents = count("SELECT COUNT(*) FROM documents")?;
    let indexed_documents = count("SELECT COUNT(*) FROM documents WHERE index_status = 'Ready'")?;
    let chunks = state.vectors.len() as i64;
    let people = count("SELECT COUNT(*) FROM people")?;
    let categories = count("SELECT COUNT(*) FROM categories")?;
    let spreadsheets = count("SELECT COUNT(*) FROM spreadsheets")?;

    let recent_documents = {
        let mut stmt = conn.prepare(
            "SELECT d.id, d.case_id, d.file_name, d.source_path, d.stored_path, d.mime,
                    d.size_bytes, d.checksum, d.page_count, d.word_count, d.index_status,
                    d.index_error, d.created_at,
                    0,
                    c.title, c.reference
               FROM documents d
               LEFT JOIN cases c ON c.id = d.case_id
              ORDER BY d.created_at DESC LIMIT 8",
        )?;
        let rows = stmt.query_map([], map_document)?;
        with_chunk_counts(&state.vectors, rows.collect::<std::result::Result<Vec<_>, _>>()?)
    };

    let cases_by_status = {
        let mut stmt = conn.prepare(
            "SELECT status, COUNT(*) FROM cases GROUP BY status ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(StatusCount { status: r.get(0)?, count: r.get(1)? })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };

    let category_breakdown = {
        let mut stmt = conn.prepare(
            "SELECT COALESCE(cat.name, 'Uncategorised'), COALESCE(cat.color, '#64748b'),
                    COUNT(c.id)
               FROM cases c
               LEFT JOIN categories cat ON cat.id = c.category_id
              GROUP BY c.category_id
              ORDER BY COUNT(c.id) DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(CategoryCount { name: r.get(0)?, color: r.get(1)?, count: r.get(2)? })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };

    Ok(DashboardStats {
        open_cases,
        total_cases,
        documents,
        indexed_documents,
        chunks,
        people,
        categories,
        spreadsheets,
        recent_documents,
        cases_by_status,
        category_breakdown,
    })
}

// ---------------------------------------------------------------------------
// Categories
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_categories(state: State<'_, AppState>) -> CmdResult<Vec<Category>> {
    let conn = state.db();
    let mut stmt = conn.prepare(
        "SELECT cat.id, cat.name, cat.practice_area, cat.color, cat.description, cat.created_at,
                (SELECT COUNT(*) FROM cases c WHERE c.category_id = cat.id)
           FROM categories cat ORDER BY cat.name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Category {
            id: r.get(0)?,
            name: r.get(1)?,
            practice_area: r.get(2)?,
            color: r.get(3)?,
            description: r.get(4)?,
            created_at: r.get(5)?,
            case_count: r.get(6)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Error::from)
}

#[tauri::command]
pub async fn save_category(
    state: State<'_, AppState>,
    id: Option<String>,
    name: String,
    practice_area: String,
    color: String,
    description: Option<String>,
) -> CmdResult<Category> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::Message("Category name cannot be empty".into()));
    }
    let id = id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let ts = now();
    state.db().execute(
        "INSERT INTO categories (id, name, practice_area, color, description, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
           name = excluded.name,
           practice_area = excluded.practice_area,
           color = excluded.color,
           description = excluded.description",
        params![id, name, practice_area, color, description, ts],
    )?;

    let conn = state.db();
    let mut stmt = conn.prepare(
        "SELECT cat.id, cat.name, cat.practice_area, cat.color, cat.description, cat.created_at,
                (SELECT COUNT(*) FROM cases c WHERE c.category_id = cat.id)
           FROM categories cat WHERE cat.id = ?1",
    )?;
    stmt.query_row([&id], |r| {
        Ok(Category {
            id: r.get(0)?,
            name: r.get(1)?,
            practice_area: r.get(2)?,
            color: r.get(3)?,
            description: r.get(4)?,
            created_at: r.get(5)?,
            case_count: r.get(6)?,
        })
    })
    .map_err(Error::from)
}

#[tauri::command]
pub async fn delete_category(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    // Cases keep existing with a null category thanks to ON DELETE SET NULL.
    state.db().execute("DELETE FROM categories WHERE id = ?1", [id])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Cases
// ---------------------------------------------------------------------------

fn map_case(r: &rusqlite::Row) -> rusqlite::Result<Case> {
    Ok(Case {
        id: r.get(0)?,
        reference: r.get(1)?,
        title: r.get(2)?,
        category_id: r.get(3)?,
        status: r.get(4)?,
        description: r.get(5)?,
        opened_at: r.get(6)?,
        closed_at: r.get(7)?,
        created_at: r.get(8)?,
        updated_at: r.get(9)?,
        category_name: r.get(10)?,
        category_color: r.get(11)?,
        document_count: r.get(12)?,
        people_count: r.get(13)?,
    })
}

const CASE_SELECT: &str = "SELECT c.id, c.reference, c.title, c.category_id, c.status, c.description,
            c.opened_at, c.closed_at, c.created_at, c.updated_at, cat.name, cat.color,
            (SELECT COUNT(*) FROM documents d WHERE d.case_id = c.id),
            (SELECT COUNT(*) FROM case_people cp WHERE cp.case_id = c.id)
       FROM cases c
       LEFT JOIN categories cat ON cat.id = c.category_id";

/// Optional filters accepted by `list_cases` and the case-filtered views.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseFilter {
    pub category_id: Option<String>,
    pub status: Option<String>,
    pub search: Option<String>,
}

#[tauri::command]
pub async fn list_cases(state: State<'_, AppState>, filter: Option<CaseFilter>) -> CmdResult<Vec<Case>> {
    let filter = filter.unwrap_or_default();
    let mut sql = CASE_SELECT.to_string();
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    let clause = |sql: &mut String, binds: &mut Vec<Box<dyn rusqlite::ToSql>>| {
        if let Some(cat) = &filter.category_id {
            sql.push_str(" AND c.category_id = ?");
            binds.push(Box::new(cat.clone()));
        }
        if let Some(status) = &filter.status {
            sql.push_str(" AND c.status = ?");
            binds.push(Box::new(status.clone()));
        }
        if let Some(term) = filter.search.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            sql.push_str(" AND (c.title LIKE ? OR c.reference LIKE ? OR c.description LIKE ?)");
            let like = format!("%{term}%");
            binds.push(Box::new(like.clone()));
            binds.push(Box::new(like.clone()));
            binds.push(Box::new(like));
        }
    };
    clause(&mut sql, &mut binds);
    sql.push_str(" ORDER BY c.updated_at DESC");

    let conn = state.db();
    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), map_case)?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Error::from)
}

#[tauri::command]
pub async fn get_case(state: State<'_, AppState>, id: String) -> CmdResult<Case> {
    let sql = format!("{CASE_SELECT} WHERE c.id = ?1");
    state
        .db()
        .query_row(&sql, [&id], map_case)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Error::Message("Case not found".into()),
            other => Error::from(other),
        })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseInput {
    pub id: Option<String>,
    pub reference: Option<String>,
    pub title: String,
    pub category_id: Option<String>,
    pub status: Option<String>,
    pub description: Option<String>,
    pub opened_at: Option<String>,
}

#[tauri::command]
pub async fn save_case(state: State<'_, AppState>, input: CaseInput) -> CmdResult<Case> {
    upsert_case(&state.db(), input)
}

/// Creates or updates a case. Separate from the command so it can be tested
/// against a real schema without a running app.
fn upsert_case(conn: &Connection, input: CaseInput) -> CmdResult<Case> {
    let title = input.title.trim();
    if title.is_empty() {
        return Err(Error::Message("Case title cannot be empty".into()));
    }
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let ts = now();

    // Auto-assign a reference like "2026-CV-0042" when the user leaves it blank.
    let reference = match input.reference.as_ref().map(|r| r.trim()).filter(|r| !r.is_empty()) {
        Some(r) => r.to_string(),
        None => next_reference(conn)?,
    };
    let status = input.status.unwrap_or_else(|| "Open".into());
    let opened = input.opened_at.unwrap_or_else(|| ts.clone());

    conn.execute(
        "INSERT INTO cases (id, reference, title, category_id, status, description, opened_at,
                            closed_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7,
                 CASE WHEN ?5 = 'Closed' THEN ?8 END, ?8, ?8)
         ON CONFLICT(id) DO UPDATE SET
           reference = excluded.reference,
           title = excluded.title,
           category_id = excluded.category_id,
           status = excluded.status,
           description = excluded.description,
           opened_at = excluded.opened_at,
           closed_at = CASE
             WHEN excluded.status = 'Closed' AND cases.closed_at IS NULL
               THEN excluded.created_at
             WHEN excluded.status <> 'Closed' THEN NULL
             ELSE cases.closed_at END,
           updated_at = excluded.updated_at",
        // ?8 is "now", used for both created_at and updated_at on insert; an
        // update keeps the original created_at and only refreshes updated_at.
        params![id, reference, title, input.category_id, status, input.description, opened, ts],
    )?;
    get_case_inner(conn, &id)
}

#[cfg(test)]
mod case_tests {
    use super::{upsert_case, CaseInput};
    use crate::db;

    fn input(title: &str) -> CaseInput {
        CaseInput {
            id: None,
            reference: Some("case-01".into()),
            title: title.into(),
            category_id: None,
            status: Some("Open".into()),
            description: Some("Breach of a supply contract".into()),
            opened_at: Some("2026-09-01T00:00:00Z".into()),
        }
    }

    #[test]
    fn creating_a_case_with_every_field_filled_succeeds() {
        let conn = db::memory();
        conn.execute(
            "INSERT INTO categories (id, name, color, created_at) VALUES ('cat1', 'Contracts', '#A45029', 'n')",
            [],
        )
        .unwrap();
        let mut full = input("Henderson v. Ardent");
        full.category_id = Some("cat1".into());
        let case = upsert_case(&conn, full).expect("insert with all fields");
        assert_eq!(case.reference, "case-01");
        assert_eq!(case.category_id.as_deref(), Some("cat1"));
        assert_eq!(case.description.as_deref(), Some("Breach of a supply contract"));
        assert_eq!(case.opened_at, "2026-09-01T00:00:00Z");
        // Timestamps are "now", not the opened date.
        assert_ne!(case.created_at, case.opened_at);
        assert!(case.closed_at.is_none());
    }

    #[test]
    fn a_blank_reference_is_assigned_automatically() {
        let conn = db::memory();
        let mut minimal = input("Unnamed matter");
        minimal.reference = Some("   ".into());
        minimal.description = None;
        minimal.opened_at = None;
        let case = upsert_case(&conn, minimal).expect("insert with defaults");
        assert!(case.reference.contains("-CV-"), "{}", case.reference);
    }

    #[test]
    fn updating_keeps_created_at_and_closing_stamps_closed_at() {
        let conn = db::memory();
        let created = upsert_case(&conn, input("Henderson v. Ardent")).unwrap();
        let mut edit = input("Henderson v. Ardent (settled)");
        edit.id = Some(created.id.clone());
        edit.status = Some("Closed".into());
        let updated = upsert_case(&conn, edit).unwrap();
        assert_eq!(updated.id, created.id);
        assert_eq!(updated.title, "Henderson v. Ardent (settled)");
        assert_eq!(updated.created_at, created.created_at);
        assert!(updated.closed_at.is_some());
    }

    #[test]
    fn a_case_created_closed_gets_a_closed_date() {
        let conn = db::memory();
        let mut closed = input("Archived matter");
        closed.status = Some("Closed".into());
        assert!(upsert_case(&conn, closed).unwrap().closed_at.is_some());
    }

    #[test]
    fn an_empty_title_is_rejected() {
        let conn = db::memory();
        assert!(upsert_case(&conn, input("   ")).is_err());
    }
}

fn get_case_inner(conn: &Connection, id: &str) -> CmdResult<Case> {
    let sql = format!("{CASE_SELECT} WHERE c.id = ?1");
    conn.query_row(&sql, [id], map_case).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Error::Message("Case not found".into()),
        other => Error::from(other),
    })
}

/// Generates a reference of the form "2026-CV-0042", incrementing past any
/// existing reference with the same year and type.
fn next_reference(conn: &Connection) -> Result<String> {
    let year = chrono::Utc::now().format("%Y").to_string();
    let prefix = format!("{year}-CV-");
    let max_seq: Option<i64> = conn
        .query_row(
            "SELECT MAX(CAST(substr(reference, ?1) AS INTEGER))
               FROM cases WHERE reference LIKE ?2",
            params![prefix.len() as i64 + 1, format!("{prefix}%")],
            |r| r.get(0),
        )
        .unwrap_or(None);
    let next = max_seq.unwrap_or(0) + 1;
    Ok(format!("{prefix}{next:04}"))
}

#[tauri::command]
pub async fn delete_case(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    // Documents and spreadsheet rows survive; their case_id is nulled by the
    // schema so nothing a lawyer uploaded is silently destroyed.
    state.db().execute("DELETE FROM cases WHERE id = ?1", [&id])?;
    // Its documents are now unfiled, so case-scoped search must not find them.
    state.vectors.clear_case(&id);
    Ok(())
}

// ---------------------------------------------------------------------------
// People
// ---------------------------------------------------------------------------

fn map_person(r: &rusqlite::Row) -> rusqlite::Result<Person> {
    Ok(Person {
        id: r.get(0)?,
        full_name: r.get(1)?,
        role: r.get(2)?,
        organization: r.get(3)?,
        email: r.get(4)?,
        phone: r.get(5)?,
        notes: r.get(6)?,
        created_at: r.get(7)?,
        case_count: r.get(8)?,
    })
}

const PERSON_SELECT: &str = "SELECT p.id, p.full_name, p.role, p.organization, p.email, p.phone,
            p.notes, p.created_at,
            (SELECT COUNT(*) FROM case_people cp WHERE cp.person_id = p.id)
       FROM people p";

#[tauri::command]
pub async fn list_people(state: State<'_, AppState>, search: Option<String>) -> CmdResult<Vec<Person>> {
    let mut sql = PERSON_SELECT.to_string();
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(term) = search.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        sql.push_str(" WHERE p.full_name LIKE ?1 OR p.organization LIKE ?1 OR p.email LIKE ?1");
        binds.push(Box::new(format!("%{term}%")));
    }
    sql.push_str(" ORDER BY p.full_name COLLATE NOCASE");

    let conn = state.db();
    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), map_person)?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Error::from)
}

#[tauri::command]
pub async fn save_person(
    state: State<'_, AppState>,
    id: Option<String>,
    full_name: String,
    role: Option<String>,
    organization: Option<String>,
    email: Option<String>,
    phone: Option<String>,
    notes: Option<String>,
) -> CmdResult<Person> {
    let full_name = full_name.trim();
    if full_name.is_empty() {
        return Err(Error::Message("Name cannot be empty".into()));
    }
    let id = id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let ts = now();
    state.db().execute(
        "INSERT INTO people (id, full_name, role, organization, email, phone, notes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(id) DO UPDATE SET
           full_name = excluded.full_name,
           role = excluded.role,
           organization = excluded.organization,
           email = excluded.email,
           phone = excluded.phone,
           notes = excluded.notes",
        params![
            id,
            full_name,
            role.unwrap_or_else(|| "Other".into()),
            organization,
            email,
            phone,
            notes,
            ts
        ],
    )?;

    let sql = format!("{PERSON_SELECT} WHERE p.id = ?1");
    state.db().query_row(&sql, [&id], map_person).map_err(Error::from)
}

#[tauri::command]
pub async fn delete_person(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.db().execute("DELETE FROM people WHERE id = ?1", [id])?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseRoster {
    pub case: Case,
    pub people: Vec<RosterEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterEntry {
    pub link: CasePerson,
    pub person: Person,
}

#[tauri::command]
pub async fn case_roster(state: State<'_, AppState>, case_id: String) -> CmdResult<CaseRoster> {
    let case = get_case_inner(&state.db(), &case_id)?;
    let conn = state.db();
    let mut stmt = conn.prepare(&format!(
        "{PERSON_SELECT}
         JOIN case_people cp ON cp.person_id = p.id
         WHERE cp.case_id = ?1
         ORDER BY cp.role_in_case, p.full_name COLLATE NOCASE"
    ))?;
    let rows = stmt.query_map([&case_id], |r| {
        let person = map_person(r)?;
        let role_in_case: String = r.get(9)?;
        let person_id = person.id.clone();
        Ok(RosterEntry {
            link: CasePerson { person_id, case_id: case_id.clone(), role_in_case },
            person,
        })
    })?;
    let people = rows.collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(CaseRoster { case, people })
}

#[tauri::command]
pub async fn link_person(
    state: State<'_, AppState>,
    case_id: String,
    person_id: String,
    role_in_case: Option<String>,
) -> CmdResult<()> {
    state.db().execute(
        "INSERT INTO case_people (case_id, person_id, role_in_case) VALUES (?1, ?2, ?3)
         ON CONFLICT(case_id, person_id) DO UPDATE SET role_in_case = excluded.role_in_case",
        params![case_id, person_id, role_in_case.unwrap_or_default()],
    )?;
    Ok(())
}

#[tauri::command]
pub async fn unlink_person(state: State<'_, AppState>, case_id: String, person_id: String) -> CmdResult<()> {
    state.db().execute(
        "DELETE FROM case_people WHERE case_id = ?1 AND person_id = ?2",
        params![case_id, person_id],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

fn map_document(r: &rusqlite::Row) -> rusqlite::Result<Document> {
    Ok(Document {
        id: r.get(0)?,
        case_id: r.get(1)?,
        file_name: r.get(2)?,
        source_path: r.get(3)?,
        stored_path: r.get(4)?,
        mime: r.get(5)?,
        size_bytes: r.get(6)?,
        checksum: r.get(7)?,
        page_count: r.get(8)?,
        word_count: r.get(9)?,
        index_status: r.get(10)?,
        index_error: r.get(11)?,
        created_at: r.get(12)?,
        chunk_count: r.get(13)?,
        case_title: r.get(14)?,
        case_reference: r.get(15)?,
    })
}

const DOC_SELECT: &str = "SELECT d.id, d.case_id, d.file_name, d.source_path, d.stored_path, d.mime,
            d.size_bytes, d.checksum, d.page_count, d.word_count, d.index_status, d.index_error,
            d.created_at,
            0,
            c.title, c.reference
       FROM documents d
       LEFT JOIN cases c ON c.id = d.case_id";

/// Chunk counts live in the vector database; fill them in after a query.
fn with_chunk_counts(vectors: &VectorDb, mut docs: Vec<Document>) -> Vec<Document> {
    for d in &mut docs {
        d.chunk_count = vectors.chunk_count(&d.id) as i64;
    }
    docs
}

fn state_chunk_count(app: &AppHandle, document_id: &str) -> i64 {
    app.state::<AppState>().vectors.chunk_count(document_id) as i64
}

#[tauri::command]
pub async fn list_documents(
    state: State<'_, AppState>,
    case_id: Option<String>,
    search: Option<String>,
) -> CmdResult<Vec<Document>> {
    let mut sql = DOC_SELECT.to_string();
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(c) = &case_id {
        sql.push_str(" WHERE d.case_id = ?");
        binds.push(Box::new(c.clone()));
    }
    if let Some(term) = search.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        let base = binds.len();
        let _ = base;
        if binds.is_empty() {
            sql.push_str(" WHERE d.file_name LIKE ?1");
        } else {
            sql.push_str(" AND d.file_name LIKE ?1");
        }
        binds.push(Box::new(format!("%{term}%")));
    }
    sql.push_str(" ORDER BY d.created_at DESC");

    let conn = state.db();
    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), map_document)?;
    let docs = rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Error::from)?;
    Ok(with_chunk_counts(&state.vectors, docs))
}

#[tauri::command]
pub async fn get_document(state: State<'_, AppState>, id: String) -> CmdResult<Document> {
    let sql = format!("{DOC_SELECT} WHERE d.id = ?1");
    let mut doc = state.db().query_row(&sql, [&id], map_document).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Error::Message("Document not found".into()),
        other => Error::from(other),
    })?;
    doc.chunk_count = state.vectors.chunk_count(&doc.id) as i64;
    Ok(doc)
}

#[tauri::command]
pub async fn list_chunks(state: State<'_, AppState>, document_id: String) -> CmdResult<Vec<Chunk>> {
    Ok(state.vectors.chunks_of(&document_id))
}

/// Copies a file into app data and records it, without waiting for indexing.
/// Returns as soon as the file is safely stored so the UI stays responsive.
#[tauri::command]
pub async fn import_documents(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
    case_id: Option<String>,
) -> CmdResult<Vec<Document>> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    let docs_dir = state.data_dir.join("documents");
    std::fs::create_dir_all(&docs_dir)?;

    let mut out = Vec::new();
    for path in &paths {
        let src = PathBuf::from(path);
        if !src.is_file() {
            out.push(placeholder_doc(&src, "File not found"));
            continue;
        }
        let bytes = std::fs::read(&src)?;
        let checksum = hex::encode(Sha256::digest(&bytes));
        let id = Uuid::new_v4().to_string();
        let dest = docs_dir.join(format!("{id}_{}", sanitize(&src)));
        std::fs::write(&dest, &bytes)?;

        let file_name = src
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("untitled")
            .to_string();
        let mime = extract::guess_mime(&src);

        state.db().execute(
            "INSERT INTO documents (id, case_id, file_name, source_path, stored_path, mime,
                                    size_bytes, checksum, index_status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'Pending', ?9)",
            params![
                id,
                case_id,
                file_name,
                src.to_string_lossy().to_string(),
                dest.to_string_lossy().to_string(),
                mime,
                bytes.len() as i64,
                checksum,
                now()
            ],
        )?;

        let doc = get_document_inner(&state.db(), &id)?;
        out.push(doc.clone());

        if extract::is_supported(mime) {
            // Kick off indexing in the background so the command returns as
            // soon as the file is safely copied into app data.
            // Clone the AppHandle into the task and resolve state inside, so
            // nothing borrowed from this command's State crosses the spawn
            // boundary.
            let handle = app.clone();
            let file_name = doc.file_name.clone();
            let index_lock = state.index_lock();
            tauri::async_runtime::spawn(async move {
                // One document at a time: embedding calls are already batched
                // inside the provider, and parallel runs would fight over the
                // single SQLite connection.
                let _guard = index_lock.lock().await;
                if let Err(e) = index_document(&handle, &id).await {
                    let worker = handle.state::<AppState>();
                    let _ = mark_failed(&worker.db(), &id, &e.to_string());
                    let _ = handle.emit(
                        "index:progress",
                        IndexProgress {
                            document_id: id.clone(),
                            file_name: file_name.clone(),
                            stage: "Failed".into(),
                            chunks_done: 0,
                            chunks_total: 0,
                            status: "Failed".into(),
                        },
                    );
                }
            });
        } else {
            let _ = mark_failed(
                &state.db(),
                &id,
                &format!("'{}' is not a supported file type", mime),
            );
        }
    }
    Ok(out)
}

fn placeholder_doc(src: &std::path::Path, err: &str) -> Document {
    Document {
        id: String::new(),
        case_id: None,
        file_name: src
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string(),
        source_path: src.to_string_lossy().to_string(),
        stored_path: String::new(),
        mime: "application/octet-stream".into(),
        size_bytes: 0,
        checksum: String::new(),
        page_count: None,
        word_count: None,
        index_status: "Failed".into(),
        index_error: Some(err.to_string()),
        created_at: now(),
        chunk_count: 0,
        case_title: None,
        case_reference: None,
    }
}

fn sanitize(p: &std::path::Path) -> String {
    p.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

pub fn get_document_inner(conn: &Connection, id: &str) -> CmdResult<Document> {
    let sql = format!("{DOC_SELECT} WHERE d.id = ?1");
    conn.query_row(&sql, [id], map_document).map_err(Error::from)
}

fn mark_failed(conn: &Connection, id: &str, err: &str) -> Result<()> {
    conn.execute(
        "UPDATE documents SET index_status = 'Failed', index_error = ?2 WHERE id = ?1",
        params![id, err],
    )?;
    Ok(())
}

/// Extracts, chunks, embeds and indexes one document. Emits progress events
/// that the UI listens for.
pub async fn index_document(app: &AppHandle, document_id: &str) -> Result<()> {
    // Clone the provider and data we need up front so no lock guard is held
    // across an await point.
    let provider = app.state::<AppState>().provider();

    let doc = {
        let state = app.state::<AppState>();
        let doc = get_document_inner(&state.db(), document_id)?;
        state.db().execute(
            "UPDATE documents SET index_status = 'Indexing', index_error = NULL WHERE id = ?1",
            [document_id],
        )?;
        // A reindex keeps the document's current passages searchable until
        // the new ones are committed below.
        doc
    };
    emit_progress(app, &doc, "Extracting text", 0, 0);

    // 1. Extract text on a blocking thread; PDF parsing is CPU bound.
    let path = PathBuf::from(&doc.stored_path);
    let mime = doc.mime.clone();
    let extracted = tokio::task::spawn_blocking(move || extract::extract(&path, &mime))
        .await
        .map_err(|e| Error::Message(format!("extraction task failed: {e}")))??;

    let text = extract::collapse_whitespace(&extracted.text);
    if text.is_empty() {
        return Err(Error::Message(
            "No readable text found. Scanned PDFs need OCR before indexing.".into(),
        ));
    }

    let pages: Vec<String> = extracted.pages.unwrap_or_default();
    let chunks = chunk::chunk_text(&pages, document_id);
    if chunks.is_empty() {
        return Err(Error::Message("Document produced no indexable chunks".into()));
    }

    let word_count = text.split_whitespace().count() as i64;
    let page_count = if pages.is_empty() { None } else { Some(pages.len() as i64) };

    // 2. Embed in batches, reporting progress as we go.
    emit_progress(app, &doc, "Embedding", 0, chunks.len() as i64);
    let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(chunks.len());
    let batch_size = 24usize;
    for (batch_idx, batch) in chunks.chunks(batch_size).enumerate() {
        let inputs: Vec<String> = batch.iter().map(|c| c.text.clone()).collect();
        let embedded = provider.embed(&inputs).await?;
        if let Some(first) = embedded.first() {
            let dim = first.len() as i64;
            db::set_setting(&app.state::<AppState>().db(), "embeddingDim", &dim.to_string())?;
        }
        vectors.extend(embedded);

        let done = ((batch_idx + 1) * batch_size).min(chunks.len()) as i64;
        emit_progress(app, &doc, "Embedding", done, chunks.len() as i64);
    }

    // 3. Commit: the vector database first (its segment write is atomic),
    //    then the keyword index and the Ready flag in one SQLite transaction.
    //    A crash between the two leaves a segment for a not-Ready document,
    //    which startup reconciliation removes.
    {
        let state = app.state::<AppState>();
        let fts_rows: Vec<(String, String)> = chunks.iter().map(|c| (c.id.clone(), c.text.clone())).collect();
        let vdb = Arc::clone(&state.vectors);
        let doc_id = document_id.to_string();
        let case_id = doc.case_id.clone();
        tokio::task::spawn_blocking(move || vdb.upsert_document(&doc_id, case_id.as_deref(), chunks, &vectors))
            .await
            .map_err(|e| Error::Message(format!("vector store task failed: {e}")))??;

        let mut conn = state.db();
        let tx = conn.transaction().map_err(Error::from)?;
        tx.execute(
            "UPDATE documents SET word_count = ?2, page_count = ?3 WHERE id = ?1",
            params![document_id, word_count, page_count],
        )?;
        tx.execute("DELETE FROM passages_fts WHERE document_id = ?1", [document_id])?;
        {
            let mut fts = tx.prepare("INSERT INTO passages_fts (body, chunk_id, document_id) VALUES (?1, ?2, ?3)")?;
            for (id, text) in &fts_rows {
                fts.execute(params![text, id, document_id])?;
            }
        }
        tx.execute(
            "UPDATE documents SET index_status = 'Ready', index_error = NULL WHERE id = ?1",
            [document_id],
        )?;
        tx.commit().map_err(Error::from)?;
    }
    let chunk_total = state_chunk_count(app, document_id);
    emit_progress(app, &doc, "Complete", chunk_total, chunk_total);
    let _ = app.emit("documents:changed", serde_json::json!({ "documentId": document_id }));
    Ok(())
}

fn emit_progress(app: &AppHandle, doc: &Document, stage: &str, done: i64, total: i64) {
    let _ = app.emit(
        "index:progress",
        IndexProgress {
            document_id: doc.id.clone(),
            file_name: doc.file_name.clone(),
            stage: stage.to_string(),
            chunks_done: done,
            chunks_total: total,
            status: if stage == "Complete" { "Ready".into() } else { "Indexing".into() },
        },
    );
}

#[tauri::command]
pub async fn reindex_document(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let _ = &state;
    index_document(&app, &id).await
}

#[tauri::command]
pub async fn delete_document(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let doc = get_document_inner(&state.db(), &id)?;
    if let Ok(rel) = std::path::Path::new(&doc.stored_path)
        .strip_prefix(&state.data_dir)
        .map(|p| p.to_path_buf())
    {
        // Only ever delete files inside our own data directory.
        let _ = std::fs::remove_file(state.data_dir.join(rel));
    }
    // The passages go with the row (see the `passages_fts_document_delete`
    // trigger); the vector database drops the document's segment.
    state.db().execute("DELETE FROM documents WHERE id = ?1", [&id])?;
    state.vectors.remove_document(&id)?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentText {
    pub document: Document,
    pub text: String,
    pub pages: Vec<String>,
}

#[tauri::command]
pub async fn read_document_text(state: State<'_, AppState>, id: String) -> CmdResult<DocumentText> {
    let doc = get_document_inner(&state.db(), &id)?;
    let path = PathBuf::from(&doc.stored_path);
    let mime = doc.mime.clone();
    let extracted = extract::extract(&path, &mime)?;
    Ok(DocumentText {
        text: extract::collapse_whitespace(&extracted.text),
        pages: extracted.pages.unwrap_or_default(),
        document: doc,
    })
}

// ---------------------------------------------------------------------------
// Chat
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn list_conversations(state: State<'_, AppState>, case_id: Option<String>) -> CmdResult<Vec<Conversation>> {
    let mut sql = String::from(
        "SELECT cv.id, cv.title, cv.case_id, cv.created_at, cv.updated_at,
                (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = cv.id)
           FROM conversations cv",
    );
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(c) = &case_id {
        sql.push_str(" WHERE cv.case_id = ?");
        binds.push(Box::new(c.clone()));
    }
    sql.push_str(" ORDER BY cv.updated_at DESC");

    let conn = state.db();
    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), |r| {
        Ok(Conversation {
            id: r.get(0)?,
            title: r.get(1)?,
            case_id: r.get(2)?,
            created_at: r.get(3)?,
            updated_at: r.get(4)?,
            message_count: r.get(5)?,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Error::from)
}

#[tauri::command]
pub async fn create_conversation(
    state: State<'_, AppState>,
    title: Option<String>,
    case_id: Option<String>,
) -> CmdResult<Conversation> {
    let id = Uuid::new_v4().to_string();
    let ts = now();
    let title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "New conversation".into());
    state.db().execute(
        "INSERT INTO conversations (id, title, case_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id, title, case_id, ts],
    )?;
    Ok(Conversation {
        id,
        title,
        case_id,
        created_at: ts.clone(),
        updated_at: ts,
        message_count: 0,
    })
}

#[tauri::command]
pub async fn delete_conversation(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.db().execute("DELETE FROM conversations WHERE id = ?1", [id])?;
    Ok(())
}

fn map_message(r: &rusqlite::Row) -> rusqlite::Result<Message> {
    let citations_json: String = r.get(4)?;
    Ok(Message {
        id: r.get(0)?,
        conversation_id: r.get(1)?,
        role: r.get(2)?,
        content: r.get(3)?,
        citations: serde_json::from_str(&citations_json).unwrap_or_default(),
        error: r.get(5)?,
        created_at: r.get(6)?,
    })
}

const MSG_SELECT: &str = "SELECT id, conversation_id, role, content, citations, error, created_at
       FROM messages";

#[tauri::command]
pub async fn list_messages(state: State<'_, AppState>, conversation_id: String) -> CmdResult<Vec<Message>> {
    let sql = format!("{MSG_SELECT} WHERE conversation_id = ?1 ORDER BY created_at, rowid");
    let conn = state.db();
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([&conversation_id], map_message)?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Error::from)
}

/// How many prior messages the model sees. Six exchanges keeps follow-up
/// questions coherent without crowding retrieved context out of the window.
const HISTORY_TURNS: usize = 12;

/// The `limit` most recent messages before `exclude_id`, oldest first,
/// skipping failed replies.
fn recent_history(
    conn: &Connection,
    conversation_id: &str,
    exclude_id: &str,
    limit: usize,
) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT role, content FROM (
             SELECT role, content, created_at, rowid FROM messages
              WHERE conversation_id = ?1 AND id <> ?2 AND error IS NULL
                AND role IN ('user', 'assistant')
              ORDER BY created_at DESC, rowid DESC
              LIMIT ?3)
          ORDER BY created_at, rowid",
    )?;
    let rows = stmt.query_map(params![conversation_id, exclude_id, limit as i64], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    rows.collect()
}

const SYSTEM_PROMPT: &str = "\
You are satō, an assistant embedded in a law firm's case management desktop app.

Rules you must follow:
1. Answer only from the provided case context. Treat it as confidential, privileged material.
2. Cite every factual claim with the bracketed source markers given to you, for example [1] or [2].
3. If the context does not contain the answer, say so plainly and suggest what to upload or which case to open. Never invent facts, citations, statute numbers, or case names.
4. Distinguish clearly between what the documents state and your own analysis. Label analysis as such.
5. You are not a licensed attorney and your output is not legal advice. Note this once, briefly, when the user asks for a recommendation on how to act.
6. Prefer direct, precise language. Quote key operative wording when it matters, and keep quotes short.

Response format:
- Lead with the answer in one or two sentences.
- Then any supporting detail, organised with short headings or bullets.
- Close with a 'Sources' line listing the markers you actually relied on.";

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    content: String,
    document_ids: Option<Vec<String>>,
) -> CmdResult<Message> {
    let content = content.trim().to_string();
    if content.is_empty() {
        return Err(Error::Message("Message cannot be empty".into()));
    }

    // Clone the provider up front so the two network calls below never hold a
    // lock guard across an await.
    let provider = state.provider();

    // Everything up to the model call is pure database work, so it lives in a
    // scope whose guard drops before the first await.
    let (case_id, limit, history): (Option<String>, usize, Vec<(String, String)>) = {
        let conn = state.db();
        let user_id = Uuid::new_v4().to_string();
        let ts = now();

        // 1. Persist the user turn.
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, citations, created_at)
             VALUES (?1, ?2, 'user', ?3, '[]', ?4)",
            params![user_id, conversation_id, content, ts],
        )?;

        // Title the conversation from its first user message.
        let msg_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
            [&conversation_id],
            |r| r.get(0),
        )?;
        if msg_count == 1 {
            let title: String = content.chars().take(60).collect();
            conn.execute(
                "UPDATE conversations SET title = ?2 WHERE id = ?1 AND title = 'New conversation'",
                params![conversation_id, title],
            )?;
        }

        // 2. Scope retrieval to the conversation's case when it has one.
        let case_id: Option<String> = conn.query_row(
            "SELECT case_id FROM conversations WHERE id = ?1",
            [&conversation_id],
            |r| r.get(0),
        )?;
        let limit = db::get_setting(&conn, "retrievalLimit")?
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(8);

        // The latest turns give the model conversational context — the most
        // recent ones, oldest first, excluding the question just stored (it is
        // appended separately below) and failed replies, which are UI notices
        // rather than things the model said.
        let history = recent_history(&conn, &conversation_id, &user_id, HISTORY_TURNS)?;

        (case_id, limit, history)
    };

    emit_step(&app, &conversation_id, "embed", "running", None);
    // Without an embedding the question can still be answered from keyword
    // matches, so a failure here degrades retrieval instead of ending the
    // turn. The panel is told why, so it can say so.
    let query_vector = match provider.embed(&[content.clone()]).await {
        Ok(v) => v.into_iter().next().unwrap_or_default(),
        Err(e) => {
            let problem = crate::providers::classify(&e);
            let _ = app.emit(
                "chat:step",
                serde_json::json!({
                    "conversationId": conversation_id,
                    "step": "embed",
                    "state": "skipped",
                    "problem": problem,
                }),
            );
            Vec::new()
        }
    };

    // Retrieval is CPU-bound (a cosine pass over every candidate vector) and
    // holds the database lock, so it runs on a blocking thread: the async
    // runtime stays free to serve every other command while it works.
    let hits = {
        let app = app.clone();
        let query_vector = query_vector.clone();
        let content = content.clone();
        let case_id = case_id.clone();
        let doc_ids = document_ids.unwrap_or_default();
        tauri::async_runtime::spawn_blocking(move || -> Result<Vec<crate::models::SearchHit>> {
            let state = app.state::<AppState>();
            // The read-only connection: indexing writes never block this.
            let conn = state.reader();
            Ok(crate::search::hybrid_search(
                &conn,
                &state.vectors,
                &query_vector,
                &content,
                limit,
                case_id.as_deref(),
                &doc_ids,
            )?)
        })
        .await
        .map_err(|e| Error::Message(format!("Search task failed: {e}")))??
    };
    emit_step(&app, &conversation_id, "embed", "done", None);
    {
        // Counts, not a sentence: the panel words it in the user's language.
        let docs: std::collections::HashSet<&str> =
            hits.iter().map(|h| h.chunk.document_id.as_str()).collect();
        emit_step(&app, &conversation_id, "search", "done", Some((hits.len(), docs.len())));
    }

    let mut context_blocks: Vec<String> = Vec::new();
    let mut citations: Vec<Citation> = Vec::new();
    for (i, hit) in hits.iter().enumerate() {
        let marker = i + 1;
        let where_clause = match (&hit.case_reference, hit.chunk.page) {
            (Some(reference), Some(page)) => format!("{reference}, p. {page}"),
            (Some(reference), None) => reference.clone(),
            (None, Some(page)) => format!("p. {page}"),
            (None, None) => "page unknown".to_string(),
        };
        context_blocks.push(format!(
            "[{marker}] {} ({})\n{}",
            hit.document_name, where_clause, hit.chunk.text
        ));
        citations.push(Citation {
            chunk_id: hit.chunk.id.clone(),
            document_id: hit.chunk.document_id.clone(),
            document_name: hit.document_name.clone(),
            case_reference: hit.case_reference.clone(),
            page: hit.chunk.page,
            score: hit.score,
            snippet: snippet(&hit.chunk.text, 320),
        });
    }

    let context = if context_blocks.is_empty() {
        "No relevant passages were found in the indexed documents for this question.".to_string()
    } else {
        context_blocks.join("\n\n---\n\n")
    };

    let system = format!(
        "{SYSTEM_PROMPT}\n\n## Case context retrieved for this question\n\n{context}\n\n\
         ## End of context"
    );

    let mut messages: Vec<(String, String)> = vec![("system".into(), system)];
    for (role, c) in &history {
        if role == "user" || role == "assistant" {
            messages.push((role.clone(), c.clone()));
        }
    }
    messages.push(("user".into(), content.clone()));

    // 4. Generate, and surface failures as a visible message rather than a
    //    silent empty bubble.
    emit_step(&app, &conversation_id, "generate", "running", None);
    // Stream the answer: each batch of new text goes to the panel as a
    // `chat:delta`, so the user reads it as it is written. Deltas are batched
    // to ~25 events a second so a fast model cannot flood the IPC channel.
    let mut pending = String::new();
    let mut last_emit = std::time::Instant::now();
    let flush = |pending: &mut String| {
        if !pending.is_empty() {
            let _ = app.emit(
                "chat:delta",
                serde_json::json!({ "conversationId": conversation_id, "text": pending.as_str() }),
            );
            pending.clear();
        }
    };
    let streamed = provider
        .chat_stream(&messages, |piece| {
            pending.push_str(piece);
            if last_emit.elapsed() >= std::time::Duration::from_millis(40) {
                flush(&mut pending);
                last_emit = std::time::Instant::now();
            }
        })
        .await;
    flush(&mut pending);
    let reply = match streamed {
        Ok(text) if !text.trim().is_empty() => text,
        Ok(_) => "(The model returned an empty response.)".to_string(),
        Err(e) => {
            let cfg = provider.config();
            let problem = crate::providers::classify(&e);
            let detail = crate::providers::describe(&e);
            // Structured, so the panel can explain the failure in the user's
            // language with the exact fix (start Ollama, pull a model…).
            let error = serde_json::json!({
                "stage": "chat",
                "problem": problem,
                "provider": cfg.kind,
                "url": cfg.base_url,
                "model": cfg.chat_model,
                "embeddingModel": cfg.embedding_model,
                "detail": detail,
            })
            .to_string();
            let msg = persist_failed_reply(
                &state,
                &conversation_id,
                &format!("The chat model could not be used. ({detail})"),
                &error,
            )?;
            let _ = app.emit("chat:reply", &msg);
            return Ok(msg);
        }
    };

    let msg_id = Uuid::new_v4().to_string();
    let reply_ts = now();
    let citations_json = serde_json::to_string(&citations)?;
    state.db().execute(
        "INSERT INTO messages (id, conversation_id, role, content, citations, created_at)
         VALUES (?1, ?2, 'assistant', ?3, ?4, ?5)",
        params![msg_id, conversation_id, reply, citations_json, reply_ts],
    )?;
    state.db().execute(
        "UPDATE conversations SET updated_at = ?2 WHERE id = ?1",
        params![conversation_id, reply_ts],
    )?;

    let sql = format!("{MSG_SELECT} WHERE id = ?1");
    let msg = state.db().query_row(&sql, [&msg_id], map_message).map_err(Error::from)?;
    let _ = app.emit("chat:reply", &msg);
    Ok(msg)
}

/// Reports progress through a chat turn so the panel can show the agent's
/// steps as they happen rather than a single opaque spinner.
fn emit_step(
    app: &AppHandle,
    conversation_id: &str,
    step: &str,
    state: &str,
    found: Option<(usize, usize)>,
) {
    let _ = app.emit(
        "chat:step",
        serde_json::json!({
            "conversationId": conversation_id,
            "step": step,
            "state": state,
            "passages": found.map(|f| f.0),
            "documents": found.map(|f| f.1),
        }),
    );
}

/// Stores a turn that failed. `content` is a plain-English record for the
/// transcript; `error` is what the panel renders (a JSON diagnosis when the
/// provider failed, so it can be explained and translated).
fn persist_failed_reply(
    state: &State<'_, AppState>,
    conversation_id: &str,
    content: &str,
    error: &str,
) -> CmdResult<Message> {
    let id = Uuid::new_v4().to_string();
    let ts = now();
    state.db().execute(
        "INSERT INTO messages (id, conversation_id, role, content, citations, error, created_at)
         VALUES (?1, ?2, 'assistant', ?3, '[]', ?4, ?5)",
        params![id, conversation_id, content, error, ts],
    )?;
    state.db().execute(
        "UPDATE conversations SET updated_at = ?2 WHERE id = ?1",
        params![conversation_id, ts],
    )?;
    let sql = format!("{MSG_SELECT} WHERE id = ?1");
    state.db().query_row(&sql, [&id], map_message).map_err(Error::from)
}

fn snippet(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let truncated: String = text.chars().take(max).collect();
    format!("{}...", truncated.trim_end())
}

/// Full-text search view: keyword (BM25) matches only, best first. Scores are
/// scaled to 0..1 against the best hit so the UI can draw a relevance bar.
#[tauri::command]
pub async fn search_documents(
    state: State<'_, AppState>,
    query: String,
    case_id: Option<String>,
    limit: Option<i64>,
) -> CmdResult<Vec<SearchHit>> {
    let limit = limit.unwrap_or(10).clamp(1, 50) as usize;
    let conn = state.reader();
    let hits = crate::search::keyword_search(&conn, &query, limit, case_id.as_deref(), &[])?;
    let max = hits.first().map(|(_, s)| *s).unwrap_or(1.0).max(f64::EPSILON);
    let mut out = Vec::with_capacity(hits.len());
    for (chunk_id, score) in hits {
        if let Some(hit) = crate::search::hydrate(&conn, &state.vectors, &chunk_id, score / max)? {
            out.push(hit);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Spreadsheets
// ---------------------------------------------------------------------------

const SHEET_SELECT: &str = "SELECT s.id, s.name, s.case_id, s.rows, s.cols, s.data, s.created_at,
        s.updated_at, c.title, c.reference
   FROM spreadsheets s LEFT JOIN cases c ON c.id = s.case_id";

fn map_spreadsheet(r: &rusqlite::Row) -> rusqlite::Result<Spreadsheet> {
    Ok(Spreadsheet {
        id: r.get(0)?,
        name: r.get(1)?,
        case_id: r.get(2)?,
        rows: r.get(3)?,
        cols: r.get(4)?,
        data: r.get(5)?,
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
        case_title: r.get(8)?,
        case_reference: r.get(9)?,
    })
}

#[tauri::command]
pub async fn list_spreadsheets(state: State<'_, AppState>, case_id: Option<String>) -> CmdResult<Vec<Spreadsheet>> {
    let mut sql = format!("{SHEET_SELECT} WHERE 1 = 1");
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(c) = &case_id {
        sql.push_str(" AND s.case_id = ?");
        binds.push(Box::new(c.clone()));
    }
    sql.push_str(" ORDER BY s.updated_at DESC");

    let conn = state.db();
    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), map_spreadsheet)?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Error::from)
}

#[tauri::command]
pub async fn get_spreadsheet(state: State<'_, AppState>, id: String) -> CmdResult<Spreadsheet> {
    let conn = state.db();
    let mut stmt = conn.prepare(&format!("{SHEET_SELECT} WHERE s.id = ?1"))?;
    stmt.query_row([&id], map_spreadsheet)
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Error::Message("Spreadsheet not found".into()),
            other => Error::from(other),
        })
}

#[tauri::command]
pub async fn create_spreadsheet(
    state: State<'_, AppState>,
    name: Option<String>,
    case_id: Option<String>,
) -> CmdResult<Spreadsheet> {
    let id = Uuid::new_v4().to_string();
    let ts = now();
    let name = name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "Untitled sheet".into());
    state.db().execute(
        "INSERT INTO spreadsheets (id, name, case_id, rows, cols, data, created_at, updated_at)
         VALUES (?1, ?2, ?3, 200, 26, '{}', ?4, ?4)",
        params![id, name, case_id, ts],
    )?;
    get_spreadsheet_inner(&state.db(), &id)
}

fn get_spreadsheet_inner(conn: &Connection, id: &str) -> CmdResult<Spreadsheet> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, s.case_id, s.rows, s.cols, s.data, s.created_at, s.updated_at,
                c.title, c.reference
           FROM spreadsheets s LEFT JOIN cases c ON c.id = s.case_id WHERE s.id = ?1",
    )?;
    stmt.query_row([id], |r| {
        Ok(Spreadsheet {
            id: r.get(0)?,
            name: r.get(1)?,
            case_id: r.get(2)?,
            rows: r.get(3)?,
            cols: r.get(4)?,
            data: r.get(5)?,
            created_at: r.get(6)?,
            updated_at: r.get(7)?,
            case_title: r.get(8)?,
            case_reference: r.get(9)?,
        })
    })
    .map_err(Error::from)
}

#[tauri::command]
pub async fn save_spreadsheet(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    data: String,
    rows: Option<i64>,
    cols: Option<i64>,
) -> CmdResult<()> {
    // Reject a malformed payload rather than persisting it.
    serde_json::from_str::<serde_json::Value>(&data).map_err(|e| {
        Error::Message(format!("Invalid cell data: {e}"))
    })?;
    state.db().execute(
        "UPDATE spreadsheets SET name = COALESCE(?2, name), data = ?3,
                rows = COALESCE(?4, rows), cols = COALESCE(?5, cols), updated_at = ?6
          WHERE id = ?1",
        params![id, name, data, rows, cols, now()],
    )?;
    Ok(())
}

#[tauri::command]
pub async fn delete_spreadsheet(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.db().execute("DELETE FROM spreadsheets WHERE id = ?1", [id])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Calendar
// ---------------------------------------------------------------------------

fn map_event(r: &rusqlite::Row) -> rusqlite::Result<CalendarEvent> {
    Ok(CalendarEvent {
        id: r.get(0)?,
        case_id: r.get(1)?,
        title: r.get(2)?,
        description: r.get(3)?,
        kind: r.get(4)?,
        starts_at: r.get(5)?,
        ends_at: r.get(6)?,
        all_day: r.get::<_, i64>(7)? != 0,
        location: r.get(8)?,
        color: r.get(9)?,
        created_at: r.get(10)?,
        updated_at: r.get(11)?,
        case_title: r.get(12)?,
        case_reference: r.get(13)?,
    })
}

const EVENT_SELECT: &str = "SELECT e.id, e.case_id, e.title, e.description, e.kind,
        e.starts_at, e.ends_at, e.all_day, e.location, e.color, e.created_at, e.updated_at,
        c.title, c.reference
   FROM events e
   LEFT JOIN cases c ON c.id = e.case_id";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventInput {
    pub id: Option<String>,
    pub case_id: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub kind: Option<String>,
    pub starts_at: String,
    pub ends_at: Option<String>,
    pub all_day: Option<bool>,
    pub location: Option<String>,
    pub color: Option<String>,
}

/// Events in a window, used by the month grid and the agenda list.
#[tauri::command]
pub async fn list_events(
    state: State<'_, AppState>,
    case_id: Option<String>,
    from: Option<String>,
    to: Option<String>,
) -> CmdResult<Vec<CalendarEvent>> {
    let mut sql = format!("{EVENT_SELECT} WHERE 1 = 1");
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(c) = case_id {
        sql.push_str(" AND e.case_id = ?");
        binds.push(Box::new(c));
    }
    if let Some(f) = from {
        sql.push_str(" AND e.starts_at >= ?");
        binds.push(Box::new(f));
    }
    if let Some(t) = to {
        sql.push_str(" AND e.starts_at <= ?");
        binds.push(Box::new(t));
    }
    sql.push_str(" ORDER BY e.starts_at");

    let conn = state.db();
    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), map_event)?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Error::from)
}

#[tauri::command]
pub async fn save_event(state: State<'_, AppState>, input: EventInput) -> CmdResult<CalendarEvent> {
    let title = input.title.trim().to_string();
    if title.is_empty() {
        return Err(Error::Message("Event needs a title".into()));
    }
    if input.starts_at.trim().is_empty() {
        return Err(Error::Message("Event needs a start date".into()));
    }
    let ts = now();
    let conn = state.db();

    let id = match input.id {
        Some(id) if !id.is_empty() => {
            conn.execute(
                "UPDATE events SET case_id = ?2, title = ?3, description = ?4, kind = ?5,
                        starts_at = ?6, ends_at = ?7, all_day = ?8, location = ?9, color = ?10,
                        updated_at = ?11
                  WHERE id = ?1",
                params![
                    id,
                    input.case_id,
                    title,
                    input.description,
                    input.kind.unwrap_or_else(|| "Deadline".into()),
                    input.starts_at,
                    input.ends_at,
                    input.all_day.unwrap_or(false),
                    input.location,
                    input.color,
                    ts
                ],
            )?;
            id
        }
        _ => {
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO events (id, case_id, title, description, kind, starts_at, ends_at,
                        all_day, location, color, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                params![
                    id,
                    input.case_id,
                    title,
                    input.description,
                    input.kind.unwrap_or_else(|| "Deadline".into()),
                    input.starts_at,
                    input.ends_at,
                    input.all_day.unwrap_or(false),
                    input.location,
                    input.color,
                    ts
                ],
            )?;
            id
        }
    };

    let sql = format!("{EVENT_SELECT} WHERE e.id = ?1");
    conn.query_row(&sql, [&id], map_event).map_err(Error::from)
}

#[tauri::command]
pub async fn delete_event(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.db().execute("DELETE FROM events WHERE id = ?1", [id])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Case tree
// ---------------------------------------------------------------------------

/// Builds the whole case tree in one round trip.
///
/// The chat mention syntax is `@case-ref/file-name`, so every node carries a
/// `node_key` the frontend can match tokens against without another query.
///
/// Split from the command so the shaping logic can be tested against a plain
/// connection, without spinning up Tauri's managed state.
#[tauri::command]
pub async fn case_tree(state: State<'_, AppState>) -> CmdResult<CaseTree> {
    build_case_tree(&state.db())
}

fn build_case_tree(conn: &Connection) -> Result<CaseTree> {
    let mut nodes: Vec<TreeNode> = Vec::new();

    // Cases -----------------------------------------------------------------
    let cases = {
        let mut stmt = conn.prepare(&format!("{CASE_SELECT} ORDER BY c.updated_at DESC"))?;
        let rows = stmt.query_map([], map_case)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    for c in &cases {
        nodes.push(TreeNode {
            id: format!("case:{}", c.id),
            node_key: c.reference.to_lowercase(),
            kind: "case".into(),
            label: c.title.clone(),
            parent_id: None,
            depth: 0,
            ordinal: nodes.len() as i64,
            status: Some(c.status.clone()),
            detail: Some(c.reference.clone()),
            color: c.category_color.clone(),
            case_id: Some(c.id.clone()),
            document_id: None,
            size_bytes: None,
            updated_at: c.updated_at.clone(),
            starts_at: None,
        });
    }

    // Documents -------------------------------------------------------------
    let documents = {
        let mut stmt = conn.prepare(&format!("{DOC_SELECT} ORDER BY d.created_at DESC"))?;
        let rows = stmt.query_map([], map_document)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    for d in &documents {
        let parent = d.case_id.as_ref().map(|id| format!("case:{id}"));
        // An unfiled upload hangs off the root so it is still reachable.
        let depth = if parent.is_some() { 1 } else { 0 };
        nodes.push(TreeNode {
            id: format!("document:{}", d.id),
            node_key: match (&d.case_reference, parent.is_some()) {
                (Some(reference), true) => {
                    format!("{}/{}", reference.to_lowercase(), d.file_name.to_lowercase())
                }
                _ => d.file_name.to_lowercase(),
            },
            kind: "document".into(),
            label: d.file_name.clone(),
            parent_id: parent.clone(),
            depth,
            ordinal: nodes.len() as i64,
            status: Some(d.index_status.clone()),
            detail: Some(d.mime.clone()),
            color: None,
            case_id: d.case_id.clone(),
            document_id: Some(d.id.clone()),
            size_bytes: Some(d.size_bytes),
            updated_at: d.created_at.clone(),
            starts_at: None,
        });
    }

    // People ----------------------------------------------------------------
    let roster: Vec<(String, String, String, String, String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT cp.case_id, p.id, p.full_name, p.role, COALESCE(p.organization, ''),
                    COALESCE(cp.role_in_case, '')
               FROM case_people cp
               JOIN people p ON p.id = cp.person_id
              ORDER BY p.full_name",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    for (case_id, person_id, full_name, role, organization, role_in_case) in roster {
        let reference = cases
            .iter()
            .find(|c| c.id == case_id)
            .map(|c| c.reference.to_lowercase());
        let key = match &reference {
            Some(r) => format!("{}/{}", r, person_id),
            None => person_id.to_lowercase(),
        };
        nodes.push(TreeNode {
            id: format!("person:{person_id}:{case_id}"),
            node_key: key,
            kind: "person".into(),
            label: full_name,
            parent_id: Some(format!("case:{case_id}")),
            depth: 1,
            ordinal: nodes.len() as i64,
            status: Some(if role_in_case.is_empty() { role } else { role_in_case }),
            detail: Some(organization).filter(|s| !s.is_empty()),
            color: None,
            case_id: Some(case_id),
            document_id: None,
            size_bytes: None,
            updated_at: now(),
            starts_at: None,
        });
    }

    // Spreadsheets ----------------------------------------------------------
    let sheets = {
        let mut stmt = conn.prepare(&format!("{SHEET_SELECT} ORDER BY s.updated_at DESC"))?;
        let rows = stmt.query_map([], map_spreadsheet)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    for s in &sheets {
        let parent = s.case_id.as_ref().map(|id| format!("case:{id}"));
        let depth = if parent.is_some() { 1 } else { 0 };
        nodes.push(TreeNode {
            id: format!("sheet:{}", s.id),
            node_key: match &s.case_reference {
                Some(reference) if parent.is_some() => {
                    format!("{}/{}", reference.to_lowercase(), s.name.to_lowercase())
                }
                _ => s.name.to_lowercase(),
            },
            kind: "sheet".into(),
            label: s.name.clone(),
            parent_id: parent,
            depth,
            ordinal: nodes.len() as i64,
            status: Some(format!("{} x {}", s.rows, s.cols)),
            detail: None,
            color: None,
            case_id: s.case_id.clone(),
            document_id: None,
            size_bytes: None,
            updated_at: s.updated_at.clone(),
            starts_at: None,
        });
    }

    // Events ----------------------------------------------------------------
    let events = {
        let mut stmt = conn.prepare(&format!("{EVENT_SELECT} ORDER BY e.starts_at"))?;
        let rows = stmt.query_map([], map_event)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    for e in &events {
        let parent = e.case_id.as_ref().map(|id| format!("case:{id}"));
        let depth = if parent.is_some() { 1 } else { 0 };
        nodes.push(TreeNode {
            id: format!("event:{}", e.id),
            node_key: match &e.case_reference {
                Some(reference) if parent.is_some() => {
                    format!("{}/{}", reference.to_lowercase(), e.title.to_lowercase())
                }
                _ => e.title.to_lowercase(),
            },
            kind: "event".into(),
            label: e.title.clone(),
            parent_id: parent,
            depth,
            ordinal: nodes.len() as i64,
            status: Some(e.kind.clone()),
            detail: e.location.clone(),
            color: e.color.clone(),
            case_id: e.case_id.clone(),
            document_id: None,
            size_bytes: None,
            updated_at: e.updated_at.clone(),
            starts_at: Some(e.starts_at.clone()),
        });
    }

    Ok(CaseTree { nodes, generated_at: now() })
}

#[cfg(test)]
mod tree_tests {
    use super::build_case_tree;
    use crate::db;

    fn node<'a>(tree: &'a crate::models::CaseTree, id: &str) -> &'a crate::models::TreeNode {
        tree.nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap_or_else(|| panic!("no node {id} in tree"))
    }

    fn seeded() -> rusqlite::Connection {
        let conn = db::memory();
        conn.execute(
            "INSERT INTO cases (id, reference, title, opened_at, created_at, updated_at)
             VALUES ('c1', 'case-01', 'Henderson v. Ardent', 'n', 'n', 'n')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO documents (id, case_id, file_name, source_path, stored_path, mime,
                    size_bytes, checksum, index_status, created_at)
             VALUES ('d1', 'c1', 'document-abc.pdf', 's', 'p', 'application/pdf',
                    1024, 'x', 'Ready', 'n')",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn a_case_and_its_document_get_composable_mention_keys() {
        let tree = build_case_tree(&seeded()).unwrap();
        assert_eq!(node(&tree, "case:c1").node_key, "case-01");
        // The document is addressable as `case-01/document-abc.pdf`, which is
        // exactly what a user types after the `@`.
        assert_eq!(
            node(&tree, "document:d1").node_key,
            "case-01/document-abc.pdf"
        );
    }

    #[test]
    fn children_point_at_their_case_node() {
        let tree = build_case_tree(&seeded()).unwrap();
        let doc = node(&tree, "document:d1");
        assert_eq!(doc.parent_id.as_deref(), Some("case:c1"));
        assert_eq!(doc.depth, 1);
        assert_eq!(doc.document_id.as_deref(), Some("d1"));
    }

    #[test]
    fn an_unfiled_document_sits_at_the_root_with_a_bare_key() {
        let conn = db::memory();
        conn.execute(
            "INSERT INTO documents (id, case_id, file_name, source_path, stored_path, mime,
                    size_bytes, checksum, index_status, created_at)
             VALUES ('d9', NULL, 'orphan-notes.pdf', 's', 'p', 'application/pdf',
                    10, 'x', 'Ready', 'n')",
            [],
        )
        .unwrap();
        let tree = build_case_tree(&conn).unwrap();
        let doc = node(&tree, "document:d9");
        assert!(doc.parent_id.is_none(), "an orphan has no parent");
        assert_eq!(doc.depth, 0);
        // No case prefix to borrow, so the file name is the whole handle.
        assert_eq!(doc.node_key, "orphan-notes.pdf");
    }

    #[test]
    fn every_node_id_is_namespaced_by_kind() {
        let conn = seeded();
        conn.execute(
            "INSERT INTO people (id, full_name, role, created_at)
             VALUES ('p1', 'Dana Whitfield', 'Client', 'n')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO case_people (case_id, person_id, role_in_case) VALUES ('c1', 'p1', 'Client')",
            [],
        )
        .unwrap();
        let tree = build_case_tree(&conn).unwrap();
        // A person appears once per case, so its id carries both.
        assert!(tree.nodes.iter().any(|n| n.id == "person:p1:c1"));
    }

    #[test]
    fn a_person_linked_to_two_cases_produces_two_nodes() {
        let conn = seeded();
        conn.execute(
            "INSERT INTO cases (id, reference, title, opened_at, created_at, updated_at)
             VALUES ('c2', 'case-02', 'Okonkwo', 'n', 'n', 'n')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO people (id, full_name, role, created_at)
             VALUES ('p1', 'Dana Whitfield', 'Expert', 'n')",
            [],
        )
        .unwrap();
        for c in ["c1", "c2"] {
            conn.execute(
                "INSERT INTO case_people (case_id, person_id, role_in_case) VALUES (?1, 'p1', 'Expert')",
                [c],
            )
            .unwrap();
        }
        let tree = build_case_tree(&conn).unwrap();
        let people: Vec<_> = tree.nodes.iter().filter(|n| n.kind == "person").collect();
        assert_eq!(people.len(), 2, "one node per case, not per person");
    }

    #[test]
    fn node_ordinals_are_unique_and_following_order() {
        let conn = seeded();
        conn.execute(
            "INSERT INTO events (id, case_id, title, starts_at, created_at, updated_at)
             VALUES ('e1', 'c1', 'Hearing', '2030-01-01T09:00:00Z', 'n', 'n')",
            [],
        )
        .unwrap();
        let tree = build_case_tree(&conn).unwrap();
        let ordinals: Vec<i64> = tree.nodes.iter().map(|n| n.ordinal).collect();
        assert_eq!(ordinals, vec![0, 1, 2], "three nodes, in order");
    }

    #[test]
    fn an_empty_database_yields_an_empty_tree() {
        let tree = build_case_tree(&db::memory()).unwrap();
        assert!(tree.nodes.is_empty());
    }
}

// ---------------------------------------------------------------------------
// Setup helper
// ---------------------------------------------------------------------------

/// Creates a small set of starter categories so a new install is not empty.
pub fn seed_defaults(conn: &Connection) -> Result<()> {
    let existing: i64 =
        conn.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0)).unwrap_or(0);
    if existing > 0 {
        return Ok(());
    }
    let defaults = [
        ("Corporate", "#6366f1", "M&A, financing, governance"),
        ("Litigation", "#ef4444", "Disputes, filings, discovery"),
        ("Family", "#ec4899", "Divorce, custody, support"),
        ("Real Estate", "#f59e0b", "Purchase, lease, title"),
        ("Employment", "#10b981", "Contracts, disputes, compliance"),
    ];
    for (name, color, desc) in defaults {
        conn.execute(
            "INSERT INTO categories (id, name, practice_area, color, description, created_at)
             VALUES (?1, ?2, '', ?3, ?4, ?5)",
            params![Uuid::new_v4().to_string(), name, color, desc, now()],
        )?;
    }
    Ok(())
}

/// One-time move of chunks and embeddings out of SQLite into the vector
/// database, for installs that predate it. Safe to re-run: segments are
/// overwritten, and the legacy tables are only dropped once every document
/// has been written. Returns how many documents were moved.
fn migrate_legacy_chunks(conn: &Connection, vectors: &Arc<VectorDb>) -> Result<usize> {
    if !db::table_exists(conn, "chunks")? {
        return Ok(0);
    }
    let has_embeddings = db::table_exists(conn, "embeddings")?;
    let docs: Vec<(String, Option<String>)> = {
        let mut stmt = conn.prepare("SELECT id, case_id FROM documents WHERE index_status = 'Ready'")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<std::result::Result<_, _>>()?
    };

    let mut moved = 0;
    for (doc, case) in &docs {
        if !has_embeddings {
            break;
        }
        let mut stmt = conn.prepare(
            "SELECT c.id, c.ordinal, c.start_char, c.end_char, c.page, c.text, c.token_estimate, e.vector
               FROM chunks c JOIN embeddings e ON e.chunk_id = c.id
              WHERE c.document_id = ?1 ORDER BY c.ordinal",
        )?;
        let mut chunks = Vec::new();
        let mut vecs = Vec::new();
        let mut rows = stmt.query([doc])?;
        while let Some(r) = rows.next()? {
            chunks.push(Chunk {
                id: r.get(0)?,
                document_id: doc.clone(),
                ordinal: r.get(1)?,
                start_char: r.get(2)?,
                end_char: r.get(3)?,
                page: r.get(4)?,
                text: r.get(5)?,
                token_estimate: r.get(6)?,
            });
            let blob: Vec<u8> = r.get(7)?;
            vecs.push(blob.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect::<Vec<f32>>());
        }
        if chunks.is_empty() {
            continue;
        }
        vectors.upsert_document(doc, case.as_deref(), chunks, &vecs)?;
        moved += 1;
    }

    // The keyword index moves to `passages_fts`, which knows each passage's
    // document; then the legacy tables go.
    conn.execute_batch(
        "BEGIN;
         DELETE FROM passages_fts WHERE document_id IN (SELECT DISTINCT document_id FROM chunks);
         INSERT INTO passages_fts (body, chunk_id, document_id)
              SELECT c.text, c.id, c.document_id FROM chunks c
               JOIN documents d ON d.id = c.document_id AND d.index_status = 'Ready';
         DROP TABLE IF EXISTS embeddings;
         DROP TABLE IF EXISTS chunks;
         DROP TABLE IF EXISTS chunks_fts;
         COMMIT;",
    )?;
    Ok(moved)
}

/// Makes the vector database and SQLite agree at startup: drop segments for
/// documents SQLite does not consider indexed (a crash mid-commit, or a
/// document deleted while the app was closed), flag indexed documents whose
/// vectors are missing or unreadable so the user can reindex them, and load
/// case membership for filtering.
fn reconcile_vectors(conn: &Connection, vectors: &Arc<VectorDb>, corrupt: &[String]) -> Result<()> {
    let ready: std::collections::HashSet<String> = {
        let mut stmt = conn.prepare("SELECT id FROM documents WHERE index_status = 'Ready'")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    vectors.retain_documents(&ready)?;

    let present: std::collections::HashSet<String> = vectors.documents().into_iter().collect();
    for doc in ready.iter().filter(|d| !present.contains(*d)).chain(corrupt.iter()) {
        mark_failed(conn, doc, "Search data for this document is missing or damaged. Reindex it to search it again.")?;
        conn.execute("DELETE FROM passages_fts WHERE document_id = ?1", [doc])?;
    }

    let cases: Vec<(String, Option<String>)> = {
        let mut stmt = conn.prepare("SELECT id, case_id FROM documents")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    vectors.set_document_cases(cases);
    vectors.maintain();
    Ok(())
}

/// The database file inside the app data directory.
const DB_FILE: &str = "sato.db";

pub fn init(app: &AppHandle) -> Result<()> {
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    let conn = db::open(&data_dir.join(DB_FILE))?;
    seed_defaults(&conn)?;
    let reader = db::open_reader(&data_dir.join(DB_FILE))?;

    let (vectors, report) = VectorDb::open(&data_dir.join("vectors"))?;
    migrate_legacy_chunks(&conn, &vectors)?;
    reconcile_vectors(&conn, &vectors, &report.corrupt)?;

    let provider_cfg = load_provider_config(&conn)?;
    let provider = Provider::new(provider_cfg)?;

    app.manage(AppState {
        conn: Mutex::new(conn),
        reader: Mutex::new(reader),
        vectors,
        provider: RwLock::new(provider),
        data_dir,
        index_lock: Arc::new(tokio::sync::Mutex::new(())),
    });
    Ok(())
}

#[cfg(test)]
mod vector_migration_tests {
    use super::{migrate_legacy_chunks, reconcile_vectors};
    use crate::db;
    use crate::providers::vec_to_blob;
    use crate::vectordb::VectorDb;
    use std::path::PathBuf;

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sato-mig-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    /// An install from before the vector database: chunks, embeddings and the
    /// old keyword index all in SQLite.
    fn legacy_db() -> rusqlite::Connection {
        let conn = db::memory();
        conn.execute_batch(db::LEGACY_SCHEMA).unwrap();
        conn.execute_batch(
            "INSERT INTO cases (id, reference, title, opened_at, created_at, updated_at)
               VALUES ('c1', 'case-01', 'Henderson', 'n', 'n', 'n');
             INSERT INTO documents (id, case_id, file_name, source_path, stored_path, checksum, index_status, created_at)
               VALUES ('d1', 'c1', 'brief.pdf', 'a', 'b', 's1', 'Ready', 'n'),
                      ('d2', NULL, 'scan.pdf', 'a', 'b', 's2', 'Failed', 'n');",
        )
        .unwrap();
        for (id, doc, ord, text) in [
            ("d1:0", "d1", 0, "The lessee shall give notice of termination."),
            ("d1:1", "d1", 1, "Termination takes effect after sixty days."),
            ("d2:0", "d2", 0, "Unreadable scan."),
        ] {
            conn.execute(
                "INSERT INTO chunks (id, document_id, ordinal, start_char, end_char, page, text, token_estimate)
                 VALUES (?1, ?2, ?3, 0, 10, 1, ?4, 10)",
                rusqlite::params![id, doc, ord, text],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO embeddings (chunk_id, document_id, model, dim, vector) VALUES (?1, ?2, 'm', 3, ?3)",
                rusqlite::params![id, doc, vec_to_blob(&[ord as f32 + 1.0, 0.5, 0.25])],
            )
            .unwrap();
            conn.execute("INSERT INTO chunks_fts (body, chunk_id) VALUES (?1, ?2)", rusqlite::params![text, id])
                .unwrap();
        }
        conn
    }

    #[test]
    fn legacy_chunks_move_into_the_vector_database() {
        let conn = legacy_db();
        let dir = tempdir("move");
        let (vdb, _) = VectorDb::open(&dir).unwrap();

        assert_eq!(migrate_legacy_chunks(&conn, &vdb).unwrap(), 1, "only the Ready document moves");
        assert_eq!(vdb.chunk_count("d1"), 2);
        assert_eq!(vdb.chunk_count("d2"), 0);
        let chunks = vdb.chunks_of("d1");
        assert_eq!(chunks[1].text, "Termination takes effect after sixty days.");
        assert_eq!(chunks[0].page, Some(1));

        for t in ["chunks", "embeddings", "chunks_fts"] {
            assert!(!db::table_exists(&conn, t).unwrap(), "{t} should be dropped");
        }
        let passages: Vec<(String, String)> = conn
            .prepare("SELECT chunk_id, document_id FROM passages_fts WHERE passages_fts MATCH 'termination' ORDER BY chunk_id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(passages, vec![("d1:0".into(), "d1".into()), ("d1:1".into(), "d1".into())]);

        // Idempotent, and durable: a second run is a no-op, a reopen sees it all.
        assert_eq!(migrate_legacy_chunks(&conn, &vdb).unwrap(), 0);
        let (reopened, report) = VectorDb::open(&dir).unwrap();
        assert_eq!((report.documents, reopened.chunk_count("d1")), (1, 2));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn startup_reconciles_the_store_with_sqlite() {
        let conn = db::memory();
        conn.execute_batch(
            "INSERT INTO documents (id, file_name, source_path, stored_path, checksum, index_status, created_at)
               VALUES ('ready-with-vectors', 'a.pdf', 'a', 'b', 's1', 'Ready', 'n'),
                      ('ready-without', 'b.pdf', 'a', 'b', 's2', 'Ready', 'n'),
                      ('not-ready', 'c.pdf', 'a', 'b', 's3', 'Indexing', 'n');",
        )
        .unwrap();
        let dir = tempdir("reconcile");
        let (vdb, _) = VectorDb::open(&dir).unwrap();
        let chunk = |doc: &str| crate::models::Chunk {
            id: format!("{doc}:0"),
            document_id: doc.into(),
            ordinal: 0,
            start_char: 0,
            end_char: 1,
            page: None,
            text: "t".into(),
            token_estimate: 1,
        };
        vdb.upsert_document("ready-with-vectors", None, vec![chunk("ready-with-vectors")], &[vec![1.0, 0.0]]).unwrap();
        // A crash between the segment write and the Ready commit.
        vdb.upsert_document("not-ready", None, vec![chunk("not-ready")], &[vec![0.0, 1.0]]).unwrap();

        reconcile_vectors(&conn, &vdb, &[]).unwrap();

        assert_eq!(vdb.documents(), vec!["ready-with-vectors".to_string()], "orphan segment dropped");
        let status: String = conn
            .query_row("SELECT index_status FROM documents WHERE id = 'ready-without'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "Failed", "an indexed document without vectors is flagged for reindex");
        std::fs::remove_dir_all(&dir).ok();
    }
}
