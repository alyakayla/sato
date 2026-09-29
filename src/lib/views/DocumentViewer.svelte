<script lang="ts">
  import { confirmDelete } from '$lib/confirm.svelte';
  import { intlLocale, t } from '$lib/i18n/index.svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import type { Chunk, Document, DocumentText } from '$lib/types';
  import { api, formatBytes, formatDate, formatRelative } from '$lib/api';
  import {
    askAbout,
    guard,
    indexing,
    notify,
    openPane,
    refreshTree,
    treeNodes,
  } from '$lib/stores.svelte';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import PageHeader, { type PageProp } from '$lib/components/PageHeader.svelte';

  let { documentId }: { documentId: string } = $props();

  let doc = $state<Document | null>(null);
  let text = $state<DocumentText | null>(null);
  let chunks = $state<Chunk[]>([]);
  let loading = $state(true);
  let showPanel = $state(true);
  let showText = $state(false);
  let page = $state(0);

  const live = $derived(indexing.value[documentId] ?? null);

  const isPdf = $derived(doc?.mime === 'application/pdf' || doc?.fileName.toLowerCase().endsWith('.pdf'));
  const isTextual = $derived(
    !!doc && (isPdf || doc.mime.startsWith('text/') || /\.(txt|md|markdown|csv|json|log)$/i.test(doc.fileName)),
  );
  /** Asset-protocol URL for the stored file; only valid for PDF rendering. */
  const fileUrl = $derived(doc && isPdf ? convertFileSrc(doc.storedPath) : '');

  const docProps = $derived.by((): PageProp[] => {
    if (!doc) return [];
    return [
      { label: t('common.case'), content: caseProp },
      { label: t('common.index'), content: indexProp },
      { label: t('common.pages'), value: doc.pageCount },
      { label: t('common.size'), value: formatBytes(doc.sizeBytes) },
      { label: t('common.imported'), value: formatRelative(doc.createdAt), title: doc.createdAt },
    ];
  });

  const nodeKey = $derived(treeNodes.value.find((n) => n.documentId === documentId)?.nodeKey ?? null);

  async function load(id: string): Promise<void> {
    loading = true;
    const meta = await guard(t('doc.err.open'), () => api.getDocument(id));
    doc = meta;
    // The extracted text powers the "text" tab and the chunk count; a failure
    // there is not fatal, since the PDF still renders.
    const full = await guard(t('doc.err.text'), () => api.readDocumentText(id));
    text = full;
    const cs = await guard(t('doc.err.chunks'), () => api.listChunks(id));
    chunks = cs ?? [];
    loading = false;
  }

  $effect(() => {
    const id = documentId;
    page = 0;
    showText = false;
    void load(id);
  });

  async function reindex(): Promise<void> {
    if (!doc) return;
    const ok = await guard(t('doc.reindex'), () => api.reindexDocument(doc!.id));
    if (ok !== null) notify('ok', t('doc.requeued'));
  }

  async function remove(): Promise<void> {
    if (!doc) return;
    if (!(await confirmDelete({ title: t('confirm.document.title'), message: t('confirm.document.body', { name: doc.fileName }) }))) return;
    const ok = await guard(t('doc.err.delete'), () => api.deleteDocument(doc!.id));
    if (ok !== null) {
      notify('ok', t('doc.deleted'));
      await refreshTree();
      openPane({ kind: 'grid' });
    }
  }

  function openFileExternally(): void {
    if (!doc) return;
    // The asset protocol URL opens in the system default handler.
    window.open(fileUrl, '_blank', 'noopener');
  }
</script>

