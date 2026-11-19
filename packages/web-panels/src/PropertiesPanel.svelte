<!--
  The properties panel: the values of the one selected node. Each field reports its text when the person leaves it or
  presses Enter; the editor page turns that into one action (see properties.ts), and a value that makes no action
  is put back.
-->
<script lang="ts">
  import { tick } from 'svelte';
  import type { Properties } from '@meldframe/protocol';
  import type { BoxField, PropertyField } from '@meldframe/web-document/properties';

  interface Props {
    properties: Properties | null;
    onProperty: (field: PropertyField, raw: string) => void;
    onBox: (field: BoxField, raw: string) => void;
  }
  let { properties, onProperty, onBox }: Props = $props();

  const input = 'w-full min-w-0 rounded-sm border border-border bg-field px-2 py-1 text-text outline-none focus:border-accent';

  /** Commits on Enter and leaves on Escape without a commit; a blur commits through `onchange`. */
  function keys(e: KeyboardEvent) {
    const el = e.currentTarget as HTMLInputElement;
    if (e.key === 'Enter') el.blur();
    else if (e.key === 'Escape') {
      el.value = el.defaultValue;
      el.blur();
    }
    e.stopPropagation(); // typing here is text, not a command of the editor
  }
  const round = (v: number) => String(Math.round(v * 100) / 100);
  /** What the document holds for a field now. */
  const current = (f: PropertyField | BoxField): string => {
    const p = properties;
    if (!p) return '';
    return typeof p[f] === 'number' ? round(p[f] as number) : String(p[f]);
  };
  /** After the document took the change (or did not), the field shows what the document holds. */
  const restore = (el: HTMLInputElement, f: PropertyField | BoxField) => void tick().then(() => (el.value = current(f)));
</script>

<aside class="flex w-64 shrink-0 flex-col border-l border-hairline bg-sidebar text-small text-text" aria-label="Properties" data-testid="properties">
  <header class="border-b border-hairline px-3 py-2 text-text-muted">Properties</header>
  {#if properties}
    {@const p = properties}
    {#key p.id}
      <div class="flex flex-col gap-3 overflow-y-auto p-3">
        <label class="flex flex-col gap-1">Name
          <input class={input} value={p.name} data-testid="prop-name" onkeydown={keys} onchange={(e) => { onProperty('name', e.currentTarget.value); restore(e.currentTarget, 'name'); }} />
        </label>
        <div class="grid grid-cols-2 gap-2">
          {#each [['x', p.x], ['y', p.y], ['w', p.w], ['h', p.h]] as [f, v] (f)}
            <label class="flex flex-col gap-1">{String(f).toUpperCase()}
              <input class="{input} font-mono" value={round(Number(v))} data-testid="prop-{f}" onkeydown={keys} onchange={(e) => { onBox(f as BoxField, e.currentTarget.value); restore(e.currentTarget, f as BoxField); }} />
            </label>
          {/each}
        </div>
        <label class="flex flex-col gap-1">Fill
          <input class="{input} font-mono" value={p.fill} data-testid="prop-fill" onkeydown={keys} onchange={(e) => { onProperty('fill', e.currentTarget.value); restore(e.currentTarget, 'fill'); }} />
        </label>
        {#if p.kind !== 'text'}
          <label class="flex flex-col gap-1">Radius
            <input class="{input} font-mono" value={String(p.radius)} data-testid="prop-radius" onkeydown={keys} onchange={(e) => { onProperty('radius', e.currentTarget.value); restore(e.currentTarget, 'radius'); }} />
          </label>
        {:else}
          <label class="flex flex-col gap-1">Font size
            <input class="{input} font-mono" value={String(p.fontSize)} data-testid="prop-fontSize" onkeydown={keys} onchange={(e) => { onProperty('fontSize', e.currentTarget.value); restore(e.currentTarget, 'fontSize'); }} />
          </label>
        {/if}
      </div>
    {/key}
  {:else}
    <p class="px-3 py-3 text-text-faint">Select one node to edit it.</p>
  {/if}
</aside>
