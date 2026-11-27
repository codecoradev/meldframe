import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

// The three dialogs the Variables buttons open (the manage view, Import variables, Export variables) are open by the state of the app layer, and
// the editor page draws them beside the Export dialog, always mounted: a dialog drawn inside the Variables panel went when the panel did (Option+2
// folds the right card, and an Alt key is not owned by a dialog), leaving `manage.open` set with nothing on the screen. The browser checks are
// `manage-dialog-e2e.mjs` and `io-dialog-e2e.mjs`; this reads where each dialog is drawn.
const source = (file: string) => readFileSync(new URL(file, import.meta.url), 'utf8');
const editor = source('./Editor.svelte');

describe('where the dialogs of the Variables section are drawn', () => {
  it('draws the manage view, Import variables and Export variables in the editor page, from the app layer state, after the layout', () => {
    const afterLayout = editor.slice(editor.indexOf('</Layout>'));
    expect(afterLayout).toContain('<ManageDialog open={snap.state.manage.open}');
    expect(afterLayout).toContain('<ImportVariablesDialog open={snap.state.variablesIo.importOpen}');
    expect(afterLayout).toContain('<ExportVariablesDialog dialog={snap.state.variablesIo.export ?? null}');
  });
  it('finds the button each gives the focus back to by its test id, in whichever panel is drawn', () => {
    for (const id of ['var-manage', 'var-import', 'var-export']) expect(editor).toContain(`[data-testid=${id}]`);
  });
  it('draws none in the panel switch', () => {
    expect(source('./react-pilot/VariablesPanelSwitch.svelte').replace(/<!--[\s\S]*?-->/g, '')).not.toMatch(/Dialog/);
  });
});
