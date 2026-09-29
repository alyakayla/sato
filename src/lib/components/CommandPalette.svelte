<script lang="ts">
  import type { TreeNode } from '$lib/types';
  import { KIND_META, quickFind } from '$lib/tree';
  import { toggleTheme } from '$lib/theme';
  import { LOCALES, getLocale, setLocale, t } from '$lib/i18n/index.svelte';
  import Modal from './Modal.svelte';
  import { panelLayout, resetLayout, setLayout } from '$lib/panels.svelte';
  import { RAIL_DOCKS, isDefaultLayout, opposite } from '$lib/layout';
  import {
    askAbout,
    cases,
    chatCollapsed,
    createRequest,
    openNode,
    openPane,
    pane,
    paletteOpen,
    paneHistory,
    paneTitle,
    toggleChat,
    treeNodes,
    type PaneView,
  } from '$lib/stores.svelte';

  /**
   * Quick find (Ctrl+K). One list, three sources: recently visited panes when
   * the query is empty, tree matches grouped by kind when it is not, and a
   * short set of commands that match either way.
   */

  interface Item {
    id: string;
    glyph: string;
    label: string;
    hint?: string;
    run: () => void;
  }
  interface Group {
    title: string;
    items: Item[];
  }

  let query = $state('');
  let cursor = $state(0);
  let input = $state<HTMLInputElement | null>(null);
  let list = $state<HTMLElement | null>(null);

  /** The tree node the workspace is showing, if any — the target of "Ask". */
  const currentNode = $derived.by((): TreeNode | undefined => {
    const p = pane.value;
    const nodes = treeNodes.value;
    if (p.kind === 'document') return nodes.find((n) => n.documentId === p.documentId);
    if (p.kind === 'sheet') return nodes.find((n) => n.id === `sheet:${p.sheetId}`);
    if (p.kind === 'case') return nodes.find((n) => n.kind === 'case' && n.caseId === p.caseId);
    return undefined;
  });

  const recent = $derived.by((): Item[] => {
    const seen = new Set<string>();
    const out: Item[] = [];
    for (const p of [...paneHistory.value].reverse()) {
      const key = JSON.stringify(p);
      if (seen.has(key)) continue;
      seen.add(key);
      out.push({
        id: `recent:${key}`,
        glyph: '↺',
        label: paneTitle(p, treeNodes.value, cases.value),
        run: () => openPane(p as PaneView),
      });
      if (out.length === 5) break;
    }
    return out;
  });

  const commands = $derived.by((): Item[] => {
    const out: Item[] = [];
    if (currentNode) {
      const node = currentNode;
      out.push({
        id: 'cmd:ask',
        glyph: '§',
        label: t('palette.askAbout', { name: node.label }),
        run: () => {
          if (chatCollapsed.value) toggleChat();
          askAbout(node.nodeKey);
        },
      });
    }
    out.push(
      { id: 'cmd:case', glyph: '◆', label: t('create.case'), run: () => createRequest.set('case') },
      { id: 'cmd:import', glyph: '⤒', label: t('create.import'), run: () => createRequest.set('import') },
      { id: 'cmd:sheet', glyph: '▦', label: t('create.sheet'), run: () => createRequest.set('sheet') },
      {
        id: 'cmd:chat',
        glyph: '§',
        label: chatCollapsed.value ? t('palette.showAssistant') : t('palette.hideAssistant'),
        hint: 'Ctrl+J',
        run: toggleChat,
      },
      { id: 'cmd:theme', glyph: '◐', label: t('palette.toggleTheme'), run: () => void toggleTheme() },
      // Every other language is one Enter away; typing its name finds it.
      ...LOCALES.filter((l) => l.id !== getLocale()).map((l) => ({
        id: `cmd:lang:${l.id}`,
        glyph: l.short,
        label: t('palette.language', { language: l.name }),
        run: () => setLocale(l.id),
      })),
      { id: 'cmd:settings', glyph: '⚙', label: t('view.settings'), run: () => openPane({ kind: 'settings' }) },
    );
    const layout = panelLayout.value;
    const other = opposite(layout.chat);
    out.push({
      id: 'cmd:chat-side',
      glyph: other === 'left' ? '←' : '→',
      label: t(`panel.chatTo.${other}`),
      run: () => setLayout({ ...layout, chat: other }),
    });
    for (const dock of RAIL_DOCKS.filter((d) => d !== layout.rail)) {
      out.push({
        id: `cmd:rail-${dock}`,
        glyph: '⠿',
        label: t(`panel.railTo.${dock}`),
        run: () => setLayout({ ...layout, rail: dock }),
      });
    }
    if (!isDefaultLayout(layout)) {
      out.push({ id: 'cmd:layout-reset', glyph: '⠿', label: t('panel.reset'), run: resetLayout });
    }
    out.push(
    );
    const q = query.trim().toLowerCase();
    return q ? out.filter((c) => c.label.toLowerCase().includes(q)) : out;
  });

  const groups = $derived.by((): Group[] => {
    const out: Group[] = [];
    if (!query.trim()) {
      if (recent.length) out.push({ title: t('palette.recent'), items: recent });
    } else {
      for (const g of quickFind(treeNodes.value, query)) {
        out.push({
          title: t(`kinds.${g.kind}`),
          items: g.nodes.map((n) => ({
            id: n.id,
            glyph: KIND_META[n.kind].icon,
            label: n.kind === 'case' ? (n.detail ?? n.label) : n.label,
            hint: n.nodeKey,
            run: () => openNode(n),
          })),
        });
      }
    }
    if (commands.length) out.push({ title: t('common.actions'), items: commands });
    return out;
  });

  const flat = $derived(groups.flatMap((g) => g.items));

  // Reset on every open so the palette never reopens mid-query.
  $effect(() => {
    if (!paletteOpen.value) return;
    query = '';
    cursor = 0;
    queueMicrotask(() => input?.focus());
  });

  $effect(() => {
    void query;
    cursor = 0;
  });

  $effect(() => {
    list?.querySelector(`[data-idx="${cursor}"]`)?.scrollIntoView({ block: 'nearest' });
  });

  function close(): void {
    paletteOpen.set(false);
  }

  function run(item: Item | undefined): void {
    if (!item) return;
    close();
    item.run();
  }

  function onKeydown(e: KeyboardEvent): void {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      cursor = Math.min(flat.length - 1, cursor + 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      cursor = Math.max(0, cursor - 1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      run(flat[cursor]);
    }
  }

  /** Index into `flat` for an item, so the cursor spans groups. */
  function indexOf(item: Item): number {
    return flat.indexOf(item);
  }
</script>

<Modal open={paletteOpen.value} onclose={close} width="560px">
  <div class="qf">
    <input
      bind:this={input}
      bind:value={query}
      onkeydown={onKeydown}
      class="qf-input"
      placeholder={t('palette.placeholder')}
      aria-label={t('rail.quickFind')}
      role="combobox"
      aria-expanded="true"
      aria-controls="qf-list"
      aria-activedescendant={flat[cursor] ? `qf-${cursor}` : undefined}
      spellcheck="false"
      autocomplete="off"
    />

    <div class="qf-list" id="qf-list" role="listbox" bind:this={list}>
      {#if flat.length === 0}
        <div class="qf-empty">{t('palette.noMatches', { query })}</div>
      {/if}
      {#each groups as g (g.title)}
        <div class="qf-group" role="group" aria-label={g.title}>
          <div class="qf-title">{g.title}</div>
          {#each g.items as it (it.id)}
            {@const i = indexOf(it)}
            <button
              class="qf-item"
              class:on={i === cursor}
              id="qf-{i}"
              data-idx={i}
              role="option"
              aria-selected={i === cursor}
              tabindex="-1"
              onmousemove={() => (cursor = i)}
              onclick={() => run(it)}
            >
              <span class="qf-glyph" aria-hidden="true">{it.glyph}</span>
              <span class="truncate">{it.label}</span>
              {#if it.hint}<span class="qf-hint mono truncate">{it.hint}</span>{/if}
            </button>
          {/each}
        </div>
      {/each}
    </div>

    <div class="qf-foot">
      <span><kbd>↑</kbd><kbd>↓</kbd> {t('palette.move')}</span>
      <span><kbd>Enter</kbd> {t('palette.open')}</span>
      <span><kbd>Esc</kbd> {t('palette.close')}</span>
    </div>
  </div>
</Modal>

<style>
  .qf {
    display: flex;
    flex-direction: column;
  }

  /* The input is the header: borderless, large, on the overlay itself. */
  .qf-input {
    border: none;
    border-bottom: 1px solid var(--border);
    border-radius: var(--radius-lg) var(--radius-lg) 0 0;
    background: transparent;
    padding: 14px 16px;
    font-size: 15px;
    min-height: 0;
    box-shadow: none;
  }
  .qf-input:focus,
  .qf-input:focus-visible {
    outline: none;
    box-shadow: none;
    border-color: var(--border);
  }

  /* A fixed height, not a maximum: the dialog is centred, so a list that grew
     and shrank with each keystroke would make the whole palette jump. */
  .qf-list {
    height: min(420px, 56vh);
    overflow-y: auto;
    padding: 6px;
  }

  .qf-group + .qf-group {
    margin-top: 6px;
  }
  .qf-title {
    font-size: 11.5px;
    color: var(--text-tertiary);
    padding: 6px 8px 4px;
  }

  .qf-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 32px;
    padding: 0 8px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text);
    font-size: 13.5px;
    text-align: left;
    cursor: pointer;
  }
  .qf-item.on {
    background: var(--sunken);
  }
  .qf-glyph {
    width: 14px;
    text-align: center;
    font-size: 11px;
    color: var(--text-tertiary);
    flex-shrink: 0;
  }
  .qf-hint {
    margin-left: auto;
    font-size: 11.5px;
    color: var(--text-tertiary);
    max-width: 45%;
  }

  .qf-empty {
    padding: 14px 10px;
    font-size: 12.5px;
    color: var(--text-secondary);
  }

  .qf-foot {
    display: flex;
    gap: 14px;
    padding: 8px 14px;
    border-top: 1px solid var(--border);
    font-size: 11.5px;
    color: var(--text-tertiary);
  }
  .qf-foot kbd + kbd {
    margin-left: 2px;
  }
</style>
