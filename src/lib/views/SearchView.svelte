<script lang="ts">
  import { documentTarget, openArtifactMenu } from '$lib/artifactMenu';
  import { t } from '$lib/i18n/index.svelte';
  import type { SearchHit } from '$lib/types';
  import { api } from '$lib/api';
  import { cases, guard, openPane } from '$lib/stores.svelte';

  let { caseId = null }: { caseId?: string | null } = $props();

  /**
   * Full-text search over indexed passages.
   *
   * This is the BM25 half of the retrieval pipeline exposed directly: the chat
   * fuses it with vector similarity, but when you want to see every literal
   * match — and read the surrounding sentence — this is the tool.
   */

  let query = $state('');
  let hits = $state<SearchHit[]>([]);
  let searching = $state(false);
  let ran = $state(false);
  let scope = $state('');

  $effect(() => {
    // Follow the workspace's case scope until the user overrides it here.
    if (caseId && !scope) scope = caseId;
  });

  async function run(): Promise<void> {
    const q = query.trim();
    if (!q) {
      hits = [];
      ran = false;
      return;
    }
    searching = true;
    const data = await guard(t('view.search'), () => api.searchDocuments(q, scope || null, 50));
    hits = data ?? [];
    ran = true;
    searching = false;
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'Enter') void run();
  }

  /** Bold the matched terms so the reason for a hit is obvious. */
  function highlight(text: string): { s: string; hit: boolean }[] {
    const terms = query
      .trim()
      .split(/\s+/)
      .filter((t) => t.length > 2)
      .map((t) => t.toLowerCase());
    if (terms.length === 0) return [{ s: text, hit: false }];

    const re = new RegExp(`(${terms.map(escapeRe).join('|')})`, 'gi');
    return text
      .split(re)
      .filter((part) => part.length > 0)
      .map((part) => ({ s: part, hit: terms.includes(part.toLowerCase()) }));
  }

  function escapeRe(s: string): string {
    return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }

  /** Trims the snippet to the first match so the hit is not off-screen. */
  function snippet(text: string): string {
    const limit = 280;
    if (text.length <= limit) return text;
    const lower = text.toLowerCase();
    const terms = query.toLowerCase().split(/\s+/).filter((t) => t.length > 2);
    const at = terms.map((t) => lower.indexOf(t)).filter((i) => i >= 0).sort((a, b) => a - b)[0];
    if (at === undefined || at < limit / 2) return `${text.slice(0, limit)}…`;
    const start = Math.max(0, at - Math.floor(limit / 3));
    return `…${text.slice(start, start + limit)}…`;
  }
</script>

<div class="wrap">
  <div class="bar">
    <div class="field-inline">
      <input
        class="q"
        type="search"
        placeholder={t('search.placeholder')}
        bind:value={query}
        onkeydown={onKeydown}
        spellcheck="false"
      />
    </div>

    <select bind:value={scope} class="scope" title={t('search.scope')}>
      <option value="">{t('common.allCases')}</option>
      {#each cases.value as c (c.id)}
        <option value={c.id}>{c.reference} · {c.title}</option>
      {/each}
    </select>

    <button class="btn btn-primary btn-sm" onclick={() => void run()} disabled={!query.trim() || searching}>
      {#if searching}<span class="spinner"></span>{:else}{t('common.search')}{/if}
    </button>

    <div class="spacer"></div>
    {#if ran}
      <span class="faint count">
        {t('step.passages', { n: hits.length })}
      </span>
    {/if}
  </div>

  <div class="results">
    {#if searching}
      <div class="empty" style="height: 100%"><span class="spinner"></span></div>
    {:else if hits.length === 0}
      <div class="empty" style="height: 100%">
        <div class="empty-glyph">⌕</div>
        <div class="empty-title">
          {ran ? t('search.noMatches') : t('search.emptyTitle')}
        </div>
        <div class="empty-hint">
          {#if ran}
            {t('search.noMatchesHint')}
          {:else}
            {t('search.emptyHint')}
          {/if}
        </div>
      </div>
    {:else}
      {#each hits as hit (hit.chunk.id)}
        {@const parts = highlight(snippet(hit.chunk.text))}
        <article class="hit" data-artifact={`document:${hit.chunk.documentId}`}>
          <header>
            <button
              class="doc"
              onclick={() => openPane({ kind: 'document', documentId: hit.chunk.documentId })}
              oncontextmenu={(e) => openArtifactMenu(e, documentTarget(hit.chunk.documentId, hit.documentName))}
              title={t('search.openDocument')}
            >
              ▤ {hit.documentName}
            </button>
            {#if hit.caseReference}
              <span class="badge badge-muted">{hit.caseReference}</span>
            {/if}
            {#if hit.chunk.page}
              <span class="badge badge-muted">p. {hit.chunk.page}</span>
            {/if}
            <div class="spacer"></div>
            <span class="score" title={t('search.relevance')}>
              <span class="score-bar" style={`--p:${Math.round(hit.score * 100)}%`}></span>
              <span class="mono">{Math.round(hit.score * 100)}%</span>
            </span>
          </header>
          <p class="text">
            {#each parts as p, i (i)}{#if p.hit}<mark>{p.s}</mark>{:else}{p.s}{/if}{/each}
          </p>
        </article>
      {/each}
    {/if}
  </div>
</div>

<style>
  .wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    flex-shrink: 0;
  }

  .field-inline {
    flex: 1;
    max-width: 460px;
  }
  .q {
    width: 100%;
  }

  .scope {
    width: auto;
    max-width: 200px;
    padding: 4px 22px 4px 7px;
    font-size: 11.5px;
    background-position: right 5px center;
  }

  .count {
    font-size: 11px;
  }

  .results {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 8px 12px 30px;
    background: var(--surface);
  }

  .hit {
    padding: 9px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface);
    margin-bottom: 6px;
  }
  .hit:hover {
    border-color: var(--border-strong);
  }

  .hit header {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-bottom: 5px;
  }

  .doc {
    background: transparent;
    border: none;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
    font-size: 12px;
    font-weight: 500;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
  }
  .doc:hover {
    text-decoration: underline;
  }

  .score {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  /* The relevance meter. Named apart from the toolbar's `.bar`: sharing the
     class shrank the toolbar to this 3px strip. */
  .score-bar {
    position: relative;
    width: 34px;
    height: 3px;
    border-radius: var(--radius-full);
    background: var(--border);
  }
  .score-bar::after {
    content: '';
    position: absolute;
    inset: 0 auto 0 0;
    width: var(--p);
    border-radius: var(--radius-full);
    background: var(--accent);
  }
  .score .mono {
    font-size: 10px;
    color: var(--text-tertiary);
  }

  .text {
    font-size: 12.5px;
    line-height: 1.6;
    color: var(--text-secondary);
    margin: 0;
  }
  mark {
    background: var(--accent-soft);
    color: var(--text);
    border-radius: var(--radius-xs);
    padding: 0 1px;
  }
</style>
