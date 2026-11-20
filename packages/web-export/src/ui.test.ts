import { describe, expect, it } from 'vitest';
import { START, afterFormat, dialogFrame, reviewSteps, viewFor } from './ui';

describe('the view of the dialog starts again for every dialog', () => {
  const used = { tab: 'png' as const, shown: 'css' as const, said: { ok: true, text: 'x.html copied.' } };

  it('opening a dialog starts from the HTML tab, the HTML file and nothing said', () => {
    expect(viewFor({ frame: null, view: used }, 'f1')).toEqual({ frame: 'f1', view: START });
  });

  it('closing resets it too, so the next dialog does not show the last one\'s tab', () => {
    const closed = viewFor({ frame: 'f1', view: used }, null);
    expect(closed).toEqual({ frame: null, view: START });
    expect(viewFor(closed, 'f1').view).toEqual(START);
  });

  it('a dialog on another frame starts again; the same frame keeps what the person did', () => {
    expect(viewFor({ frame: 'f1', view: used }, 'f2')).toEqual({ frame: 'f2', view: START });
    const same = viewFor({ frame: 'f1', view: used }, 'f1');
    expect(same.view).toBe(used);
  });

  it('a change of the format shows the HTML file again and forgets what the last copy said', () => {
    expect(afterFormat(used)).toEqual({ tab: 'png', shown: 'html', said: null });
  });
});

describe('what the dialog acts on', () => {
  it('the PNG is of the dialog\'s frame, not of what is selected now', () => {
    expect(dialogFrame({ frame: 'f1', name: 'Landing' })).toEqual({ id: 'f1', name: 'Landing' });
    expect(dialogFrame(null)).toBeNull();
  });

  it('Review closes the dialog and opens the right panel, in that order', () => {
    expect(reviewSteps()).toEqual([{ type: 'closeExport' }, { type: 'setPanel', panel: 'properties', open: true }]);
  });
});
