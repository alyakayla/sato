<script lang="ts">
  import { deleteWithUndo } from '$lib/undoDelete';
  import { confirmDelete } from '$lib/confirm.svelte';
  import { t } from '$lib/i18n/index.svelte';
  import { api, formatRelative } from '$lib/api';
  import SheetGrid from '$lib/components/SheetGrid.svelte';
  import PageHeader, { type PageProp } from '$lib/components/PageHeader.svelte';
  import { computeGrid, type ComputedCell, type Grid } from '$lib/formula';
  import { guard, notify, openPane, refreshTree, takeEditRequest } from '$lib/stores.svelte';
  import type { Spreadsheet } from '$lib/types';

  let { sheetId }: { sheetId: string } = $props();

  let sheet = $state<Spreadsheet | null>(null);
  let grid = $state<Grid>({});
  let loading = $state(true);
  let dirty = $state(false);
  let renaming = $state(false);
  let nameValue = $state('');

  const sheetProps = $derived.by((): PageProp[] => {
    if (!sheet) return [];
    const out: PageProp[] = [
      { label: t('common.case'), content: caseProp },
      { label: t('common.size'), value: `${sheet.rows} × ${sheet.cols}` },
      { label: t('common.updated'), value: formatRelative(sheet.updatedAt), title: sheet.updatedAt },
    ];
    if (dirty) out.push({ label: t('common.state'), content: dirtyProp });
    return out;
  });

  const computed = $derived<Record<string, ComputedCell>>(computeGrid(grid));

  async function load(id: string): Promise<void> {
    loading = true;
    const full = await guard(t('sheet.err.open'), () => api.getSpreadsheet(id));
    if (full) {
      sheet = full;
      nameValue = full.name;
      try {
        grid = JSON.parse(full.data || '{}') as Grid;
      } catch {
        // A malformed payload should not brick the view; start it empty so the
        // user can retype rather than seeing a parse error on every cell.
        grid = {};
        notify('warn', t('sheet.reset'));
      }
      dirty = false;
      if (takeEditRequest('sheet', id)) renaming = true;
    }
    loading = false;
  }

  $effect(() => {
    void load(sheetId);
  });

  function onCellChange(key: string, value: string, formula: string | undefined) {
    const next = { ...grid, [key]: formula ? { v: value, f: formula } : value ? { v: value } : {} };
    if (!value && !formula) delete next[key];
    grid = next;
    dirty = true;
  }

  async function save(): Promise<boolean> {
    if (!sheet) return false;
    const ok = await guard(t('sheet.err.save'), () =>
      api.saveSpreadsheet({ id: sheet!.id, data: JSON.stringify(grid) }),
    );
    if (ok === null) return false;
    dirty = false;
    await refreshTree();
    return true;
  }

  // Debounced autosave: a crash should cost at most the last keystroke.
  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    if (!dirty) return;
    void grid;
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void save(), 900);
  });

  async function rename(): Promise<void> {
    if (!sheet) return;
    const name = nameValue.trim();
    if (!name) {
      nameValue = sheet.name;
      renaming = false;
      return;
    }
    const ok = await guard(t('sheet.err.rename'), () =>
      api.saveSpreadsheet({ id: sheet!.id, name, data: JSON.stringify(grid) }),
    );
    if (ok !== null) {
      sheet = { ...sheet, name };
      dirty = false;
      renaming = false;
      await refreshTree();
    }
  }

  async function remove(): Promise<void> {
    if (!sheet) return;
    if (!(await confirmDelete({ title: t('confirm.sheet.title'), message: t('confirm.sheet.body', { name: sheet.name }) }))) return;
    const id = sheet.id;
    await deleteWithUndo({
      key: `sheet:${id}`,
      message: t('sheet.deleted'),
      before: () => openPane({ kind: 'grid' }),
      commit: async () => (await guard(t('sheet.err.delete'), () => api.deleteSpreadsheet(id))) !== null,
      after: () => refreshTree(),
    });
  }

  function exportCsv(): void {
    if (!sheet) return;
    // Only export the used range, not 200 blank lines.
    let maxRow = 0;
    let maxCol = 0;
    for (const key of Object.keys(grid)) {
      const [r, c] = key.split(',').map(Number);
      if (r > maxRow) maxRow = r;
      if (c > maxCol) maxCol = c;
    }
    const lines: string[] = [];
    for (let r = 0; r <= maxRow; r++) {
      const cells: string[] = [];
      for (let c = 0; c <= maxCol; c++) {
        const v = computed[`${r},${c}`]?.v ?? '';
        cells.push(v.includes(',') || v.includes('"') ? `"${v.replace(/"/g, '""')}"` : v);
      }
      lines.push(cells.join(','));
    }
    const url = URL.createObjectURL(new Blob([lines.join('\n')], { type: 'text/csv' }));
    const a = document.createElement('a');
    a.href = url;
    a.download = `${sheet.name.replace(/[^\w-]+/g, '_')}.csv`;
    a.click();
    URL.revokeObjectURL(url);
    notify('ok', t('sheet.exported'));
  }

  const examples = $derived([
    { id: 'sum', label: 'SUM', f: '=SUM(B2:B10)' },
    { id: 'rate', label: t('sheet.ex.rate'), f: '=SUMPRODUCT(A2:A10,B2:B10)' },
    { id: 'tax', label: t('sheet.ex.tax'), f: '=ROUND(B2*1.15, 2)' },
    { id: 'count', label: t('sheet.ex.count'), f: '=COUNTA(A2:A100)' },
  ]);

  /** Drop an example formula into the first empty cell in column A. */
  function insertExample(f: string): void {
    for (let r = 0; r < 200; r++) {
      if (!computed[`${r},0`]) {
        onCellChange(`${r},0`, f.slice(1), f);
        return;
      }
    }
    onCellChange('0,0', f.slice(1), f);
  }
