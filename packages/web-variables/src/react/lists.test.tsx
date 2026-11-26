// The React copies of the lists and forms of the manage view (CollectionList, NameForm, RemoveConfirm, RenameField), checked as the
// Svelte ones are in manage-view.test.ts and rename-view.test.ts: the markup of a server render, in Node. Each is also compared
// with the Svelte copy's render, token by token (`tokens`: tags, attributes, classes, text), so that a class, a role, a name or a word
// that one copy has and the other does not fails here. What a server render cannot show (focus, the keys, the dialog around them) is the
// job of packages/web-variables/scripts/manage-e2e.mjs with MELDFRAME_UI=react, and the real pixels that of pixels-e2e.mjs.
import type { CollectionRow, ManageLimits, NameDraft, Removal, RenameField as RenameState, ManageTarget } from '@meldframe/protocol';
import { readFileSync } from 'node:fs';
import { renderToStaticMarkup } from 'react-dom/server';
import { render } from 'svelte/server';
import { describe, expect, it } from 'vitest';
import CollectionListSvelte from '../CollectionList.svelte';
import NameFormSvelte from '../NameForm.svelte';
import RemoveConfirmSvelte from '../RemoveConfirm.svelte';
import RenameFieldSvelte from '../RenameField.svelte';
import { removalWords } from '../manage';
import { CollectionList } from './CollectionList';
import { NameForm } from './NameForm';
import { RemoveConfirm } from './RemoveConfirm';
import { RenameField } from './RenameField';
import { tokens } from './markup.testutil';

describe('the token list that compares the two copies', () => {
  it('reads tags with sorted attributes and classes, text by words, and drops comments and empty classes', () => {
    expect(tokens('<!--[--><p class="b  a" id="x" disabled>one\n  two &gt; <i class="">3</i></p><!--]-->')).toEqual(['<p class="a b" disabled="" id="x">', 'one two >', '<i>', '3', '</i>', '</p>']);
  });
  it('tells two renders apart by a class, an attribute or a word', () => {
    const a = '<p class="a" title="t">x</p>';
    expect(tokens(a)).not.toEqual(tokens('<p class="a b" title="t">x</p>'));
    expect(tokens(a)).not.toEqual(tokens('<p class="a" title="u">x</p>'));
    expect(tokens(a)).not.toEqual(tokens('<p class="a" title="t">y</p>'));
  });
});

const noop = () => {};
const limits: ManageLimits = { nameMax: 256, modeNameMax: 64, collections: 32, modes: 16, variables: 5000 };
const modes = [{ id: 'm1', name: 'light' }, { id: 'm2', name: 'dark' }];
const col = (over: Partial<CollectionRow> = {}): CollectionRow => ({ id: 'c1', name: 'theme', modes, active: 'm1', matched: 2, variables: [], ...over });
const draft = (text: string, check: Partial<NameDraft['check']> = {}): NameDraft => ({ text, check: { name: text.trim(), ...check } });
const empty = draft('', { name: '', error: 'empty' });
const svelte = (component: unknown, props: Record<string, unknown>) => render(component as never, { props: props as never }).body;

const list = (collections: CollectionRow[], chosen: string | null, d: NameDraft = empty) =>
  renderToStaticMarkup(<CollectionList collections={collections} chosen={chosen} onChoose={noop} draft={d} limits={limits} onInput={noop} onAdd={noop} />);
const listS = (collections: CollectionRow[], chosen: string | null, d: NameDraft = empty) => svelte(CollectionListSvelte, { collections, chosen, onChoose: noop, draft: d, limits, onInput: noop, onAdd: noop });

describe('the React list of collections', () => {
  const many = [col(), col({ id: 'c2', name: 'brand', matched: 1, modes: [modes[0]!] })];
  it('has one button for each collection, the chosen one current, with its counts', () => {
    const out = list(many, 'c1');
    expect(out.match(/data-testid="manage-collection"/g)).toHaveLength(2);
    expect(out).toMatch(/aria-current="true"[^>]*data-id="c1"|data-id="c1"[^>]*aria-current="true"/);
    expect(out.match(/aria-current/g)).toHaveLength(1);
    expect(out).toContain('2 variables · 2 modes');
    expect(out).toContain('1 variable · 1 mode<');
  });
  it('says what to do in a file with no collection, and only then', () => {
    expect(list([], null)).toMatch(/data-testid="manage-no-collections"[^>]*>This file has no collections yet\. Add one to start\./);
    expect(list(many, 'c1')).not.toContain('manage-no-collections');
  });
  it('draws a name that shows as nothing as (no name), and markup in a name as text', () => {
    const out = list([col({ name: '' }), col({ id: 'c2', name: '<b>x</b>' })], 'c1');
    expect(out).toContain('(no name)');
    expect(out).toContain('&lt;b&gt;x&lt;/b&gt;');
    expect(out).not.toContain('<b>x');
  });
  it('is the same as the Svelte list: tags, attributes, classes and words', () => {
    for (const [c, chosen] of [[many, 'c1'], [many, 'c2'], [[], null], [[col({ name: '' })], 'c1']] as const) expect(tokens(list([...c], chosen))).toEqual(tokens(listS([...c], chosen)));
    const bad = draft('Colour', { error: 'notPath' });
    expect(tokens(list(many, 'c1', bad))).toEqual(tokens(listS(many, 'c1', bad)));
  });
});

