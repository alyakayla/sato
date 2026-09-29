<script lang="ts">
  import { nodeIndex } from '$lib/tree';
  import { caseTarget, documentTarget, eventTarget, openArtifactMenu, personTarget, sheetTarget, type ArtifactTarget } from '$lib/artifactMenu';
  import { isContextMenuKey } from '$lib/contextMenu.svelte';
  import { label, t } from '$lib/i18n/index.svelte';
  import type { CalendarEvent, Case, Document, Person, Spreadsheet } from '$lib/types';
  import type { MessageKey } from '$lib/i18n/index.svelte';
  import { api, formatBytes, formatDate, formatRelative } from '$lib/api';
  import {
    artifactsVersion,
    askAbout,
    chatCollapsed,
    guard,
    openPane,
    refreshPeople,
    toggleChat,
    treeNodes,
  } from '$lib/stores.svelte';
  import StatusBadge from '$lib/components/StatusBadge.svelte';
  import { CASE_STATUSES } from '$lib/types';

  let { caseId = null }: { caseId?: string | null } = $props();

  /**
   * A sortable, filterable view over any of the app's flat datasets. The column
   * set is data rather than markup so every dataset shares one implementation
   * of search, sort and CSV export.
   */

  type Dataset = 'cases' | 'documents' | 'people' | 'events' | 'sheets';

  interface Column {
    key: string;
    label: string;
    /** Cell text. */
    get: (r: never) => string;
    /** Sort key; omit for columns that should not be sortable. */
    sort?: (r: never) => string | number;
    numeric?: boolean;
    mono?: boolean;
  }

  const DATASETS: { id: Dataset; label: MessageKey; icon: string; scoped: boolean }[] = [
    { id: 'cases', label: 'kinds.case', icon: '◆', scoped: false },
    { id: 'documents', label: 'kinds.document', icon: '▤', scoped: true },
    { id: 'people', label: 'kinds.person', icon: '●', scoped: true },
    { id: 'events', label: 'kinds.event', icon: '◷', scoped: true },
    { id: 'sheets', label: 'kinds.sheet', icon: '▦', scoped: true },
  ];

  let dataset = $state<Dataset>('cases');
  let cases = $state<Case[]>([]);
  let documents = $state<Document[]>([]);
  let people = $state<Person[]>([]);
  let events = $state<CalendarEvent[]>([]);
  let sheets = $state<Spreadsheet[]>([]);
  let search = $state('');
  let statusFilter = $state<string>('');
  let sortKey = $state<string>('');
  let sortDir = $state<1 | -1>(1);
  let loading = $state(true);
  /** The row the user last opened, so the table shows where they are. */
  let selectedId = $state<string | null>(null);

  const defs = $derived.by((): Record<Dataset, Column[]> => {
    return {
      cases: [
        { key: 'reference', label: t('field.reference'), get: (r: Case) => r.reference, sort: (r: Case) => r.reference, mono: true },
        { key: 'title', label: t('field.title'), get: (r: Case) => r.title, sort: (r: Case) => r.title },
        {
          key: 'category',
          label: t('common.category'),
          get: (r: Case) => r.categoryName ?? '—',
          sort: (r: Case) => r.categoryName ?? '',
        },
        { key: 'status', label: t('common.status'), get: (r: Case) => r.status, sort: (r: Case) => r.status },
        {
          key: 'documents',
          label: t('table.col.docs'),
          get: (r: Case) => String(r.documentCount),
          sort: (r: Case) => r.documentCount,
          numeric: true,
        },
        {
          key: 'people',
          label: t('view.people'),
          get: (r: Case) => String(r.peopleCount),
          sort: (r: Case) => r.peopleCount,
          numeric: true,
        },
        { key: 'opened', label: t('common.opened'), get: (r: Case) => r.openedAt, sort: (r: Case) => r.openedAt },
        { key: 'updated', label: t('common.updated'), get: (r: Case) => r.updatedAt, sort: (r: Case) => r.updatedAt },
      ],
      documents: [
        { key: 'name', label: t('table.col.file'), get: (r: Document) => r.fileName, sort: (r: Document) => r.fileName.toLowerCase() },
        {
          key: 'case',
          label: t('common.case'),
          get: (r: Document) => r.caseReference ?? '—',
          sort: (r: Document) => r.caseReference ?? '',
          mono: true,
        },
        { key: 'type', label: t('table.col.type'), get: (r: Document) => r.mime, sort: (r: Document) => r.mime },
        { key: 'status', label: t('common.index'), get: (r: Document) => r.indexStatus, sort: (r: Document) => r.indexStatus },
        {
          key: 'size',
          label: t('common.size'),
          get: (r: Document) => formatBytes(r.sizeBytes),
          sort: (r: Document) => r.sizeBytes,
          numeric: true,
        },
        {
          key: 'pages',
          label: t('common.pages'),
          get: (r: Document) => (r.pageCount == null ? '—' : String(r.pageCount)),
          sort: (r: Document) => r.pageCount ?? -1,
          numeric: true,
        },
        {
          key: 'chunks',
          label: t('table.col.chunks'),
          get: (r: Document) => String(r.chunkCount),
          sort: (r: Document) => r.chunkCount,
          numeric: true,
        },
        { key: 'added', label: t('table.col.added'), get: (r: Document) => r.createdAt, sort: (r: Document) => r.createdAt },
      ],
      people: [
        { key: 'name', label: t('common.name'), get: (r: Person) => r.fullName, sort: (r: Person) => r.fullName.toLowerCase() },
        { key: 'role', label: t('common.role'), get: (r: Person) => r.role, sort: (r: Person) => r.role },
        {
          key: 'org',
          label: t('field.organisation'),
          get: (r: Person) => r.organization ?? '—',
          sort: (r: Person) => r.organization ?? '',
        },
        {
          key: 'email',
          label: t('field.email'),
          get: (r: Person) => r.email ?? '—',
          sort: (r: Person) => r.email ?? '',
        },
        {
          key: 'phone',
          label: t('field.phone'),
          get: (r: Person) => r.phone ?? '—',
          sort: (r: Person) => r.phone ?? '',
          mono: true,
        },
        {
          key: 'cases',
          label: t('kinds.case'),
          get: (r: Person) => String(r.caseCount),
          sort: (r: Person) => r.caseCount,
          numeric: true,
        },
      ],
      events: [
        { key: 'title', label: t('kind.event'), get: (r: CalendarEvent) => r.title, sort: (r: CalendarEvent) => r.title.toLowerCase() },
        { key: 'kind', label: t('tree.kind'), get: (r: CalendarEvent) => r.kind, sort: (r: CalendarEvent) => r.kind },
        {
          key: 'case',
          label: t('common.case'),
          get: (r: CalendarEvent) => r.caseReference ?? '—',
          sort: (r: CalendarEvent) => r.caseReference ?? '',
          mono: true,
        },
        { key: 'start', label: t('table.col.starts'), get: (r: CalendarEvent) => r.startsAt, sort: (r: CalendarEvent) => r.startsAt },
        { key: 'end', label: t('table.col.ends'), get: (r: CalendarEvent) => r.endsAt ?? '—', sort: (r: CalendarEvent) => r.endsAt ?? '' },
        {
          key: 'location',
          label: t('field.location'),
          get: (r: CalendarEvent) => r.location ?? '—',
          sort: (r: CalendarEvent) => r.location ?? '',
        },
      ],
      sheets: [
        { key: 'name', label: t('common.name'), get: (r: Spreadsheet) => r.name, sort: (r: Spreadsheet) => r.name.toLowerCase() },
        {
          key: 'case',
          label: t('common.case'),
          get: (r: Spreadsheet) => r.caseReference ?? '—',
          sort: (r: Spreadsheet) => r.caseReference ?? '',
          mono: true,
        },
        {
          key: 'size',
          label: t('table.col.grid'),
          get: (r: Spreadsheet) => `${r.rows} × ${r.cols}`,
          sort: (r: Spreadsheet) => r.rows * r.cols,
          numeric: true,
        },
        { key: 'updated', label: t('common.updated'), get: (r: Spreadsheet) => r.updatedAt, sort: (r: Spreadsheet) => r.updatedAt },
      ],
    };
  });

  const columns = $derived(defs[dataset]);

  /** The active dataset, filtered by case scope, search and status. */
  const rows = $derived.by((): unknown[] => {
    const scope = caseId;
    const all: unknown[] =
      dataset === 'cases'
        ? cases
        : dataset === 'documents'
          ? documents.filter((d) => !scope || d.caseId === scope)
          : dataset === 'people'
            ? people
            : dataset === 'events'
              ? events.filter((e) => !scope || e.caseId === scope)
              : sheets.filter((s) => !scope || s.caseId === scope);

    const q = search.trim().toLowerCase();
    const col = columns.find((c) => c.key === statusFilter);
    let out = all;
    if (q) out = out.filter((r) => columns.some((c) => c.get(r as never).toLowerCase().includes(q)));
    if (col) out = out.filter((r) => c_status(r as never, dataset));

    const sortCol = columns.find((c) => c.key === sortKey);
    if (sortCol?.sort) {
      out = [...out].sort((a, b) => {
        const av = sortCol.sort!(a as never);
        const bv = sortCol.sort!(b as never);
        if (av === bv) return 0;
        return (av < bv ? -1 : 1) * sortDir;
      });
    }
    return out;
  });

  function c_status(r: Case | Document | Person | CalendarEvent | Spreadsheet, d: Dataset): boolean {
    if (d === 'cases') return (r as Case).status === statusFilter;
    if (d === 'documents') return (r as Document).indexStatus === statusFilter;
    if (d === 'events') return (r as CalendarEvent).kind === statusFilter;
    if (d === 'people') return (r as Person).role === statusFilter;
    return true;
  }

  /**
   * Loads the dataset on screen, and only that one. Fetching every dataset up
   * front meant opening the cases table also shipped every document in the
   * firm across IPC. A later load for another tab wins; a stale one is
   * dropped.
   */
  let loadSeq = 0;
  async function load(ds: Dataset): Promise<void> {
    const seq = ++loadSeq;
    loading = true;
    if (ds === 'cases') {
      const c = await guard(t('kinds.case'), () => api.listCases());
      if (seq === loadSeq && c) cases = c;
    } else if (ds === 'documents') {
      const d = await guard(t('kinds.document'), () => api.listDocuments({ caseId }));
      if (seq === loadSeq && d) documents = d;
    } else if (ds === 'people') {
      const p = await guard(t('kinds.person'), () => api.listPeople());
      if (seq === loadSeq && p) people = p;
    } else if (ds === 'sheets') {
      const sh = await guard(t('kinds.sheet'), () => api.listSpreadsheets(caseId));
      if (seq === loadSeq && sh) sheets = sh;
    } else {
      // Events are unbounded, so only fetch a wide window around today.
      const now = Date.now();
      const e = await guard(t('kinds.event'), () =>
        api.listEvents({
          caseId,
          from: new Date(now - 365 * 864e5).toISOString(),
          to: new Date(now + 365 * 864e5).toISOString(),
        }),
      );
      if (seq === loadSeq && e) events = e;
    }
    if (seq === loadSeq) loading = false;
  }

  $effect(() => {
    const ds = dataset;
    void caseId;
    void artifactsVersion.value;
    selectedId = null;
    void load(ds);
  });

  // --- Virtualised rows ----------------------------------------------------
  // Only rows in (or near) view are in the DOM. Rendering every row put tens
  // of thousands of <tr> into the page for the documents table, freezing it
  // for a minute and slowing every interaction after. Spacer rows above and
  // below keep the scrollbar honest.
  const OVERSCAN = 12;
  let wrap = $state<HTMLDivElement | null>(null);
  let scrollTop = $state(0);
  let viewH = $state(800);
  /** Measured from a real row; a table row's height depends on its borders. */
  let rowH = $state(31);

  const first = $derived(Math.max(0, Math.floor(scrollTop / rowH) - OVERSCAN));
  const last = $derived(Math.min(rows.length, Math.ceil((scrollTop + viewH) / rowH) + OVERSCAN));
  const windowed = $derived(rows.slice(first, last));

  function measure(): void {
    if (!wrap) return;
    scrollTop = wrap.scrollTop;
    viewH = wrap.clientHeight;
    const tr = wrap.querySelector<HTMLElement>('tbody tr.data-row');
    const h = tr?.getBoundingClientRect().height;
    if (h && Math.abs(h - rowH) > 0.5) rowH = h;
  }

  $effect(() => {
    const el = wrap;
    if (!el) return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  });

  // A new dataset starts at the top.
  $effect(() => {
    void dataset;
    if (wrap) wrap.scrollTop = 0;
    scrollTop = 0;
  });

  function pickSort(key: string): void {
    if (sortKey === key) {
      sortDir = sortDir === 1 ? -1 : 1;
    } else {
      sortKey = key;
      sortDir = 1;
    }
  }

  function idOf(r: unknown): string {
    return (r as { id: string }).id;
  }

  function openRow(r: unknown): void {
    selectedId = idOf(r);
    if (dataset === 'cases') openPane({ kind: 'case', caseId: (r as Case).id });
    else if (dataset === 'documents') openPane({ kind: 'document', documentId: (r as Document).id });
    else if (dataset === 'sheets') openPane({ kind: 'sheet', sheetId: (r as Spreadsheet).id });
    else if (dataset === 'events') openPane({ kind: 'calendar', caseId: (r as CalendarEvent).caseId });
    else openPane({ kind: 'people', caseId: caseId });
  }

  /**
   * The mention handle for a row, when it maps onto a tree node the assistant
   * can be asked about. People and events are not individually addressable.
   */
  function handleOf(r: unknown): string | null {
    const idx = nodeIndex(treeNodes.value);
    let node;
    if (dataset === 'cases') node = idx.byCaseId.get((r as Case).id);
    else if (dataset === 'documents') node = idx.byDocumentId.get((r as Document).id);
    else if (dataset === 'sheets') node = idx.byId.get(`sheet:${(r as Spreadsheet).id}`);
    return node?.nodeKey ?? null;
  }

  /** The context-menu target for a row of the current dataset. */
  function targetOfRow(r: unknown): ArtifactTarget {
    if (dataset === 'cases') return caseTarget(r as Case);
    if (dataset === 'documents') return documentTarget((r as Document).id, (r as Document).fileName);
    if (dataset === 'sheets') return sheetTarget(r as Spreadsheet);
    if (dataset === 'events') return eventTarget(r as CalendarEvent);
    return personTarget(r as Person, caseId);
  }

  function ask(handle: string): void {
    if (chatCollapsed.value) toggleChat();
    askAbout(handle);
  }

  /** Rendered cell text, with two columns that get special treatment. */
  function cell(r: unknown, key: string): string {
    if (dataset === 'cases' && key === 'opened') return formatDate((r as Case).openedAt);
    if (dataset === 'cases' && key === 'updated') return formatRelative((r as Case).updatedAt);
    if (dataset === 'documents' && key === 'added') return formatRelative((r as Document).createdAt);
    if (dataset === 'events' && key === 'start') return formatDate((r as CalendarEvent).startsAt);
    if (dataset === 'sheets' && key === 'updated') return formatRelative((r as Spreadsheet).updatedAt);
    const col = columns.find((c) => c.key === key);
    return col ? col.get(r as never) : '';
  }

  function isBadge(r: unknown, key: string): boolean {
    return (
      (dataset === 'cases' && key === 'status') ||
      (dataset === 'documents' && key === 'status') ||
      (dataset === 'events' && key === 'kind')
    );
  }

  const statusOptions = $derived(
    dataset === 'cases'
      ? [...CASE_STATUSES]
      : dataset === 'documents'
        ? ['Pending', 'Indexing', 'Ready', 'Failed']
        : dataset === 'events'
          ? ['Hearing', 'Filing', 'Meeting', 'Deadline', 'Other']
          : dataset === 'people'
            ? ['Client', 'Opposing Party', 'Witness', 'Judge', 'Expert', 'Other']
            : [],
  );

  function exportCsv(): void {
    const head = columns.map((c) => c.label).join(',');
    const body = rows
      .map((r) => columns.map((c) => escapeCsv(c.get(r as never))).join(','))
      .join('\n');
    const url = URL.createObjectURL(new Blob([`${head}\n${body}`], { type: 'text/csv' }));
    const a = document.createElement('a');
    a.href = url;
    a.download = `counsel-${dataset}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }

  function escapeCsv(v: string): string {
    return v.includes(',') || v.includes('"') || v.includes('\n') ? `"${v.replace(/"/g, '""')}"` : v;
  }
