<script lang="ts">
  import type { TreeNode } from '$lib/types';
  import { KIND_META } from '$lib/tree';
  import { layoutCaseGraph, type GraphNode } from '$lib/caseGraph';
  import { t } from '$lib/i18n/index.svelte';
  import { openNode } from '$lib/stores.svelte';
  import { openNodeMenu } from '$lib/artifactMenu';
  import { isContextMenuKey } from '$lib/contextMenu.svelte';

  /**
   * A case's connections as a tree on an endless grid: the case, a group per
   * kind of thing filed under it, and each item. Drag (or scroll) to pan,
   * Ctrl+wheel to zoom about the pointer; arrows, +/− and 0 from the keyboard.
   * Click an item to open it, right-click for its menu.
   */

  let { caseNode, nodes }: { caseNode: TreeNode; nodes: TreeNode[] } = $props();

  const graph = $derived(layoutCaseGraph(caseNode, nodes, (k) => t(`kinds.${k}`)));

  /** Dot spacing of the grid, in graph units. */
  const GRID = 22;
  const MIN_K = 0.3;
  const MAX_K = 2.5;
  const uid = `cg-${Math.random().toString(36).slice(2, 8)}`;

  let svg = $state<SVGSVGElement | null>(null);
  let width = $state(0);
  let height = $state(0);
  let tx = $state(0);
  let ty = $state(0);
  let k = $state(1);
  let hovered = $state<string | null>(null);
  let panning = $state(false);

  /** Frames the whole tree, never enlarging past 1:1. */
  function fit(): void {
    if (!width || !height) return;
    const pad = 24;
    const b = graph.bounds;
    k = Math.max(MIN_K, Math.min(1, (width - pad * 2) / b.w, (height - pad * 2) / b.h));
    tx = (width - b.w * k) / 2 - b.x * k;
    ty = (height - b.h * k) / 2 - b.y * k;
  }

  // Re-frame when the case changes or the box first gets a size; not on every
  // data refresh, so a user's pan survives a background reload.
  let framedFor = '';
  $effect(() => {
    const key = `${caseNode.id}:${width > 0 && height > 0}`;
    if (key === framedFor || !width || !height) return;
    framedFor = key;
    fit();
  });

  function zoomAt(px: number, py: number, factor: number): void {
    const next = Math.max(MIN_K, Math.min(MAX_K, k * factor));
    tx = px - (px - tx) * (next / k);
    ty = py - (py - ty) * (next / k);
    k = next;
  }

  // Wheel needs `passive: false` to stop the page scrolling, which an
  // `onwheel` attribute cannot request.
  $effect(() => {
    const el = svg;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      if (e.ctrlKey || e.metaKey) {
        const r = el.getBoundingClientRect();
        zoomAt(e.clientX - r.left, e.clientY - r.top, Math.exp(-e.deltaY * 0.0015));
      } else {
        tx -= e.shiftKey ? e.deltaY : e.deltaX;
        ty -= e.shiftKey ? 0 : e.deltaY;
      }
    };
    el.addEventListener('wheel', onWheel, { passive: false });
    return () => el.removeEventListener('wheel', onWheel);
  });

  function startPan(e: PointerEvent): void {
    if (e.button !== 0 || (e.target as Element).closest('[data-node]')) return;
    panning = true;
    const sx = e.clientX - tx;
    const sy = e.clientY - ty;
    const el = e.currentTarget as SVGSVGElement;
    el.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      tx = ev.clientX - sx;
      ty = ev.clientY - sy;
    };
    const up = () => {
      panning = false;
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerup', up);
      el.removeEventListener('pointercancel', up);
    };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.target !== e.currentTarget) return;
    const step = 48;
    const keys: Record<string, () => void> = {
      ArrowLeft: () => (tx += step),
      ArrowRight: () => (tx -= step),
      ArrowUp: () => (ty += step),
      ArrowDown: () => (ty -= step),
      '+': () => zoomAt(width / 2, height / 2, 1.2),
      '=': () => zoomAt(width / 2, height / 2, 1.2),
      '-': () => zoomAt(width / 2, height / 2, 1 / 1.2),
      '0': fit,
    };
    const act = keys[e.key];
    if (act) {
      e.preventDefault();
      act();
    }
  }

  /** The group an item hangs from, so hovering an item lights its whole branch. */
  function branchOf(id: string | null): Set<string> {
    const lit = new Set<string>();
    if (!id) return lit;
    lit.add(id);
    for (const e of graph.edges) {
      if (e.to === id) lit.add(e.from);
      if (e.from === id) lit.add(e.to);
    }
    // An item's group also lights the edge back to the case.
    for (const e of graph.edges) if (lit.has(e.to) && e.from === caseNode.id) lit.add(e.from);
    return lit;
  }
  const lit = $derived(branchOf(hovered));

  function activate(n: GraphNode): void {
    if (n.node) openNode(n.node);
  }

  /** Items are keyboard targets too: Enter/Space opens, Shift+F10 the menu. */
  function onNodeKey(e: KeyboardEvent, n: GraphNode): void {
    if (!n.node) return;
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      activate(n);
    } else if (isContextMenuKey(e)) {
      e.preventDefault();
      openNodeMenu(e.currentTarget as HTMLElement, n.node);
    }
  }

  function glyph(n: GraphNode): string {
    if (n.kind === 'group') return KIND_META[n.groupOf!].icon;
    return KIND_META[n.kind as keyof typeof KIND_META].icon;
  }
</script>

