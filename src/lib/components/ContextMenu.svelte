<script lang="ts">
  import { tick } from 'svelte';
  import { closeContextMenu, menu, type MenuItem } from '$lib/contextMenu.svelte';

  /**
   * Renders the shared context menu. Floating surface: `--overlay`,
   * `--shadow-lg`, no border (DESIGN §9). Keyboard: ↑/↓/Home/End move, Enter or
   * Space runs, Esc closes and returns focus to where it was.
   */

  let el = $state<HTMLDivElement | null>(null);
  let left = $state(0);
  let top = $state(0);

  const actionable = $derived(menu.items.filter((i): i is MenuItem => i !== 'separator' && !i.disabled));

  // Place at the pointer, then flip or clamp so the whole menu stays on screen.
  $effect(() => {
    if (!menu.open) return;
    const x = menu.x;
    const y = menu.y;
    left = x;
    top = y;
    void tick().then(() => {
      if (!el) return;
      const r = el.getBoundingClientRect();
      const pad = 8;
      left = x + r.width > innerWidth - pad ? Math.max(pad, x - r.width) : x;
      top = y + r.height > innerHeight - pad ? Math.max(pad, y - r.height) : y;
      el.querySelector<HTMLButtonElement>('button[role="menuitem"]:not(:disabled)')?.focus();
    });
  });

  // Anything outside the menu closes it: a press elsewhere, a scroll, a
  // resize, or the window losing focus.
  $effect(() => {
    if (!menu.open) return;
    const outside = (e: Event) => {
      if (el && e.target instanceof Node && el.contains(e.target)) return;
      closeContextMenu();
    };
    const close = () => closeContextMenu();
    window.addEventListener('pointerdown', outside, true);
    window.addEventListener('scroll', outside, true);
    window.addEventListener('resize', close);
    window.addEventListener('blur', close);
    return () => {
      window.removeEventListener('pointerdown', outside, true);
      window.removeEventListener('scroll', outside, true);
      window.removeEventListener('resize', close);
      window.removeEventListener('blur', close);
    };
  });

  function focusAt(i: number): void {
    const buttons = [...(el?.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]:not(:disabled)') ?? [])];
    if (buttons.length === 0) return;
    buttons[(i + buttons.length) % buttons.length].focus();
  }

  function onKeydown(e: KeyboardEvent): void {
    const buttons = [...(el?.querySelectorAll<HTMLButtonElement>('button[role="menuitem"]:not(:disabled)') ?? [])];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      focusAt(at + 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      focusAt(at - 1);
    } else if (e.key === 'Home') {
      e.preventDefault();
      focusAt(0);
    } else if (e.key === 'End') {
      e.preventDefault();
      focusAt(buttons.length - 1);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      closeContextMenu(true);
    } else if (e.key === 'Tab') {
      e.preventDefault();
      closeContextMenu(true);
    }
  }

  function run(item: MenuItem): void {
    closeContextMenu(true);
    void item.run();
  }
</script>

{#if menu.open && actionable.length > 0}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="ctx"
    role="menu"
    tabindex="-1"
    aria-label={menu.title ?? undefined}
    style:left="{left}px"
    style:top="{top}px"
    bind:this={el}
    onkeydown={onKeydown}
    oncontextmenu={(e) => e.preventDefault()}
  >
    {#if menu.title}
      <div class="ctx-title truncate">{menu.title}</div>
    {/if}
    {#each menu.items as item, i (item === 'separator' ? `sep-${i}` : item.id)}
      {#if item === 'separator'}
        <div class="ctx-sep" role="separator"></div>
      {:else}
        <button
          class="ctx-item"
          class:danger={item.danger}
          role="menuitem"
          disabled={item.disabled}
          onclick={() => run(item)}
        >
          <span class="ctx-glyph" aria-hidden="true">{item.glyph ?? ''}</span>
          <span class="ctx-label truncate">{item.label}</span>
          {#if item.hint}<span class="ctx-hint">{item.hint}</span>{/if}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .ctx {
    position: fixed;
    z-index: 60;
    min-width: 208px;
    max-width: 300px;
    padding: 4px;
    background: var(--overlay);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-lg);
    animation: menu-drop var(--dur-fast) var(--ease);
    outline: none;
  }

  .ctx-title {
    padding: 6px 8px 5px;
    font-size: 11.5px;
    color: var(--text-tertiary);
  }

  .ctx-sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--border);
  }

  .ctx-item {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    min-height: 30px;
    padding: 0 8px;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .ctx-item:hover,
  .ctx-item:focus-visible {
    background: var(--sunken);
    outline: none;
  }
  .ctx-item:disabled {
    color: var(--text-disabled);
    cursor: default;
    background: transparent;
  }
  /* Destructive is the one place colour appears: a semantic, not decoration. */
  .ctx-item.danger {
    color: var(--danger);
  }
  .ctx-item.danger:hover,
  .ctx-item.danger:focus-visible {
    background: var(--danger-soft);
  }

  .ctx-glyph {
    width: 14px;
    text-align: center;
    font-size: 11px;
    color: var(--text-tertiary);
    flex-shrink: 0;
  }
  .danger .ctx-glyph {
    color: inherit;
  }
  .ctx-label {
    flex: 1;
    min-width: 0;
  }
  .ctx-hint {
    font-size: 11.5px;
    color: var(--text-tertiary);
    flex-shrink: 0;
  }
</style>
