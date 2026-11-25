// React copy of StateMatrix.svelte. The state matrix page (/?states&react): every control of the chrome in its six
// states, in dark and light, as the StateMatrix and Accessibility boards draw them. Hover, pressed and focus are drawn
// with `data-preview` on a wrapper (the variants of tailwind.css answer to it), so the page needs no pointer; every
// enabled control is a real, focusable control.
import type { ReactNode } from 'react';
import { Button } from './Button';
import { Field } from './Field';
import { IconButton } from './IconButton';
import { Section } from './Section';
import { Segmented } from './Segmented';
import { Tabs } from './Tabs';
import { TooltipProvider } from './Tooltip';

const states = ['default', 'hover', 'pressed', 'focus', 'disabled', 'selected'] as const;
type State = (typeof states)[number];
const themes = ['dark', 'light'] as const;
const preview = (s: State) => (s === 'hover' || s === 'pressed' || s === 'focus' ? s : undefined);
const noop = () => {};
const seg = [{ value: 'a', label: 'Layers' }];
const tab = [{ id: 'design', label: 'Design' }];

type Cell = (s: State, theme: string) => ReactNode;

const cells: Record<string, Cell> = {
  primary: (s) => (
    <Button variant="primary" disabled={s === 'disabled'} onClick={noop}>
      Save
    </Button>
  ),
  secondary: (s) => (
    <Button disabled={s === 'disabled'} onClick={noop}>
      Cancel
    </Button>
  ),
  destructive: (s) => (
    <Button variant="destructive" disabled={s === 'disabled'} onClick={noop}>
      Revoke
    </Button>
  ),
  icon: (s) => <IconButton label="Layers panel" icon="panel-left" pressed={s === 'selected'} disabled={s === 'disabled'} onClick={noop} />,
  field: (s) => <Field label="Width" prefix="W" value="240" disabled={s === 'disabled'} />,
  bound: (s) => <Field label="Spacing variable" value="space/md · 16" bound disabled={s === 'disabled'} />,
  segmented: (s) => <Segmented label="Panel" options={seg} value={s === 'selected' ? 'a' : ''} onChange={noop} disabled={s === 'disabled'} />,
  fold: (s) => (
    <Section title="Fill" open={s === 'selected'} onToggle={noop}>
      <span className="text-text-muted">Body</span>
    </Section>
  ),
  tabs: (s, t) => <Tabs label="Right panel" idPrefix={`m-${t}-${s}`} tabs={tab} selected={s === 'selected' ? 'design' : ''} onSelect={noop} />,
};

function Row({ title, note, cell, theme, skip = [] }: { title: string; note: string; cell: Cell; theme: string; skip?: State[] }) {
  return (
    <tr className="border-t border-hairline">
      <th scope="row" className="px-4 py-3 text-left align-middle">
        <b className="block text-small font-semibold text-text-strong">{title}</b>
        <span className="text-caption font-normal text-text-muted">{note}</span>
      </th>
      {states.map((s) => (
        <td key={s} className="px-4 py-3 align-middle">
          {skip.includes(s) ? (
            <span className="text-caption text-text-muted">not applicable</span>
          ) : (
            <span data-preview={preview(s)} className="contents">
              {cell(s, theme)}
            </span>
          )}
        </td>
      ))}
    </tr>
  );
}

export function StateMatrix() {
  return (
    <TooltipProvider>
      <main className="font-sans text-body text-text">
        {themes.map((t) => (
          <section key={t} data-theme={t} aria-label={`States, ${t}`} className="bg-canvas p-8 text-text">
            <h1 className="m-0 mb-1 text-display font-bold tracking-tight text-text-strong">Component states, {t}</h1>
            <p className="m-0 mb-4 max-w-3xl text-text-muted">Default, hover, pressed, focus, disabled and selected. Hover, pressed and focus are drawn, not live; Tab through the page for the real focus ring.</p>
            <table className="w-full table-fixed border-collapse overflow-hidden rounded-lg border border-border bg-sidebar">
              <thead>
                <tr>
                  <td className="w-(--mf-size-layers)"></td>
                  {states.map((s) => (
                    <th key={s} scope="col" className="px-4 py-2 text-left text-caption font-semibold text-text-muted">
                      {s}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                <Row title="Primary button" note="accent-strong, on-accent" cell={cells.primary!} theme={t} skip={['selected']} />
                <Row title="Secondary button" note="control" cell={cells.secondary!} theme={t} skip={['selected']} />
                <Row title="Destructive button" note="danger-strong" cell={cells.destructive!} theme={t} skip={['selected']} />
                <Row title="Icon button" note="pressed = panel open" cell={cells.icon!} theme={t} />
                <Row title="Field" note="field, outline divider" cell={cells.field!} theme={t} skip={['selected']} />
                <Row title="Variable-bound field" note="chip colours" cell={cells.bound!} theme={t} skip={['selected']} />
                <Row title="Segmented control" note="selected = control-selected" cell={cells.segmented!} theme={t} />
                <Row title="Section toggle" note="selected = open" cell={cells.fold!} theme={t} skip={['disabled']} />
                <Row title="Tab" note="selected = hairline fill" cell={cells.tabs!} theme={t} skip={['disabled']} />
              </tbody>
            </table>
          </section>
        ))}
      </main>
    </TooltipProvider>
  );
}
