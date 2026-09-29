//! What gets sent to the embedding model — distinct from what is stored and
//! cited. Two things are added to the raw passage:
//!
//! 1. **The model's task prefix.** Retrieval embedding models are trained with
//!    asymmetric instructions: `nomic-embed-text` expects `search_document:`
//!    before passages and `search_query:` before questions, e5 expects
//!    `passage:` / `query:`, and bge / mxbai / arctic want an instruction on
//!    the query side. Sending bare text leaves quality on the table for free.
//!
//! 2. **A contextual header** — `{case reference} — {file name} — p. N` — on
//!    passages. A chunk such as "The tenant shall give sixty days' notice"
//!    says nothing about *which* case or lease it belongs to, so a question
//!    naming the case cannot find it among look-alike clauses from other
//!    matters. The header puts that context into the vector; the stored and
//!    cited text stays untouched.
//!
//! Changing either changes every document vector, so the recipe is versioned
//! (`format_id`) and a change triggers a background re-embed.

use crate::models::Chunk;

/// Bump when the document or query recipe changes, so stored vectors are
/// rebuilt to match.
pub const RECIPE_VERSION: u32 = 2;

/// Whether task prefixes are used in production. Off, by measurement: on the
/// retrieval eval (eval.rs) with nomic-embed-text, prefixes never improved a
/// question and cost a few — MRR 0.843 plain vs 0.787 with prefixes, and
/// 0.972 headers-only vs 0.944 headers+prefixes. The documented usage says
/// otherwise, so the eval keeps measuring both; flip this (and bump
/// `RECIPE_VERSION`) if a model or a larger corpus shows a gain.
pub const USE_TASK_PREFIX: bool = false;

fn prefix(model: &str, role: Role) -> &'static str {
    if USE_TASK_PREFIX {
        task_prefix(model, role)
    } else {
        ""
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Query,
    Document,
}

/// The model family's task prefix. Unknown models (OpenAI's
/// `text-embedding-3-*`, `all-minilm`, …) get none: they are symmetric.
pub fn task_prefix(model: &str, role: Role) -> &'static str {
    let m = model.to_ascii_lowercase();
    if m.contains("nomic-embed") {
        return match role {
            Role::Query => "search_query: ",
            Role::Document => "search_document: ",
        };
    }
    if m.contains("e5") && (m.contains("multilingual") || m.contains("e5-")) {
        return match role {
            Role::Query => "query: ",
            Role::Document => "passage: ",
        };
    }
    if m.contains("mxbai-embed") || m.contains("snowflake-arctic-embed") || m.contains("bge-") {
        return match role {
            Role::Query => "Represent this sentence for searching relevant passages: ",
            Role::Document => "",
        };
    }
    ""
}

/// `case-01 — lease.pdf — p. 3`, leaving out whatever is unknown.
pub fn context_header(case_reference: Option<&str>, file_name: &str, page: Option<i64>) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(3);
    if let Some(c) = case_reference.filter(|c| !c.trim().is_empty()) {
        parts.push(c.trim().to_string());
    }
    if !file_name.trim().is_empty() {
        parts.push(file_name.trim().to_string());
    }
    if let Some(p) = page {
        parts.push(format!("p. {p}"));
    }
    parts.join(" — ")
}

/// The text embedded for a stored passage.
pub fn document_input(model: &str, chunk: &Chunk, case_reference: Option<&str>, file_name: &str) -> String {
    let header = context_header(case_reference, file_name, chunk.page);
    let prefix = prefix(model, Role::Document);
    if header.is_empty() {
        format!("{prefix}{}", chunk.text)
    } else {
        format!("{prefix}{header}\n\n{}", chunk.text)
    }
}

/// The text embedded for a user's question.
pub fn query_input(model: &str, question: &str) -> String {
    format!("{}{}", prefix(model, Role::Query), question)
}

/// Identifies how stored vectors were produced: recipe version and model.
/// When it differs from the one recorded, every document is re-embedded.
pub fn format_id(model: &str) -> String {
    format!("v{RECIPE_VERSION}|{model}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(text: &str, page: Option<i64>) -> Chunk {
        Chunk {
            id: "d:0".into(),
            document_id: "d".into(),
            ordinal: 0,
            start_char: 0,
            end_char: text.len() as i64,
            page,
            text: text.into(),
            token_estimate: 1,
        }
    }

    #[test]
    fn task_prefixes_by_model_family() {
        assert_eq!(task_prefix("nomic-embed-text:latest", Role::Query), "search_query: ");
        assert_eq!(task_prefix("nomic-embed-text", Role::Document), "search_document: ");
        assert!(task_prefix("mxbai-embed-large", Role::Query).starts_with("Represent this sentence"));
        assert_eq!(task_prefix("mxbai-embed-large", Role::Document), "");
        assert_eq!(task_prefix("text-embedding-3-small", Role::Query), "");
    }

    #[test]
    fn production_inputs_carry_the_header_and_follow_the_prefix_switch() {
        let d = document_input("nomic-embed-text", &chunk("Sixty days.", Some(3)), Some("case-01"), "lease.pdf");
        let q = query_input("nomic-embed-text", "notice period?");
        if USE_TASK_PREFIX {
            assert_eq!(d, "search_document: case-01 — lease.pdf — p. 3\n\nSixty days.");
            assert_eq!(q, "search_query: notice period?");
        } else {
            assert_eq!(d, "case-01 — lease.pdf — p. 3\n\nSixty days.");
            assert_eq!(q, "notice period?");
        }
    }

    #[test]
    fn a_passage_without_context_is_embedded_as_is() {
        let d = document_input("text-embedding-3-small", &chunk("Body.", None), None, "");
        assert_eq!(d, "Body.");
    }

    #[test]
    fn the_header_omits_what_is_unknown() {
        assert_eq!(context_header(None, "a.pdf", None), "a.pdf");
        assert_eq!(context_header(Some("  "), "", Some(2)), "p. 2");
        assert_eq!(context_header(None, "", None), "");
    }

    #[test]
    fn the_stored_text_is_never_changed() {
        let c = chunk("The defendant shall pay.", Some(1));
        let _ = document_input("nomic-embed-text", &c, Some("case-1"), "order.pdf");
        assert_eq!(c.text, "The defendant shall pay.");
    }

    #[test]
    fn the_format_changes_with_the_model() {
        assert_ne!(format_id("nomic-embed-text"), format_id("mxbai-embed-large"));
    }
}
