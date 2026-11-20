<!--
  The page for a file that did not open, or an editor that could not start: what happened, whether the file changed and
  what to do, as the ErrorPages board asks. The detail is text from the server or the browser, drawn as plain text.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import type { ErrorPageView } from './failure';

  interface Props {
    page: ErrorPageView;
    onAction: () => void;
  }
  let { page, onAction }: Props = $props();
  let heading: HTMLElement | undefined = $state();
  // This page replaces the editor, so a reader is told (the alert) and the focus starts at the heading.
  onMount(() => heading?.focus());
</script>

<main data-testid="error-page" data-kind={page.kind} class="fixed inset-0 grid place-items-center overflow-auto bg-canvas p-8 font-sans text-body text-text">
  <div class="flex w-full max-w-md flex-col gap-3 rounded-lg border border-border bg-surface p-6 shadow-float">
    <div role="alert" class="flex flex-col gap-3">
      <h1 bind:this={heading} tabindex="-1" class="m-0 text-heading font-semibold text-text-strong outline-none">{page.title}</h1>
      <p class="m-0 text-text-secondary">{page.text}</p>
    </div>
    {#if page.detail}
      <p data-testid="error-detail" class="m-0 max-h-40 overflow-auto rounded-md bg-field p-3 font-mono text-small wrap-anywhere whitespace-pre-wrap text-text-muted">{page.detail}</p>
    {/if}
    {#if page.action}
      <div class="flex justify-end"><Button variant="primary" onclick={onAction}>{page.action}</Button></div>
    {/if}
  </div>
</main>
