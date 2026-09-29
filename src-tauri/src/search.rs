use rusqlite::{params, Connection};

use crate::models::SearchHit;
use crate::vectordb::{Filter, VectorDb};

/// Full-text search over chunk bodies using FTS5 BM25 ranking, optionally
/// restricted to one case and/or a set of documents.
pub fn keyword_search(
    conn: &Connection,
    query: &str,
    limit: usize,
    case_filter: Option<&str>,
    docs: &[String],
) -> rusqlite::Result<Vec<(String, f64)>> {
    let match_query = to_fts_query(query);
    if match_query.is_empty() {
        return Ok(Vec::new());
    }

    let mut sql = String::from(
        "SELECT f.chunk_id, bm25(passages_fts) AS rank
           FROM passages_fts f
           JOIN documents d ON d.id = f.document_id
          WHERE passages_fts MATCH ?1 AND d.index_status = 'Ready'",
    );
    let mut binds: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(match_query)];
    // Placeholders are numbered by hand because both filters are optional, so
    // the LIMIT index moves depending on what is bound.
    let mut param = 2;
    if let Some(case) = case_filter {
        sql.push_str(&format!(" AND d.case_id = ?{param}"));
        binds.push(Box::new(case.to_string()));
        param += 1;
    }
    if !docs.is_empty() {
        // One query for every mentioned document, so their passages compete in
        // a single BM25 ranking instead of separate per-document lists.
        let marks: Vec<String> = docs
            .iter()
            .map(|d| {
                binds.push(Box::new(d.clone()));
                let m = format!("?{param}");
                param += 1;
                m
            })
            .collect();
        sql.push_str(&format!(" AND d.id IN ({})", marks.join(", ")));
    }
    sql.push_str(&format!(" ORDER BY rank LIMIT ?{param}"));
    binds.push(Box::new(limit as i64));

    let mut stmt = conn.prepare(&sql)?;
    let refs: Vec<&dyn rusqlite::ToSql> = binds.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), |r| {
        // bm25 returns a negative number where more negative is a better match.
        Ok((r.get::<_, String>(0)?, -r.get::<_, f64>(1)?))
    })?;
    rows.collect()
}

/// Words that carry no retrieval signal in English or Portuguese questions.
/// Matching on them only adds noise to BM25.
const STOPWORDS: &[&str] = &[
    // English
    "a", "an", "and", "are", "as", "at", "be", "by", "can", "did", "do", "does", "for", "from",
    "had", "has", "have", "how", "i", "in", "is", "it", "its", "me", "my", "of", "on", "or",
    "our", "please", "that", "the", "their", "there", "this", "to", "was", "we", "were", "what",
    "when", "where", "which", "who", "why", "will", "with", "you", "your", "about", "any", "all",
    "tell", "show", "list", "give", "find",
    // Portuguese
    "o", "os", "as", "um", "uma", "uns", "umas", "de", "da", "do", "das", "dos", "em", "na",
    "no", "nas", "nos", "e", "ou", "que", "se", "por", "para", "com", "sem", "ao", "aos", "à",
    "às", "é", "foi", "ser", "são", "qual", "quais", "quando", "onde", "como", "quem", "porque",
    "isso", "isto", "esse", "essa", "este", "esta", "meu", "minha", "seu", "sua", "me", "nos",
    "há", "tem", "sobre", "todos", "todas", "mostre", "liste",
];

