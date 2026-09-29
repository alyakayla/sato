<script lang="ts">
  import { deleteWithUndo } from '$lib/undoDelete';
  import { confirmDelete } from '$lib/confirm.svelte';
  import { openNodeMenu } from '$lib/artifactMenu';
  import { label, t } from '$lib/i18n/index.svelte';
  import type { Case, TreeNode } from '$lib/types';
  import { CASE_STATUSES } from '$lib/types';
  import { api, formatDate, formatRelative } from '$lib/api';
  import {
    askAbout,
    cases,
    categories,
    expandTo,
    guard,
    notify,
    takeEditRequest,
    openNode,
    openPane,
    refreshAll,
    treeNodes,
  } from '$lib/stores.svelte';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import PageHeader, { type PageProp } from '$lib/components/PageHeader.svelte';
  import CaseGraph from '$lib/components/CaseGraph.svelte';
  import LifeWaves from '$lib/components/LifeWaves.svelte';
  import { connectionCount } from '$lib/caseGraph';

  let { caseId }: { caseId: string } = $props();

  let detail = $state<Case | null>(null);
  let loading = $state(true);
  let editing = $state(false);
  let draft = $state({ title: '', status: 'Open', description: '', openedAt: '' });

  const record = $derived(cases.value.find((c) => c.id === caseId) ?? detail);
  const children = $derived(treeNodes.value.filter((n) => n.caseId === caseId));
  /** The case's own tree node: the root of its connection graph. */
  const caseNode = $derived(children.find((n) => n.kind === 'case') ?? null);
  const category = $derived(
    categories.value.find((c) => c.id === (record?.categoryId ?? null)) ?? null,
  );

  const caseProps = $derived.by((): PageProp[] => {
    if (!record) return [];
    const out: PageProp[] = [{ label: t('common.status'), content: statusProp }];
    if (category) out.push({ label: t('common.category'), value: category.name });
    out.push({ label: t('common.opened'), value: formatDate(record.openedAt) });
    out.push(
      record.closedAt
        ? { label: t('common.closed'), value: formatDate(record.closedAt) }
        : { label: t('common.updated'), value: formatRelative(record.updatedAt), title: record.updatedAt },
    );
    return out;
  });

  const groups = $derived.by(() => {
    const pick = (kind: TreeNode['kind']) => children.filter((n) => n.kind === kind);
    return [
      { kind: 'document' as const, label: t('kinds.document'), nodes: pick('document') },
      { kind: 'person' as const, label: t('kinds.person'), nodes: pick('person') },
      { kind: 'sheet' as const, label: t('kinds.sheet'), nodes: pick('sheet') },
      { kind: 'event' as const, label: t('kinds.event'), nodes: pick('event') },
    ].filter((g) => g.nodes.length > 0);
  });

  async function load(id: string): Promise<void> {
    loading = true;
    const data = await guard(t('case.err.load'), () => api.getCase(id));
    if (data) {
      detail = data;
      draft = {
        title: data.title,
        status: data.status,
        description: data.description ?? '',
        openedAt: data.openedAt.slice(0, 10),
      };
      // "Edit details…" from a context menu lands here once the data is in.
      if (takeEditRequest('case', id)) editing = true;
    }
    loading = false;
  }

  $effect(() => {
    editing = false;
    void load(caseId);
  });

  async function save(): Promise<void> {
    if (!record) return;
    if (!draft.title.trim()) {
      notify('warn', t('qa.needsTitle'));
      return;
    }
    const saved = await guard(t('case.err.save'), () =>
      api.saveCase({
        id: caseId,
        title: draft.title.trim(),
        status: draft.status,
        description: draft.description.trim() || null,
        openedAt: new Date(draft.openedAt || Date.now()).toISOString(),
      }),
    );
    if (saved) {
      detail = saved;
      editing = false;
      await refreshAll();
      notify('ok', t('case.updated'));
    }
  }

  async function remove(): Promise<void> {
    if (!record) return;
    const go = await confirmDelete({
      title: t('confirm.case.title'),
      message: t('confirm.case.body', { name: record.title }),
    });
    if (!go) return;
    const id = caseId;
    await deleteWithUndo({
      key: `case:${id}`,
      message: t('case.deleted'),
      before: () => openPane({ kind: 'grid' }),
      commit: async () => (await guard(t('case.err.delete'), () => api.deleteCase(id))) !== null,
      after: () => refreshAll(),
    });
  }

  function reveal(node: TreeNode): void {
    expandTo(node.id, treeNodes.value);
    openNode(node);
  }
