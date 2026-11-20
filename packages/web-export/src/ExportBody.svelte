<!--
  What is inside the Export dialog (D9): the tabs, the HTML tab (style, file, the preview with line numbers, the report,
  Copy and Download) and the PNG tab. It is a function of what it is given: the dialog of the app layer, the view
  (tab, file shown, what the last Copy said) that the shell of the dialog keeps, and the PNG plan of the dialog's frame.
  Every text from the file (names, the report) is drawn as plain text, without the characters that reorder it.
-->
<script lang="ts">
  import type { Action, ExportDialog as Model } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import Segmented from '@meldframe/web-chrome/Segmented.svelte';
  import Tabs from '@meldframe/web-chrome/Tabs.svelte';
  import type { ExportScale } from '@meldframe/web-document/export';
  import { type PngPlan, pngText } from './png';
  import { copyText, saveText } from './save';
  import { type View, afterFormat } from './ui';
  import { errorText, fileOf, findingRows, plain, previewOf, reviewText, sizeText } from './view';

  interface Props {
    dialog: Model;
    /** `state.activity.unreviewedCount`. */
    unreviewed: number;
    view: View;
    onView: (v: View) => void;
    scale: ExportScale;
    onScale: (s: ExportScale) => void;
    /** The PNG of the dialog's frame as the canvas would draw it now (null: nothing to draw). */
    plan: PngPlan | null;
    onPng: () => void;
    dispatch: (a: Action) => void;
    /** The dialog closes and the page opens the Activity tab. */
    onReview: () => void;
    onClose: () => void;
  }
  let { dialog, unreviewed, view, onView, scale, onScale, plan, onPng, dispatch, onReview, onClose }: Props = $props();

  // A plain CSS form has the stylesheet inside the HTML and no second file.
  const file = $derived(fileOf(dialog, dialog.format === 'tailwind' && view.shown === 'css' ? 'css' : 'html'));
  const preview = $derived(previewOf(file.text));
  const problem = $derived(errorText(dialog));
  const review = $derived(reviewText(unreviewed));

  async function copy() {
    const said = (await copyText(file.text)) ? { ok: true, text: `${file.name} copied.` } : { ok: false, text: 'Your browser did not allow copying.' };
    onView({ ...view, said });
  }
</script>

