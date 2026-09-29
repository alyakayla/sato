//! Counsel's embedded vector database.
//!
//! One collection of *points*: a chunk of a document (its text and position,
//! the payload) together with its embedding. It lives in memory — the app is
//! a standalone desktop program, so there is no server to run — and persists
//! itself as one segment file per document, so importing or deleting a
//! document rewrites only that document's file, never the whole store.
//!
//! Search is planned per query, the way server vector databases do it:
//!
//! - **Exact** (a full dot-product scan) when the collection is small, or when
//!   a filter narrows the candidates enough — an @mentioned document, a case.
//!   Exact is also the most precise answer, so it is preferred whenever it is
//!   cheap.
//! - **HNSW** (approximate nearest neighbours, via `hnsw_rs`) once the
//!   collection is large. The graph is built on a background thread; until
//!   it is ready, and for points added since it was built, queries fall back
//!   to exact scanning, so results never disappear while the index catches up.
//!
//! Deletes leave tombstones that searches skip; when too many accumulate the
//! store compacts and rebuilds the graph in the background.
//!
//! Vectors are stored L2-normalised, so cosine similarity is a plain dot
//! product. Scores are cosines in (0, 1].

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use hnsw_rs::prelude::{Distance, Hnsw};

use crate::models::Chunk;

/// Below this many candidate points a query is answered exactly: at this size
/// a scan is a few milliseconds and loses nothing to approximation.
const EXACT_MAX: usize = 20_000;
/// Build the graph only once the collection is big enough to need it.
const GRAPH_MIN: usize = 20_000;
/// Points added since the graph was built are scanned exactly; past this many
/// they are folded into the graph.
const TAIL_MAX: usize = 4_000;
/// Compact and rebuild once this share of slots is dead.
const TOMBSTONE_RATIO: f32 = 0.25;

const HNSW_M: usize = 24;
const HNSW_EF_CONSTRUCTION: usize = 200;
const HNSW_MAX_LAYER: usize = 16;

/// Cosine distance over unit vectors. `hnsw_rs`'s own `DistDot` asserts that
/// `1 - dot` is non-negative, which rounding on two near-identical unit
/// vectors can violate — a panic in the middle of a search. This clamps.
#[derive(Default, Clone, Copy)]
pub struct UnitCosine;

impl Distance<f32> for UnitCosine {
    fn eval(&self, a: &[f32], b: &[f32]) -> f32 {
        (1.0 - dot(a, b)).max(0.0)
    }
}

#[inline]
fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Scales to unit length. None for a zero or non-finite vector.
fn normalized(v: &[f32]) -> Option<Vec<f32>> {
    let n = dot(v, v).sqrt();
    if n == 0.0 || !n.is_finite() {
        return None;
    }
    Some(v.iter().map(|x| x / n).collect())
}

struct Point {
    chunk: Chunk,
    vector: Arc<[f32]>,
}

struct Graph {
    hnsw: Hnsw<'static, f32, UnitCosine>,
    /// Slots below this are in the graph; slots at or above it are the tail.
    /// Atomic so a background pass can extend a graph that searches share.
    covers: AtomicUsize,
    dim: usize,
}

#[derive(Default)]
struct Inner {
    /// Slot → point. `None` is a tombstone.
    points: Vec<Option<Point>>,
    by_chunk: HashMap<String, usize>,
    /// Document → its live slots, in chunk order.
    by_doc: HashMap<String, Vec<usize>>,
    doc_case: HashMap<String, Option<String>>,
    /// How many live points have each vector width. The widest population is
    /// the one the graph indexes (the others are left over from a changed
    /// embedding model and are reindexed away over time).
    dims: HashMap<usize, usize>,
    live: usize,
    graph: Option<Arc<Graph>>,
}

impl Inner {
    fn main_dim(&self) -> usize {
        self.dims.iter().max_by_key(|(_, n)| **n).map(|(d, _)| *d).unwrap_or(0)
    }

    fn tombstones(&self) -> usize {
        self.points.len() - self.live
    }

