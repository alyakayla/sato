use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::{Error, Result};

/// Which backend handles embeddings and chat. Kept as a plain string in
/// settings so switching providers never requires a migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    /// "ollama" or "openai".
    pub kind: String,
    pub base_url: String,
    pub chat_model: String,
    pub embedding_model: String,
    pub api_key: String,
}

/// Which backend handles embeddings and chat. Kept as a plain string in
/// settings so switching providers never requires a migration.
impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            kind: "ollama".into(),
            base_url: "http://localhost:11434".into(),
            chat_model: "llama3.1".into(),
            embedding_model: "nomic-embed-text".into(),
            api_key: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAiEmbedResponse {
    data: Vec<OpenAiEmbedDatum>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAiEmbedDatum {
    embedding: Vec<f32>,
}

#[derive(Debug, Clone, Serialize)]
struct OpenAiChatRequest<'a> {
    model: &'a str,
    messages: Vec<OpenAiRequestMessage<'a>>,
    stream: bool,
    temperature: f32,
}

#[derive(Debug, Clone, Serialize)]
struct OpenAiRequestMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Clone, Serialize)]
struct OpenAiEmbedRequest<'a> {
    model: &'a str,
    input: Vec<&'a str>,
}

#[derive(Clone)]
pub struct Provider {
    cfg: ProviderConfig,
    client: reqwest::Client,
}

impl Provider {
    pub fn new(cfg: ProviderConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()?;
        Ok(Self { cfg, client })
    }

    pub fn config(&self) -> &ProviderConfig {
        &self.cfg
    }