{#snippet report(title: string, rows: ReturnType<typeof findingRows>, note: string, id: string)}
  {#if rows.length}
    <section aria-label={title} data-testid={id} class="overflow-hidden rounded-md border border-border">
      <header class="flex h-(--mf-size-control-lg) items-center gap-2 bg-field px-3 text-small font-semibold text-text-strong">
        {title}<span class="rounded-pill border border-border bg-warning-bg px-2 text-caption font-bold text-warning">{rows.length}</span>
        <span class="ml-auto font-normal text-text-muted">{note}</span>
      </header>
      <ul class="m-0 max-h-32 list-none overflow-y-auto p-0">
        {#each rows as r (r.key)}
          <li class="flex items-baseline gap-3 border-t border-hairline px-3 py-2 text-small">
            <span class="w-32 shrink-0 truncate font-medium text-text-strong" title={r.node}>{r.node}</span>
            <span class="w-36 shrink-0 truncate font-mono text-warning">{r.code}</span>
            <span class="min-w-0 flex-1 text-text-secondary">{r.message}</span>
          </li>
        {/each}
      </ul>
    </section>
  {/if}
{/snippet}

<div class="flex shrink-0 px-4 pt-3">
  <Tabs label="Export format" idPrefix="export" selected={view.tab} onSelect={(t) => onView({ ...view, tab: t === 'png' ? 'png' : 'html' })} tabs={[{ id: 'html', label: 'HTML + Tailwind' }, { id: 'png', label: 'PNG' }]} />
</div>

{#if view.tab === 'html'}
  <div id="export-panel-html" role="tabpanel" aria-labelledby="export-tab-html" class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto px-4 pt-3">
    <div class="flex items-center gap-3">
      <Segmented label="Style" options={[{ value: 'tailwind', label: 'Tailwind' }, { value: 'css', label: 'Plain CSS' }]} value={dialog.format} onChange={(v) => { onView(afterFormat(view)); dispatch({ type: 'setExportFormat', format: v === 'css' ? 'css' : 'tailwind' }); }} data-testid="export-format" />
      {#if dialog.format === 'tailwind' && !problem}
        <Segmented label="File" options={[{ value: 'html', label: 'HTML' }, { value: 'css', label: plain(dialog.cssFile) }]} value={view.shown} onChange={(v) => onView({ ...view, shown: v === 'css' ? 'css' : 'html', said: null })} data-testid="export-file" />
      {/if}
      {#if !problem}<span class="ml-auto font-mono text-small text-text-muted">{sizeText(file.bytes)}</span>{/if}
    </div>
    {#if problem}
      <p role="alert" data-testid="export-error" class="m-0 rounded-md bg-danger-bg px-3 py-2 text-small text-danger-text">{problem}</p>
    {:else}
      <!-- A scrollable region must be reachable by keyboard. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div class="h-72 shrink-0 overflow-auto rounded-md border border-border bg-sidebar py-2 font-mono text-small leading-relaxed" data-testid="export-code" tabindex="0" role="region" aria-label="{plain(file.name)} preview">
        {#each preview.lines as line, i (i)}
          <div class="flex w-max min-w-full whitespace-pre"><span aria-hidden="true" class="sticky left-0 w-12 shrink-0 bg-sidebar pr-3 text-right text-text-muted select-none">{i + 1}</span><span class="pr-4">{line}</span></div>
        {/each}
      </div>
      {#if preview.total > preview.lines.length}
        <p class="m-0 text-caption text-text-muted" data-testid="export-cut">Showing the first {preview.lines.length} of {preview.total} lines. Copy and Download take all of it.</p>
      {/if}
      <p class="m-0 text-caption text-text-muted">The HTML uses the Inter font file inter.woff2 beside it{dialog.format === 'tailwind' ? `, and links ${plain(dialog.cssFile)}, the input of the Tailwind v4 compiler` : ''}. Text sizes are estimated here, not measured in a browser, so a text can break differently there.</p>
    {/if}
    {@render report('Unsupported properties', findingRows(dialog.unsupported), 'Left out or approximated. Your file is unchanged.', 'export-unsupported')}
    {@render report('Warnings', findingRows(dialog.warnings), 'Your file is unchanged.', 'export-warnings')}
  </div>
  <div class="flex shrink-0 flex-col gap-3 px-4 pt-3 pb-4">
    {#if review}
      <div role="status" data-testid="export-review" class="flex items-center gap-3 rounded-md bg-review-bg px-3 py-2 text-small text-review-text">
        <span class="size-2 shrink-0 rounded-pill bg-agent-1"></span><span class="flex-1">{review}</span>
        <Button size="sm" onclick={onReview}>Review</Button>
      </div>
    {/if}
    <div class="flex items-center justify-end gap-2">
      <span role="status" class="mr-auto text-small {view.said?.ok === false ? 'text-danger' : 'text-text-muted'}" data-testid="export-said">{view.said?.text ?? ''}</span>
      <Button onclick={onClose}>Close</Button>
      <Button disabled={!!problem} onclick={copy} data-testid="export-copy">Copy</Button>
      <Button variant="primary" disabled={!!problem} onclick={() => saveText(file.text, file.name, file.mime)} data-testid="export-download">Download</Button>
    </div>
  </div>
{:else}
  <div id="export-panel-png" role="tabpanel" aria-labelledby="export-tab-png" class="flex flex-col gap-3 px-4 pt-4 pb-4">
    <div class="flex items-center justify-between"><span class="font-semibold text-text-strong">Scale</span><Segmented label="Scale" options={[{ value: '1', label: '1x' }, { value: '2', label: '2x' }]} value={String(scale)} onChange={(v) => onScale(v === '2' ? 2 : 1)} data-testid="export-scale" /></div>
    <p class="m-0 font-mono text-small text-text-muted" data-testid="export-png-plan">{plan ? pngText(plan) : 'There is nothing to export.'}</p>
    <div class="flex justify-end gap-2">
      <Button onclick={onClose}>Close</Button>
      <Button variant="primary" disabled={!plan?.ok} onclick={onPng} data-testid="export-png">Export as PNG</Button>
    </div>
  </div>
{/if}