    fn remove_doc(&mut self, doc: &str) {
        if let Some(slots) = self.by_doc.remove(doc) {
            for s in slots {
                if let Some(p) = self.points[s].take() {
                    self.by_chunk.remove(&p.chunk.id);
                    let n = self.dims.entry(p.vector.len()).or_default();
                    *n = n.saturating_sub(1);
                    self.live -= 1;
                }
            }
        }
        self.dims.retain(|_, n| *n > 0);
    }

    fn insert_doc(&mut self, doc: &str, points: Vec<Point>) {
        let mut slots = Vec::with_capacity(points.len());
        for p in points {
            let slot = self.points.len();
            self.by_chunk.insert(p.chunk.id.clone(), slot);
            *self.dims.entry(p.vector.len()).or_default() += 1;
            self.points.push(Some(p));
            self.live += 1;
            slots.push(slot);
        }
        if !slots.is_empty() {
            self.by_doc.insert(doc.to_string(), slots);
        }
    }
}

/// A search restriction. Empty `docs` means every document.
#[derive(Default, Clone, Copy)]
pub struct Filter<'a> {
    pub docs: &'a [String],
    pub case: Option<&'a str>,
}

impl Filter<'_> {
    fn allows_doc(&self, inner: &Inner, doc: &str) -> bool {
        (self.docs.is_empty() || self.docs.iter().any(|d| d == doc))
            && self.case.map_or(true, |c| {
                inner.doc_case.get(doc).and_then(|x| x.as_deref()) == Some(c)
            })
    }
}

pub struct VectorDb {
    dir: PathBuf,
    inner: RwLock<Inner>,
    maintaining: AtomicBool,
}

/// What `open` found on disk.
pub struct OpenReport {
    pub documents: usize,
    pub points: usize,
    /// Segment files that could not be read. Their documents must be
    /// reindexed; the files have been removed.
    pub corrupt: Vec<String>,
}

impl VectorDb {
    /// Opens (or creates) the store in `dir`, loading every segment.
    pub fn open(dir: &Path) -> io::Result<(Arc<Self>, OpenReport)> {
        fs::create_dir_all(dir)?;
        let mut inner = Inner::default();
        let mut report = OpenReport { documents: 0, points: 0, corrupt: Vec::new() };
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            match path.extension().and_then(|e| e.to_str()) {
                Some("seg") => match read_segment(&path) {
                    Ok((doc, chunks)) => {
                        let points = to_points(chunks);
                        report.points += points.len();
                        report.documents += 1;
                        inner.insert_doc(&doc, points);
                    }
                    Err(_) => {
                        let doc = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
                        report.corrupt.push(doc);
                        let _ = fs::remove_file(&path);
                    }
                },
                // A write interrupted mid-way; the previous segment is intact.
                Some("tmp") => {
                    let _ = fs::remove_file(&path);
                }
                _ => {}
            }
        }
        let db = Arc::new(VectorDb {
            dir: dir.to_path_buf(),
            inner: RwLock::new(inner),
            maintaining: AtomicBool::new(false),
        });
        Ok((db, report))
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Inner> {
        self.inner.write().unwrap_or_else(|e| e.into_inner())
    }

    fn segment_path(&self, doc: &str) -> PathBuf {
        self.dir.join(format!("{}.seg", sanitize(doc)))
    }

