<script lang="ts">
  import { t } from '$lib/i18n/index.svelte';
  import {
    cellKey,
    colName,
    computeGrid,
    isNumericText,
    parseKey,
    type ComputedCell,
    type Grid,
  } from '$lib/formula';

  interface Props {
    grid: Grid;
    computed: Record<string, ComputedCell>;
    onchange: (key: string, value: string, formula: string | undefined) => void;
  }

  let { grid, computed, onchange }: Props = $props();

  const ROWS = 200;
  const COLS = 26;

  let editing = $state<{ row: number; col: number } | null>(null);
  let draft = $state('');
  let inputEl = $state<HTMLInputElement | null>(null);
  let selection = $state<{ row: number; col: number }>({ row: 0, col: 0 });
  let gridEl = $state<HTMLDivElement | null>(null);

  // Autoscroll the viewport so the selected cell is always visible.
  $effect(() => {
    if (!editing || !gridEl) return;
    void inputEl?.focus();
    void inputEl?.select();
  });

  $effect(() => {
    if (!gridEl) return;
    const el = gridEl.querySelector(`[data-cell="${selection.row},${selection.col}"]`);
    el?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  });

  function startEdit(row: number, col: number) {
    const key = cellKey(row, col);
    const cell = computed[key];
    // Show the formula when there is one, otherwise the last raw entry.
    draft = cell?.f ?? cell?.v ?? '';
    editing = { row, col };
    selection = { row, col };
  }

  function commit(moveDown = true) {
    if (!editing) return;
    const { row, col } = editing;
    const key = cellKey(row, col);
    const text = draft.trim();
    if (text.startsWith('=')) {
      onchange(key, text.slice(1), text);
    } else {
      onchange(key, text, undefined);
    }
    editing = null;
    if (moveDown) selection = { row: Math.min(row + 1, ROWS - 1), col };
  }

  function cancel() {
    editing = null;
  }

  function onKeydown(e: KeyboardEvent, row: number, col: number) {
    if (e.key === 'Enter') {
      e.preventDefault();
      commit(true);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      cancel();
    } else if (e.key === 'Tab') {
      e.preventDefault();
      commit(false);
      selection = { row, col: Math.min(col + 1, COLS - 1) };
    } else if (e.key.startsWith('Arrow')) {
      e.preventDefault();
      const next = { ...selection };
      if (e.key === 'ArrowUp') next.row = Math.max(0, row - 1);
      if (e.key === 'ArrowDown') next.row = Math.min(ROWS - 1, row + 1);
      if (e.key === 'ArrowLeft') next.col = Math.max(0, col - 1);
      if (e.key === 'ArrowRight') next.col = Math.min(COLS - 1, col + 1);
      selection = next;
    }
  }

  function onPaste(e: ClipboardEvent) {
    // Tab-separated multi-line paste expands into a block of cells, which is
    // what makes pasting a table from Excel or a CSV work.
    const text = e.clipboardData?.getData('text');
    if (!text || !text.includes('\n') && !text.includes('\t')) return;
    e.preventDefault();
    const lines = text.replace(/\r/g, '').split('\n').filter((l) => l.length > 0);
    const { row: startRow, col: startCol } = selection;
    lines.forEach((line, r) => {
      line.split('\t').forEach((value, c) => {
        const row = startRow + r;
        const col = startCol + c;
        if (row >= ROWS || col >= COLS) return;
        const v = value.trim();
        if (v.startsWith('=')) {
          onchange(cellKey(row, col), v.slice(1), v);
        } else {
          onchange(cellKey(row, col), v, undefined);
        }
      });
    });
  }

  const activeRef = $derived(
    editing
      ? ''
      : `${colName(selection.col)}${selection.row + 1}`,
  );

  /** Formula-bar text: the formula if present, else the displayed value. */
  const formulaBar = $derived.by(() => {
    if (editing) return draft;
    const cell = computed[cellKey(selection.row, selection.col)];
    if (!cell) return '';
    return cell.f ?? cell.v ?? '';
  });
</script>

