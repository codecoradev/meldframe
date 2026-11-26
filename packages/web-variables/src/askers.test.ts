import { describe, expect, it } from 'vitest';
import { focusAsker, focusEntity } from './askers';

// The two ways the manage view moves the focus to something it drew: to a thing it just added, and back to the button that asked.
// Elements are stubs: the Node tests have no DOM, so a root answers `querySelectorAll` with these and each records what was done to it.
const el = (data: Record<string, string>) => {
  const calls: string[] = [];
  return { dataset: data, focus: () => calls.push('focus'), scrollIntoView: () => calls.push('scroll'), calls };
};
const rootOf = (...els: ReturnType<typeof el>[]) => ({ querySelectorAll: () => els }) as unknown as ParentNode;

describe('the focus goes to a thing that was just added', () => {
  it('scrolls it into view and focuses it', () => {
    const a = el({ entityId: 'v1' });
    const b = el({ entityId: 'v2' });
    focusEntity(rootOf(a, b), 'v2');
    expect(b.calls).toEqual(['scroll', 'focus']);
    expect(a.calls).toEqual([]);
  });
  it('does nothing when the thing is not on the page, or there is no root', () => {
    const a = el({ entityId: 'v1' });
    focusEntity(rootOf(a), 'gone');
    focusEntity(null, 'v1');
    expect(a.calls).toEqual([]);
  });
});

describe('the focus goes back to the button that asked', () => {
  const remove = el({ remove: 'variable:v1' });
  const rename = el({ rename: 'variable:v1' });
  const title = el({ testid: 'manage-title' });
  const collection = el({ testid: 'manage-collection' });
  const heading = el({ testid: 'manage-collections-title' });
  const all = () => rootOf(heading, collection, title, rename, remove);
  const reset = () => [remove, rename, title, collection, heading].forEach((e) => (e.calls.length = 0));
  it('finds the Remove button of the thing, or its Rename button when asked', () => {
    reset();
    focusAsker(all(), { kind: 'variable', id: 'v1' });
    expect(remove.calls).toEqual(['focus']);
    focusAsker(all(), { kind: 'variable', id: 'v1' }, 'rename');
    expect(rename.calls).toEqual(['focus']);
  });
  it('names a mode by its own id, as the buttons do', () => {
    const mode = el({ remove: 'mode:m2' });
    focusAsker(rootOf(mode), { kind: 'mode', collection: 'c1', id: 'm2' });
    expect(mode.calls).toEqual(['focus']);
  });
  it('falls back, when that thing is gone, to the title of the collection, then a collection, then the heading of the list', () => {
    reset();
    focusAsker(rootOf(heading, collection, title), { kind: 'variable', id: 'gone' });
    expect(title.calls).toEqual(['focus']);
    reset();
    focusAsker(rootOf(heading, collection), { kind: 'variable', id: 'gone' });
    expect(collection.calls).toEqual(['focus']);
    reset();
    focusAsker(rootOf(heading), { kind: 'variable', id: 'gone' });
    expect(heading.calls).toEqual(['focus']);
    focusAsker(null, { kind: 'variable', id: 'v1' });
  });
});
