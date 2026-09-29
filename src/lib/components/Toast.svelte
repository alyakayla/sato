<script lang="ts">
  import { dismissToast, toast } from '$lib/stores.svelte';
</script>

{#if toast.value}
  <div class="toast {toast.value.kind}" role="status">
    <span class="mark">
      {toast.value.kind === 'ok' ? '✓' : toast.value.kind === 'warn' ? '!' : '✕'}
    </span>
    <span class="text">{toast.value.text}</span>
    {#if toast.value.action}
      <button
        class="action"
        onclick={() => {
          // Take the action before dismissing: the toast (and so its action)
          // is gone the moment it is dismissed.
          const run = toast.value?.action?.run;
          dismissToast();
          run?.();
        }}
      >
        <span aria-hidden="true">→</span>
        <span class="action-label">{toast.value.action.label}</span>
      </button>
    {/if}
  </div>
{/if}

<style>
  /* A toast floats, so it gets a shadow and no border - never both. */
  .toast {
    position: fixed;
    bottom: 18px;
    right: 18px;
    z-index: 200;
    display: flex;
    align-items: flex-start;
    gap: 9px;
    padding: 10px 14px;
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-md);
    font-size: 12.5px;
    max-width: 440px;
    animation: rise var(--dur) var(--ease);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }

  .ok {
    background: var(--ok-soft);
    color: var(--ok);
  }
  .danger {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .warn {
    background: var(--warn-soft);
    color: var(--warn);
  }

  .mark {
    font-weight: 700;
    flex-shrink: 0;
  }

  .text {
    line-height: 1.45;
    word-break: break-word;
  }

  /* The toast's one action: an arrow and an underlined word, in the toast's
     own colour, so it reads as part of the message rather than a button. */
  .action {
    display: inline-flex;
    align-items: baseline;
    gap: 5px;
    margin-left: 6px;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
    flex-shrink: 0;
  }
  .action-label {
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .action:hover .action-label {
    text-decoration-thickness: 2px;
  }
</style>