</script>

{#if loading}
  <div class="empty" style="height: 100%"><span class="spinner"></span></div>
{:else if !sheet}
  <div class="empty" style="height: 100%">
    <div class="empty-glyph">▦</div>
    <div class="empty-title">{t('sheet.notFound')}</div>
  </div>
{:else}
  <div class="sheetwrap">
    <div class="page">
      <PageHeader kind="sheet" title={sheet.name} properties={sheetProps}>
        {#snippet titleContent()}
          {#if renaming}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="name-input"
              bind:value={nameValue}
              aria-label={t('sheet.nameField')}
              onblur={() => void rename()}
              onkeydown={(e) => {
                if (e.key === 'Enter') void rename();
                if (e.key === 'Escape') ((renaming = false), (nameValue = sheet!.name));
              }}
            />
          {:else}
            <h2 class="title" ondblclick={() => (renaming = true)} title={t('sheet.renameHint')}>
              {sheet!.name}
            </h2>
          {/if}
        {/snippet}
        {#snippet actions()}
          <button class="btn btn-sm" onclick={exportCsv}>{t('table.exportCsv')}</button>
          <button class="btn btn-sm" onclick={() => void save()} disabled={!dirty}>{t('common.save')}</button>
          <button class="btn btn-danger btn-sm" onclick={() => void remove()}>{t('common.delete')}</button>
        {/snippet}
      </PageHeader>
    </div>

    <div class="toolbar">
      <span class="faint tiny">{t('sheet.insert')}</span>
      {#each examples as ex (ex.id)}
        <button class="btn btn-ghost btn-sm" title={ex.f} onclick={() => insertExample(ex.f)}>
          {ex.label}
        </button>
      {/each}
    </div>

    <SheetGrid {grid} {computed} onchange={onCellChange} />
  </div>
{/if}

{#snippet caseProp()}
  {#if sheet?.caseId}
    <button class="link" onclick={() => openPane({ kind: 'case', caseId: sheet!.caseId! })} title={sheet.caseTitle ?? ''}>
      {sheet.caseReference}
    </button>
  {:else}
    <span class="faint">{t('common.unfiled')}</span>
  {/if}
{/snippet}

{#snippet dirtyProp()}
  <span class="badge badge-warn">{t('common.unsaved')}</span>
{/snippet}

<style>
  .sheetwrap {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .page {
    padding: 14px 16px 10px;
    background: var(--surface);
    flex-shrink: 0;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 4px 12px;
    border-top: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    flex-shrink: 0;
  }

  .title {
    font-size: 16px;
    line-height: 1.35;
    letter-spacing: -0.01em;
    cursor: text;
    white-space: nowrap;
    margin: 0;
  }

  /* Matches the `h2` it replaces, so the name does not reflow on rename. */
  .name-input {
    width: 220px;
    padding: 3px 7px;
    font-size: 16px;
    line-height: 1.35;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .tiny {
    font-size: 10.5px;
  }
</style>
