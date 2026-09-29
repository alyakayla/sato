<script lang="ts">
  import { label } from '$lib/i18n/index.svelte';

  interface Props {
    status: string;
  }

  let { status }: Props = $props();

  const tone = $derived(
    status === 'Ready' || status === 'Closed'
      ? 'ok'
      : status === 'Indexing' || status === 'Pending' || status === 'On Hold'
        ? 'warn'
        : status === 'Failed'
          ? 'danger'
          : 'accent',
  );

  /** A dot only where the status is genuinely in flux; "Open" is a resting
   *  state, not a machine that is either running or broken. */
  const dot = $derived(tone !== 'accent');
</script>

<span class="badge badge-{tone}" class:badge-dot={dot}>{label(status)}</span>
