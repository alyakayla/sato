use std::path::Path;

use crate::error::{Error, Result};

/// Text pulled out of a source file, with page boundaries preserved when the
/// format allows it. Page breaks let citations point at a real page number.
pub struct Extracted {
    pub text: String,
    pub pages: Option<Vec<String>>,
}

pub fn guess_mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("pdf") => "application/pdf",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("doc") => "application/msword",
        Some("txt") | Some("md") | Some("markdown") => "text/plain",
        Some("rtf") => "application/rtf",
        Some("html") | Some("htm") => "text/html",
        Some("csv") => "text/csv",
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("eml") => "message/rfc822",
        Some("msg") => "application/vnd.ms-outlook",
        _ => "application/octet-stream",
    }
}

/// Returns true for formats we can pull readable text out of.
pub fn is_supported(mime: &str) -> bool {
    matches!(
        mime,
        "application/pdf"
            | "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            | "application/msword"
            | "text/plain"
            | "application/rtf"
            | "text/html"
            | "text/csv"
            | "text/markdown"
            | "application/json"
            | "application/xml"
    )
}

pub fn extract(path: &Path, mime: &str) -> Result<Extracted> {
    match mime {
        "application/pdf" => extract_pdf(path),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        | "application/msword" => extract_word(path, mime),
        "application/rtf" => extract_rtf(path),
        "text/html" => {
            let html = read_text(path)?;
            Ok(Extracted {
                text: strip_html(&html),
                pages: None,
            })
        }
        "text/plain" | "text/csv" | "application/json" | "application/xml" | "text/markdown" => {
            let text = read_text(path)?;
            Ok(Extracted { text, pages: None })
        }
        other => Err(Error::Extract {
            path: path.display().to_string(),
            reason: format!("unsupported file type '{other}'"),
        }),
    }
}

fn read_text(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    // Honour a UTF-8/UTF-16 BOM, otherwise fall back to Windows-1252 which is
    // what most legacy .doc exports use.
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Ok(String::from_utf8_lossy(&bytes[3..]).into_owned());
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let body = &bytes[2..];
        let units: Vec<u16> = body
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&p| u16::from_le_bytes(p))
            .collect();
        return Ok(String::from_utf16_lossy(&units));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        // Big-endian UTF-16: swap each byte pair so we can reuse from_utf16.
        let body = &bytes[2..];
        let units: Vec<u16> = body
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&p| u16::from_be_bytes(p))
            .collect();
        return Ok(String::from_utf16_lossy(&units));
    }
    let (text, _, _) = encoding_rs::WINDOWS_1252.decode(&bytes);
    if text.contains('\u{FFFD}') {
        return Ok(String::from_utf8_lossy(&bytes).into_owned());
    }
    Ok(text.into_owned())
}

