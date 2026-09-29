<script lang="ts">
  import type { Snippet } from 'svelte';
  import { t } from '$lib/i18n/index.svelte';

  interface Props {
    open: boolean;
    /** Omit for a headless dialog such as quick find. */
    title?: string;
    onclose: () => void;
    children: Snippet;
    footer?: Snippet;
    width?: string;
  }

  let { open, title, onclose, children, footer, width }: Props = $props();

  let dialog = $state<HTMLDialogElement | null>(null);

  // Drive the native dialog element so we get focus trapping and Esc-to-close
  // for free, but keep the open state owned by the parent.
  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  });

  function onCancel(e: Event) {
    e.preventDefault();
    onclose();
  }
</script>

<dialog
  bind:this={dialog}
  oncancel={onCancel}
  aria-label={title}
  style={width ? `max-width:${width}` : undefined}
>
  {#if title}
    <div class="dialog-head">
      <h2>{title}</h2>
      <button class="btn btn-ghost btn-sm" onclick={onclose} aria-label={t('common.close')}>✕</button>
    </div>
  {/if}
  <div class="dialog-body" class:flush={!title}>
    {@render children()}
  </div>
  {#if footer}
    <div class="dialog-foot">
      {@render footer()}
    </div>
  {/if}
</dialog>
