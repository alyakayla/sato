<script lang="ts">
  import { t } from '$lib/i18n/index.svelte';
  import type { TreeNode } from '$lib/types';
  import { KIND_META, ancestorsOf } from '$lib/tree';
  import QuickAdd from './QuickAdd.svelte';
  import PanelGrip from './PanelGrip.svelte';
  import {
    canGoBack,
    canGoForward,
    cases,
    goBack,
    goForward,
    openPane,
    pane,
    treeNodes,
  } from '$lib/stores.svelte';

  /**
   * The workspace header: back/forward, a breadcrumb built from the tree path,
   * and page-level actions. Global destinations live in the rail.
   */

  interface Crumb {
    label: string;
    icon?: string;
    onClick: () => void;
    /** True for the last crumb, which represents the current view. */
    last?: boolean;
  }

  const crumbs = $derived.by((): Crumb[] => {
    const p = pane.value;
    const nodes = treeNodes.value;
    const out: Crumb[] = [{ label: t('view.workspace'), onClick: () => openPane({ kind: 'grid' }) }];

    if (p.kind === 'grid') return out;

    if (p.kind === 'settings') {
      out.push({ label: t('view.settings'), onClick: () => openPane({ kind: 'settings' }) });
      return out;
    }

    if (p.kind === 'document') {
      const node = nodes.find((n) => n.documentId === p.documentId);
      pushTreePath(out, node, nodes);
      return out;
    }

    if (p.kind === 'sheet') {
      const node = nodes.find((n) => n.id === `sheet:${p.sheetId}`);
      pushTreePath(out, node, nodes);
      return out;
    }

    if (p.kind === 'case') {
      const node = nodes.find((n) => n.kind === 'case' && n.caseId === p.caseId);
      pushTreePath(out, node, nodes);
      return out;
    }

    if (p.kind === 'people' && p.caseId) {
      out.push(...caseCrumbs(p.caseId, nodes));
      out.push({ label: t('view.roster'), onClick: () => openPane({ kind: 'people', caseId: p.caseId }) });
      return out;
    }

    if (p.kind === 'search' && p.caseId) {
      out.push(...caseCrumbs(p.caseId, nodes));
    }
    const viewLabel =
      p.kind === 'table'
        ? t('view.table')
        : p.kind === 'search'
          ? t('view.search')
          : p.kind === 'calendar'
            ? t('view.calendar')
            : null;
    if (viewLabel) out.push({ label: viewLabel, last: true, onClick: () => openPane(p) });
    return out;
  });

  function caseCrumbs(caseId: string, nodes: TreeNode[]): Crumb[] {
    const node = nodes.find((n) => n.kind === 'case' && n.caseId === caseId);
    if (!node) return [];
    return [
      {
        label: node.detail ?? node.nodeKey,
        icon: KIND_META.case.icon,
        onClick: () => openPane({ kind: 'case', caseId }),
      },
    ];
  }

  /**
   * Walks a node up to the root, rendering the full tree path. `ancestorsOf`
   * returns ids nearest-first, so the chain is reversed back to root order.
   */
  function pushTreePath(out: Crumb[], node: TreeNode | undefined, nodes: TreeNode[]): void {
    if (!node) {
      out.push({ label: t('common.open'), onClick: () => {}, last: true });
      return;
    }
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const chain = [...ancestorsOf(nodes, node.id).reverse(), node.id]
      .map((id) => byId.get(id))
      .filter((n): n is TreeNode => n !== undefined);

    chain.forEach((n, i) => {
      const isLast = i === chain.length - 1;
      out.push({
        label: n.kind === 'case' ? (n.detail ?? n.nodeKey) : n.label,
        icon: KIND_META[n.kind].icon,
        last: isLast,
        onClick: () => {
          // The current node is already open; ancestors jump to their own view.
          if (n.kind === 'case' && n.caseId) openPane({ kind: 'case', caseId: n.caseId });
        },
      });
    });
  }

  const scopeId = $derived(
    pane.value.kind === 'case' ||
      pane.value.kind === 'people' ||
      pane.value.kind === 'table' ||
      pane.value.kind === 'search' ||
      pane.value.kind === 'calendar'
      ? pane.value.caseId
      : null,
  );
  const scopeLabel = $derived(
    scopeId ? cases.value.find((c) => c.id === scopeId)?.reference ?? t('kind.case') : t('common.allCases'),
  );
</script>

<div class="head">
  <PanelGrip id="workspace" />
  <div class="nav">
    <button class="icon" onclick={goBack} disabled={!canGoBack.value} title={t('header.back')}>‹</button>
    <button class="icon" onclick={goForward} disabled={!canGoForward.value} title={t('header.forward')}>›</button>
  </div>

  <nav class="crumbs" aria-label={t('header.breadcrumb')}>
    {#each crumbs as c, i (i)}
      {#if i > 0}<span class="sep">/</span>{/if}
      <button
        class="crumb"
        class:last={c.last}
        onclick={c.onClick}
        title={c.label}
      >
        {#if c.icon}<span class="c-icon">{c.icon}</span>{/if}
        <span class="truncate">{c.label}</span>
      </button>
    {/each}
  </nav>

  <div class="spacer"></div>

  {#if scopeId}
    <span class="scope" title={t('header.scoped')}>
      {scopeLabel}
    </span>
    <button
      class="btn btn-ghost btn-sm"
      onclick={() => openPane({ kind: 'table', caseId: null })}
      title={t('header.clearScope')}
    >
      {t('common.clear')}
    </button>
  {/if}

  <QuickAdd />
</div>

<style>
  /* Chrome. It sits on the canvas tone rather than the content tone, so the
     view below it does the tone change that groups the workspace. */
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 10px;
    border-bottom: 1px solid var(--border);
    background: var(--bg);
    flex-shrink: 0;
  }

  .nav {
    display: flex;
    gap: 1px;
  }
  .icon {
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 15px;
    line-height: 1;
  }
  .icon:hover:not(:disabled) {
    background: var(--sunken);
    color: var(--text);
  }
  .icon:disabled {
    color: var(--text-disabled);
    cursor: default;
  }

  .crumbs {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
    overflow: hidden;
  }
  .crumb {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: transparent;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 12.5px;
    padding: 2px 5px;
    border-radius: var(--radius-sm);
    max-width: 190px;
  }
  .crumb:hover {
    background: var(--sunken);
    color: var(--text);
  }
  .crumb.last {
    color: var(--text);
    font-weight: 600;
  }
  .c-icon {
    font-size: 9px;
    color: var(--text-tertiary);
  }
  .sep {
    color: var(--text-disabled);
    font-size: 11.5px;
  }

  /* The scope chip is structural chrome, not a link, so it stays neutral: the
     pill says "tag" and nothing else. */
  .scope {
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--sunken);
    border: 1px solid var(--border);
    border-radius: var(--radius-full);
    padding: 1px 8px;
    white-space: nowrap;
  }
</style>
