<script lang="ts">
  import { openNodeMenu } from '$lib/artifactMenu';
  import { isContextMenuKey } from '$lib/contextMenu.svelte';
  import { label, t } from '$lib/i18n/index.svelte';
  import type { TreeNode, TreeNodeKind } from '$lib/types';
  import { KIND_META, KIND_ORDER, countByKind, filterTree, visibleRows } from '$lib/tree';
  import {
    askAbout,
    chatCollapsed,
    collapsed,
    guard,
    indexing,
    openNode,
    refreshTree,
    toggleChat,
    toggleCollapsed,
    treeNodes,
  } from '$lib/stores.svelte';
  import { formatBytes, formatRelative } from '$lib/api';
  import StatusBadge from '$lib/components/StatusBadge.svelte';

  /**
   * The whole case tree as a virtualised grid.
   *
   * Only the rows in view are in the DOM, and the scrollable height is allowed
   * to grow past the end of the data so the surface keeps scrolling rather than
   * dead-ending at the last case.
   */

  /** Must match the `.row` height in the stylesheet below. */
  const ROW_H = 30;
  const OVERSCAN = 8;
  /** Phantom rows kept below the data so there is always somewhere to scroll. */
  const MIN_FILLER = 80;
  const MAX_FILLER = 4000;

  let query = $state('');
  let hiddenKinds = $state<Set<TreeNodeKind>>(new Set());
  let scroller = $state<HTMLDivElement | null>(null);
  let scrollTop = $state(0);
  let viewportH = $state(600);
  let selected = $state<string | null>(null);
  let refreshing = $state(false);

  const filtered = $derived(filterTree(treeNodes.value, query));
  const rows = $derived(
    visibleRows(
      filtered.filter((n) => hiddenKinds.size === 0 || !hiddenKinds.has(n.kind)),
      collapsed.value,
    ),
  );
  const counts = $derived(countByKind(treeNodes.value));

  /** Ids that have at least one child, so only real parents show a chevron. */
  const parents = $derived(new Set(filtered.map((n) => n.parentId).filter(Boolean) as string[]));

  /**
   * Virtual height. Grows as the user scrolls so the view never bottoms out,
   * but is capped so a stray fast scroll cannot allocate a huge spacer.
   */
  const totalRows = $derived(
    Math.min(
      rows.length + MIN_FILLER,
      Math.max(rows.length, Math.ceil((scrollTop + viewportH) / ROW_H) + MIN_FILLER),
    ),
  );
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW_H) - OVERSCAN));
  const last = $derived(Math.min(rows.length, Math.ceil((scrollTop + viewportH) / ROW_H) + OVERSCAN));
  const windowed = $derived(rows.slice(first, last));

  /** The real data ends here; below it the rows are blank. */
  const hasFiller = $derived(rows.length < totalRows);

  $effect(() => {
    if (!scroller) return;
    const el = scroller;
    const ro = new ResizeObserver(() => {
      viewportH = el.clientHeight;
    });
    ro.observe(el);
    viewportH = el.clientHeight;
    return () => ro.disconnect();
  });

  function onScroll(): void {
    if (scroller) scrollTop = scroller.scrollTop;
  }

  function toggleKind(kind: TreeNodeKind): void {
    hiddenKinds = new Set(hiddenKinds.has(kind) ? [...hiddenKinds].filter((k) => k !== kind) : [...hiddenKinds, kind]);
  }

  function expandAll(): void {
    collapsed.set(new Set());
  }

  function collapseAll(): void {
    collapsed.set(new Set(treeNodes.value.filter((n) => n.parentId === null).map((n) => n.id)));
  }

  function activate(node: TreeNode): void {
    selected = node.id;
    openNode(node);
  }

  /** Drops the node into the composer, revealing the assistant if hidden. */
  function ask(node: TreeNode): void {
    selected = node.id;
    if (chatCollapsed.value) toggleChat();
    askAbout(node.nodeKey);
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const i = rows.findIndex((n) => n.id === selected);
      const next = Math.max(0, Math.min(rows.length - 1, (i < 0 ? 0 : i) + (e.key === 'ArrowDown' ? 1 : -1)));
      const node = rows[next];
      if (!node) return;
      selected = node.id;
      scrollIntoView(next);
      return;
    }
    const node = rows.find((n) => n.id === selected);
    if (!node) return;
    if (isContextMenuKey(e)) {
      e.preventDefault();
      openNodeMenu(scroller?.querySelector<HTMLElement>('.row.sel') ?? scroller!, node);
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      activate(node);
    } else if (e.key === 'a' && !e.ctrlKey && !e.metaKey && !e.altKey) {
      // The row's hover-only "Ask" action, reachable from the keyboard.
      e.preventDefault();
      ask(node);
    } else if (e.key === 'ArrowRight') {
      e.preventDefault();
      if (parents.has(node.id)) collapsed.update((s) => new Set([...s].filter((id) => id !== node.id)));
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault();
      if (parents.has(node.id) && !collapsed.value.has(node.id)) {
        toggleCollapsed(node.id);
      } else if (node.parentId) {
        const i = rows.findIndex((n) => n.id === node.parentId);
        if (i >= 0) {
          selected = node.parentId;
          scrollIntoView(i);
        }
      }
    }
  }

  /** Keeps the keyboard cursor inside the window, not just the scroll box. */
  function scrollIntoView(index: number): void {
    if (!scroller) return;
    const top = index * ROW_H;
    const head = scroller.scrollTop + 30;
    if (top < head) scroller.scrollTop = Math.max(0, top - 30);
    else if (top + ROW_H > scroller.scrollTop + scroller.clientHeight - 4) {
      scroller.scrollTop = top + ROW_H - scroller.clientHeight + 4;
    }
  }

  async function reload(): Promise<void> {
    refreshing = true;
    await refreshTree();
    refreshing = false;
  }

  /**
   * A child node's mention key is `case-ref/leaf`, so the case reference is the
   * leading segment. A case node is its own reference; an unfiled item has none.
   */
  function caseRefOf(node: TreeNode): string {
    if (node.kind === 'case') return node.nodeKey;
    const slash = node.nodeKey.indexOf('/');
    return slash > 0 ? node.nodeKey.slice(0, slash) : '—';
  }
