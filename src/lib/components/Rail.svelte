<script lang="ts">
  import { getTheme, toggleTheme } from '$lib/theme';
  import PanelGrip from './PanelGrip.svelte';
  import { t } from '$lib/i18n/index.svelte';
  import {
    chatCollapsed,
    openPane,
    pane,
    paletteOpen,
    toggleChat,
    type PaneView,
  } from '$lib/stores.svelte';

  /**
   * The global rail: where you can go, never where you are. Location lives in
   * the workspace breadcrumb; the rail only holds destinations and the
   * app-level toggles.
   *
   * It docks to any window edge: a column on the sides, a bar on the top or
   * bottom. Only the axis changes; the contents and their order do not.
   */

  let { horizontal = false }: { horizontal?: boolean } = $props();

  type Kind = PaneView['kind'];

  // The theme module is plain TS, so mirror it locally for the glyph.
  let theme = $state(getTheme());

  /** Cross-case views keep the current case scope, as the header switcher did. */
  const scopeId = $derived.by(() => {
    const p = pane.value;
    return 'caseId' in p ? p.caseId : null;
  });

  const destinations = $derived<{ kind: Kind; glyph: string; label: string; go: () => void }[]>([
    { kind: 'grid', glyph: '▤', label: t('view.tree'), go: () => openPane({ kind: 'grid' }) },
    { kind: 'table', glyph: '▦', label: t('view.table'), go: () => openPane({ kind: 'table', caseId: scopeId }) },
    { kind: 'calendar', glyph: '◷', label: t('view.calendar'), go: () => openPane({ kind: 'calendar', caseId: scopeId }) },
    { kind: 'search', glyph: '⌕', label: t('rail.fullText'), go: () => openPane({ kind: 'search', caseId: scopeId }) },
    { kind: 'people', glyph: '●', label: t('view.people'), go: () => openPane({ kind: 'people', caseId: scopeId }) },
  ]);
</script>

<nav class="rail" class:horizontal aria-label={t('rail.main')}>
  <PanelGrip id="rail" wide={!horizontal} />
  <button class="r" onclick={() => paletteOpen.set(true)} title={t('rail.quickFindHint')} aria-label={t('rail.quickFind')}>
    <span aria-hidden="true">⌘</span>
  </button>
  <button
    class="r"
    class:on={!chatCollapsed.value}
    onclick={toggleChat}
    title={chatCollapsed.value ? t('rail.showAssistant') : t('rail.hideAssistant')}
    aria-label={t('rail.toggleAssistant')}
    aria-pressed={!chatCollapsed.value}
  >
    <span aria-hidden="true">§</span>
  </button>

  <div class="gap" role="separator"></div>

  {#each destinations as d (d.kind)}
    <button
      class="r"
      class:on={pane.value.kind === d.kind}
      onclick={d.go}
      title={d.label}
      aria-label={d.label}
      aria-current={pane.value.kind === d.kind ? 'page' : undefined}
    >
      <span aria-hidden="true">{d.glyph}</span>
    </button>
  {/each}

  <div class="spacer"></div>

  <button
    class="r"
    onclick={() => (theme = toggleTheme())}
    title={theme === 'dark' ? t('rail.lightTheme') : t('rail.darkTheme')}
    aria-label={theme === 'dark' ? t('rail.lightTheme') : t('rail.darkTheme')}
  >
    <span class="theme-ico" class:flip={theme === 'dark'} aria-hidden="true">◐</span>
  </button>
  <button
    class="r"
    class:on={pane.value.kind === 'settings'}
    onclick={() => openPane({ kind: 'settings' })}
    title={t('view.settings')}
    aria-label={t('view.settings')}
    aria-current={pane.value.kind === 'settings' ? 'page' : undefined}
  >
    <span aria-hidden="true">⚙</span>
  </button>
</nav>

<style>
  /* Canvas tone, no border on the sides: the tone change against the chat and
     workspace does the grouping (DESIGN §10, §15). */
  .rail {
    width: 44px;
    height: 100%;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 8px 0;
    background: var(--bg);
    /* A short window scrolls the rail rather than squeezing its buttons. */
    overflow: auto;
    scrollbar-width: none;
  }

  /* Docked top or bottom: the same contents on the other axis. */
  .rail.horizontal {
    width: 100%;
    height: 44px;
    flex-direction: row;
    padding: 0 8px;
  }

  /* Never shrink: buttons that give up their size overlap each other. */
  .rail > :global(*) {
    flex-shrink: 0;
  }

  .r {
    width: 32px;
    height: 32px;
    display: grid;
    place-items: center;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-secondary);
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  .r:hover {
    background: var(--sunken);
    color: var(--text);
  }
  /* Selection is a tone, not a colour: accent is reserved for data (§5.4). */
  .r.on {
    background: var(--sunken);
    color: var(--text);
  }

  .gap {
    width: 16px;
    height: 1px;
    margin: 6px 0;
    background: var(--border);
  }
  .horizontal .gap {
    width: 1px;
    height: 16px;
    margin: 0 6px;
  }

  /* The spacer pushes theme and settings to the far end on either axis. */
  .rail > .spacer {
    flex: 1 1 auto;
    flex-shrink: 1;
  }

  .theme-ico {
    display: inline-block;
    transition: transform var(--dur-slow) var(--ease);
  }
  .theme-ico.flip {
    transform: rotate(180deg);
  }
</style>
