<script lang="ts">
  import { t } from '$lib/i18n/index.svelte';
  import type { TreeNode } from '$lib/types';
  import type { MessageKey } from '$lib/i18n/index.svelte';
  import { KIND_META, mentionFragment, mentionLookupFor, mentionedDocumentIds, resolveMention } from '$lib/tree';
  import { pendingMention } from '$lib/stores.svelte';
  import { expandSlash, matchSlash, slashFragment, type SlashCommand } from '$lib/commands';

  let {
    nodes,
    disabled = false,
    placeholder,
    onSend,
  }: {
    nodes: TreeNode[];
    disabled?: boolean;
    placeholder?: string;
    onSend: (text: string, documentIds: string[]) => void | Promise<void>;
  } = $props();

  let text = $state('');
  let busy = $state(false);
  let textarea: HTMLTextAreaElement | undefined = $state();
  let active = $state(0);
  /** The text the user pressed Esc on while the command menu was open. */
  let dismissedText = $state<string | null>(null);
  /** The textarea lost focus; menus close until it is focused again. */
  let blurred = $state(false);
  /** Bumped on caret moves that do not change the text, so the fragments re-derive. */
  let caretTick = $state(0);
  /** The mention start the user pressed Esc on; its hint stays hidden. */
  let dismissedAt = $state<number | null>(null);

  /**
   * The `@` fragment currently being typed, or null when the caret is not in a
   * mention. Derived from the text before the caret rather than tracked on
   * input, so clicking elsewhere or deleting back into the token still works.
   */
  const mention = $derived.by((): { start: number; query: string } | null => {
    const el = textarea;
    void caretTick;
    if (!el) return null;
    return mentionFragment(text, el.selectionStart ?? text.length);
  });

  /**
   * What the fragment points at — a confirmation, not a search. It is a binary
   * search over handles sorted once per tree, so typing stays instant however
   * large the tree is. Retrieval over the documents runs only on send.
   */
  const resolved = $derived(
    mention && mention.query ? resolveMention(mentionLookupFor(nodes), nodes, mention.query) : null,
  );


  /** Nothing left to complete: the fragment already is the full handle. */
  const complete = $derived(
    !!resolved && !!mention && resolved.node.nodeKey.toLowerCase() === mention.query.toLowerCase(),
  );

  const slash = $derived.by((): string | null => {
    const el = textarea;
    void caretTick;
    if (!el) return null;
    return slashFragment(text, el.selectionStart ?? text.length);
  });

  // Derived, never assigned in an effect. An effect that set `commands` and
  // then read `commands.length` depended on its own output and re-ran itself
  // ~1000 times per keystroke until Svelte aborted it — the chat's lag.
  /** Slash commands matching a `/` typed at the start; empty when not in one. */
  const commands = $derived<SlashCommand[]>(slash !== null && dismissedText !== text ? matchSlash(slash) : []);
  const menuOpen = $derived(commands.length > 0 && !blurred);
  /** The highlighted command, kept in range as the list narrows. */
  const activeIdx = $derived(Math.min(active, Math.max(0, commands.length - 1)));

  const showHint = $derived(!!mention && !!mention.query && dismissedAt !== mention.start && commands.length === 0);

  // A mention requested from another view (the document viewer's "Ask about
  // this") lands here, so the user types their question on top of it.
  $effect(() => {
    const key = pendingMention.value;
    if (!key) return;
    pendingMention.set(null);
    text = text.trim() ? `${text.trim()} @${key} ` : `@${key} `;
    requestAnimationFrame(() => {
      const el = textarea;
      if (!el) return;
      el.focus();
      const pos = el.value.length;
      el.setSelectionRange(pos, pos);
      autosize();
    });
  });

  /** Tab: replace the fragment with the resolved node's full handle. */
  function completeMention(): void {
    const el = textarea;
    if (!el || !mention || !resolved) return;
    const insert = `@${resolved.node.nodeKey} `;
    const caret = el.selectionStart ?? text.length;
    const before = text.slice(0, mention.start);
    // Swallow the rest of a partially typed handle to the right of the caret.
    const after = text.slice(caret).replace(/^[A-Za-z0-9._\-/]*\s?/, '');
    text = before + insert + after;
    requestAnimationFrame(() => {
      const pos = before.length + insert.length;
      el.focus();
      el.setSelectionRange(pos, pos);
      caretTick++;
      autosize();
    });
  }

  /** Dictionary key for a command's translated hint or template. */
  function slashKey(cmd: SlashCommand, part: 'hint' | 'template'): MessageKey {
    return `slash.${cmd.name}.${part}` as MessageKey;
  }

  function runCommand(cmd: SlashCommand): void {
    const el = textarea;
    if (!el) return;
    const next = expandSlash(t(slashKey(cmd, 'template')), text, el.selectionStart ?? text.length);
    text = next.text;
    active = 0;
    requestAnimationFrame(() => {
      el.focus();
      el.setSelectionRange(next.caret, next.caret);
      caretTick++;
      autosize();
    });
  }

  /**
   * Grows the textarea with its content. Chromium (WebView2) does this in CSS
   * via `field-sizing: content`; measuring in JS forced a synchronous layout
   * of the whole chat column on every keystroke, so it is only a fallback.
   */
  const CSS_AUTOSIZE = typeof CSS !== 'undefined' && CSS.supports('field-sizing', 'content');

  function autosize(): void {
    const el = textarea;
    if (!el || CSS_AUTOSIZE) return;
    el.style.height = 'auto';
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`;
  }

  function onKeydown(e: KeyboardEvent): void {
    if (menuOpen && commands.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        active = (activeIdx + 1) % commands.length;
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        active = (activeIdx - 1 + commands.length) % commands.length;
        return;
      }
      if (e.key === 'Tab' || (e.key === 'Enter' && !e.shiftKey)) {
        e.preventDefault();
        runCommand(commands[activeIdx]);
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        dismissedText = text;
        return;
      }
    }

    if (showHint) {
      if (e.key === 'Tab' && !e.shiftKey && resolved && !complete) {
        e.preventDefault();
        completeMention();
        return;
      }
      if (e.key === 'Escape' && mention) {
        e.preventDefault();
        dismissedAt = mention.start;
        return;
      }
    }

    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      void submit();
    }
  }

  async function submit(): Promise<void> {
    const body = text.trim();
    if (!body || busy || disabled) return;
    // The one place mentions are searched: resolve them against the live tree
    // so the backend scopes retrieval to exactly the documents that were named.
    const ids = mentionedDocumentIds(body, nodes);
    busy = true;
    text = '';
    dismissedAt = null;
    autosize();
    try {
      await onSend(body, ids);
    } finally {
      busy = false;
      requestAnimationFrame(() => textarea?.focus());
    }
  }
</script>

<div class="composer">
  {#if menuOpen && commands.length > 0}
    <!-- Floats clear of the well, on a lifted surface, with a shadow rather
         than a border. One edge per surface: this one is above the page. -->
    <div class="menu" role="listbox">
      {#each commands as c, i (c.name)}
        <button
          class="opt"
          class:active={i === activeIdx}
          role="option"
          aria-selected={i === activeIdx}
          onmousedown={(e) => {
            e.preventDefault();
            runCommand(c);
          }}
        >
          <span class="o-icon" aria-hidden="true">/</span>
          <span class="o-label truncate">{c.name}</span>
          <span class="o-key truncate">{t(slashKey(c, 'hint'))}</span>
        </button>
      {/each}
    </div>
  {/if}

  {#if showHint}
    <!-- A confirmation of what the @ points at, not a list to choose from:
         the full path, and Tab to fill it in. -->
    <div class="mention-hint" class:none={!resolved} role="status" aria-live="polite">
      {#if resolved}
        <span class="mh-icon" aria-hidden="true">{KIND_META[resolved.node.kind].icon}</span>
        <!-- mousedown, not click: the textarea must keep focus and caret. -->
        <button
          class="mh-path mono truncate"
          title={resolved.node.label}
          onmousedown={(e) => {
            e.preventDefault();
            completeMention();
          }}
        >
          @{resolved.node.nodeKey}
        </button>
        {#if resolved.more > 0}
          <span class="mh-more">{t('composer.moreMatches', { n: resolved.more })}</span>
        {/if}
        {#if !complete}
          <span class="mh-key"><kbd>Tab</kbd> {t('composer.tabComplete')}</span>
        {:else}
          <span class="mh-key">✓ {t('composer.found')}</span>
        {/if}
      {:else}
        <span class="mh-icon" aria-hidden="true">?</span>
        <span class="mh-missing truncate">{t('composer.noMatch', { query: `@${mention?.query ?? ''}` })}</span>
      {/if}
    </div>
  {/if}

  <!-- One sunken well, not a bordered field with a button bolted on. The
       toolbar lives inside the well so the whole thing reads as a single
       control. -->
  <div class="well" class:focused={menuOpen}>
    <textarea
      bind:this={textarea}
      bind:value={text}
      oninput={autosize}
      onkeydown={onKeydown}
      onclick={() => caretTick++}
      onkeyup={(e) => {
        if (e.key === 'ArrowLeft' || e.key === 'ArrowRight' || e.key === 'Home' || e.key === 'End') caretTick++;
      }}
      onfocus={() => (blurred = false)}
      onblur={() => setTimeout(() => (blurred = true), 120)}
      placeholder={placeholder ?? t('composer.placeholder')}
      rows="1"
      disabled={disabled}
    ></textarea>

    <div class="bar">
      <span class="hint faint">{t('composer.hint')}</span>
      <button
        class="send"
        class:ready={text.trim().length > 0 && !busy && !disabled}
        onclick={() => void submit()}
        disabled={busy || disabled || text.trim().length === 0}
        title={t('composer.send')}
      >
        {#if busy}<span class="spinner"></span>{:else}↑{/if}
      </button>
    </div>
  </div>
</div>

<style>
  .composer {
    position: relative;
    border-top: 1px solid var(--border);
    padding: 8px 10px 10px;
    background: var(--bg);
    flex-shrink: 0;
  }

  .menu {
    animation: menu-rise var(--dur-fast) var(--ease);
    position: absolute;
    bottom: calc(100% - 6px);
    left: 10px;
    right: 10px;
    background: var(--overlay);
    /* Floating: shadow, no border. */
    border: none;
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-lg);
    z-index: 20;
    max-height: 240px;
    overflow-y: auto;
    padding: 4px;
  }

  /* The @ confirmation strip. It sits in flow above the well rather than
     floating, so it never covers the conversation, and it is quiet: one line
     of tertiary text with the path as the only emphasis. */
  .mention-hint {
    animation: menu-rise var(--dur-fast) var(--ease);
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 26px;
    padding: 0 4px 6px;
    font-size: 12px;
    color: var(--text-tertiary);
  }
  .mh-icon {
    width: 12px;
    text-align: center;
    font-size: 10px;
    flex-shrink: 0;
  }
  .mh-path {
    min-width: 0;
    padding: 0;
    border: none;
    background: none;
    font-size: 12px;
    color: var(--text);
    cursor: pointer;
    text-align: left;
  }
  .mh-path:hover {
    text-decoration: underline;
  }
  .mh-more {
    flex-shrink: 0;
  }
  .mh-key {
    margin-left: auto;
    flex-shrink: 0;
    white-space: nowrap;
  }
  .mh-missing {
    min-width: 0;
  }

  .opt {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 30px;
    padding: 5px 8px;
    background: transparent;
    border: none;
    border-radius: var(--radius-sm);
    cursor: pointer;
    text-align: left;
    font-size: 12.5px;
  }
  .opt:hover {
    background: var(--sunken);
  }
  /* Keyboard selection has to beat :hover, otherwise a mouse user scrolling the
     list cannot keep the highlight under the arrow keys. */
  .opt.active {
    background: var(--sunken);
  }
  .opt.active:hover {
    background: var(--sunken);
  }
  .o-icon {
    color: var(--text-tertiary);
    font-size: 10px;
    width: 12px;
    text-align: center;
    flex-shrink: 0;
  }
  .o-label {
    flex: 1;
    min-width: 0;
  }
  .o-key {
    font-size: 11px;
    color: var(--text-tertiary);
    max-width: 45%;
  }

  .well {
    display: flex;
    flex-direction: column;
    gap: 4px;
    background: var(--sunken);
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    padding: 7px 7px 6px 11px;
    transition:
      border-color var(--dur) var(--ease),
      box-shadow var(--dur) var(--ease),
      background-color var(--dur) var(--ease);
  }
  /* Hover is a sunken fill, not a darker border: a 3:1 outline on the primary
     input would be a shout. Focus is the only time the well speaks up. */
  .well:hover {
    background: var(--sunken);
  }
  .well:focus-within {
    background: var(--input);
    border-color: var(--ink);
    box-shadow: var(--ring);
  }

  textarea {
    field-sizing: content;
    border: none;
    background: transparent;
    padding: 1px 0 0;
    resize: none;
    width: 100%;
    min-height: 22px;
    font-size: 13.5px;
    line-height: 1.5;
    max-height: 160px;
  }
  textarea:focus {
    box-shadow: none;
    border: none;
    background: transparent;
  }
  textarea::placeholder {
    color: var(--text-tertiary);
  }

  .bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .bar .hint {
    margin-top: 0;
    font-size: 11px;
  }

  .send {
    width: 30px;
    height: 30px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-disabled);
    cursor: pointer;
    display: grid;
    place-items: center;
    font-size: 14px;
    flex-shrink: 0;
    transition:
      background-color var(--dur-fast) var(--ease),
      border-color var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease),
      transform var(--dur-fast) var(--ease);
  }
  .send:not(:disabled):hover {
    background: var(--surface);
    color: var(--text-secondary);
  }
  /* Send is the primary action, so it is ink like every other primary. */
  .send.ready {
    background: var(--ink);
    color: var(--ink-ink);
  }
  .send.ready:hover {
    background: var(--ink-hover);
    color: var(--ink-ink);
  }
  .send:active:not(:disabled) {
    transform: translateY(1px);
  }
  .send:disabled {
    cursor: not-allowed;
  }
</style>
