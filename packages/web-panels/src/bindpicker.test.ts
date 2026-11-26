import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { composeProps } from '@meldframe/web-chrome/compose';

const source = readFileSync(new URL('./BindPicker.svelte', import.meta.url), 'utf8');

describe('the bind button of a field opens the picker with one click', () => {
  // The trigger of a popover hands its button an `onclick` that toggles it, and `composeProps` chains every handler
  // it is given: a second `onclick` that does the same thing runs the toggle twice, so a click opened the picker
  // and closed it again at once (the keyboard worked, the mouse never did).
  it('chains two handlers, so a button handed the trigger twice would toggle twice', () => {
    let toggles = 0;
    const trigger = { onclick: () => toggles++ };
    (composeProps(trigger, { onclick: trigger.onclick }).onclick as () => void)();
    expect(toggles).toBe(2);
  });
  it('hands the button the trigger props once and no handler of its own', () => {
    const button = source.match(/<IconButton[^>]*>/s)?.[0] ?? '';
    expect(button).toContain('triggerProps={props}');
    expect(button).not.toMatch(/\bonclick\b/);
    expect(source.match(/props\.onclick/g) ?? []).toHaveLength(0);
  });
});
