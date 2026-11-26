// Moving the focus in the manage view to something it drew: to a thing it just added, and back to the button that asked for a
// removal or a rename. Plain DOM and no framework, so that the Svelte and the React copy of the body call the same code. Each takes
// the element the view is drawn in (null before it is on the page: nothing happens) and finds its targets by the attributes the
// copies put on them (`data-entity-id`, `data-remove`, `data-rename` and the test ids of the titles).
import type { ManageTarget } from '@meldframe/protocol';

const TARGETS = '[data-entity-id], [data-remove], [data-rename], [data-testid=manage-title], [data-testid=manage-collection], [data-testid=manage-collections-title]';

const find = (root: ParentNode | null, match: (el: HTMLElement) => boolean): HTMLElement | undefined => [...(root?.querySelectorAll<HTMLElement>(TARGETS) ?? [])].find(match);

/** To the thing with this id, scrolled into view: the collection, mode or variable that was just added. */
export function focusEntity(root: ParentNode | null, id: string): void {
  const el = find(root, (e) => e.dataset.entityId === id);
  el?.scrollIntoView({ block: 'nearest' });
  el?.focus();
}

/** Back to the button that asked (`attr` says which: Remove or Rename); when that thing is gone, to the title of the collection, then to a collection, then to the heading of the list. */
export function focusAsker(root: ParentNode | null, a: ManageTarget, attr: 'remove' | 'rename' = 'remove'): void {
  const key = `${a.kind}:${a.id}`;
  (find(root, (e) => e.dataset[attr] === key) ?? find(root, (e) => e.dataset.testid === 'manage-title') ?? find(root, (e) => e.dataset.testid === 'manage-collection') ?? find(root, (e) => e.dataset.testid === 'manage-collections-title'))?.focus();
}
