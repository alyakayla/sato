<script lang="ts">
  import { tick } from 'svelte';
  import Modal from './Modal.svelte';
  import { t } from '$lib/i18n/index.svelte';
  import { confirmDialog, resolveDelete } from '$lib/confirm.svelte';

  /**
   * The one delete confirmation. Focus starts on Cancel, so an Enter pressed
   * out of habit never deletes anything; Esc cancels.
   */

  let cancelBtn = $state<HTMLButtonElement | null>(null);

  $effect(() => {
    if (!confirmDialog.open) return;
    void tick().then(() => cancelBtn?.focus());
  });
</script>

<Modal open={confirmDialog.open} title={confirmDialog.title} onclose={() => resolveDelete(false)} width="440px">
  <p class="msg">{confirmDialog.message}</p>
  <p class="warn">{t('confirm.irreversible')}</p>

  <label class="dont-ask">
    <input type="checkbox" bind:checked={confirmDialog.dontAskAgain} />
    <span>{t('confirm.dontAsk')}</span>
  </label>

  {#snippet footer()}
    <button class="btn" bind:this={cancelBtn} onclick={() => resolveDelete(false)}>{t('common.cancel')}</button>
    <button class="btn btn-danger solid" onclick={() => resolveDelete(true)}>{t('common.delete')}</button>
  {/snippet}
</Modal>

<style>
  .msg {
    margin: 0 0 6px;
    font-size: 13.5px;
    line-height: 1.55;
    color: var(--text);
  }
  .warn {
    margin: 0 0 14px;
    font-size: 12.5px;
    color: var(--text-secondary);
  }
  .dont-ask {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12.5px;
    color: var(--text-secondary);
    cursor: pointer;
    user-select: none;
  }
  .dont-ask input {
    width: 14px;
    height: 14px;
    margin: 0;
    accent-color: var(--ink);
    cursor: pointer;
  }
  /* The irreversible choice gets the one semantic fill in the dialog. */
  .solid {
    background: var(--danger-soft);
    border-color: var(--danger-border);
  }
  .solid:hover:not(:disabled) {
    background: var(--danger-soft);
    border-color: var(--danger);
  }
</style>
