<!--
  What is inside the Export variables dialog (D8): the one-line summary, the preview of the CSS with line numbers, the help,
  what could not be written as stored, and Close, Copy and Download. It is a function of what it is given: the dialog of the
  app layer (`state.variablesIo.export`) and what the last Copy said, which the shell of the dialog keeps. Every text from
  the file is drawn as plain text, without the characters that reorder it.
-->
<script lang="ts">
  import type { VariablesExportDialog as Model } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import { findingRows, previewOf } from '@meldframe/web-export/view';
  import { EXPORT_FILE, EXPORT_HELP, exportProblem, exportSummary } from './io';

  interface Props {
    dialog: Model;
    /** What the last Copy said. */
    said: { ok: boolean; text: string } | null;
    onCopy: () => void;
    onDownload: () => void;
    onClose: () => void;
  }
  let { dialog, said, onCopy, onDownload, onClose }: Props = $props();

  const problem = $derived(exportProblem(dialog));
  const summary = $derived(exportSummary(dialog));
  const preview = $derived(previewOf(dialog.css));
  const empty = $derived(!problem && dialog.variables === 0);
  const warnings = $derived(findingRows(dialog.warnings));
</script>

<div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-4 pt-3">
  {#if problem}
    <p role="alert" data-testid="export-vars-error" class="m-0 rounded-md bg-danger-bg px-3 py-2 text-small text-danger-text">{problem}</p>
  {:else if empty}
    <p data-testid="export-vars-empty" class="m-0 text-text-muted">This file has no variables yet, so there is nothing to export.</p>
  {:else}
    <p class="m-0 font-mono text-small text-text-muted" data-testid="export-vars-summary">{summary}</p>
    <!-- A scrollable region must be reachable by keyboard. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="h-72 shrink-0 overflow-auto rounded-md border border-border bg-sidebar py-2 font-mono text-small leading-relaxed" data-testid="export-vars-code" tabindex="0" role="region" aria-label="{EXPORT_FILE} preview">
      {#each preview.lines as line, i (i)}
        <div class="flex w-max min-w-full whitespace-pre"><span aria-hidden="true" class="sticky left-0 w-12 shrink-0 bg-sidebar pr-3 text-right text-text-muted select-none">{i + 1}</span><span class="pr-4">{line}</span></div>
      {/each}
    </div>
    {#if preview.total > preview.lines.length}
      <p class="m-0 text-caption text-text-muted" data-testid="export-vars-cut">Showing the first {preview.lines.length} of {preview.total} lines. Copy and Download take all of it.</p>
    {/if}
  {/if}
  {#each EXPORT_HELP as line (line)}<p class="m-0 text-caption text-text-muted" data-testid="export-vars-help">{line}</p>{/each}
  {#if warnings.length}
    <section aria-label="Warnings" data-testid="export-vars-warnings" class="overflow-hidden rounded-md border border-border">
      <header class="flex h-(--mf-size-control-lg) items-center gap-2 bg-field px-3 text-small font-semibold text-text-strong">
        Warnings<span class="rounded-pill border border-border bg-warning-bg px-2 text-caption font-bold text-warning">{warnings.length}</span>
        <span class="ml-auto font-normal text-text-muted">Your file is unchanged.</span>
      </header>
      <ul class="m-0 max-h-32 list-none overflow-y-auto p-0">
        {#each warnings as r (r.key)}
          <li class="flex items-baseline gap-3 border-t border-hairline px-3 py-2 text-small">
            <span class="w-36 shrink-0 truncate font-mono text-warning">{r.code}</span>
            <span class="min-w-0 flex-1 text-text-secondary">{r.message}</span>
          </li>
        {/each}
      </ul>
    </section>
  {/if}
</div>
<div class="flex shrink-0 items-center justify-end gap-2 px-4 pt-3 pb-4">
  <span role="status" class="mr-auto text-small {said?.ok === false ? 'text-danger' : 'text-text-muted'}" data-testid="export-vars-said">{said?.text ?? ''}</span>
  <Button onclick={onClose} data-testid="export-vars-done">Close</Button>
  <Button disabled={!!problem || empty} onclick={onCopy} data-testid="export-vars-copy">Copy</Button>
  <Button variant="primary" disabled={!!problem || empty} onclick={onDownload} data-testid="export-vars-download">Download</Button>
</div>