<div class="doc" class:panel-closed={!showPanel}>
  {#if loading}
    <div class="empty" style="height: 100%">
      <span class="spinner"></span>
    </div>
  {:else if !doc}
    <div class="empty" style="height: 100%">
      <div class="empty-glyph">▤</div>
      <div class="empty-title">{t('doc.notFound')}</div>
      <div class="empty-hint">{t('doc.notFoundHint')}</div>
    </div>
  {:else}
    <div class="main">
      <div class="page">
        <PageHeader kind="document" title={doc.fileName} properties={docProps}>
          {#snippet actions()}
            {#if nodeKey}
              <button
                class="btn btn-sm"
                onclick={() => askAbout(nodeKey)}
                title={t('doc.askHint')}
              >
                {t('doc.ask')}
              </button>
            {/if}
            {#if isPdf}
              <button class="btn btn-ghost btn-sm" onclick={openFileExternally} title={t('doc.openExternalHint')}>
                ↗ {t('common.open')}
              </button>
            {/if}
          {/snippet}
        </PageHeader>
      </div>

      <div class="tabs">
        {#if isPdf}
          <button class="tab" class:on={!showText} onclick={() => (showText = false)}>{t('kind.document')}</button>
        {/if}
        {#if isTextual && text}
          <button class="tab" class:on={showText} onclick={() => (showText = true)}>
            {t('doc.extracted')}
          </button>
        {/if}
        <div class="spacer"></div>
        <button class="btn btn-ghost btn-sm" onclick={() => (showPanel = !showPanel)} title={t('doc.toggleDetails')}>
          {showPanel ? '⟩' : '⟨'}
        </button>
      </div>

      <div class="stage">
        {#if live}
          <div class="overlay">
            <span class="spinner"></span>
            {live.stage}
            {#if live.chunksTotal > 0}
              — {t('doc.chunksProgress', { done: live.chunksDone, total: live.chunksTotal })}
            {/if}
          </div>
        {/if}

        {#if !isPdf && !isTextual}
          <div class="empty" style="height: 100%">
            <div class="empty-glyph">{iconFor(doc.fileName)}</div>
            <div class="empty-title">{doc.fileName}</div>
            <div class="empty-hint">
              {t('doc.noPreview', { mime: doc.mime })}
            </div>
            {#if text?.text}
              <button class="btn btn-sm empty-action" onclick={() => (showText = true)}>
                {t('doc.showExtracted')}
              </button>
            {/if}
          </div>
        {:else if showText}
          <div class="reader">
            {#if text?.text}
              {#each text.pages as p, i (i)}
                <section>
                  {#if text.pages.length > 1}
                    <h4>{t('doc.page', { page: i + 1 })}</h4>
                  {/if}
                  <p>{p}</p>
                </section>
              {/each}
            {:else}
              <div class="empty">
                <div class="empty-hint">{t('doc.noText')}</div>
              </div>
            {/if}
          </div>
        {:else if isPdf}
          {#key fileUrl}
            <iframe class="pdf" src={fileUrl} title={doc.fileName}></iframe>
          {/key}
        {:else}
          <pre class="plain">{text?.text ?? ''}</pre>
        {/if}
      </div>
    </div>

    {#if showPanel}
      <aside>
        <div class="p-head">
          <div class="p-title">{t('doc.details')}</div>
        </div>

        <dl>
          <dt>{t('table.col.type')}</dt>
          <dd class="mono tiny">{doc.mime}</dd>

          <dt>{t('doc.words')}</dt>
          <dd>{doc.wordCount?.toLocaleString(intlLocale()) ?? '—'}</dd>

          <dt>{t('table.col.chunks')}</dt>
          <dd>{chunks.length}</dd>

          <dt>{t('common.imported')}</dt>
          <dd title={doc.createdAt}>{formatRelative(doc.createdAt)}</dd>

          <dt>{t('doc.checksum')}</dt>
          <dd class="mono tiny truncate" title={doc.checksum}>
            {doc.checksum.slice(0, 16)}…
          </dd>
        </dl>

        {#if doc.indexError}
          <div class="alert alert-danger tiny">{doc.indexError}</div>
        {/if}

        <div class="p-actions">
          <div class="two">
            <button class="btn btn-sm" onclick={() => void reindex()}>{t('doc.reindex')}</button>
            <button class="btn btn-danger btn-sm" onclick={() => void remove()}>{t('common.delete')}</button>
          </div>
        </div>

        <p class="faint tiny foot">
          {t('doc.importedOn', { date: formatDate(doc.createdAt) })}
        </p>
      </aside>
    {/if}
  {/if}
</div>

{#snippet caseProp()}
  {#if doc?.caseId}
    <button class="link" onclick={() => openPane({ kind: 'case', caseId: doc!.caseId! })} title={doc.caseTitle ?? ''}>
      {doc.caseReference}
    </button>
  {:else}
    <span class="faint">{t('common.unfiled')}</span>
  {/if}
{/snippet}

{#snippet indexProp()}
  {#if doc}<StatusBadge status={doc.indexStatus} />{/if}
{/snippet}

<script lang="ts" module>
  const EXT_ICONS: [RegExp, string][] = [
    [/\.docx?$/i, '📄'],
    [/\.xlsx?$/i, '▦'],
    [/\.pptx?$/i, '▤'],
    [/\.pdf$/i, '▤'],
    [/\.(png|jpe?g|gif|webp|svg)$/i, '▣'],
    [/\.(zip|rar|7z)$/i, '▨'],
  ];

  function iconFor(name: string): string {
    for (const [re, icon] of EXT_ICONS) if (re.test(name)) return icon;
    return '▤';
  }
</script>

<style>
  .doc {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 252px;
    overflow: hidden;
  }
  .doc.panel-closed {
    grid-template-columns: minmax(0, 1fr);
  }

  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }

  .page {
    padding: 14px 16px 12px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .tabs {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    flex-shrink: 0;
  }
  .tab {
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 12px;
    padding: 3px 8px;
    transition:
      color var(--dur-fast) var(--ease),
      border-color var(--dur-fast) var(--ease);
  }
  /* An underlined tab is a nav affordance, and nav is chrome: ink, not accent. */
  .tab.on {
    color: var(--text);
    border-bottom-color: var(--ink);
  }
  .tiny {
    font-size: 10.5px;
  }

  .stage {
    flex: 1;
    min-height: 0;
    position: relative;
    background: var(--surface);
  }

  /* A flat veil rather than a blurred one: a blur is heavy, dated, and it
     makes the page behind a progress message harder to read, not easier. */
  .overlay {
    position: absolute;
    inset: 0;
    z-index: 4;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 9px;
    background: var(--veil);
    font-size: 12.5px;
    color: var(--text-secondary);
  }

  .pdf {
    width: 100%;
    height: 100%;
    border: none;
    /* The WebView's built-in PDF viewer paints its own dark chrome regardless
       of page theme, so the well stays dark to avoid a white flash while the
       plugin boots. */
    background: var(--media-well);
  }

  .reader {
    height: 100%;
    overflow: auto;
    padding: 26px 32px 60px;
  }
  .reader section + section {
    margin-top: 22px;
  }
  .reader h4 {
    font-size: 11.5px;
    font-weight: 600;
    text-transform: none;
    letter-spacing: 0;
    color: var(--text-tertiary);
    margin-bottom: 7px;
    position: sticky;
    top: -26px;
    background: var(--surface);
    padding: 3px 0;
  }
  .reader p {
    font-size: 13.5px;
    line-height: 1.72;
    color: var(--text);
    white-space: pre-wrap;
    max-width: 70ch;
  }

  .plain {
    height: 100%;
    overflow: auto;
    margin: 0;
    padding: 22px 26px;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.6;
    color: var(--text-secondary);
    white-space: pre-wrap;
  }

  aside {
    border-left: 1px solid var(--border);
    background: var(--surface-2);
    overflow: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .p-head {
    display: flex;
    flex-direction: column;
    gap: 6px;
    align-items: flex-start;
  }
  .p-title {
    font-size: 12.5px;
    font-weight: 600;
    width: 100%;
  }

  dl {
    display: grid;
    grid-template-columns: 66px minmax(0, 1fr);
    gap: 5px 8px;
    font-size: 11.5px;
    margin: 0;
  }
  dt {
    color: var(--text-tertiary);
  }
  dd {
    margin: 0;
    color: var(--text);
    min-width: 0;
    overflow: hidden;
  }

  .p-actions {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: auto;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }

  .foot {
    margin: 0;
  }
</style>