<div class="sheet">
  <div class="bar">
    <div class="ref mono">{activeRef}</div>
    <input
      class="fx"
      value={formulaBar}
      oninput={(e) => (draft = (e.currentTarget as HTMLInputElement).value)}
      onkeydown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          commit(true);
        }
      }}
      placeholder={t('sheet.formulaPlaceholder')}
    />
  </div>

  <div
    class="grid"
    bind:this={gridEl}
    role="grid"
    tabindex="0"
    onpaste={onPaste}
    onkeydown={(e) => {
      // Typing overwrites the selected cell, matching spreadsheet convention.
      if (editing) return;
      const target = e.target as HTMLElement;
      if (target.tagName === 'INPUT') return;
      if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) {
        e.preventDefault();
        draft = e.key;
        startEdit(selection.row, selection.col);
      } else if (e.key === 'Enter') {
        e.preventDefault();
        startEdit(selection.row, selection.col);
      } else if (e.key === 'Delete' || e.key === 'Backspace') {
        e.preventDefault();
        onchange(cellKey(selection.row, selection.col), '', undefined);
      } else if (e.key.startsWith('Arrow')) {
        e.preventDefault();
        const next = { ...selection };
        if (e.key === 'ArrowUp') next.row = Math.max(0, selection.row - 1);
        if (e.key === 'ArrowDown') next.row = Math.min(ROWS - 1, selection.row + 1);
        if (e.key === 'ArrowLeft') next.col = Math.max(0, selection.col - 1);
        if (e.key === 'ArrowRight') next.col = Math.min(COLS - 1, selection.col + 1);
        selection = next;
      }
    }}
  >
    <div class="corner"></div>
    {#each Array(COLS) as _, c (c)}
      <div class="colhead" class:sel={selection.col === c}>{colName(c)}</div>
    {/each}

    {#each Array(ROWS) as _, r (r)}
      <div class="rowhead" class:sel={selection.row === r}>{r + 1}</div>
      {#each Array(COLS) as _, c (c)}
        {@const key = cellKey(r, c)}
        {@const cell = computed[key]}
        {@const isEditing = editing?.row === r && editing?.col === c}
        {@const isSelected = selection.row === r && selection.col === c}
        <div
          class="cell"
          class:sel={isSelected}
          class:num={isNumericText(cell?.v)}
          class:err={!!cell?.error}
          data-cell="{r},{c}"
          onmousedown={() => (selection = { row: r, col: c })}
          ondblclick={() => startEdit(r, c)}
          role="gridcell"
          aria-selected={isSelected}
          tabindex="-1"
        >
          {#if isEditing}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              bind:this={inputEl}
              class="cell-input"
              bind:value={draft}
              onblur={() => commit(true)}
              onkeydown={(e) => onKeydown(e, r, c)}
              autocomplete="off"
              spellcheck="false"
            />
          {:else}
            <span class="cell-text">{cell?.error ? cell.v : (cell?.v ?? '')}</span>
          {/if}
        </div>
      {/each}
    {/each}
  </div>
</div>

<style>
  .sheet {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--surface);
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 0;
    border-bottom: 1px solid var(--border);
    background: var(--sunken);
    flex-shrink: 0;
  }

  .ref {
    width: 82px;
    flex-shrink: 0;
    padding: 6px 10px;
    text-align: center;
    font-size: 11.5px;
    color: var(--text-secondary);
    border-right: 1px solid var(--border);
  }

  .fx {
    flex: 1;
    border: none;
    border-radius: 0;
    background: transparent;
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .fx:focus {
    box-shadow: none;
    border-color: transparent;
  }

  .grid {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: grid;
    /* Column track is fixed; row height matches so headers line up. */
    grid-template-columns: 42px repeat(26, 116px);
    grid-auto-rows: 25px;
    align-content: start;
    outline: none;
  }

  .corner,
  .colhead,
  .rowhead {
    position: sticky;
    background: var(--surface-2);
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    font-size: 11.5px;
    color: var(--text-secondary);
    font-weight: 600;
    display: grid;
    place-items: center;
    user-select: none;
  }

  .corner {
    top: 0;
    left: 0;
    z-index: 3;
  }

  .colhead {
    top: 0;
    z-index: 2;
  }

  .rowhead {
    left: 0;
    z-index: 1;
  }

  /* The selected headers are a thumb on the band, not an accent state. */
  .colhead.sel,
  .rowhead.sel {
    background: var(--sunken);
    color: var(--text);
  }

  .cell {
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    padding: 0 6px;
    display: flex;
    align-items: center;
    font-size: 12px;
    overflow: hidden;
    background: var(--surface);
    cursor: cell;
  }

  .cell:hover {
    background: var(--stripe-hover);
  }

  .cell.sel {
    box-shadow: inset 0 0 0 2px var(--ink);
  }

  .cell.err {
    color: var(--danger);
    background: var(--danger-soft);
  }

  .cell.num {
    justify-content: flex-end;
    font-variant-numeric: tabular-nums;
  }

  .cell-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .cell-input {
    border: none;
    padding: 0;
    height: 100%;
    background: var(--input);
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .cell-input:focus {
    outline: none;
    box-shadow: none;
  }
</style>
