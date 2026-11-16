<script lang="ts">
  import type { Action, Properties } from '@meldframe/protocol';
  import { parseNumber } from '../input';

  let { properties, dispatch }: { properties: Properties | null; dispatch: (a: Action) => unknown } = $props();
  /** Run `go` with the typed number; a field that is empty or not a number goes back to the value it had. */
  const withNum = (e: Event, was: number, go: (n: number) => void) => {
    const input = e.currentTarget as HTMLInputElement;
    const n = parseNumber(input.value);
    if (n === null) input.value = String(was);
    else go(n);
  };
  const str = (e: Event) => (e.currentTarget as HTMLInputElement).value;
</script>

<aside data-testid="properties">
  <h2>Properties</h2>
  {#if properties}
    {@const p = properties}
    <label>name <input data-testid="prop-name" value={p.name} onchange={(e) => dispatch({ type: 'setProperty', id: p.id, property: { property: 'name', value: str(e) } })} /></label>
    <label>x <input data-testid="prop-x" type="number" value={p.x} onchange={(e) => withNum(e, p.x, (x) => dispatch({ type: 'move', id: p.id, x, y: p.y }))} /></label>
    <label>y <input data-testid="prop-y" type="number" value={p.y} onchange={(e) => withNum(e, p.y, (y) => dispatch({ type: 'move', id: p.id, x: p.x, y }))} /></label>
    <label>w <input data-testid="prop-w" type="number" value={p.w} onchange={(e) => withNum(e, p.w, (w) => dispatch({ type: 'resize', id: p.id, w, h: p.h }))} /></label>
    <label>h <input data-testid="prop-h" type="number" value={p.h} onchange={(e) => withNum(e, p.h, (h) => dispatch({ type: 'resize', id: p.id, w: p.w, h }))} /></label>
    <label>fill <input data-testid="prop-fill" type="color" value={p.fill} onchange={(e) => dispatch({ type: 'setProperty', id: p.id, property: { property: 'fill', value: str(e) } })} /></label>
    <label>radius <input data-testid="prop-radius" type="number" value={p.radius} onchange={(e) => withNum(e, p.radius, (value) => dispatch({ type: 'setProperty', id: p.id, property: { property: 'radius', value } }))} /></label>
    {#if p.kind === 'text'}
      <label>text <input data-testid="prop-text" value={p.text} onchange={(e) => dispatch({ type: 'editText', id: p.id, text: str(e) })} /></label>
      <label>size <input data-testid="prop-font-size" type="number" value={p.fontSize} onchange={(e) => withNum(e, p.fontSize, (value) => dispatch({ type: 'setProperty', id: p.id, property: { property: 'fontSize', value } }))} /></label>
    {/if}
  {:else}
    <p class="muted">Select one layer.</p>
  {/if}
</aside>

<style>
  aside {
    width: var(--size-panel-width);
    background: var(--colour-panel);
    border-left: var(--size-border) solid var(--colour-border);
    padding: var(--space-2);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    overflow-y: auto;
  }
  h2 {
    margin: 0;
    font-size: var(--font-size-sm);
    color: var(--colour-text-muted);
    font-weight: normal;
  }
  label {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  input {
    width: var(--size-input-width);
    background: var(--colour-bg);
    color: var(--colour-text);
    border: var(--size-border) solid var(--colour-border);
    border-radius: var(--size-radius);
    font: inherit;
  }
  .muted {
    color: var(--colour-text-muted);
  }
</style>