</script>

{#if loading && !record}
  <div class="empty" style="height: 100%"><span class="spinner"></span></div>
{:else if !record}
  <div class="empty" style="height: 100%">
    <div class="empty-glyph">◆</div>
    <div class="empty-title">{t('case.notFound')}</div>
    <div class="empty-hint">{t('case.notFoundHint')}</div>
  </div>
{:else}
  <div class="detail">
    <PageHeader
      kind="case"
      title={record.title}
      serif
      eyebrow={record.reference}
      eyebrowColor={category?.color}
      properties={editing ? [] : caseProps}
    >
      {#snippet titleContent()}
        {#if editing}
          <!-- svelte-ignore a11y_autofocus -->
          <input class="title-input" bind:value={draft.title} aria-label={t('case.titleField')} />
        {:else}
          <h1>{record!.title}</h1>
        {/if}
      {/snippet}
      {#snippet actions()}
        <button class="btn btn-ghost btn-sm" onclick={() => askAbout(record!.reference.toLowerCase())}>
          {t('common.askAssistant')}
        </button>
        <button class="btn btn-sm" onclick={() => openPane({ kind: 'table', caseId })}>{t('view.table')}</button>
        <button class="btn btn-sm" onclick={() => openPane({ kind: 'calendar', caseId })}>{t('view.calendar')}</button>
        <button class="btn btn-sm" onclick={() => openPane({ kind: 'people', caseId })}>{t('view.roster')}</button>
        {#if editing}
          <button class="btn btn-sm" onclick={() => ((editing = false), load(caseId))}>{t('common.cancel')}</button>
          <button class="btn btn-primary btn-sm" onclick={() => void save()}>{t('common.save')}</button>
        {:else}
          <button class="btn btn-sm" onclick={() => (editing = true)}>{t('common.edit')}</button>
          <button class="btn btn-danger btn-sm" onclick={() => void remove()}>{t('common.delete')}</button>
        {/if}
      {/snippet}
    </PageHeader>

    {#if editing}
      <div class="edit">
        <div class="field-row">
          <div class="field">
            <label for="c-status">{t('common.status')}</label>
            <select id="c-status" bind:value={draft.status}>
              {#each CASE_STATUSES as s (s)}<option value={s}>{label(s)}</option>{/each}
            </select>
          </div>
          <div class="field">
            <label for="c-opened">{t('common.opened')}</label>
            <input id="c-opened" type="date" bind:value={draft.openedAt} />
          </div>
        </div>
        <div class="field">
          <label for="c-desc">{t('common.description')}</label>
          <textarea id="c-desc" rows="3" bind:value={draft.description}></textarea>
        </div>
      </div>
    {:else}
      <div class="stats">
        <div class="stat">
          <span class="s-n">{record.documentCount}</span>
          <span class="s-l faint">{t('kinds.document')}</span>
        </div>
        <div class="stat">
          <span class="s-n">{record.peopleCount}</span>
          <span class="s-l faint">{t('kinds.person')}</span>
        </div>
        <div class="stat">
          <span class="s-n">{children.filter((n) => n.kind === 'sheet').length}</span>
          <span class="s-l faint">{t('kinds.sheet')}</span>
        </div>
        <div class="stat">
          <span class="s-n">{children.filter((n) => n.kind === 'event').length}</span>
          <span class="s-l faint">{t('kinds.event')}</span>
        </div>
      </div>

      {#if record.description}
        <p class="desc">{record.description}</p>
      {/if}
    {/if}

    <!-- The case's connections on an endless grid; Life when there are none. -->
    <section class="connections" aria-label={t('graph.title')}>
      <header>
        <span class="eyebrow">{t('graph.title')}</span>
        {#if caseNode && connectionCount(caseNode, children) > 0}
          <span class="g-count faint">{t('graph.hint')}</span>
        {/if}
      </header>
      <div class="graph-box">
        {#if caseNode && connectionCount(caseNode, children) > 0}
          <CaseGraph {caseNode} nodes={children} />
        {:else}
          <LifeWaves label={t('graph.empty')} />
        {/if}
      </div>
    </section>

    <div class="groups">
      {#if groups.length === 0}
        <div class="empty" style="padding: 40px">
          <div class="empty-glyph">▤</div>
          <div class="empty-title">{t('case.emptyTitle')}</div>
          <div class="empty-hint">
            {t('case.emptyHint')}
          </div>
        </div>
      {/if}
      {#each groups as g (g.kind)}
        <section>
          <header>
            <span class="eyebrow">{g.label}</span>
            <span class="g-count faint">{g.nodes.length}</span>
          </header>
          <div class="chips">
            {#each g.nodes as n (n.id)}
              <button class="chip" data-artifact={n.id} onclick={() => reveal(n)} oncontextmenu={(e) => openNodeMenu(e, n)} title={n.nodeKey}>
                {#if g.kind === 'document'}<span class="c-i">▤</span>{/if}
                {#if g.kind === 'person'}<span class="c-i">●</span>{/if}
                {#if g.kind === 'sheet'}<span class="c-i">▦</span>{/if}
                {#if g.kind === 'event'}<span class="c-i">◷</span>{/if}
                <span class="truncate">{n.label}</span>
                {#if n.status && g.kind !== 'person'}<span class="c-s faint">{label(n.status)}</span>{/if}
              </button>
            {/each}
          </div>
        </section>
      {/each}
    </div>
  </div>
{/if}

{#snippet statusProp()}
  {#if record}<StatusBadge status={record.status} />{/if}
{/snippet}

<style>
  .detail {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 16px 20px 40px;
  }

  /* The `h1` display role lives in app.css and is the only serif in the app:
     it is the name of the matter, so it reads as prose rather than as a cell.
     The edit field has to match it exactly, or the title visibly reflows on
     click. */
  .title-input {
    font-family: var(--font-display);
    font-size: 25px;
    font-weight: 500;
    line-height: 1.2;
    letter-spacing: -0.02em;
    width: 340px;
  }

  .edit {
    margin-top: 14px;
    padding: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface);
  }

  .stats {
    display: flex;
    gap: 22px;
    align-items: flex-end;
    margin: 16px 0 4px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface);
    flex-wrap: wrap;
  }
  .stat {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .s-n {
    font-size: 20px;
    font-weight: 600;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
  }
  .s-l {
    font-size: 10.5px;
  }

  .desc {
    font-size: 13px;
    line-height: 1.65;
    color: var(--text-secondary);
    max-width: 70ch;
    white-space: pre-wrap;
  }

  .connections {
    margin-top: 16px;
  }
  /* A small window onto the graph: bordered like the stats card above it,
     since it sits in the layout rather than floating. */
  .graph-box {
    height: 240px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--surface);
    overflow: hidden;
  }

  .groups {
    margin-top: 20px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  section header {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-bottom: 7px;
  }
  .g-count {
    font-size: 10.5px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    max-width: 280px;
    padding: 4px 9px;
    border: 1px solid var(--border);
    border-radius: var(--radius-full);
    background: var(--surface);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease),
      border-color var(--dur-fast) var(--ease);
  }
  .chip:hover {
    border-color: var(--border-strong);
    background: var(--sunken);
  }
  .c-i {
    color: var(--text-tertiary);
    font-size: 9px;
  }
  .c-s {
    font-size: 10px;
  }
</style>