    /// Replaces a document's points: persists its segment first (atomically),
    /// then swaps it in memory. `chunks` and `vectors` pair up by index.
    pub fn upsert_document(
        self: &Arc<Self>,
        doc: &str,
        case: Option<&str>,
        chunks: Vec<Chunk>,
        vectors: &[Vec<f32>],
    ) -> io::Result<()> {
        if chunks.len() != vectors.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "one vector per chunk is required"));
        }
        let pairs: Vec<(Chunk, Vec<f32>)> = chunks
            .into_iter()
            .zip(vectors)
            .filter_map(|(c, v)| normalized(v).map(|n| (c, n)))
            .collect();
        write_segment(&self.segment_path(doc), doc, &pairs)?;
        {
            let mut inner = self.write();
            inner.remove_doc(doc);
            inner.insert_doc(doc, pairs.into_iter().map(|(chunk, v)| Point { chunk, vector: v.into() }).collect());
            inner.doc_case.insert(doc.to_string(), case.map(str::to_string));
        }
        self.maintain();
        Ok(())
    }

    pub fn remove_document(self: &Arc<Self>, doc: &str) -> io::Result<()> {
        match fs::remove_file(self.segment_path(doc)) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        {
            let mut inner = self.write();
            inner.remove_doc(doc);
            inner.doc_case.remove(doc);
        }
        self.maintain();
        Ok(())
    }

    /// Case membership is owned by SQLite; the store mirrors it for filtering.
    pub fn set_document_cases(&self, cases: impl IntoIterator<Item = (String, Option<String>)>) {
        let mut inner = self.write();
        for (doc, case) in cases {
            inner.doc_case.insert(doc, case);
        }
    }

    /// A case was deleted: its documents are unfiled.
    pub fn clear_case(&self, case: &str) {
        let mut inner = self.write();
        for c in inner.doc_case.values_mut() {
            if c.as_deref() == Some(case) {
                *c = None;
            }
        }
    }

    /// Drops every document not in `keep` (and its segment). Used at startup
    /// to discard points whose document SQLite no longer considers indexed.
    pub fn retain_documents(self: &Arc<Self>, keep: &std::collections::HashSet<String>) -> io::Result<Vec<String>> {
        let stale: Vec<String> = self.read().by_doc.keys().filter(|d| !keep.contains(*d)).cloned().collect();
        for d in &stale {
            self.remove_document(d)?;
        }
        Ok(stale)
    }

    pub fn documents(&self) -> Vec<String> {
        self.read().by_doc.keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.read().live
    }

    pub fn chunk_count(&self, doc: &str) -> usize {
        self.read().by_doc.get(doc).map_or(0, Vec::len)
    }

    /// A document's chunks, in reading order.
    pub fn chunks_of(&self, doc: &str) -> Vec<Chunk> {
        let inner = self.read();
        let mut out: Vec<Chunk> = inner
            .by_doc
            .get(doc)
            .into_iter()
            .flatten()
            .filter_map(|&s| inner.points[s].as_ref().map(|p| p.chunk.clone()))
            .collect();
        out.sort_by_key(|c| c.ordinal);
        out
    }

    pub fn chunk(&self, id: &str) -> Option<Chunk> {
        let inner = self.read();
        inner.by_chunk.get(id).and_then(|&s| inner.points[s].as_ref()).map(|p| p.chunk.clone())
    }

    /// True when a chunk is searchable (its document is fully indexed).
    #[cfg(test)]
    pub fn contains(&self, chunk_id: &str) -> bool {
        self.read().by_chunk.contains_key(chunk_id)
    }

    /// The `k` chunks most similar to `query`, best first, as (chunk id, cosine).
    pub fn search(&self, query: &[f32], k: usize, filter: Filter<'_>) -> Vec<(String, f64)> {
        let Some(q) = normalized(query) else { return Vec::new() };
        if k == 0 {
            return Vec::new();
        }
        let inner = self.read();

        // Candidate slots, when the filter names them cheaply.
        let restricted: Option<Vec<usize>> = if filter.docs.is_empty() && filter.case.is_none() {
            None
        } else {
            Some(
                inner
                    .by_doc
                    .iter()
                    .filter(|(d, _)| filter.allows_doc(&inner, d))
                    .flat_map(|(_, s)| s.iter().copied())
                    .collect(),
            )
        };
        let candidates = restricted.as_ref().map_or(inner.live, Vec::len);

        let graph = inner.graph.as_ref().filter(|g| g.dim == q.len());
        match (graph, restricted) {
            (Some(g), None) if candidates > EXACT_MAX => {
                self.graph_search(&inner, g, &q, k, &filter)
            }
            (Some(g), Some(slots)) if candidates > EXACT_MAX => {
                let hits = self.graph_search(&inner, g, &q, k, &filter);
                // A tight filter can starve the graph walk; never return
                // fewer results than exist.
                if hits.len() < k.min(candidates) {
                    exact(&inner, &q, k, slots.into_iter())
                } else {
                    hits
                }
            }
            (_, Some(slots)) => exact(&inner, &q, k, slots.into_iter()),
            (_, None) => exact(&inner, &q, k, 0..inner.points.len()),
        }
    }

    fn graph_search(&self, inner: &Inner, g: &Graph, q: &[f32], k: usize, filter: &Filter<'_>) -> Vec<(String, f64)> {
        let ok = |slot: &usize| -> bool {
            inner.points.get(*slot).and_then(|p| p.as_ref()).map_or(false, |p| {
                filter.allows_doc(inner, &p.chunk.document_id)
            })
        };
        let ef = (k * 8).max(96);
        let mut hits: Vec<(usize, f32)> = g
            .hnsw
            .search_filter(q, k, ef, Some(&ok))
            .into_iter()
            .map(|n| (n.d_id, 1.0 - n.distance))
            .collect();
        // Points added after the graph was built are scanned exactly.
        let covers = g.covers.load(Ordering::Acquire);
        let tail = exact_slots(inner, q, k, (covers..inner.points.len()).filter(|s| ok(s)));
        hits.extend(tail);
        finish(inner, hits, k)
    }

    /// Background upkeep: build the graph once the collection is large,
    /// fold a long tail of new points into it, or compact after many deletes.
    /// At most one maintenance pass runs at a time; searches never wait on it.
    pub fn maintain(self: &Arc<Self>) {
        let (need_build, need_tail) = {
            let inner = self.read();
            let dim = inner.main_dim();
            let big = inner.live >= GRAPH_MIN;
            let dirty = inner.points.len() > 0
                && inner.tombstones() as f32 / inner.points.len() as f32 > TOMBSTONE_RATIO;
            match &inner.graph {
                None => (big, false),
                Some(g) => (
                    !big || dirty || g.dim != dim,
                    inner.points.len().saturating_sub(g.covers.load(Ordering::Acquire)) > TAIL_MAX,
                ),
            }
        };
        if !(need_build || need_tail) || self.maintaining.swap(true, Ordering::AcqRel) {
            return;
        }
        let db = Arc::clone(self);
        std::thread::Builder::new()
            .name("vectordb-maintain".into())
            .spawn(move || {
                if need_build {
                    db.rebuild();
                } else {
                    db.extend_graph();
                }
                db.maintaining.store(false, Ordering::Release);
            })
            .ok();
    }

    /// Compacts tombstones and builds a fresh graph off-lock, then swaps it in.
    fn rebuild(&self) {
        // 1. Compact under the write lock: cheap moves, no vector copies.
        let snapshot: Vec<(usize, Arc<[f32]>)> = {
            let mut inner = self.write();
            inner.graph = None;
            let old = std::mem::take(&mut inner.points);
            let mut points = Vec::with_capacity(inner.live);
            let mut by_chunk = HashMap::with_capacity(inner.live);
            let mut by_doc: HashMap<String, Vec<usize>> = HashMap::new();
            for p in old.into_iter().flatten() {
                let slot = points.len();
                by_chunk.insert(p.chunk.id.clone(), slot);
                by_doc.entry(p.chunk.document_id.clone()).or_default().push(slot);
                points.push(Some(p));
            }
            inner.points = points;
            inner.by_chunk = by_chunk;
            inner.by_doc = by_doc;
            if inner.live < GRAPH_MIN {
                return;
            }
            let dim = inner.main_dim();
            inner
                .points
                .iter()
                .enumerate()
                .filter_map(|(s, p)| p.as_ref().filter(|p| p.vector.len() == dim).map(|p| (s, Arc::clone(&p.vector))))
                .collect()
        };
        let Some(dim) = snapshot.first().map(|(_, v)| v.len()) else { return };
        let covers = snapshot.last().map_or(0, |(s, _)| s + 1);

        // 2. Build off-lock; searches keep using exact scans meanwhile.
        let hnsw = Hnsw::<f32, UnitCosine>::new(HNSW_M, snapshot.len(), HNSW_MAX_LAYER, HNSW_EF_CONSTRUCTION, UnitCosine);
        let batch: Vec<(&[f32], usize)> = snapshot.iter().map(|(s, v)| (&v[..], *s)).collect();
        hnsw.parallel_insert_slice(&batch);

        // 3. Swap in, unless the store was compacted again meanwhile.
        let mut inner = self.write();
        if inner.points.len() >= covers {
            inner.graph = Some(Arc::new(Graph { hnsw, covers: AtomicUsize::new(covers), dim }));
        }
    }

    /// Inserts the tail (points added since the build) into the live graph.
    fn extend_graph(&self) {
        let (graph, tail) = {
            let inner = self.read();
            let Some(g) = inner.graph.clone() else { return };
            let from = g.covers.load(Ordering::Acquire);
            let tail: Vec<(usize, Arc<[f32]>)> = (from..inner.points.len())
                .filter_map(|s| {
                    inner.points[s].as_ref().filter(|p| p.vector.len() == g.dim).map(|p| (s, Arc::clone(&p.vector)))
                })
                .collect();
            (g, tail)
        };
        let Some(end) = tail.last().map(|(s, _)| s + 1) else { return };
        let batch: Vec<(&[f32], usize)> = tail.iter().map(|(s, v)| (&v[..], *s)).collect();
        graph.hnsw.parallel_insert_slice(&batch);
        // Searches scan the tail exactly until this moves, so nothing is
        // missed while the insert runs.
        graph.covers.fetch_max(end, Ordering::AcqRel);
    }

    /// For tests and diagnostics: whether an HNSW graph is currently serving.
    #[cfg(test)]
    fn has_graph(&self) -> bool {
        self.read().graph.is_some()
    }
}