/// Turns a natural-language question into an FTS5 MATCH expression.
///
/// Terms are OR-ed, not AND-ed: requiring every word of "what did the
/// contract say about termination" to appear in one passage meant keyword
/// search almost never matched a real question, and hybrid retrieval quietly
/// degraded to vectors only. With OR, BM25 still ranks passages holding more
/// (and rarer) terms first — exactly what statute numbers and defined terms
/// need. Stopwords are dropped, every term is a quoted prefix term so
/// punctuation cannot break the parser, and very long questions are capped.
pub(crate) fn to_fts_query(query: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        // Keep numbers of any length ("5", "12"): section and clause numbers matter.
        .filter(|t| t.chars().all(|c| c.is_ascii_digit()) || (t.chars().count() > 1 && !STOPWORDS.contains(&t.as_str())))
        .filter(|t| seen.insert(t.clone()))
        .take(16)
        .map(|t| format!("\"{}\"*", t.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// Blends vector and keyword results using reciprocal rank fusion.
///
/// Pure cosine similarity under-weights rare exact terms such as statute
/// numbers, while pure FTS misses paraphrase. RRF combines rankings without
/// needing the two score scales to be comparable. `docs` restricts both sides
/// to the documents a message @mentioned.
pub fn hybrid_search(
    conn: &Connection,
    vectors: &VectorDb,
    query: &[f32],
    query_text: &str,
    limit: usize,
    case_filter: Option<&str>,
    docs: &[String],
) -> rusqlite::Result<Vec<SearchHit>> {
    let over_fetch = (limit * 4).max(20);
    let vec_hits = vectors.search(query, over_fetch, Filter { docs, case: case_filter });
    let kw_hits = keyword_search(conn, query_text, over_fetch, case_filter, docs)?;

    const K: f64 = 60.0;
    let mut scores: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for (rank, (id, _)) in vec_hits.iter().enumerate() {
        *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (K + rank as f64 + 1.0);
    }
    for (rank, (id, _)) in kw_hits.iter().enumerate() {
        *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (K + rank as f64 + 1.0);
    }

    let mut fused: Vec<(String, f64)> = scores.into_iter().collect();
    // Ties break on chunk id so the same question always yields the same context.
    fused.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    fused.truncate(limit);

    // Normalize RRF scores to 0..1 so the UI can show a confidence percentage.
    let max = fused.first().map(|(_, s)| *s).unwrap_or(1.0).max(f64::EPSILON);

    let mut out = Vec::with_capacity(fused.len());
    for (chunk_id, score) in fused {
        if let Some(hit) = hydrate(conn, vectors, &chunk_id, score / max)? {
            out.push(hit);
        }
    }
    Ok(out)
}

/// A chunk id → a displayable hit: the passage from the vector database, its
/// document name and case reference from SQLite. None when the chunk is no
/// longer searchable (its document was deleted or is being reindexed).
pub fn hydrate(
    conn: &Connection,
    vectors: &VectorDb,
    chunk_id: &str,
    score: f64,
) -> rusqlite::Result<Option<SearchHit>> {
    let Some(chunk) = vectors.chunk(chunk_id) else { return Ok(None) };
    let found = conn.query_row(
        "SELECT d.file_name, c.reference
           FROM documents d
           LEFT JOIN cases c ON c.id = d.case_id
          WHERE d.id = ?1",
        params![chunk.document_id],
        |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)),
    );
    let (document_name, case_reference) = match found {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(e),
    };
    Ok(Some(SearchHit { chunk, score, document_name, case_reference }))
}

#[cfg(test)]
mod tests {
    use super::to_fts_query;

    #[test]
    fn questions_become_or_queries_without_stopwords() {
        assert_eq!(
            to_fts_query("What did the contract say about termination?"),
            "\"contract\"* OR \"say\"* OR \"termination\"*"
        );
    }

    #[test]
    fn portuguese_stopwords_are_dropped_too() {
        assert_eq!(to_fts_query("Qual é o prazo do recurso?"), "\"prazo\"* OR \"recurso\"*");
    }

    #[test]
    fn numbers_survive_even_when_short() {
        assert_eq!(to_fts_query("section 5 of the lease"), "\"section\"* OR \"5\"* OR \"lease\"*");
    }

    #[test]
    fn duplicates_and_punctuation_are_handled() {
        assert_eq!(to_fts_query("\"notice\" notice; NOTICE"), "\"notice\"*");
        assert_eq!(to_fts_query("the of and"), "");
    }
}