const form = (kind: 'collection' | 'mode' | 'variable', d: NameDraft) =>
  renderToStaticMarkup(<NameForm kind={kind} label="New thing name" submit="Add thing" placeholder="color" draft={d} limits={limits} onInput={noop} onSubmit={noop} testId="t-new" />);
const formS = (kind: 'collection' | 'mode' | 'variable', d: NameDraft) => svelte(NameFormSvelte, { kind, label: 'New thing name', submit: 'Add thing', placeholder: 'color', draft: d, limits, onInput: noop, onSubmit: noop, testId: 't-new' });
const addOff = (out: string) => /\sdisabled(=|\s|>)/.test(out.match(/<button[^>]*data-testid="t-new-add"[^>]*>/)![0]);

describe('the React form that adds a name', () => {
  it('has a labelled field with the text it was given, Add off while the name cannot be sent, and a hint of the rule for a collection and a variable', () => {
    const out = form('variable', empty);
    expect(out).toContain('aria-label="New thing name"');
    expect(out).toMatch(/data-testid="t-new"/);
    expect(out).toContain('placeholder="color"');
    expect(addOff(out)).toBe(true);
    expect(out).toMatch(/data-testid="t-new-note"[^>]*data-tone="hint"[^>]*>Use lowercase letters, digits and single hyphens/);
    expect(form('mode', empty)).toMatch(/data-testid="t-new-note"[^>]*><\/p>/);
  });
  it('shows a refusal in the words of its code only once something was typed, with the field invalid and Add off', () => {
    const bad = form('collection', draft('Colour', { error: 'notPath' }));
    expect(bad).toMatch(/<input[^>]*value="Colour"/);
    expect(bad).toContain('aria-invalid="true"');
    expect(bad).toMatch(/data-testid="t-new-note"[^>]*data-tone="error"[^>]*>Use lowercase letters/);
    expect(addOff(bad)).toBe(true);
    expect(form('collection', draft('   ', { name: '', error: 'empty' }))).not.toContain('data-tone="error"');
  });
  it('says a name that exists already is a warning and leaves Add on', () => {
    const out = form('mode', draft('dark', { warning: 'same' }));
    expect(out).toMatch(/data-tone="warning"[^>]*>A mode with this name already exists in this collection\./);
    expect(addOff(out)).toBe(false);
    expect(out).not.toContain('aria-invalid');
  });
  it('is the same as the Svelte form for each kind and state', () => {
    const states = [empty, draft('colour'), draft('Colour', { error: 'notPath' }), draft('x'.repeat(300), { error: 'tooLong' }), draft('dark', { warning: 'same' }), draft('   ', { name: '', error: 'empty' })];
    for (const kind of ['collection', 'mode', 'variable'] as const) for (const d of states) expect(tokens(form(kind, d)), `${kind} ${d.text.slice(0, 8)}`).toEqual(tokens(formS(kind, d)));
  });
  it('keeps Enter for itself: it is read before the field sees it, with the keys of a composing input left alone', () => {
    const src = readFileSync(new URL('./NameForm.tsx', import.meta.url), 'utf8');
    expect(src).toMatch(/onKeyDownCapture/);
    expect(src).toMatch(/isComposing/);
  });
});

const asked = (over: Partial<Removal> = {}): Removal => ({ target: { kind: 'variable', id: 'v1' }, name: 'color/bg', detach: true, uses: 3, variables: 0, ...over });
const confirm = (plan = removalWords(asked())) => renderToStaticMarkup(<RemoveConfirm plan={plan} onConfirm={noop} onCancel={noop} />);