fn exact_slots(inner: &Inner, q: &[f32], k: usize, slots: impl Iterator<Item = usize>) -> Vec<(usize, f32)> {
    let mut scored: Vec<(usize, f32)> = slots
        .filter_map(|s| inner.points.get(s)?.as_ref().filter(|p| p.vector.len() == q.len()).map(|p| (s, dot(&p.vector, q))))
        .filter(|(_, score)| *score > 0.0)
        .collect();
    if scored.len() > k {
        scored.select_nth_unstable_by(k - 1, |a, b| b.1.total_cmp(&a.1));
        scored.truncate(k);
    }
    scored
}

fn exact(inner: &Inner, q: &[f32], k: usize, slots: impl Iterator<Item = usize>) -> Vec<(String, f64)> {
    let hits = exact_slots(inner, q, k, slots);
    finish(inner, hits, k)
}

/// Dedupes, ranks (ties by chunk id, so results are deterministic), trims.
fn finish(inner: &Inner, mut hits: Vec<(usize, f32)>, k: usize) -> Vec<(String, f64)> {
    hits.sort_by(|a, b| a.0.cmp(&b.0));
    hits.dedup_by_key(|h| h.0);
    let mut out: Vec<(String, f64)> = hits
        .into_iter()
        .filter(|(_, s)| *s > 0.0)
        .filter_map(|(slot, s)| inner.points.get(slot)?.as_ref().map(|p| (p.chunk.id.clone(), s as f64)))
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out.truncate(k);
    out
}

