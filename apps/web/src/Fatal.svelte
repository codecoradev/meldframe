<script lang="ts">
  let { message }: { message: string } = $props();
</script>

<div role="alert" data-testid="fatal" class="fixed inset-0 grid place-items-center bg-canvas p-8 font-sans text-body text-danger">
  <p class="max-w-prose whitespace-pre-wrap">{message}</p>
</div>
