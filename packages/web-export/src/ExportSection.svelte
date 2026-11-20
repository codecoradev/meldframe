<!--
  The Export section of the inspector, as the Properties board draws it: for the one selected frame, the scale and
  Export as PNG, and the dialog for the HTML. Which frame can be exported is the app layer's (`state.export.target`);
  there is no section for any other selection.
-->
<script lang="ts">
  import type { ExportTarget } from '@meldframe/protocol';
  import Button from '@meldframe/web-chrome/Button.svelte';
  import Section from '@meldframe/web-chrome/Section.svelte';
  import Segmented from '@meldframe/web-chrome/Segmented.svelte';
  import type { ExportScale } from '@meldframe/web-document/export';
  import { type PngPlan, pngText } from './png';

  interface Props {
    target: ExportTarget | null;
    scale: ExportScale;
    onScale: (s: ExportScale) => void;
    plan: PngPlan | null;
    onPng: () => void;
    onHtml: () => void;
  }
  let { target, scale, onScale, plan, onPng, onHtml }: Props = $props();
</script>

{#if target}
  <Section title="Export" data-testid="export-section">
    <div class="flex items-center justify-between gap-2">
      <Segmented label="PNG scale" options={[{ value: '1', label: '1x' }, { value: '2', label: '2x' }]} value={String(scale)} onChange={(v) => onScale(v === '2' ? 2 : 1)} />
      <Button variant="primary" disabled={!plan?.ok} onclick={onPng} data-testid="export-section-png">Export as PNG</Button>
    </div>
    <p class="m-0 font-mono text-caption text-text-muted" data-testid="export-section-plan">{plan ? pngText(plan) : ''}</p>
    <Button onclick={onHtml} data-testid="export-section-html">HTML + Tailwind…</Button>
    <p class="m-0 text-caption text-text-muted">Exports the selected frame. An agent can export the same way through MCP.</p>
  </Section>
{/if}
