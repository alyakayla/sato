use once_cell::sync::Lazy;
use regex::Regex;

use crate::models::Chunk;

/// A section of text plus the page it came from, so citations stay accurate.
struct Slice<'a> {
    text: &'a str,
    page: Option<i64>,
    offset: usize,
}

pub const CHUNK_TARGET_CHARS: usize = 1400;
pub const CHUNK_OVERLAP_CHARS: usize = 220;

/// Rough token estimate. Good enough for budgeting without pulling in a
/// tokenizer per provider model.
pub fn estimate_tokens(text: &str) -> usize {
    (text.len() as f64 / 4.0).ceil() as usize + 1
}

/// Splits text into overlapping chunks that respect sentence and paragraph
/// boundaries, and records the page each chunk came from.
pub fn chunk_text(pages: &[String], doc_id: &str) -> Vec<Chunk> {
    let slices = build_slices(pages);
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut ordinal = 0i64;

    let mut i = 0usize;
    while i < slices.len() {
        let mut buf = String::new();
        let start_offset = slices[i].offset;
        let mut end_offset = slices[i].offset;
        let mut page = slices[i].page;
        let mut j = i;

        while j < slices.len() {
            let s = &slices[j];
            let candidate_len = buf.len() + s.text.len() + 1;
            if !buf.is_empty() && candidate_len > CHUNK_TARGET_CHARS {
                break;
            }
            if !buf.is_empty() {
                buf.push(' ');
            }
            buf.push_str(s.text);
            end_offset = s.offset + s.text.len();
            if page.is_none() {
                page = s.page;
            }
            j += 1;
            if buf.len() >= CHUNK_TARGET_CHARS {
                break;
            }
        }

        if buf.trim().is_empty() {
            i = j.max(i + 1);
            continue;
        }

        chunks.push(Chunk {
            id: format!("{doc_id}:{ordinal}"),
            document_id: doc_id.to_string(),
            ordinal,
            start_char: start_offset as i64,
            end_char: end_char_i64(end_offset),
            page,
            token_estimate: estimate_tokens(&buf) as i64,
            text: buf.clone(),
        });
        ordinal += 1;

        if j >= slices.len() {
            break;
        }

        // Step back by roughly one overlap width so context spans chunk borders.
        let overlap_target = CHUNK_OVERLAP_CHARS;
        let mut back = j;
        let mut acc = 0usize;
        while back > i {
            back -= 1;
            acc += slices[back].text.len();
            if acc >= overlap_target {
                back += 1;
                break;
            }
        }
        i = if back > i { back } else { j };
    }

    chunks
}

fn end_char_i64(v: usize) -> i64 {
    v as i64
}

/// Produces page-aware slices: paragraphs when we have pages, sentences
/// otherwise.
fn build_slices(pages: &[String]) -> Vec<Slice<'_>> {
    let mut out: Vec<Slice<'_>> = Vec::new();
    let mut offset = 0usize;

    for (page_idx, page_text) in pages.iter().enumerate() {
        let page_no = Some(page_idx as i64 + 1);
        for para in split_paragraphs(page_text) {
            let start = offset;
            out.push(Slice { text: para, page: page_no, offset: start });
            offset += para.len() + 1;
        }
    }
    out
}

/// Splits into paragraph-sized units, then breaks any oversized unit on
/// sentence boundaries so a single huge paragraph cannot blow past the target.
fn split_paragraphs(text: &str) -> Vec<&str> {
    static PARAGRAPH: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\n\s*\n").expect("valid paragraph regex"));

    let mut units: Vec<&str> = Vec::new();
    for para in PARAGRAPH.split(text) {
        let para = para.trim();
        if para.is_empty() {
            continue;
        }
        if para.len() <= CHUNK_TARGET_CHARS {
            units.push(para);
            continue;
        }
        units.extend(split_sentences(para));
    }
    units
}

/// Splits on sentence boundaries. Written by hand rather than with a regex
/// because the `regex` crate has no look-around, which is exactly what a
/// "period followed by whitespace followed by a capital" rule needs.
fn split_sentences(para: &str) -> Vec<&str> {
    let bytes = para.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        if !matches!(bytes[i], b'.' | b'!' | b'?') {
            i += 1;
            continue;
        }
        // Consume runs like "?!" or "..." as one terminator.
        let mut j = i;
        while j + 1 < bytes.len() && matches!(bytes[j + 1], b'.' | b'!' | b'?') {
            j += 1;
        }
        // Require whitespace or end of text after the terminator.
        let next = para[j + 1..].chars().next();
        let boundary = match next {
            None => true,
            Some(c) => c.is_whitespace(),
        };
        if !boundary {
            i = j + 1;
            continue;
        }
        // Look ahead past the whitespace for a capital, digit, or opening quote;
        // otherwise the period is probably an abbreviation ("v.", "No. 5").
        let after = para[j + 1..].trim_start();
        let starts_sentence = after
            .chars()
            .next()
            .map(|c| c.is_uppercase() || c.is_ascii_digit() || c == '"' || c == '\'')
            .unwrap_or(true);

        if starts_sentence {
            let end = j + 1;
            let piece = &para[start..end];
            if !piece.trim().is_empty() {
                out.push(piece);
            }
            start = end;
            // Skip the run of whitespace so the next slice begins on a letter.
            i = end + (para[end..].len() - after.len());
        } else {
            i = j + 1;
        }
    }
    if start < para.len() {
        let piece = &para[start..];
        if !piece.trim().is_empty() {
            out.push(piece);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_stay_near_target() {
        let para = "The court finds that the defendant breached the agreement. ".repeat(60);
        let pages = vec![para];
        let chunks = chunk_text(&pages, "doc1");
        assert!(chunks.len() > 1, "expected multiple chunks, got {}", chunks.len());
        for c in &chunks {
            assert!(c.text.len() <= CHUNK_TARGET_CHARS * 2, "chunk too large: {}", c.text.len());
            assert_eq!(c.page, Some(1));
        }
    }

    #[test]
    fn chunks_are_sequential_and_overlap() {
        let para = "Sentence number one is here. Sentence number two follows. ".repeat(40);
        let chunks = chunk_text(&[para], "doc2");
        for (i, c) in chunks.iter().enumerate() {
            assert_eq!(c.ordinal, i as i64);
            assert!(c.end_char > c.start_char);
        }
        if chunks.len() > 1 {
            assert!(chunks[1].start_char < chunks[0].end_char, "expected overlap");
        }
    }

    #[test]
    fn empty_text_produces_no_chunks() {
        assert!(chunk_text(&["   \n\n  ".to_string()], "doc3").is_empty());
    }

    #[test]
    fn page_numbers_are_tracked() {
        let pages = vec!["First page content.".to_string(), "Second page content.".to_string()];
        let chunks = chunk_text(&pages, "doc4");
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].page, Some(1));
        if chunks.len() > 1 {
            assert_eq!(chunks[1].page, Some(2));
        }
    }
}
