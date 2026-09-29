<script lang="ts">
  import '../app.css';
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import ChatPanel from '$lib/chat/ChatPanel.svelte';
  import WorkspaceHeader from '$lib/components/WorkspaceHeader.svelte';
  import Rail from '$lib/components/Rail.svelte';
  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import ContextMenu from '$lib/components/ContextMenu.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import Toast from '$lib/components/Toast.svelte';
  import { watchSystemTheme } from '$lib/theme';
  import TreeGrid from '$lib/views/TreeGrid.svelte';
  import TableView from '$lib/views/TableView.svelte';
  import CalendarView from '$lib/views/CalendarView.svelte';
  import SheetView from '$lib/views/SheetView.svelte';
  import DocumentViewer from '$lib/views/DocumentViewer.svelte';
  import CaseDetail from '$lib/views/CaseDetail.svelte';
  import SearchView from '$lib/views/SearchView.svelte';
  import People from '$lib/views/People.svelte';
  import Settings from '$lib/views/Settings.svelte';
  import {
    chatCollapsed,
    chatPanelWidth,
    errorText,
    goBack,
    goForward,
    indexing,
    notify,
    pane,
    paletteOpen,
    refreshAll,
    refreshPeople,
    scopedCaseId,
    setChatPanelWidth,
    toggleChat,
    treeNodes,
  } from '$lib/stores.svelte';
  import type { IndexProgress } from '$lib/types';
  import { proposeLayout, type Layout, type PanelId } from '$lib/layout';
  import { drag, panelLayout, registerDropResolver } from '$lib/panels.svelte';
  import { t } from '$lib/i18n/index.svelte';
  import { mentionIndexFor, mentionLookupFor, nodeIndex, nodeKeyMap } from '$lib/tree';

  let { children } = $props();

  /** Must match the rail's thickness in Rail.svelte. */
  const RAIL = 44;

  let resizing = $state(false);
  let shell = $state<HTMLDivElement | null>(null);
  let content = $state<HTMLDivElement | null>(null);
  let chatEl = $state<HTMLDivElement | null>(null);
  let width = $state(chatPanelWidth.value);

  $effect(() => {
    width = chatPanelWidth.value;
  });

  const layout = $derived(panelLayout.value);
  const railHorizontal = $derived(layout.rail === 'top' || layout.rail === 'bottom');

  onMount(() =>
    registerDropResolver((id: PanelId, x: number, y: number) => {
      if (!shell || !content) return panelLayout.value;
      return proposeLayout(
        panelLayout.value,
        id,
        shell.getBoundingClientRect(),
        content.getBoundingClientRect(),
        x,
        y,
      );
    }),
  );

  /**
   * Preview of where a dragged panel will land, in shell coordinates: a band
   * along the target edge for the rail, a column on the target side for the
   * assistant or workspace.
   */
  const preview = $derived.by((): { left: number; top: number; width: number; height: number } | null => {
    const target: Layout | null = drag.target;
    if (!drag.active || !target || !shell || !content) return null;
    const s = shell.getBoundingClientRect();
    const c = content.getBoundingClientRect();
    if (drag.id === 'rail') {
      if (target.rail === 'left') return { left: 0, top: 0, width: RAIL, height: s.height };
      if (target.rail === 'right') return { left: s.width - RAIL, top: 0, width: RAIL, height: s.height };
      if (target.rail === 'top') return { left: 0, top: 0, width: s.width, height: RAIL };
      return { left: 0, top: s.height - RAIL, width: s.width, height: RAIL };
    }
    // The dragged panel's own width, so the preview reads as "it goes here".
    const w = drag.id === 'chat' && !chatCollapsed.value ? width : Math.max(c.width - width, c.width / 2);
    const landsLeft = (drag.id === 'chat') === (target.chat === 'left');
    return {
      left: (landsLeft ? c.left : c.right - w) - s.left,
      top: c.top - s.top,
      width: w,
      height: c.height,
    };
  });

  onMount(() => {
    // Seed the shared reference data every view depends on.
    void (async () => {
      try {
        await refreshAll();
        await refreshPeople();
      } catch (e) {
        notify('danger', t('app.err.load', { error: errorText(e) }));
      }
    })();

    // Indexing runs in Rust and reports progress over events; the tree view
    // reads the store for its banner and per-row status.
    const unlisten = listen<IndexProgress>('index:progress', (e) => {
      const p = e.payload;
      if (p.status === 'Indexing') {
        indexing.update((map) => ({ ...map, [p.documentId]: p }));
      } else {
        indexing.update((map) => {
          const next = { ...map };
          delete next[p.documentId];
          return next;
        });
        // A finished or failed index changes what the tree can show, so
        // refresh it once rather than on every chunk.
        if (treeNodes.value.some((n) => n.documentId === p.documentId)) void refreshAll();
      }
    });

    return () => {
      void unlisten.then((fn) => fn());
    };
  });

  // The theme is applied synchronously by lib/theme at import time, so there is
  // nothing to synchronise on mount.
  onMount(() => watchSystemTheme());

  function startResize(e: PointerEvent): void {
    resizing = true;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    window.addEventListener('pointermove', onResize);
    window.addEventListener('pointerup', stopResize, { once: true });
  }

  /** The assistant resizes from the edge that faces the workspace. */
  function onResize(e: PointerEvent): void {
    if (!chatEl) return;
    const r = chatEl.getBoundingClientRect();
    setChatPanelWidth(layout.chat === 'left' ? e.clientX - r.left : r.right - e.clientX);
  }

  function stopResize(): void {
    resizing = false;
    window.removeEventListener('pointermove', onResize);
  }

  function onKeydown(e: KeyboardEvent): void {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      paletteOpen.set(!paletteOpen.value);
    } else if (mod && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'j') {
      e.preventDefault();
      toggleChat();
    } else if (e.altKey && e.key === 'ArrowLeft') {
      e.preventDefault();
      goBack();
    } else if (e.altKey && e.key === 'ArrowRight') {
      e.preventDefault();
      goForward();
    }
  }

  // Every per-tree index (the @ lookup, quick find's search index, id and
  // handle maps) costs up to ~150 ms to build on a 50k-node tree. Build them
  // while the app is idle after the tree changes, so no keystroke or click
  // ever pays for it. Reads the tree only and writes nothing, so it cannot
  // loop.
  $effect(() => {
    const tree = treeNodes.value;
    if (tree.length === 0) return;
    const idle = window.requestIdleCallback ?? ((cb: () => void) => setTimeout(cb, 200));
    const cancel = window.cancelIdleCallback ?? clearTimeout;
    const steps = [mentionLookupFor, mentionIndexFor, nodeIndex, nodeKeyMap];
    let handle: number;
    // One index per idle slot, so warming never becomes a long task itself.
    const next = () => {
      const build = steps.shift();
      if (!build) return;
      build(tree);
      handle = idle(next) as number;
    };
    handle = idle(next) as number;
    return () => cancel(handle);
  });

  // The roster view reads its case scope from a store, so keep it in step with
  // whichever case the workspace is currently showing.
  $effect(() => {
    const p = pane.value;
    scopedCaseId.set(p.kind === 'people' ? p.caseId : null);
  });
