<!--
  What is inside the Import variables dialog (D8): the field for the CSS text, the file chooser, the help, the result of the
  last import (what was added, or why nothing was) and its report, and Close and Import. It is a function of what it is
  given: the text of the field (the shell owns it), the note about the file, and the result of the app layer
  (`state.variablesIo.import`). A file is handed to `onFile`; reading it is the shell's. Every text from the file is
  drawn as plain text, without the characters that reorder it.
-->
<script lang="ts">
  import type { VariablesImportResult } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import { IMPORT_HELP, entryRows, importWords, omittedText } from './io';

  interface Props {
    result: VariablesImportResult | null;
    text: string;
    onText: (text: string) => void;
    /** What the last file chosen came to: read, or why not. */
    note: { ok: boolean; text: string } | null;
    onFile: (file: File) => void;
    onClear: () => void;
    onImport: () => void;
    onClose: () => void;
  }
  let { result, text, onText, note, onFile, onClear, onImport, onClose }: Props = $props();

  let chooser = $state<HTMLInputElement | null>(null);
  const words = $derived(result ? importWords(result) : null);
  const rows = $derived(result?.report ? entryRows(result.report.entries) : []);
  const omitted = $derived(result?.report ? omittedText(result.report.omitted) : null);
  const tone = { ok: 'bg-field text-text-strong', neutral: 'bg-field text-text', error: 'bg-danger-bg text-danger-text' } as const;
  const badge = { converted: 'bg-warning-bg text-warning', skipped: 'bg-field text-text-muted', note: 'bg-field text-text-secondary' } as const;
</script>

<div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-4 pt-3">
  <label for="import-vars-text" class="text-small font-semibold text-text-strong">CSS to import</label>
  <textarea
    id="import-vars-text"
    value={text}
    oninput={(e) => onText(e.currentTarget.value)}
    data-testid="import-vars-text"
    rows="10"
    spellcheck="false"
    placeholder="Paste CSS with a :root, .dark or @theme block"
    class="min-h-40 w-full resize-y rounded-md border border-divider bg-field p-3 font-mono text-small text-text hov:border-border-strong foc:border-selection foc:outline-(length:--mf-stroke-focus) foc:outline-offset-(--mf-stroke-focus-offset) foc:outline-selection"
    onkeydown={(e) => { if (e.key !== 'Escape') e.stopPropagation(); }}
  ></textarea>
  <div class="flex items-center gap-3">
    <input
      bind:this={chooser}
      type="file"
      accept=".css,text/css,text/plain"
      class="sr-only"
      tabindex="-1"
      aria-hidden="true"
      data-testid="import-vars-file"
      onchange={(e) => {
        const file = e.currentTarget.files?.[0];
        e.currentTarget.value = '';
        if (file) onFile(file);
      }}
    />
    <Button onclick={() => chooser?.click()} data-testid="import-vars-choose">Choose a file…</Button>
    <Button disabled={text === ''} onclick={onClear} data-testid="import-vars-clear">Clear</Button>
    <span role="status" class="min-w-0 flex-1 truncate text-small {note?.ok === false ? 'text-danger' : 'text-text-muted'}" data-testid="import-vars-note">{note?.text ?? ''}</span>
  </div>
  {#each IMPORT_HELP as line (line)}<p class="m-0 text-caption text-text-muted" data-testid="import-vars-help">{line}</p>{/each}
  <!-- One live region for the result, error or not: a status inside an alert would be read twice. -->
  <div role="status" aria-live="polite" data-testid="import-vars-result" data-status={result?.status ?? ''}>
    {#if words && result}
      <div class="flex flex-col gap-1 rounded-md px-3 py-2 text-small {tone[words.tone]}">
        <p class="m-0 flex items-center gap-2 font-semibold" data-testid="import-vars-title">{#if words.tone === 'ok'}<span class="size-2 shrink-0 rounded-pill bg-success" aria-hidden="true"></span>{/if}{words.title}</p>
        {#each words.lines as line (line)}<p class="m-0" data-testid="import-vars-line">{line}</p>{/each}
      </div>
      {#if rows.length}
        <section aria-label="Import report" data-testid="import-vars-report" class="mt-3 overflow-hidden rounded-md border border-border">
          <header class="flex h-(--mf-size-control-lg) items-center gap-2 bg-field px-3 text-small font-semibold text-text-strong">Report<span class="rounded-pill border border-border px-2 text-caption font-bold">{rows.length}</span></header>
          <ul class="m-0 max-h-48 list-none overflow-y-auto p-0">
            {#each rows as r (r.key)}
              <li class="flex items-baseline gap-3 border-t border-hairline px-3 py-2 text-small" data-kind={r.kind}>
                <span class="w-20 shrink-0 rounded-pill px-2 text-center text-caption font-semibold {badge[r.kind]}">{r.kind}</span>
                <span class="w-36 shrink-0 truncate font-mono text-text-strong" title={r.name}>{r.name}</span>
                <span class="min-w-0 flex-1 text-text-secondary">{r.message}</span>
              </li>
            {/each}
          </ul>
          {#if omitted}<p class="m-0 border-t border-hairline px-3 py-2 text-caption text-text-muted" data-testid="import-vars-omitted">{omitted}</p>{/if}
        </section>
      {/if}
    {/if}
  </div>
</div>
<div class="flex shrink-0 items-center justify-end gap-2 px-4 pt-3 pb-4">
  <Button onclick={onClose} data-testid="import-vars-done">Close</Button>
  <Button variant="primary" disabled={text.trim() === ''} onclick={onImport} data-testid="import-vars-import">Import</Button>
</div>