</script>

<div class="wrap">
  <div class="tools">
    <div class="search">
      <span class="mag">⌕</span>
      <input
        type="search"
        placeholder={t('tree.filter')}
        bind:value={query}
        spellcheck="false"
      />
    </div>

    <div class="chips">
      {#each KIND_ORDER as k (k)}
        <button
          class="chip"
          class:off={hiddenKinds.has(k)}
          class:zero={counts[k] === 0}
          onclick={() => toggleKind(k)}
          title={t(`tree.count.${k}`, { n: counts[k] })}
        >
          <span class="c-icon">{KIND_META[k].icon}</span>
          {t(`kinds.${k}`)}
          <span class="c-n">{counts[k]}</span>
        </button>
      {/each}
    </div>

    <div class="spacer"></div>
    <button class="btn btn-ghost btn-sm" onclick={expandAll}>{t('tree.expand')}</button>
    <button class="btn btn-ghost btn-sm" onclick={collapseAll}>{t('tree.collapse')}</button>
    <button class="btn btn-ghost btn-sm" onclick={() => void reload()} disabled={refreshing} title={t('tree.refresh')}>
      {#if refreshing}<span class="spinner"></span>{:else}↻{/if}
    </button>
  </div>

  {#if indexing.value && Object.keys(indexing.value).length > 0}
    <div class="indexing">
      <span class="spinner"></span>
      Indexing {Object.keys(indexing.value).length} document{Object.keys(indexing.value).length === 1 ? '' : 's'}…
    </div>
  {/if}

  <div
    class="body"
    bind:this={scroller}
    onscroll={onScroll}
    onkeydown={onKeydown}
    tabindex="0"
    role="tree"
    aria-label={t('view.tree')}
  >
    <div class="head">
      <div class="h gutter"></div>
      <div class="h name">{t('common.name')}</div>
      <div class="h">{t('tree.kind')}</div>
      <div class="h">{t('common.case')}</div>
      <div class="h">{t('common.status')}</div>
      <div class="h">{t('tree.detail')}</div>
      <div class="h right">{t('common.updated')}</div>
    </div>

    <div class="rows" style="height: {totalRows * ROW_H}px">
      <div class="win" style="transform: translateY({first * ROW_H}px)">
        {#each windowed as node (node.id)}
          {@const isCollapsed = collapsed.value.has(node.id)}
          {@const hasKids = parents.has(node.id)}
          <div
            class="row"
            data-artifact={node.id}
            class:sel={selected === node.id}
            class:case-row={node.kind === 'case'}
            role="treeitem"
            aria-expanded={hasKids ? !isCollapsed : undefined}
            aria-level={node.depth + 1}
            aria-selected={selected === node.id}
            tabindex="-1"
            onmousedown={() => (selected = node.id)}
            ondblclick={() => activate(node)}
            oncontextmenu={(e) => {
              selected = node.id;
              openNodeMenu(e, node);
            }}
          >
            <div class="gutter" style="width: {8 + node.depth * 15}px">
              {#if hasKids}
                <button
                  class="twisty"
                  class:closed={isCollapsed}
                  onclick={(e) => {
                    e.stopPropagation();
                    toggleCollapsed(node.id);
                  }}
                  title={isCollapsed ? t('tree.expand') : t('tree.collapse')}
                  tabindex="-1"
                >
                  ▾
                </button>
              {/if}
            </div>

            <div class="name" title={node.nodeKey}>
              <span class="kind-icon" data-kind={node.kind} style={node.color ? `color:${node.color}` : ''}>
                {KIND_META[node.kind].icon}
              </span>
              <span class="truncate label">{node.label}</span>
              <span class="row-actions">
                <button
                  class="row-action"
                  tabindex="-1"
                  onmousedown={(e) => e.stopPropagation()}
                  onclick={() => activate(node)}
                  title={t('row.openHint')}>{t('common.open')}</button
                >
                <button
                  class="row-action"
                  tabindex="-1"
                  onmousedown={(e) => e.stopPropagation()}
                  onclick={() => ask(node)}
                  title={t('row.askHint')}>{t('common.ask')}</button
                >
              </span>
            </div>

            <div class="kind" data-kind={node.kind}>{t(`kind.${node.kind}`)}</div>

            <div class="case-ref mono truncate">{node.caseId ? caseRefOf(node) : '—'}</div>

            <div class="status">
              {#if node.status}
                {#if node.kind === 'event'}
                  <span class="badge badge-muted">{label(node.status)}</span>
                {:else if node.kind === 'sheet'}
                  <span class="badge badge-muted">{label(node.status)}</span>
                {:else}
                  <StatusBadge status={node.status} />
                {/if}
              {/if}
            </div>

            <div class="detail truncate faint">
              {node.detail ?? '—'}{node.sizeBytes != null && node.sizeBytes > 0 ? ` · ${formatBytes(node.sizeBytes)}` : ''}
            </div>

            <div class="right faint">{formatRelative(node.updatedAt)}</div>
          </div>
        {/each}
      </div>
    </div>

    {#if hasFiller}
      <div class="tail faint">{t('tree.end', { n: rows.length })}</div>
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

  .tools {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .search {
    position: relative;
    width: 268px;
  }
  .mag {
    position: absolute;
    left: 8px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text-tertiary);
    font-size: 13px;
    pointer-events: none;
  }
  .search input {
    width: 100%;
    padding: 5px 8px 5px 24px;
    font-size: 12.5px;
  }

  .chips {
    display: flex;
    gap: 4px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 8px;
    border-radius: var(--radius-full);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-secondary);
    font-size: 11px;
    cursor: pointer;
    transition:
      background-color var(--dur-fast) var(--ease),
      border-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .chip:hover {
    border-color: var(--border-strong);
    color: var(--text);
  }
  .chip.off {
    opacity: 0.42;
    text-decoration: line-through;
  }
  .chip.zero {
    opacity: 0.4;
  }
  .c-icon {
    font-size: 9px;
  }
  .c-n {
    font-variant-numeric: tabular-nums;
    color: var(--text-tertiary);
    font-size: 10px;
  }

  /* A background task, so it reads as one: muted, flat, no accent. */
  .indexing {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 5px 12px;
    background: var(--sunken);
    border-bottom: 1px solid var(--border);
    font-size: 11.5px;
    color: var(--text-secondary);
    flex-shrink: 0;
  }

  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    position: relative;
    outline: none;
    background: var(--surface);
  }
  .body:focus-visible {
    box-shadow: inset 0 0 0 2px var(--ink);
  }

  /* Sentence-case column headers on the inner band. A tracked-out uppercase
     micro-label is the A13 tell: it adds nothing a 12px weight-600 header
     does not already say. */
  .head {
    position: sticky;
    top: 0;
    z-index: 3;
    display: grid;
    grid-template-columns:
      8px minmax(220px, 2.2fr) 88px 120px 128px minmax(120px, 1fr) 100px;
    align-items: center;
    background: var(--surface-2);
    border-bottom: 1px solid var(--border);
    font-size: 11.5px;
    line-height: 1.45;
    font-weight: 600;
    letter-spacing: 0;
    text-transform: none;
    color: var(--text-secondary);
  }
  .h {
    padding: 7px 8px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .h.right {
    text-align: right;
  }

  .rows {
    position: relative;
  }

  .win {
    position: absolute;
    inset: 0 0 auto 0;
    will-change: transform;
  }

  .row {
    display: grid;
    grid-template-columns:
      8px minmax(220px, 2.2fr) 88px 120px 128px minmax(120px, 1fr) 100px;
    align-items: center;
    /* Must stay in lockstep with ROW_H in the script, since the virtualiser
       positions rows by multiplication rather than by measurement. */
    height: 30px;
    border-bottom: 1px solid var(--border);
    cursor: default;
    font-size: 12.5px;
  }
  .row:hover {
    background: var(--stripe-hover);
  }
  .row.sel {
    background: var(--accent-soft);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .row.case-row {
    background: var(--stripe);
    font-weight: 500;
  }
  .row.case-row:hover {
    background: var(--stripe-hover);
  }
  .row.case-row.sel {
    background: var(--accent-soft);
  }

  .gutter {
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: flex-start;
    flex-shrink: 0;
  }
  .twisty {
    width: 15px;
    height: 15px;
    display: grid;
    place-items: center;
    background: transparent;
    border: none;
    color: var(--text-tertiary);
    cursor: pointer;
    font-size: 8px;
    transition:
      transform var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .twisty:hover {
    color: var(--text);
  }
  .twisty.closed {
    transform: rotate(-90deg);
  }

  /* The keyboard cursor reveals row actions just as the pointer does. */
  .row.sel :global(.row-actions) {
    opacity: 1;
  }

  .name {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding-right: 8px;
  }
  .kind-icon {
    font-size: 9px;
    color: var(--text-tertiary);
    flex-shrink: 0;
    width: 11px;
    text-align: center;
  }
  .kind-icon[data-kind='document'] {
    color: var(--accent);
  }
  .kind-icon[data-kind='case'] {
    color: var(--ok);
  }
  .label {
    min-width: 0;
  }

  .kind {
    font-size: 10.5px;
    color: var(--text-secondary);
    padding-right: 8px;
  }
  .kind[data-kind='document'] {
    color: var(--accent-hover);
  }

  .case-ref {
    font-size: 10.5px;
    color: var(--text-tertiary);
    padding-right: 8px;
  }

  .status {
    padding-right: 8px;
  }

  .detail {
    font-size: 11px;
    padding-right: 8px;
  }

  .right {
    font-size: 11px;
    text-align: right;
    padding-right: 10px;
  }

  .tail {
    position: sticky;
    bottom: 0;
    padding: 6px 12px;
    font-size: 10.5px;
    background: linear-gradient(to top, var(--surface), transparent);
    pointer-events: none;
  }
</style>