<div class="graph" bind:clientWidth={width} bind:clientHeight={height}>
  <!-- A focusable, pannable surface is what role="application" is for; the
       checker counts that role as non-interactive, so these two are silenced
       deliberately. Its items are real buttons, reachable with Tab. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <svg
    bind:this={svg}
    class:panning
    role="application"
    aria-roledescription="graph"
    tabindex="0"
    aria-label={t('graph.label')}
    onpointerdown={startPan}
    ondblclick={(e) => !(e.target as Element).closest('[data-node]') && fit()}
    onkeydown={onKeydown}
  >
    <defs>
      <!-- The grid tiles forever: its origin follows the pan, its pitch the zoom. -->
      <pattern
        id="{uid}-dots"
        width={GRID * k}
        height={GRID * k}
        x={tx}
        y={ty}
        patternUnits="userSpaceOnUse"
      >
        <circle class="dot" cx={1} cy={1} r={Math.max(0.6, 0.9 * k)} />
      </pattern>
    </defs>
    <rect class="grid" width="100%" height="100%" fill="url(#{uid}-dots)" />

    <g transform="translate({tx} {ty}) scale({k})">
      {#each graph.edges as e (e.from + '>' + e.to)}
        <path class="edge" class:lit={lit.has(e.from) && lit.has(e.to)} d={e.d} />
      {/each}

      {#each graph.nodes as n (n.id)}
        <g
          data-node
          data-artifact={n.id}
          class="node {n.kind}"
          class:lit={lit.has(n.id)}
          class:openable={!!n.node}
          transform="translate({n.x} {n.y})"
          role="button"
          tabindex={n.node ? 0 : -1}
          aria-label={n.count != null ? `${n.title}: ${n.count}` : n.title}
          aria-disabled={!n.node}
          onpointerenter={() => (hovered = n.id)}
          onpointerleave={() => (hovered = hovered === n.id ? null : hovered)}
          onfocus={() => (hovered = n.id)}
          onblur={() => (hovered = hovered === n.id ? null : hovered)}
          onclick={() => activate(n)}
          onkeydown={(e) => onNodeKey(e, n)}
          oncontextmenu={(e) => n.node && openNodeMenu(e, n.node)}
        >
          <title>{n.title}</title>
          <rect class="box" width={n.w} height={n.h} rx="7" />
          <text class="glyph" x="11" y={n.h / 2}>{glyph(n)}</text>
          <text class="label" x="26" y={n.h / 2}>{n.label}</text>
          {#if n.count != null}
            <text class="count" x={n.w - 10} y={n.h / 2}>{n.count}</text>
          {/if}
        </g>
      {/each}
    </g>
  </svg>

  <div class="controls">
    <button class="c" onclick={() => zoomAt(width / 2, height / 2, 1 / 1.2)} title={t('graph.zoomOut')} aria-label={t('graph.zoomOut')}>−</button>
    <button class="c" onclick={() => zoomAt(width / 2, height / 2, 1.2)} title={t('graph.zoomIn')} aria-label={t('graph.zoomIn')}>+</button>
    <button class="c" onclick={fit} title={t('graph.fit')} aria-label={t('graph.fit')}>⤢</button>
  </div>
</div>

<style>
  .graph {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
  }

  svg {
    display: block;
    width: 100%;
    height: 100%;
    cursor: grab;
    touch-action: none;
    user-select: none;
    outline: none;
  }
  svg.panning {
    cursor: grabbing;
  }
  svg:focus-visible {
    box-shadow: inset 0 0 0 2px var(--ink);
  }

  .grid {
    pointer-events: all;
  }
  .dot {
    fill: var(--border-strong);
    opacity: 0.45;
  }

  .edge {
    fill: none;
    stroke: var(--border-strong);
    stroke-width: 1.2;
    opacity: 0.55;
    transition:
      opacity var(--dur-fast) var(--ease),
      stroke var(--dur-fast) var(--ease);
  }
  .edge.lit {
    stroke: var(--ink);
    opacity: 1;
  }

  .node .box {
    fill: var(--surface);
    stroke: var(--border);
    transition:
      stroke var(--dur-fast) var(--ease),
      fill var(--dur-fast) var(--ease);
  }
  .node.group .box {
    fill: var(--sunken);
    stroke: transparent;
  }
  /* The case is the root: ink, like the primary action. */
  .node.case .box {
    fill: var(--ink);
    stroke: transparent;
  }
  .node.openable {
    cursor: pointer;
  }
  .node.lit .box {
    stroke: var(--ink);
  }
  .node {
    outline: none;
  }
  .node:focus-visible .box {
    stroke: var(--ink);
    stroke-width: 2;
  }

  text {
    dominant-baseline: central;
    font-family: var(--font);
    pointer-events: none;
  }
  .label {
    font-size: 11.5px;
    fill: var(--text);
  }
  .glyph {
    font-size: 9.5px;
    fill: var(--text-tertiary);
    text-anchor: middle;
  }
  .count {
    font-size: 11px;
    fill: var(--text-tertiary);
    text-anchor: end;
  }
  .node.group .label {
    font-weight: 600;
  }
  .node.case .label,
  .node.case .glyph {
    fill: var(--ink-ink);
    font-weight: 600;
  }

  .controls {
    position: absolute;
    right: 8px;
    bottom: 8px;
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: var(--radius-sm);
    background: var(--overlay);
    box-shadow: var(--shadow-sm);
  }
  .c {
    width: 24px;
    height: 24px;
    border: none;
    border-radius: var(--radius-xs);
    background: transparent;
    color: var(--text-secondary);
    font-size: 13px;
    line-height: 1;
    cursor: pointer;
  }
  .c:hover {
    background: var(--sunken);
    color: var(--text);
  }
</style>
