# sato

a case and document manager for lawyers. Manages matters, documents, people, deadlines, and a chat assistant that answers questions grounded in your indexed files with citations.

Built with Tauri, SvelteKit, and Rust, runs on Windows, macOS, and Linux.

## features

- organize matters with statuses, categories, and cross-references
- import PDFs, DOCX, TXT, HTML, CSV, RTF, XML, and JSON; full-text indexed automatically
- track clients, opposing parties, witnesses, judges, and experts linked to cases
- built-in sheet editor with a formula engine
- hearings, deadlines, filings, and meetings
- ask questions about your documents and get cited answers; hybrid retrieval (vector + keyword search with reciprocal rank fusion) over an embedded vector database.
- English and Portuguese available.

everything is local. documents are stored on disk, metadata in SQLite, and embeddings in an in-memory vector database persisted as checksummed segment files. Nothing leaves your machine unless you configure an external AI provider.

## Prerequisites

- [Bun](https://bun.sh)
- [Rust](https://rustup.rs) (stable)
- [Ollama](https://ollama.com) (for the AI assistant)

pull the default models:

```
ollama pull llama3.1
ollama pull nomic-embed-text
```

on Linux, install the tauri system libraries:

```
sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```

## development

```
bun install
bun run tauri dev
```

## checks

```
bun run check
bun test
bun run design:contrast  # design system contrast gate (57 pairs, both themes)
cd src-tauri && cargo test --lib  # Backend tests
```

## building

```
bun run tauri build
```

installers are produced for each platform. builds are currently unsigned, but available in the [releases.](https://github.com/alyakayla/sato/releases/)

## Project structure

```
src/                    Frontend (SvelteKit)
  lib/                  Stores, types, i18n, layout, formulas, tree logic
  routes/               Pages and layout
src-tauri/              Backend (Rust)
  src/
    commands.rs         Tauri command handlers
    db.rs               SQLite schema and queries
    vectordb.rs         Embedded vector database (HNSW + exact scan)
    search.rs           Hybrid retrieval (vector + BM25 keyword, RRF merge)
    chunk.rs            Document chunking
    embedtext.rs        Embedding pipeline with contextual headers
    extract.rs          Text extraction (PDF, DOCX, HTML, etc.)
    providers.rs        LLM and embedding provider abstraction (Ollama, OpenAI-compatible)
    eval.rs             Retrieval evaluation harness
scripts/
  contrast-gate.ts      Automated WCAG contrast verification
```

## License

[GNU General Public License v3.0](LICENSE)