fn to_points(chunks: Vec<(Chunk, Vec<f32>)>) -> Vec<Point> {
    chunks.into_iter().map(|(chunk, v)| Point { chunk, vector: v.into() }).collect()
}

/// Document ids are UUIDs, but a file name must never escape the directory.
fn sanitize(doc: &str) -> String {
    doc.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

// --- Segment files -------------------------------------------------------------
//
// Little-endian, versioned, self-describing enough to be rejected when
// damaged:
//
//   magic "CVSEG" + version u8
//   str document_id
//   u32 point count
//   per point:
//     str chunk_id, i64 ordinal, i64 start_char, i64 end_char,
//     u8 has_page + i64 page, i64 token_estimate, str text,
//     u32 dim, dim × f32 (unit length)
//   u64 FNV-1a checksum of everything above
//
// `str` is a u32 byte length followed by UTF-8.

const MAGIC: &[u8; 5] = b"CVSEG";
const VERSION: u8 = 1;

fn write_segment(path: &Path, doc: &str, points: &[(Chunk, Vec<f32>)]) -> io::Result<()> {
    let mut buf: Vec<u8> = Vec::with_capacity(64 + points.iter().map(|(c, v)| c.text.len() + v.len() * 4 + 64).sum::<usize>());
    buf.extend_from_slice(MAGIC);
    buf.push(VERSION);
    put_str(&mut buf, doc);
    buf.extend_from_slice(&(points.len() as u32).to_le_bytes());
    for (c, v) in points {
        put_str(&mut buf, &c.id);
        buf.extend_from_slice(&c.ordinal.to_le_bytes());
        buf.extend_from_slice(&c.start_char.to_le_bytes());
        buf.extend_from_slice(&c.end_char.to_le_bytes());
        buf.push(c.page.is_some() as u8);
        buf.extend_from_slice(&c.page.unwrap_or(0).to_le_bytes());
        buf.extend_from_slice(&c.token_estimate.to_le_bytes());
        put_str(&mut buf, &c.text);
        buf.extend_from_slice(&(v.len() as u32).to_le_bytes());
        for x in v {
            buf.extend_from_slice(&x.to_le_bytes());
        }
    }
    let sum = fnv1a(&buf);
    buf.extend_from_slice(&sum.to_le_bytes());

    // Write-then-rename: a crash leaves either the old segment or the new one.
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&buf)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

fn read_segment(path: &Path) -> io::Result<(String, Vec<(Chunk, Vec<f32>)>)> {
    let mut data = Vec::new();
    fs::File::open(path)?.read_to_end(&mut data)?;
    let bad = || io::Error::new(io::ErrorKind::InvalidData, "corrupt segment");
    if data.len() < MAGIC.len() + 1 + 8 {
        return Err(bad());
    }
    let (body, sum) = data.split_at(data.len() - 8);
    if fnv1a(body) != u64::from_le_bytes(sum.try_into().map_err(|_| bad())?) {
        return Err(bad());
    }
    let mut r = Reader { buf: body, at: 0 };
    if r.take(MAGIC.len())? != MAGIC || r.u8()? != VERSION {
        return Err(bad());
    }
    let doc = r.str()?;
    let n = r.u32()? as usize;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let id = r.str()?;
        let ordinal = r.i64()?;
        let start_char = r.i64()?;
        let end_char = r.i64()?;
        let has_page = r.u8()? != 0;
        let page = r.i64()?;
        let token_estimate = r.i64()?;
        let text = r.str()?;
        let dim = r.u32()? as usize;
        let raw = r.take(dim * 4)?;
        let vector: Vec<f32> = raw.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        out.push((
            Chunk {
                id,
                document_id: doc.clone(),
                ordinal,
                start_char,
                end_char,
                page: has_page.then_some(page),
                text,
                token_estimate,
            },
            vector,
        ));
    }
    if r.at != body.len() {
        return Err(bad());
    }
    Ok((doc, out))
}

