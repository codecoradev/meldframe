// React copy of ExportSection.svelte. The Export section of the inspector, as the Properties board draws it: for the
// one selected frame, the scale and Export as PNG, and the dialog for the HTML. Which frame can be exported is the app
// layer's (`state.export.target`); there is no section for any other selection.
import type { ExportTarget } from '@meldframe/protocol';
import { Button } from '@meldframe/web-chrome/react/Button';
import { Section } from '@meldframe/web-chrome/react/Section';
import { Segmented } from '@meldframe/web-chrome/react/Segmented';
import type { ExportScale } from '@meldframe/web-document/export';
import { type PngPlan, SCALE_OPTIONS, pngText, scaleOf } from '../png';

export interface ExportSectionProps {
  target: ExportTarget | null;
  scale: ExportScale;
  onScale: (s: ExportScale) => void;
  plan: PngPlan | null;
  onPng: () => void;
  onHtml: () => void;
}

export function ExportSection({ target, scale, onScale, plan, onPng, onHtml }: ExportSectionProps) {
  if (!target) return null;
  return (
    <Section title="Export" data-testid="export-section">
      <Segmented label="PNG scale" options={SCALE_OPTIONS} value={String(scale)} onChange={(v) => onScale(scaleOf(v))} data-testid="export-section-scale" />
      <Button variant="primary" disabled={!plan?.ok} onClick={onPng} data-testid="export-section-png">
        Export as PNG
      </Button>
      <p role="status" className={`m-0 text-caption ${plan?.ok === false && plan.why === 'too_large' ? 'text-danger-text' : 'font-mono text-text-muted'}`} data-testid="export-section-plan">
        {plan ? pngText(plan) : ''}
      </p>
      <Button onClick={onHtml} data-testid="export-section-html">
        HTML + Tailwind…
      </Button>
      <p className="m-0 text-caption text-text-muted">Exports the selected frame. An agent can export the same way through MCP.</p>
    </Section>
  );
}