</script>

<svelte:window onkeydown={onKeydown} />

<!--
  Panels are placed with CSS `order`, never by moving DOM nodes: a layout
  change must not remount the assistant (its draft and scroll would be lost)
  or the workspace view.
-->
<div
  class="shell rail-{layout.rail} chat-{layout.chat}"
  class:resizing
  class:dragging={drag.active}
  bind:this={shell}
>
  <div class="rail-slot" class:lifted={drag.active && drag.id === 'rail'}>
    <Rail horizontal={railHorizontal} />
  </div>

  <div class="content" bind:this={content}>
    <!-- Hidden rather than unmounted, so a collapsed chat keeps its draft and
         scroll position. -->
    <div
      class="chat-col"
      class:lifted={drag.active && drag.id === 'chat'}
      style:width="{width}px"
      hidden={chatCollapsed.value}
      bind:this={chatEl}
    >
      <ChatPanel />
    </div>

    {#if !chatCollapsed.value}
      <!-- Doubles as the chat's divider, on whichever side faces the workspace. -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="resizer"
        onpointerdown={startResize}
        role="separator"
        aria-orientation="vertical"
        aria-label={t('app.resize')}
        title={t('app.resizeHint')}
      ></div>
    {/if}

    <div class="workspace" class:lifted={drag.active && drag.id === 'workspace'}>
      <WorkspaceHeader />
      <div class="stage">
        {#if pane.value.kind === 'grid'}
          <TreeGrid />
        {:else if pane.value.kind === 'table'}
          <TableView caseId={pane.value.caseId} />
        {:else if pane.value.kind === 'search'}
          <SearchView caseId={pane.value.caseId} />
        {:else if pane.value.kind === 'calendar'}
          <CalendarView caseId={pane.value.caseId} />
        {:else if pane.value.kind === 'sheet'}
          <SheetView sheetId={pane.value.sheetId} />
        {:else if pane.value.kind === 'document'}
          <DocumentViewer documentId={pane.value.documentId} />
        {:else if pane.value.kind === 'case'}
          <CaseDetail caseId={pane.value.caseId} />
        {:else if pane.value.kind === 'people'}
          <People />
        {:else}
          <Settings />
        {/if}
        {@render children?.()}
      </div>
    </div>
  </div>

  {#if preview}
    <div
      class="drop-preview"
      style:left="{preview.left}px"
      style:top="{preview.top}px"
      style:width="{preview.width}px"
      style:height="{preview.height}px"
      aria-hidden="true"
    ></div>
  {/if}
  {#if drag.active && drag.id}
    <div class="drag-ghost" style:left="{drag.x + 14}px" style:top="{drag.y + 10}px" aria-hidden="true">
      {t(`panel.${drag.id}`)}
    </div>
  {/if}

  <Toast />
  <CommandPalette />
  <ContextMenu />
  <ConfirmDialog />
</div>

<style>
  /* SvelteKit mounts into a `display: contents` wrapper, so the shell sizes
     itself against the body directly and takes the whole window; the
     children manage their own internal scrolling. */
  .shell {
    position: relative;
    display: flex;
    height: 100%;
    width: 100%;
    min-height: 0;
    overflow: hidden;
  }

  /* --- Rail docking ------------------------------------------------------ */
  .shell.rail-top,
  .shell.rail-bottom {
    flex-direction: column;
  }
  .rail-slot {
    display: flex;
    flex-shrink: 0;
    min-height: 0;
    min-width: 0;
  }
  .shell.rail-right .rail-slot,
  .shell.rail-bottom .rail-slot {
    order: 2;
  }
  /* A bar across the top or bottom sits against --bg headers, so it needs
     the hairline the side rail gets from its tone change. */
  .shell.rail-top .rail-slot {
    border-bottom: 1px solid var(--border);
  }
  .shell.rail-bottom .rail-slot {
    border-top: 1px solid var(--border);
  }

  /* --- Assistant side ---------------------------------------------------- */
  .content {
    order: 1;
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
  }
  .chat-col {
    order: 0;
  }
  .resizer {
    order: 1;
  }
  .workspace {
    order: 2;
  }
  .shell.chat-right .chat-col {
    order: 2;
  }
  .shell.chat-right .workspace {
    order: 0;
  }

  /* The width is user-draggable, so suppress text selection while dragging. */
  .shell.resizing {
    cursor: col-resize;
    user-select: none;
  }

  /* While a panel is being moved, nothing underneath should react. */
  .shell.dragging {
    cursor: grabbing;
    user-select: none;
  }
  .shell.dragging .content,
  .shell.dragging .rail-slot {
    pointer-events: none;
  }

  .lifted {
    opacity: 0.45;
    transition: opacity var(--dur-fast) var(--ease);
  }

  /* Where the panel will land: an ink outline, not accent — it is an
     affordance (DESIGN §5.4). */
  .drop-preview {
    position: absolute;
    z-index: 30;
    pointer-events: none;
    border: 2px solid var(--ink);
    border-radius: var(--radius-sm);
    background: var(--sunken);
    opacity: 0.55;
    transition:
      left var(--dur-fast) var(--ease),
      top var(--dur-fast) var(--ease),
      width var(--dur-fast) var(--ease),
      height var(--dur-fast) var(--ease);
  }

  /* Floats with the pointer: shadow, no border (§9). */
  .drag-ghost {
    position: fixed;
    z-index: 40;
    padding: 4px 10px;
    border-radius: var(--radius-sm);
    background: var(--overlay);
    box-shadow: var(--shadow-md);
    color: var(--text);
    font-size: 12.5px;
    font-weight: 600;
    pointer-events: none;
    white-space: nowrap;
  }

  .chat-col[hidden] {
    display: none;
  }
  .chat-col {
    flex-shrink: 0;
    min-width: 300px;
    max-width: 720px;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .resizer {
    position: relative;
    width: 7px;
    margin: 0 -3px;
    cursor: col-resize;
    flex-shrink: 0;
    z-index: 10;
  }
  .resizer::before {
    content: '';
    position: absolute;
    inset: 0 3px;
    background: var(--border);
    transition:
      background var(--dur-fast) var(--ease),
      inset var(--dur-fast) var(--ease);
  }
  .resizer:hover::before,
  .shell.resizing .resizer::before {
    inset: 0 2px;
    background: var(--border-strong);
  }

  .workspace {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg);
  }

  .stage {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  /* Named for view transitions (see app.css): the stage cross-fades between
     views; the three panels glide when the layout changes. */
  .stage {
    view-transition-name: stage;
  }
  :global(:root[data-vt='layout']) .rail-slot {
    view-transition-name: rail;
  }
  :global(:root[data-vt='layout']) .chat-col {
    view-transition-name: assistant;
  }
  :global(:root[data-vt='layout']) .workspace {
    view-transition-name: workspace;
  }
</style>