fn put_str(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
}

fn fnv1a(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> io::Result<&'a [u8]> {
        let end = self.at.checked_add(n).filter(|e| *e <= self.buf.len());
        let end = end.ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "truncated segment"))?;
        let s = &self.buf[self.at..end];
        self.at = end;
        Ok(s)
    }
    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn i64(&mut self) -> io::Result<i64> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn str(&mut self) -> io::Result<String> {
        let n = self.u32()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad utf-8"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn tempdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("counsel-vdb-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            ((self.0 >> 40) as f32 / (1u64 << 24) as f32) - 0.5
        }
        fn vec(&mut self, dim: usize) -> Vec<f32> {
            (0..dim).map(|_| self.next()).collect()
        }
    }

    fn chunks(doc: &str, n: usize) -> Vec<Chunk> {
        (0..n)
            .map(|i| Chunk {
                id: format!("{doc}:{i}"),
                document_id: doc.into(),
                ordinal: i as i64,
                start_char: (i * 100) as i64,
                end_char: (i * 100 + 99) as i64,
                page: (i % 2 == 0).then_some(i as i64 / 2 + 1),
                text: format!("passage {i} of {doc} — «acórdão» ✓"),
                token_estimate: 25,
            })
            .collect()
    }

    fn brute(db: &VectorDb, q: &[f32], k: usize, filter: Filter<'_>) -> Vec<String> {
        let q = normalized(q).unwrap();
        let inner = db.read();
        let mut all: Vec<(String, f32)> = inner
            .points
            .iter()
            .flatten()
            .filter(|p| filter.allows_doc(&inner, &p.chunk.document_id))
            .map(|p| (p.chunk.id.clone(), dot(&p.vector, &q)))
            .filter(|(_, s)| *s > 0.0)
            .collect();
        all.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        all.into_iter().take(k).map(|(id, _)| id).collect()
    }

    #[test]
    fn segments_round_trip_exactly() {
        let dir = tempdir("roundtrip");
        let mut rng = Rng(1);
        {
            let (db, _) = VectorDb::open(&dir).unwrap();
            let c = chunks("d1", 5);
            let v: Vec<Vec<f32>> = (0..5).map(|_| rng.vec(8)).collect();
            db.upsert_document("d1", Some("c1"), c, &v).unwrap();
        }
        let (db, report) = VectorDb::open(&dir).unwrap();
        assert_eq!((report.documents, report.points), (1, 5));
        assert!(report.corrupt.is_empty());
        let got = db.chunks_of("d1");
        assert_eq!(got.len(), 5);
        assert_eq!(got[3].text, "passage 3 of d1 — «acórdão» ✓");
        assert_eq!(got[0].page, Some(1));
        assert_eq!(got[1].page, None);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_corrupt_segment_is_reported_and_removed() {
        let dir = tempdir("corrupt");
        {
            let (db, _) = VectorDb::open(&dir).unwrap();
            db.upsert_document("d1", None, chunks("d1", 2), &[vec![1.0, 0.0], vec![0.0, 1.0]]).unwrap();
        }
        let seg = dir.join("d1.seg");
        let mut bytes = fs::read(&seg).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0xff;
        fs::write(&seg, bytes).unwrap();
        let (db, report) = VectorDb::open(&dir).unwrap();
        assert_eq!(report.corrupt, vec!["d1".to_string()]);
        assert_eq!(db.len(), 0);
        assert!(!seg.exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn exact_search_matches_brute_force_with_filters() {
        let dir = tempdir("exact");
        let (db, _) = VectorDb::open(&dir).unwrap();
        let mut rng = Rng(7);
        for (i, doc) in ["d1", "d2", "d3", "d4"].iter().enumerate() {
            let v: Vec<Vec<f32>> = (0..50).map(|_| rng.vec(16)).collect();
            let case = if i < 2 { Some("c1") } else { Some("c2") };
            db.upsert_document(doc, case, chunks(doc, 50), &v).unwrap();
        }
        let q = rng.vec(16);
        let d2 = vec!["d2".to_string(), "d4".to_string()];
        for filter in [
            Filter::default(),
            Filter { docs: &[], case: Some("c2") },
            Filter { docs: &d2, case: None },
            Filter { docs: &d2, case: Some("c1") },
        ] {
            let got: Vec<String> = db.search(&q, 10, filter).into_iter().map(|(id, _)| id).collect();
            assert_eq!(got, brute(&db, &q, 10, filter));
        }
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn upsert_replaces_and_remove_deletes() {
        let dir = tempdir("replace");
        let (db, _) = VectorDb::open(&dir).unwrap();
        db.upsert_document("d1", None, chunks("d1", 3), &[vec![1.0, 0.0], vec![0.0, 1.0], vec![1.0, 1.0]]).unwrap();
        db.upsert_document("d1", None, chunks("d1", 1), &[vec![1.0, 0.0]]).unwrap();
        assert_eq!(db.len(), 1);
        assert_eq!(db.chunk_count("d1"), 1);
        assert!(!db.contains("d1:2"));
        db.remove_document("d1").unwrap();
        assert_eq!(db.len(), 0);
        assert!(db.search(&[1.0, 0.0], 5, Filter::default()).is_empty());
        let (reopened, _) = VectorDb::open(&dir).unwrap();
        assert_eq!(reopened.len(), 0, "the segment is gone from disk too");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clearing_a_case_unfiles_its_documents() {
        let dir = tempdir("case");
        let (db, _) = VectorDb::open(&dir).unwrap();
        db.upsert_document("d1", Some("c1"), chunks("d1", 1), &[vec![1.0, 0.0]]).unwrap();
        assert_eq!(db.search(&[1.0, 0.0], 5, Filter { docs: &[], case: Some("c1") }).len(), 1);
        db.clear_case("c1");
        assert!(db.search(&[1.0, 0.0], 5, Filter { docs: &[], case: Some("c1") }).is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn retain_drops_documents_sqlite_does_not_know() {
        let dir = tempdir("retain");
        let (db, _) = VectorDb::open(&dir).unwrap();
        db.upsert_document("keep", None, chunks("keep", 1), &[vec![1.0]]).unwrap();
        db.upsert_document("orphan", None, chunks("orphan", 1), &[vec![1.0]]).unwrap();
        let keep: HashSet<String> = ["keep".to_string()].into();
        assert_eq!(db.retain_documents(&keep).unwrap(), vec!["orphan".to_string()]);
        assert_eq!(db.documents(), vec!["keep".to_string()]);
        fs::remove_dir_all(&dir).ok();
    }

    /// Large enough to build a graph: HNSW must agree with exact search on
    /// nearly every result, and filtered graph search must stay correct.
    #[test]
    fn hnsw_recall_is_high_on_a_large_collection() {
        let dir = tempdir("hnsw");
        let (db, _) = VectorDb::open(&dir).unwrap();
        let mut rng = Rng(42);
        let dim = 32;
        let per_doc = 500;
        let docs = (GRAPH_MIN / per_doc) + 4; // comfortably above the threshold
        for d in 0..docs {
            let doc = format!("doc{d}");
            let v: Vec<Vec<f32>> = (0..per_doc).map(|_| rng.vec(dim)).collect();
            let case = if d % 2 == 0 { "even" } else { "odd" };
            db.upsert_document(&doc, Some(case), chunks(&doc, per_doc), &v).unwrap();
        }
        // Build synchronously for the test.
        db.rebuild();
        assert!(db.has_graph());

        let mut hit = 0usize;
        let mut total = 0usize;
        for _ in 0..20 {
            let q = rng.vec(dim);
            let approx: HashSet<String> = db.search(&q, 10, Filter::default()).into_iter().map(|(id, _)| id).collect();
            for id in brute(&db, &q, 10, Filter::default()) {
                total += 1;
                hit += approx.contains(&id) as usize;
            }
        }
        let recall = hit as f32 / total as f32;
        eprintln!("hnsw recall@10 = {recall:.3} over {} points", db.len());
        assert!(recall >= 0.9, "recall@10 was {recall}");

        // A case filter still covers ~half the collection, so it goes through
        // the graph — and must only return that case's chunks.
        let q = rng.vec(dim);
        let hits = db.search(&q, 10, Filter { docs: &[], case: Some("even") });
        assert_eq!(hits.len(), 10);
        assert!(hits.iter().all(|(id, _)| {
            let n: usize = id.trim_start_matches("doc").split(':').next().unwrap().parse().unwrap();
            n % 2 == 0
        }));

        // A single @mentioned document is answered exactly.
        let one = vec!["doc3".to_string()];
        let q = rng.vec(dim);
        let got: Vec<String> = db.search(&q, 5, Filter { docs: &one, case: None }).into_iter().map(|(id, _)| id).collect();
        assert_eq!(got, brute(&db, &q, 5, Filter { docs: &one, case: None }));
        fs::remove_dir_all(&dir).ok();
    }
}
