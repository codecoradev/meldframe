// A React island in a Svelte template: one React root on one element, drawn again when the props change and
// unmounted when the element goes. It is how a panel can be React while the shell around it is still Svelte
// (docs/react-web-plan.md, option B). Every file that imports React is .tsx, whatever it holds.
import type { ComponentType, ReactNode } from 'react';
import { createElement } from 'react';
import { createRoot } from 'react-dom/client';

export interface IslandParams<P extends object> {
  component: ComponentType<P>;
  props: P;
}

/** What the helper needs of a React root; `react-dom/client`'s `Root` fits it. */
export interface RootLike {
  render(node: ReactNode): void;
  unmount(): void;
}

export interface IslandDeps {
  createRoot(container: HTMLElement): RootLike;
  /** Runs `fn` after the current call stack, and so after any React render that is in it, has finished. */
  defer(fn: () => void): void;
}

/**
 * Builds the Svelte action. Use `island` below; the seam is for tests, which cannot create a real root in Node.
 *
 *   <div use:island={{ component: Panel, props: { title } }}></div>
 *
 * The action draws on `use:`, again on each `update` (Svelte calls it when the params expression has another
 * value) and unmounts on `destroy`. The root belongs to the element, so two islands never share one, and an element
 * that its Svelte parent takes out of the page (an `{#if}`, an `{#each}` row) ends its root. The unmount is
 * deferred: React raises an error when a root is unmounted while another render is on the stack, and a
 * Svelte teardown can run inside a React event. A draw that follows a destroy is dropped.
 */
export function createIsland(deps: IslandDeps) {
  return function island<P extends object>(node: HTMLElement, params: IslandParams<P>) {
    const root = deps.createRoot(node);
    let alive = true;
    const draw = ({ component, props }: IslandParams<P>) => {
      if (alive) root.render(createElement(component, props));
    };
    draw(params);
    return {
      update: draw,
      destroy() {
        if (!alive) return;
        alive = false;
        deps.defer(() => root.unmount());
      },
    };
  };
}

export const island = createIsland({ createRoot, defer: (fn) => queueMicrotask(fn) });
