<!--
  What the last refusal, revert or restore said, in words (`messageFor`), as a status line at the top of the panel. The
  live region is always on the page and the text is put into it, because a region that appears together with its text
  is not announced.
-->
<script lang="ts">
  import type { Message } from './view';

  let { message }: { message: Message | null } = $props();
</script>

<div role="status" data-testid="activity-message">
  {#if message}
    <p
      data-tone={message.tone}
      class="m-4 mb-0 rounded-md px-3 py-2 text-small text-text-strong {message.tone === 'warning' ? 'bg-warning-bg' : 'bg-field'}"
    >{message.text}</p>
  {/if}
</div>
