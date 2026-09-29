use rusqlite::Connection;
use std::path::Path;

use crate::error::Result;

pub fn open(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    migrate(&conn)?;
    Ok(conn)
}

/// A second, read-only connection for retrieval. WAL lets it read while the
/// main connection writes, so a long indexing transaction never makes a chat
/// question wait for the write lock. Opened after `open`, so the schema exists.
pub fn open_reader(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.pragma_update(None, "query_only", "ON")?;
    Ok(conn)
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS categories (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    practice_area TEXT NOT NULL DEFAULT '',
    color        TEXT NOT NULL DEFAULT '#6366f1',
    description  TEXT,
    created_at   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cases (
    id          TEXT PRIMARY KEY,
    reference   TEXT NOT NULL,
    title       TEXT NOT NULL,
    category_id TEXT REFERENCES categories(id) ON DELETE SET NULL,
    status      TEXT NOT NULL DEFAULT 'Open',
    description TEXT,
    opened_at   TEXT NOT NULL,
    closed_at   TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_cases_category ON cases(category_id);
CREATE INDEX IF NOT EXISTS idx_cases_status   ON cases(status);

CREATE TABLE IF NOT EXISTS people (
    id           TEXT PRIMARY KEY,
    full_name    TEXT NOT NULL,
    role         TEXT NOT NULL DEFAULT 'Other',
    organization TEXT,
    email        TEXT,
    phone        TEXT,
    notes        TEXT,
    created_at   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_people_role ON people(role);

CREATE TABLE IF NOT EXISTS case_people (
    case_id     TEXT NOT NULL REFERENCES cases(id) ON DELETE CASCADE,
    person_id   TEXT NOT NULL REFERENCES people(id) ON DELETE CASCADE,
    role_in_case TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (case_id, person_id)
);
CREATE INDEX IF NOT EXISTS idx_case_people_person ON case_people(person_id);

CREATE TABLE IF NOT EXISTS documents (
    id           TEXT PRIMARY KEY,
    case_id      TEXT REFERENCES cases(id) ON DELETE SET NULL,
    file_name    TEXT NOT NULL,
    source_path  TEXT NOT NULL,
    stored_path  TEXT NOT NULL,
    mime         TEXT NOT NULL DEFAULT 'application/octet-stream',
    size_bytes   INTEGER NOT NULL DEFAULT 0,
    checksum     TEXT NOT NULL,
    page_count   INTEGER,
    word_count   INTEGER,
    index_status TEXT NOT NULL DEFAULT 'Pending',
    index_error  TEXT,
    created_at   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_documents_case     ON documents(case_id);
CREATE INDEX IF NOT EXISTS idx_documents_checksum ON documents(checksum);

-- Chunks and their embeddings live in the embedded vector database
-- (`vectordb.rs`), one segment file per document. SQLite keeps only the
-- lexical index over the same passages, below.

CREATE TABLE IF NOT EXISTS conversations (
    id         TEXT PRIMARY KEY,
    title      TEXT NOT NULL,
    case_id    TEXT REFERENCES cases(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role            TEXT NOT NULL,
    content         TEXT NOT NULL,
    citations       TEXT NOT NULL DEFAULT '[]',
    error           TEXT,
    created_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_messages_conversation ON messages(conversation_id, created_at);

CREATE TABLE IF NOT EXISTS spreadsheets (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    case_id    TEXT REFERENCES cases(id) ON DELETE SET NULL,
    rows       INTEGER NOT NULL DEFAULT 40,
    cols       INTEGER NOT NULL DEFAULT 12,
    data       TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_spreadsheets_case ON spreadsheets(case_id);

-- Calendar entries hang off a case. They cascade on case delete: unlike an
-- uploaded PDF, a deadline or hearing has no meaning once its case is gone.
CREATE TABLE IF NOT EXISTS events (
    id          TEXT PRIMARY KEY,
    case_id     TEXT REFERENCES cases(id) ON DELETE CASCADE,
    title       TEXT NOT NULL,
    description TEXT,
    kind        TEXT NOT NULL DEFAULT 'Deadline',
    starts_at   TEXT NOT NULL,
    ends_at     TEXT,
    all_day     INTEGER NOT NULL DEFAULT 0,
    location    TEXT,
    color       TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_events_case  ON events(case_id);
CREATE INDEX IF NOT EXISTS idx_events_start ON events(starts_at);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Lexical index over passage text. Complements the vector database: keyword
-- lookups for citations and statutes that exact-match poorly under cosine
-- similarity. Carries its document so searches filter without a join through
-- the (retired) chunks table. `remove_diacritics` lets "acordao" match
-- "acórdão", which matters for Portuguese.
CREATE VIRTUAL TABLE IF NOT EXISTS passages_fts USING fts5(
    body,
    chunk_id UNINDEXED,
    document_id UNINDEXED,
    tokenize = 'unicode61 remove_diacritics 2'
);

-- FTS tables take no foreign keys; this keeps delete-a-document meaning
-- delete-its-passages at the database level.
CREATE TRIGGER IF NOT EXISTS passages_fts_document_delete
AFTER DELETE ON documents BEGIN
    DELETE FROM passages_fts WHERE document_id = old.id;
END;
"#;

/// The pre-vector-database tables, kept so tests can build an old database
/// and exercise the one-time migration out of it.
#[cfg(test)]
pub const LEGACY_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS chunks (
    id             TEXT PRIMARY KEY,
    document_id    TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    ordinal        INTEGER NOT NULL,
    start_char     INTEGER NOT NULL,
    end_char       INTEGER NOT NULL,
    page           INTEGER,
    text           TEXT NOT NULL,
    token_estimate INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_chunks_document ON chunks(document_id, ordinal);

-- Embeddings are stored as little-endian f32 blobs. We keep the dimension in
-- settings so a model change can be detected and stale vectors re-indexed.
CREATE TABLE IF NOT EXISTS embeddings (
    chunk_id    TEXT PRIMARY KEY REFERENCES chunks(id) ON DELETE CASCADE,
    document_id TEXT NOT NULL,
    model       TEXT NOT NULL,
    dim         INTEGER NOT NULL,
    vector      BLOB NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_embeddings_document ON embeddings(document_id);

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    body,
    chunk_id UNINDEXED,
    tokenize = 'unicode61'
);
"#;

pub fn table_exists(conn: &Connection, name: &str) -> Result<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table','view') AND name = ?1",
        [name],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(SCHEMA)?;
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let mut rows = stmt.query([key])?;
    Ok(match rows.next()? {
        Some(row) => Some(row.get(0)?),
        None => None,
    })
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

/// A migrated in-memory database, configured the same way `open` does.
///
/// Exposed to the crate's tests so modules that build queries against the
/// schema can exercise them without touching the user's real database.
#[cfg(test)]
pub fn memory() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrate(&conn).expect("migrate");
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A migrated in-memory database, configured the same way `open` does.
    fn memory_db() -> Connection {
        memory()
    }

    fn table_exists(conn: &Connection, name: &str) -> bool {
        super::table_exists(conn, name).unwrap()
    }

    #[test]
    fn migration_creates_every_table() {
        let conn = memory_db();
        for t in [
            "categories",
            "cases",
            "people",
            "case_people",
            "documents",
            "conversations",
            "messages",
            "spreadsheets",
            "events",
            "settings",
            "passages_fts",
        ] {
            assert!(table_exists(&conn, t), "missing table {t}");
        }
        for t in ["chunks", "embeddings", "chunks_fts"] {
            assert!(!table_exists(&conn, t), "{t} belongs to the vector database now");
        }
    }

    #[test]
    fn migration_is_idempotent() {
        let conn = memory_db();
        // A second run must not fail on the CREATE IF NOT EXISTS statements.
        migrate(&conn).expect("re-migrate");
    }

    #[test]
    fn fts5_index_matches_passage_text_ignoring_accents() {
        let conn = memory_db();
        conn.execute(
            "INSERT INTO passages_fts (body, chunk_id, document_id) VALUES
             ('O réu descumpriu o acórdão de conciliação.', 'd1:0', 'd1')",
            [],
        )
        .unwrap();
        let hits: Vec<String> = conn
            .prepare("SELECT chunk_id FROM passages_fts WHERE passages_fts MATCH ?1")
            .unwrap()
            .query_map(["acordao"], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(hits, vec!["d1:0".to_string()]);
    }

    /// Deleting a document must take its passages with it, or keyword search
    /// keeps returning text from a document that no longer exists.
    #[test]
    fn deleting_a_document_removes_its_passages() {
        let conn = memory_db();
        conn.execute(
            "INSERT INTO documents (id, file_name, source_path, stored_path, checksum, created_at)
             VALUES ('d1', 'brief.pdf', 'a', 'b', 'sum', 'now'), ('d2', 'other.pdf', 'a', 'b', 's2', 'now')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO passages_fts (body, chunk_id, document_id) VALUES
             ('settlement terms', 'd1:0', 'd1'), ('settlement offer', 'd2:0', 'd2')",
            [],
        )
        .unwrap();
        conn.execute("DELETE FROM documents WHERE id = 'd1'", []).unwrap();
        let left: Vec<String> = conn
            .prepare("SELECT document_id FROM passages_fts")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(left, vec!["d2".to_string()]);
    }

    /// Deleting a case must orphan, not destroy, the work product attached to
    /// it: uploaded documents and spreadsheets outlive the case record.
    #[test]
    fn deleting_a_case_preserves_documents_and_sheets() {
        let conn = memory_db();
        conn.execute(
            "INSERT INTO cases (id, reference, title, opened_at, created_at, updated_at)
             VALUES ('c1', 'CV-42', 'Henderson', 'now', 'now', 'now')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO documents (id, case_id, file_name, source_path, stored_path, checksum, created_at)
             VALUES ('d1', 'c1', 'brief.pdf', 'a', 'b', 'sum', 'now')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO spreadsheets (id, name, case_id, created_at, updated_at)
             VALUES ('s1', 'Damages', 'c1', 'now', 'now')",
            [],
        )
        .unwrap();

        conn.execute("DELETE FROM cases WHERE id = 'c1'", []).unwrap();

        let doc_case: Option<String> = conn
            .query_row("SELECT case_id FROM documents WHERE id = 'd1'", [], |r| r.get(0))
            .unwrap();
        let sheet_case: Option<String> = conn
            .query_row("SELECT case_id FROM spreadsheets WHERE id = 's1'", [], |r| r.get(0))
            .unwrap();
        assert!(doc_case.is_none(), "document should outlive its case");
        assert!(sheet_case.is_none(), "spreadsheet should outlive its case");
    }

    #[test]
    fn deleting_a_case_cascades_to_its_events() {
        // The counterpart to documents/spreadsheets: events are scheduling
        // metadata and are removed with their case rather than orphaned.
        let conn = memory_db();
        conn.execute(
            "INSERT INTO cases (id, reference, title, opened_at, created_at, updated_at)
             VALUES ('c1', 'CV-42', 'Henderson', 'now', 'now', 'now')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO events (id, case_id, title, starts_at, created_at, updated_at)
             VALUES ('e1', 'c1', 'Hearing', '2030-01-01T09:00:00Z', 'now', 'now')",
            [],
        )
        .unwrap();

        conn.execute("DELETE FROM cases WHERE id = 'c1'", []).unwrap();

        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "events should cascade on case delete");
    }

    #[test]
    fn settings_upsert_overwrites() {
        let conn = memory_db();
        set_setting(&conn, "provider.kind", "ollama").unwrap();
        set_setting(&conn, "provider.kind", "openai").unwrap();
        assert_eq!(
            get_setting(&conn, "provider.kind").unwrap().as_deref(),
            Some("openai")
        );
        assert!(get_setting(&conn, "missing").unwrap().is_none());
    }
}