fn extract_pdf(path: &Path) -> Result<Extracted> {
    let bytes = std::fs::read(path)?;
    let text = pdf_extract::extract_text_from_mem(&bytes).map_err(|e| Error::Extract {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    // pdf-extract returns a flat string; count form feeds as pages when present.
    let pages: Vec<String> = if text.contains('\u{0C}') {
        text.split('\u{0C}').map(|p| p.trim().to_string()).collect()
    } else {
        Vec::new()
    };
    Ok(Extracted {
        text,
        pages: if pages.is_empty() { None } else { Some(pages) },
    })
}

fn extract_word(path: &Path, mime: &str) -> Result<Extracted> {
    let bytes = std::fs::read(path)?;
    // Old binary .doc is a compound file we cannot parse without a converter;
    // .docx is just a zip of XML so we walk it directly.
    if mime == "application/msword" {
        return Err(Error::Extract {
            path: path.display().to_string(),
            reason: "legacy .doc is not supported, please save as .docx or PDF".into(),
        });
    }

    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| Error::Extract {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;

    let mut out = String::new();
    let mut names: Vec<String> = archive.file_names().map(|n| n.to_string()).collect();
    names.sort();

    for name in names {
        if !name.starts_with("word/") || !name.ends_with(".xml") {
            continue;
        }
        if !(name == "word/document.xml" || name.contains("header") || name.contains("footer")) {
            continue;
        }
        let mut file = archive.by_name(&name).map_err(|e| Error::Extract {
            path: path.display().to_string(),
            reason: e.to_string(),
        })?;
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut file, &mut xml)?;
        out.push_str(&docx_xml_to_text(&xml));
        out.push('\n');
    }
    Ok(Extracted {
        text: out,
        pages: None,
    })
}

fn docx_xml_to_text(xml: &str) -> String {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut out = String::new();
    let mut buf = Vec::new();
    let mut in_text_run = false;
    // Depth of the paragraph element we are currently inside, if any.
    let mut para_depth: Option<usize> = None;
    let mut depth = 0usize;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                match e.local_name().as_ref() {
                    b"t" => in_text_run = true,
                    b"p" if para_depth.is_none() => para_depth = Some(depth),
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                match e.local_name().as_ref() {
                    b"t" => in_text_run = false,
                    b"tab" => out.push('\t'),
                    b"br" | b"cr" => out.push('\n'),
                    b"p" if para_depth == Some(depth) => {
                        para_depth = None;
                        out.push('\n');
                    }
                    _ => {}
                }
                depth = depth.saturating_sub(1);
            }
            Ok(Event::Text(t)) => {
                if in_text_run {
                    out.push_str(&t.unescape().unwrap_or_default());
                }
            }
            Ok(Event::CData(t)) => {
                if in_text_run {
                    out.push_str(&String::from_utf8_lossy(t.as_ref()));
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

fn extract_rtf(path: &Path) -> Result<Extracted> {
    let raw = read_text(path)?;
    let mut out = String::new();
    let mut chars = raw.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.peek() {
                Some('u') => {
                    chars.next();
                    // \uNNNN unicode escape, possibly with a fallback '?' run.
                    let mut digits = String::new();
                    while let Some(d) = chars.peek() {
                        if d.is_ascii_digit() && digits.len() < 4 {
                            digits.push(*d);
                            chars.next();
                        } else if *d == ' ' && !digits.is_empty() {
                            chars.next();
                            break;
                        } else {
                            break;
                        }
                    }
                    if let Ok(code) = u32::from_str_radix(&digits, 16) {
                        if let Some(ch) = char::from_u32(code) {
                            out.push(ch);
                        }
                    }
                }
                Some(word_char) if word_char.is_ascii_alphabetic() => {
                    // Skip the control word and its numeric argument.
                    while let Some(w) = chars.peek() {
                        if w.is_ascii_alphabetic() {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if chars.peek() == Some(&'-') {
                        chars.next();
                    }
                    while let Some(n) = chars.peek() {
                        if n.is_ascii_digit() {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            },
            '{' | '}' => {}
            other => out.push(other),
        }
    }
    Ok(Extracted {
        text: out,
        pages: None,
    })
}

fn strip_html(html: &str) -> String {
    static SCRIPT: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        // No backreferences in the regex crate, so each element closes itself.
        regex::Regex::new(
            r"(?is)<script[^>]*>.*?</script>|<style[^>]*>.*?</style>|<head[^>]*>.*?</head>",
        )
        .expect("hard-coded script/style regex is valid")
    });
    static TAG: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?s)<[^>]+>").expect("hard-coded tag regex is valid")
    });
    static ENTITIES: [(&str, &str); 6] = [
        ("&nbsp;", " "),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
    ];

    let mut text = SCRIPT.replace_all(html, " ").into_owned();
    text = TAG.replace_all(&text, " ").into_owned();
    for (from, to) in ENTITIES {
        text = text.replace(from, to);
    }
    collapse_whitespace(&text)
}

pub fn collapse_whitespace(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_space = true;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(ch);
            last_was_space = false;
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod html_tests {
    use super::strip_html;

    #[test]
    fn strips_tags_and_the_contents_of_script_style_and_head() {
        let html = "<html><head><title>T</title></head><body><style>p{}</style>\
                    <p>Hello &amp; <b>bye</b></p><script>alert(1)</script></body></html>";
        assert_eq!(strip_html(html), "Hello & bye");
    }
}