describe('the React question before a removal', () => {
  it('says what will happen and offers Detach or Cancel, as an alert dialog with a name', () => {
    const out = confirm();
    expect(out).toContain('3 uses');
    expect(out).toMatch(/>Detach</);
    expect(out).toMatch(/>Cancel</);
    expect(out).toContain('role="alertdialog"');
    expect(out).toContain('aria-label="Confirm the removal"');
    expect(out).toContain('aria-describedby="remove-confirm-text"');
    expect(out).toContain('bg-danger-strong');
  });
  it('offers Remove when nothing is let go', () => {
    expect(confirm(removalWords(asked({ detach: false, uses: 0 })))).toMatch(/data-testid="remove-confirm-yes"[^>]*>Remove</);
  });
  it('is the same as the Svelte question for a variable, a mode and a collection, with and without a detach', () => {
    const targets: ManageTarget[] = [{ kind: 'variable', id: 'v1' }, { kind: 'mode', collection: 'c1', id: 'm2' }, { kind: 'collection', id: 'c1' }];
    for (const target of targets) {
      for (const over of [{ target, detach: true, uses: 1, variables: 2 }, { target, detach: false, uses: 0, variables: 0 }]) {
        const plan = removalWords(asked(over));
        expect(tokens(confirm(plan))).toEqual(tokens(svelte(RemoveConfirmSvelte, { plan, onConfirm: noop, onCancel: noop })));
      }
    }
  });
});

const state = (target: ManageTarget, current: string, over: Partial<RenameState> = {}): RenameState => ({ target, opened: current, current, text: current, check: { name: current }, unchanged: true, first: false, ...over });
const target = (kind: 'variable' | 'collection' | 'mode') => ({ kind, id: 'x', ...(kind === 'mode' ? { collection: 'c1' } : {}) }) as ManageTarget;
const field = (kind: 'variable' | 'collection' | 'mode', f: RenameState) =>
  renderToStaticMarkup(<RenameField kind={kind} subject={`${kind} ${f.current}`} field={f} limits={limits} onInput={noop} onSubmit={noop} onCancel={noop} testId="rename-field" />);
const fieldS = (kind: 'variable' | 'collection' | 'mode', f: RenameState) => svelte(RenameFieldSvelte, { kind, subject: `${kind} ${f.current}`, field: f, limits, onInput: noop, onSubmit: noop, onCancel: noop, testId: 'rename-field' });

describe('the React field of a rename', () => {
  it('has the field with the name as it is stored, Save and Cancel named for their target, and what a rename does to the export', () => {
    const out = field('variable', state(target('variable'), 'color/bg'));
    expect(out).toMatch(/<input[^>]*value="color\/bg"/);
    expect(out).toContain('aria-label="New name of variable color/bg"');
    expect(out).toContain('aria-label="Save the new name of variable color/bg"');
    expect(out).toContain('aria-label="Cancel renaming variable color/bg"');
    expect(out).toMatch(/data-tone="hint"[^>]*>Use lowercase letters[^<]*The next export uses the new name\./);
  });
  it('words a refusal only for text that is a change, a duplicate as a warning, and the first mode as the default one', () => {
    const bad = state(target('variable'), 'color/bg', { text: 'Color', check: { name: 'Color', error: 'notPath' }, unchanged: false });
    expect(field('variable', bad)).toMatch(/data-tone="error"/);
    expect(field('variable', { ...bad, unchanged: true })).toMatch(/data-tone="hint"/);
    expect(field('mode', state(target('mode'), 'dark', { text: 'default', check: { name: 'default', warning: 'same' }, unchanged: false }))).toMatch(/data-tone="warning"/);
    expect(field('mode', state(target('mode'), 'default', { first: true }))).toContain('The first mode is the default one');
  });
  it('has a status line that is always there, with the words of a name that changed under the field', () => {
    expect(field('variable', state(target('variable'), 'color/bg'))).toMatch(/role="status"[^>]*data-testid="rename-field-moved"[^>]*><\/p>/);
    const out = field('variable', state(target('variable'), 'color/bg', { current: 'color/page' }));
    expect(out).toContain('This name changed to “color/page” while you were editing.');
  });
  it('is the same as the Svelte field for each kind and state', () => {
    for (const kind of ['variable', 'collection', 'mode'] as const) {
      const base = state(target(kind), 'name');
      const states = [
        base,
        { ...base, first: kind === 'mode' },
        { ...base, text: 'Bad Name', check: { name: 'Bad Name', error: 'notPath' as const }, unchanged: false },
        { ...base, text: 'taken', check: { name: 'taken', warning: 'same' as const }, unchanged: false },
        { ...base, current: 'moved' },
        { ...base, text: '', check: { name: '', error: 'empty' as const }, unchanged: false },
      ];
      for (const f of states) expect(tokens(field(kind, f)), `${kind} ${f.text}`).toEqual(tokens(fieldS(kind, f)));
    }
  });
});
