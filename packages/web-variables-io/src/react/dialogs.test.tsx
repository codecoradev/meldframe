// The React dialogs of Export variables and Import variables. A server render shows only that a closed dialog draws nothing (the popup is a
// portal that opens on a prop); the rest of what a dialog does (the first focus, Tab, Escape, the scrim, where the focus goes back to, the
// text kept when it closes, the focus after an import) is the job of packages/web-variables-io/scripts/io-dialog-e2e.mjs and io-e2e.mjs with
// MELDFRAME_UI=react, which ask the Svelte and the React dialog the same questions. What is read here is the wiring that those scripts cannot
// see from outside: the words and test ids that the two copies share, the action a close sends, and the one place each copy takes its host calls from.
import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { ExportDialog } from './ExportDialog';
import { ImportDialog } from './ImportDialog';

const noop = () => {};
const source = (file: string) => readFileSync(new URL(file, import.meta.url), 'utf8');

describe('the React Export variables dialog', () => {
  it('draws nothing while the app layer holds no text, so the page holds no dialog', () => {
    expect(renderToStaticMarkup(<ExportDialog dialog={null} dispatch={noop} opener={() => null} />)).toBe('');
  });
  it('keeps the keys typed in it out of the editor, as every portaled popup does (ownskeys.test.ts)', () => {
    expect(source('./ExportDialog.tsx')).toMatch(/<Dialog\.Popup[^>]*data-owns-keys/);
  });
  it('names its title, description and close button as the Svelte dialog does, and tells the app layer when it closes', () => {
    const react = source('./ExportDialog.tsx');
    const svelte = source('../ExportVariablesDialog.svelte');
    for (const words of ['Export variables', 'Every variable of this file as CSS custom properties, to copy or save. Your file is not changed.', 'data-testid="export-vars-close"', 'data-testid="export-vars-dialog"', 'closeVariablesExport']) {
      expect(react, words).toContain(words);
      expect(svelte, words).toContain(words);
    }
  });
  it('asks the browser through host.ts only, as the Svelte copy does: no clipboard or file call of its own', () => {
    expect(source('./ExportDialog.tsx')).toContain("from '../host'");
    expect(source('../ExportVariablesDialog.svelte')).toContain("from './host'");
    for (const file of ['./ExportDialog.tsx', '../ExportVariablesDialog.svelte']) expect(source(file), file).not.toMatch(/navigator\.clipboard|URL\.createObjectURL|web-export\/save/);
  });
});

describe('the React Import variables dialog', () => {
  it('draws nothing while it is closed', () => {
    expect(renderToStaticMarkup(<ImportDialog open={false} result={null} source={null} dispatch={noop} opener={() => null} />)).toBe('');
  });
  it('keeps the keys typed in it out of the editor, as every portaled popup does (ownskeys.test.ts)', () => {
    expect(source('./ImportDialog.tsx')).toMatch(/<Dialog\.Popup[^>]*data-owns-keys/);
  });
  it('names its title, description and close button as the Svelte dialog does', () => {
    const react = source('./ImportDialog.tsx');
    const svelte = source('../ImportVariablesDialog.svelte');
    for (const words of ['Import variables', 'Add the variables of CSS text to this file. Paste the text or choose a file; one undo takes the import back.', 'data-testid="import-vars-close"', 'data-testid="import-vars-dialog"', 'importVariables']) {
      expect(react, words).toContain(words);
      expect(svelte, words).toContain(words);
    }
  });
  it('sends the close and the import actions that the Svelte dialog sends, and reads the file through read.ts', () => {
    const flow = source('../importflow.ts');
    expect(flow).toContain("type: 'closeVariablesImport'");
    expect(flow).toContain("type: 'clearVariablesImport'");
    expect(source('./ImportDialog.tsx')).toMatch(/type: 'importVariables', text: /);
    expect(source('./ImportDialog.tsx')).toContain("from '../read'");
  });
  it('words the note with the name the app layer made safe (importSource) and sends the raw name of a file that was read, in both copies (rule U9, issue 416)', () => {
    for (const file of ['./ImportDialog.tsx', '../ImportVariablesDialog.svelte']) expect(source(file), file).toContain('noteOf(view, source)');
    expect(source('../flow.ts')).toContain("type: 'chooseVariablesImportSource'");
    for (const file of ['../importflow.ts', '../ImportVariablesDialog.svelte']) expect(source(file), file).toContain('sendSource(');
    // No copy builds the note from the name the browser gave.
    for (const file of ['../flow.ts', '../importflow.ts', '../ImportVariablesDialog.svelte', './ImportDialog.tsx']) expect(source(file), file).not.toMatch(/Read \${[^}]*\.name/);
  });
});