</script>

<div class="wrap">
  <div class="tools">
    <div class="tabs">
      {#each DATASETS as d (d.id)}
        <button
          class="tab"
          class:on={dataset === d.id}
          onclick={() => {
            dataset = d.id;
            sortKey = '';
            statusFilter = '';
          }}
        >
          <span class="t-icon">{d.icon}</span>{t(d.label)}
        </button>
      {/each}
    </div>

    <div class="spacer"></div>

    {#if statusOptions.length > 0}
      <select bind:value={statusFilter} class="filter" title={t('table.filterStatus')}>
        <option value="">{t('table.all')}</option>
        {#each statusOptions as s (s)}
          <option value={s}>{label(s)}</option>
        {/each}
      </select>
    {/if}

    <input class="search" type="search" placeholder={t('table.searchRows')} bind:value={search} spellcheck="false" />

    <button class="btn btn-ghost btn-sm" onclick={exportCsv} disabled={rows.length === 0}>
      {t('table.exportCsv')}
    </button>
  </div>

  <div class="tablewrap" bind:this={wrap} onscroll={measure}>
    {#if loading}
      <div class="empty" style="height: 100%"><span class="spinner"></span></div>
    {:else if rows.length === 0}
      <div class="empty" style="height: 100%">
        <div class="empty-glyph">{DATASETS.find((d) => d.id === dataset)?.icon}</div>
        <div class="empty-title">{t('table.emptyTitle')}</div>
        <div class="empty-hint">
          {search || statusFilter
            ? t('table.noMatch')
            : caseId
              ? t('table.emptyCase')
              : t('table.emptyAll')}
        </div>
      </div>
    {:else}
      <table>
        <thead>
          <tr>
            {#each columns as c (c.key)}
              <th
                class:num={c.numeric}
                class:sorted={sortKey === c.key}
                class:sortable={!!c.sort}
                aria-sort={sortKey === c.key ? (sortDir === 1 ? 'ascending' : 'descending') : 'none'}
                onclick={() => c.sort && pickSort(c.key)}
              >
                {c.label}
                {#if sortKey === c.key}
                  <span class="arrow">{sortDir === 1 ? '▲' : '▼'}</span>
                {/if}
              </th>
            {/each}
            <th class="act" aria-label={t('common.actions')}></th>
          </tr>
        </thead>
        <tbody>
          {#if first > 0}
            <tr class="spacer" aria-hidden="true" style:height="{first * rowH}px"><td colspan={columns.length + 1}></td></tr>
          {/if}
          {#each windowed as r, i (first + i)}
            {@const handle = handleOf(r)}
            <tr
              class="data-row"
              class:even={(first + i) % 2 === 1}
              class:sel={selectedId === idOf(r)}
              tabindex="0"
              onclick={() => openRow(r)}
              oncontextmenu={(e) => {
                selectedId = idOf(r);
                openArtifactMenu(e, targetOfRow(r));
              }}
              onkeydown={(e) => {
                if (e.target !== e.currentTarget) return;
                if (isContextMenuKey(e)) {
                  e.preventDefault();
                  openArtifactMenu(e.currentTarget as HTMLElement, targetOfRow(r));
                } else if (e.key === 'Enter') openRow(r);
                else if (e.key === 'a' && handle) ask(handle);
              }}
            >
              {#each columns as c (c.key)}
                <td class:num={c.numeric} class:mono={c.mono}>
                  {#if isBadge(r, c.key)}
                    {#if dataset === 'events'}
                      <span class="badge badge-muted">{label(cell(r, c.key))}</span>
                    {:else}
                      <StatusBadge status={cell(r, c.key)} />
                    {/if}
                  {:else}
                    {c.key === 'role' ? label(cell(r, c.key)) : cell(r, c.key)}
                  {/if}
                </td>
              {/each}
              <td class="act">
                {#if handle}
                  <span class="row-actions">
                    <button
                      class="row-action"
                      tabindex="-1"
                      onclick={(e) => {
                        e.stopPropagation();
                        ask(handle);
                      }}
                      title={t('row.askHint')}>{t('common.ask')}</button
                    >
                  </span>
                {/if}
              </td>
            </tr>
          {/each}
          {#if last < rows.length}
            <tr class="spacer" aria-hidden="true" style:height="{(rows.length - last) * rowH}px"><td colspan={columns.length + 1}></td></tr>
          {/if}
        </tbody>
      </table>
    {/if}
  </div>

  <div class="foot faint">
    {rows.length} row{rows.length === 1 ? '' : 's'}
    {#if caseId}· scoped to one case{/if}
    {#if sortKey}· sorted by {columns.find((c) => c.key === sortKey)?.label}{/if}
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

  .tools {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .tabs {
    display: flex;
    gap: 2px;
  }
  .tab {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 12px;
    padding: 4px 9px;
    transition:
      background-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .tab:hover {
    background: var(--sunken);
    color: var(--text);
  }
  /* The thumb is the selection cue. A second coloured fill on top of it would
     be the same information twice. */
  .tab.on {
    background: var(--surface);
    color: var(--text);
    font-weight: 600;
    box-shadow: var(--shadow-xs);
  }
  .t-icon {
    font-size: 9px;
  }

  .filter {
    width: auto;
    padding: 4px 22px 4px 7px;
    font-size: 11.5px;
    background-position: right 5px center;
  }

  .search {
    width: 180px;
    padding: 4px 8px;
    font-size: 12px;
  }

  .tablewrap {
    flex: 1;
    min-height: 0;
    overflow: auto;
    background: var(--surface);
  }

  table {
    width: 100%;
    border-collapse: separate;
    border-spacing: 0;
    font-size: 13px;
  }

  /* Sentence-case headers on the inner band; the sort arrow, not colour, says
     which column is live. */
  th {
    position: sticky;
    top: 0;
    z-index: 2;
    background: var(--surface-2);
    border-bottom: 1px solid var(--border);
    text-align: left;
    font-size: 11.5px;
    line-height: 1.45;
    font-weight: 600;
    letter-spacing: 0;
    text-transform: none;
    color: var(--text-secondary);
    height: var(--control-md);
    padding: 0 10px;
    white-space: nowrap;
    user-select: none;
  }
  th.sortable {
    cursor: pointer;
  }
  th.sortable:hover {
    color: var(--text);
  }
  th.sorted {
    color: var(--text);
  }
  .arrow {
    font-size: 7px;
    margin-left: 3px;
  }

  td {
    height: 30px;
    padding: 0 10px;
    border-bottom: 1px solid var(--border);
    color: var(--text);
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  tbody tr.data-row {
    cursor: pointer;
    transition: background-color var(--dur-fast) var(--ease);
  }
  /* Striping by the row's real index: `:nth-child` would shift as the
     virtual window scrolls. */
  tbody tr.even {
    background: var(--stripe);
  }
  tbody tr.data-row:hover {
    background: var(--stripe-hover);
  }
  tbody tr.spacer td {
    height: auto;
    padding: 0;
    border: none;
  }
  tbody tr.sel {
    background: var(--accent-soft);
    box-shadow: inset 2px 0 0 var(--accent);
  }

  /* Inset, so the ring is not clipped by the scroll container. */
  tbody tr:focus-visible {
    outline-offset: -2px;
  }

  /* Shrink-wraps the hover actions at the row's end. */
  .act {
    width: 1%;
    padding: 0 6px;
  }

  td.num,
  th.num {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  td.mono {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
  }

  .foot {
    padding: 5px 12px;
    border-top: 1px solid var(--border);
    font-size: 10.5px;
    background: var(--surface-2);
    flex-shrink: 0;
  }
</style>