    fn auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if self.cfg.api_key.is_empty() {
            builder
        } else {
            builder.bearer_auth(&self.cfg.api_key)
        }
    }

    /// Checks the provider end to end, the way the chat will use it: the
    /// server answers, the key is accepted, and both configured models exist.
    /// Short timeout — this backs a status banner, not a request that must
    /// finish.
    pub async fn diagnose(&self) -> Diagnosis {
        let base = self.cfg.base_url.trim_end_matches('/');
        let wanted = [self.cfg.chat_model.clone(), self.cfg.embedding_model.clone()];
        let timeout = Duration::from_secs(4);
        match self.cfg.kind.as_str() {
            "ollama" => {
                let resp = self.client.get(format!("{base}/api/tags")).timeout(timeout).send().await;
                let resp = match resp.and_then(|r| r.error_for_status()) {
                    Ok(r) => r,
                    Err(e) => return Diagnosis::failed(&Error::Http(e)),
                };
                let installed: Vec<String> = match resp.json::<serde_json::Value>().await {
                    Ok(v) => v["models"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|m| m["name"].as_str().map(str::to_string))
                        .collect(),
                    Err(e) => return Diagnosis::failed(&Error::Http(e)),
                };
                let missing: Vec<String> = wanted
                    .iter()
                    .filter(|w| !installed.iter().any(|i| same_ollama_model(i, w)))
                    .cloned()
                    .collect();
                Diagnosis::with_missing(missing)
            }
            "openai" => {
                if self.cfg.api_key.is_empty() {
                    return Diagnosis { ok: false, problem: Some(Problem::Unauthorized), missing_models: vec![], detail: "no API key configured".into() };
                }
                let resp = self.auth(self.client.get(format!("{base}/models"))).timeout(timeout).send().await;
                let resp = match resp.and_then(|r| r.error_for_status()) {
                    Ok(r) => r,
                    Err(e) => return Diagnosis::failed(&Error::Http(e)),
                };
                let ids: Vec<String> = match resp.json::<serde_json::Value>().await {
                    Ok(v) => v["data"].as_array().into_iter().flatten().filter_map(|m| m["id"].as_str().map(str::to_string)).collect(),
                    Err(e) => return Diagnosis::failed(&Error::Http(e)),
                };
                // Some OpenAI-compatible servers do not list models; only
                // report missing ones when the list is non-empty.
                let missing = if ids.is_empty() { vec![] } else { wanted.iter().filter(|w| !ids.contains(w)).cloned().collect() };
                Diagnosis::with_missing(missing)
            }
            other => Diagnosis { ok: false, problem: Some(Problem::Other), missing_models: vec![], detail: format!("unknown provider '{other}'") },
        }
    }

    /// Embeds a batch of texts. Returns one vector per input, in order.
    pub async fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        match self.cfg.kind.as_str() {
            "ollama" => self.embed_ollama(inputs).await,
            "openai" => self.embed_openai(inputs).await,
            other => Err(Error::NoProvider(format!("unknown provider '{other}'"))),
        }
    }

    async fn embed_ollama(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        let url = format!("{}/api/embed", self.cfg.base_url.trim_end_matches('/'));
        let mut out = Vec::with_capacity(inputs.len());
        // Ollama's /api/embed handles a list, but batching in small groups keeps
        // peak memory sane on large document ingests.
        for batch in inputs.chunks(16) {
            let resp: OllamaEmbedResponse = self
                .client
                .post(&url)
                .json(&serde_json::json!({
                    "model": self.cfg.embedding_model,
                    "input": batch,
                    "keep_alive": KEEP_ALIVE,
                }))
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if resp.embeddings.len() != batch.len() {
                return Err(Error::Message(format!(
                    "embedding provider returned {} vectors for {} inputs",
                    resp.embeddings.len(),
                    batch.len()
                )));
            }
            out.extend(resp.embeddings);
        }
        Ok(out)
    }

    async fn embed_openai(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        let url = format!("{}/embeddings", self.cfg.base_url.trim_end_matches('/'));
        let mut out = Vec::with_capacity(inputs.len());
        // OpenAI caps a single embeddings request at 2048 inputs.
        for batch in inputs.chunks(256) {
            let refs: Vec<&str> = batch.iter().map(|s| s.as_str()).collect();
            let body = OpenAiEmbedRequest { model: &self.cfg.embedding_model, input: refs };
            let resp: OpenAiEmbedResponse = self
                .auth(self.client.post(&url))
                .json(&body)
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            if resp.data.len() != batch.len() {
                return Err(Error::Message(format!(
                    "embedding provider returned {} vectors for {} inputs",
                    resp.data.len(),
                    batch.len()
                )));
            }
            out.extend(resp.data.into_iter().map(|d| d.embedding));
        }
        Ok(out)
    }

    /// Chat completion, streamed. `on_delta` receives each piece of text as the
    /// model produces it, so the UI can show the answer as it is written rather
    /// than after a long silence; the full text is also returned.
    pub async fn chat_stream<F: FnMut(&str)>(&self, messages: &[(String, String)], on_delta: F) -> Result<String> {
        match self.cfg.kind.as_str() {
            "ollama" => self.chat_ollama(messages, on_delta).await,
            "openai" => self.chat_openai(messages, on_delta).await,
            other => Err(Error::NoProvider(format!("unknown provider '{other}'"))),
        }
    }

    async fn chat_ollama<F: FnMut(&str)>(&self, messages: &[(String, String)], mut on_delta: F) -> Result<String> {
        let url = format!("{}/api/chat", self.cfg.base_url.trim_end_matches('/'));
        let msgs: Vec<serde_json::Value> = messages
            .iter()
            .map(|(role, content)| serde_json::json!({ "role": role, "content": content }))
            .collect();
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "model": self.cfg.chat_model,
                "messages": msgs,
                "stream": true,
                "keep_alive": KEEP_ALIVE,
                // Retrieval-grounded answers should be sober, not creative.
                "options": { "temperature": 0.1 },
            }))
            .send()
            .await?
            .error_for_status()?;

        // Newline-delimited JSON: one object per token batch, the last `done`.
        let mut full = String::new();
        read_lines(resp, |line| {
            let v: serde_json::Value = serde_json::from_str(line)?;
            if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                return Err(Error::Message(err.to_string()));
            }
            if let Some(piece) = v.pointer("/message/content").and_then(|c| c.as_str()) {
                if !piece.is_empty() {
                    full.push_str(piece);
                    on_delta(piece);
                }
            }
            Ok(!v.get("done").and_then(|d| d.as_bool()).unwrap_or(false))
        })
        .await?;
        Ok(full)
    }

    async fn chat_openai<F: FnMut(&str)>(&self, messages: &[(String, String)], mut on_delta: F) -> Result<String> {
        let url = format!("{}/chat/completions", self.cfg.base_url.trim_end_matches('/'));
        let msgs: Vec<OpenAiRequestMessage> = messages
            .iter()
            .map(|(role, content)| OpenAiRequestMessage { role, content })
            .collect();
        let body = OpenAiChatRequest {
            model: &self.cfg.chat_model,
            messages: msgs,
            stream: true,
            temperature: 0.1,
        };
        let resp = self.auth(self.client.post(&url)).json(&body).send().await?.error_for_status()?;

        // Server-sent events: `data: {json}` lines, ending with `data: [DONE]`.
        let mut full = String::new();
        read_lines(resp, |line| {
            let Some(data) = line.strip_prefix("data:").map(str::trim) else {
                return Ok(true);
            };
            if data == "[DONE]" {
                return Ok(false);
            }
            let v: serde_json::Value = serde_json::from_str(data)?;
            if let Some(piece) = v.pointer("/choices/0/delta/content").and_then(|c| c.as_str()) {
                if !piece.is_empty() {
                    full.push_str(piece);
                    on_delta(piece);
                }
            }
            Ok(true)
        })
        .await?;
        Ok(full)
    }
}

