<script lang="ts">
  import { keyLayout, type PanelId } from '$lib/layout';
  import { beginPanelDrag, drag, panelLayout, setLayout } from '$lib/panels.svelte';
  import { t } from '$lib/i18n/index.svelte';

  /**
   * The handle that moves a panel. Drag the rail to any window edge, or the
   * assistant / workspace to either side. With the keyboard: focus it and use
   * the arrow keys.
   */

  let { id, wide = false }: { id: PanelId; wide?: boolean } = $props();

  const name = $derived(t(`panel.${id}`));

  function onKeydown(e: KeyboardEvent): void {
    const next = keyLayout(panelLayout.value, id, e.key);
    if (!next) return;
    e.preventDefault();
    setLayout(next);
  }
</script>

<button
  class="grip"
  class:wide
  class:dragging={drag.active && drag.id === id}
  onpointerdown={(e) => beginPanelDrag(id, e)}
  onkeydown={onKeydown}
  title={id === 'rail' ? t('panel.gripHintRail') : t('panel.gripHintSide', { panel: name })}
  aria-label={t('panel.gripLabel', { panel: name })}
>
  <span aria-hidden="true">⠿</span>
</button>

<style>
  /* Quiet at rest — it is chrome — and obvious on hover, where the grab
     cursor makes the affordance explicit. */
  .grip {
    flex-shrink: 0;
    width: 16px;
    height: 24px;
    display: grid;
    place-items: center;
    padding: 0;
    border: none;
    border-radius: var(--radius-xs);
    background: transparent;
    color: var(--text-tertiary);
    font-size: 12px;
    line-height: 1;
    cursor: grab;
    touch-action: none;
    transition:
      background var(--dur-fast) var(--ease),
      color var(--dur-fast) var(--ease);
  }
  /* Sits across the top of a vertical rail. */
  .grip.wide {
    width: 32px;
    height: 16px;
  }
  .grip:hover,
  .grip.dragging {
    background: var(--sunken);
    color: var(--text);
  }
  .grip.dragging {
    cursor: grabbing;
  }
</style>