/// Why a provider call failed, in terms a user can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Problem {
    /// Nothing answered: the server is not installed, not running, or at a
    /// different address.
    Unreachable,
    /// It answered too slowly — often a large model still loading.
    Timeout,
    /// The server is up but the configured model is not downloaded.
    ModelMissing,
    /// The API key was missing or rejected.
    Unauthorized,
    Other,
}

/// Classifies a provider error.
pub fn classify(e: &Error) -> Problem {
    match e {
        Error::Http(re) if re.is_connect() => Problem::Unreachable,
        Error::Http(re) if re.is_timeout() => Problem::Timeout,
        Error::Http(re) => match re.status().map(|s| s.as_u16()) {
            Some(404) => Problem::ModelMissing,
            Some(401) | Some(403) => Problem::Unauthorized,
            _ => Problem::Other,
        },
        // Ollama reports a missing model inside a streamed body:
        // `model "x" not found, try pulling it first`.
        Error::Message(m) if m.contains("not found") && m.contains("model") => Problem::ModelMissing,
        _ => Problem::Other,
    }
}

/// The whole cause chain, so "error sending request" also says *why*
/// ("…: tcp connect error: No connection could be made…").
pub fn describe(e: &Error) -> String {
    let mut out = e.to_string();
    let mut src = std::error::Error::source(e);
    while let Some(s) = src {
        let part = s.to_string();
        if !out.contains(&part) {
            out.push_str(": ");
            out.push_str(&part);
        }
        src = s.source();
    }
    out
}

/// Ollama names carry an implicit `:latest` tag.
fn same_ollama_model(installed: &str, wanted: &str) -> bool {
    let norm = |s: &str| if s.contains(':') { s.to_string() } else { format!("{s}:latest") };
    norm(installed) == norm(wanted)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    pub ok: bool,
    pub problem: Option<Problem>,
    pub missing_models: Vec<String>,
    pub detail: String,
}

impl Diagnosis {
    fn failed(e: &Error) -> Self {
        Diagnosis { ok: false, problem: Some(classify(e)), missing_models: vec![], detail: describe(e) }
    }
    fn with_missing(missing: Vec<String>) -> Self {
        if missing.is_empty() {
            Diagnosis { ok: true, problem: None, missing_models: missing, detail: "connected".into() }
        } else {
            Diagnosis { ok: false, problem: Some(Problem::ModelMissing), detail: format!("missing: {}", missing.join(", ")), missing_models: missing }
        }
    }
}

/// How long Ollama keeps a model in memory after a request. Its default (five
/// minutes) meant a lawyer returning to the chat after a short break paid a
/// multi-second model reload — during which the machine is saturated — on
/// their next question.
const KEEP_ALIVE: &str = "30m";

/// Feeds a streaming response to `on_line` one complete line at a time.
/// `on_line` returns `Ok(false)` to stop early (the stream said it is done).
async fn read_lines<F: FnMut(&str) -> Result<bool>>(mut resp: reqwest::Response, mut on_line: F) -> Result<()> {
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = resp.chunk().await? {
        buf.extend_from_slice(&chunk);
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let text = String::from_utf8_lossy(&line);
            let text = text.trim();
            if !text.is_empty() && !on_line(text)? {
                return Ok(());
            }
        }
    }
    let rest = String::from_utf8_lossy(&buf);
    if !rest.trim().is_empty() {
        on_line(rest.trim())?;
    }
    Ok(())
}

/// Packs a vector into little-endian f32 bytes — the legacy SQLite storage
/// format, kept to build old databases in tests.
#[cfg(test)]
pub fn vec_to_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

#[cfg(test)]
/// Cosine similarity against a stored little-endian `f32` blob, without
/// decoding it into a `Vec` first. `query_norm` is `query`'s L2 norm, computed
/// once per search rather than once per row.
///
/// Search scans every candidate vector, so skipping one allocation and one
/// norm per row is most of the cost of a query.
pub fn cosine_blob(query: &[f32], query_norm: f32, blob: &[u8]) -> f32 {
    if blob.len() != query.len() * 4 || query_norm == 0.0 {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut nb = 0.0f32;
    for (x, c) in query.iter().zip(blob.chunks_exact(4)) {
        let y = f32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        dot += x * y;
        nb += y * y;
    }
    if nb == 0.0 {
        return 0.0;
    }
    dot / (query_norm * nb.sqrt())
}

#[cfg(test)]
pub fn norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ollama_model_names_match_with_or_without_the_latest_tag() {
        assert!(same_ollama_model("nomic-embed-text:latest", "nomic-embed-text"));
        assert!(same_ollama_model("llama3.1:8b", "llama3.1:8b"));
        assert!(!same_ollama_model("llama3.1:8b", "llama3.1"));
    }

    #[test]
    fn a_missing_model_reported_in_a_stream_is_classified() {
        let e = Error::Message("model \"llama3.1\" not found, try pulling it first".into());
        assert_eq!(classify(&e), Problem::ModelMissing);
        assert_eq!(classify(&Error::Message("something else".into())), Problem::Other);
    }

    /// Nothing listens on port 1 on a normal machine: a real connection
    /// failure must read as "unreachable", with its cause spelled out.
    #[test]
    fn a_refused_connection_is_unreachable() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let provider = Provider::new(ProviderConfig { base_url: "http://127.0.0.1:1".into(), ..ProviderConfig::default() }).unwrap();
        let err = rt.block_on(provider.embed(&["x".to_string()])).unwrap_err();
        assert_eq!(classify(&err), Problem::Unreachable);
        assert!(describe(&err).len() > err.to_string().len(), "the cause chain is included");
        let diag = rt.block_on(provider.diagnose());
        assert_eq!((diag.ok, diag.problem), (false, Some(Problem::Unreachable)));
    }

    #[test]
    fn cosine_blob_matches_the_textbook_formula() {
        let q = [1.0f32, 2.0, 3.0];
        let v = [2.0f32, 0.5, -1.0];
        let expected = (1.0 * 2.0 + 2.0 * 0.5 + 3.0 * -1.0) / (norm(&q) * norm(&v));
        let got = cosine_blob(&q, norm(&q), &vec_to_blob(&v));
        assert!((got - expected).abs() < 1e-6, "{got} vs {expected}");
    }

    #[test]
    fn cosine_blob_is_zero_for_mismatched_or_empty_vectors() {
        let q = [1.0f32, 0.0];
        assert_eq!(cosine_blob(&q, norm(&q), &vec_to_blob(&[1.0, 0.0, 0.0])), 0.0);
        assert_eq!(cosine_blob(&q, norm(&q), &vec_to_blob(&[0.0, 0.0])), 0.0);
        assert_eq!(cosine_blob(&[0.0, 0.0], 0.0, &vec_to_blob(&[1.0, 0.0])), 0.0);
    }
}
