<!--
  The state matrix page (/?states): every control of the chrome in its six states, in dark and light, as the StateMatrix
  and Accessibility boards draw them. Hover, pressed and focus are drawn with `data-preview` on a wrapper (the variants
  of tailwind.css answer to it), so the page needs no pointer; every enabled control is a real, focusable control.
-->
<script lang="ts">
  import Button from './Button.svelte';
  import Field from './Field.svelte';
  import IconButton from './IconButton.svelte';
  import Section from './Section.svelte';
  import TooltipProvider from './TooltipProvider.svelte';
  import Segmented from './Segmented.svelte';
  import Tabs from './Tabs.svelte';
  import type { Snippet } from 'svelte';

  const states = ['default', 'hover', 'pressed', 'focus', 'disabled', 'selected'] as const;
  type State = (typeof states)[number];
  const themes = ['dark', 'light'] as const;
  const preview = (s: State) => (s === 'hover' || s === 'pressed' || s === 'focus' ? s : undefined);
  const noop = () => {};
  const seg = [{ value: 'a', label: 'Layers' }];
  const tab = [{ id: 'design', label: 'Design' }];
</script>

{#snippet row(title: string, note: string, cell: Snippet<[State, string]>, theme: string, skip: State[] = [])}
  <tr class="border-t border-hairline">
    <th scope="row" class="px-4 py-3 text-left align-middle"><b class="block text-small font-semibold text-text-strong">{title}</b><span class="text-caption font-normal text-text-muted">{note}</span></th>
    {#each states as s (s)}
      <td class="px-4 py-3 align-middle">
        {#if skip.includes(s)}<span class="text-caption text-text-muted">not applicable</span>{:else}<span data-preview={preview(s)} class="contents">{@render cell(s, theme)}</span>{/if}
      </td>
    {/each}
  </tr>
{/snippet}

{#snippet primary(s: State)}<Button variant="primary" disabled={s === 'disabled'} onclick={noop}>Save</Button>{/snippet}
{#snippet secondary(s: State)}<Button disabled={s === 'disabled'} onclick={noop}>Cancel</Button>{/snippet}
{#snippet destructive(s: State)}<Button variant="destructive" disabled={s === 'disabled'} onclick={noop}>Revoke</Button>{/snippet}
{#snippet icon(s: State)}<IconButton label="Layers panel" icon="panel-left" pressed={s === 'selected'} disabled={s === 'disabled'} onclick={noop} />{/snippet}
{#snippet field(s: State)}<Field label="Width" prefix="W" value="240" disabled={s === 'disabled'} />{/snippet}
{#snippet bound(s: State)}<Field label="Spacing variable" value="space/md · 16" bound disabled={s === 'disabled'} />{/snippet}
{#snippet segmented(s: State)}<Segmented label="Panel" options={seg} value={s === 'selected' ? 'a' : ''} onChange={noop} disabled={s === 'disabled'} />{/snippet}
{#snippet fold(s: State)}<Section title="Fill" open={s === 'selected'} onToggle={noop}><span class="text-text-muted">Body</span></Section>{/snippet}
{#snippet tabs(s: State, t: string)}<Tabs label="Right panel" idPrefix="m-{t}-{s}" tabs={tab} selected={s === 'selected' ? 'design' : ''} onSelect={noop} />{/snippet}

<TooltipProvider>
<main class="font-sans text-body text-text">
  {#each themes as t (t)}
    <section data-theme={t} aria-label="States, {t}" class="bg-canvas p-8 text-text">
      <h1 class="m-0 mb-1 text-display font-bold tracking-tight text-text-strong">Component states, {t}</h1>
      <p class="m-0 mb-4 max-w-3xl text-text-muted">Default, hover, pressed, focus, disabled and selected. Hover, pressed and focus are drawn, not live; Tab through the page for the real focus ring.</p>
      <table class="w-full table-fixed border-collapse overflow-hidden rounded-lg border border-border bg-sidebar">
        <thead>
          <tr>
            <td class="w-(--mf-size-layers)"></td>
            {#each states as s (s)}<th scope="col" class="px-4 py-2 text-left text-caption font-semibold text-text-muted">{s}</th>{/each}
          </tr>
        </thead>
        <tbody>
          {@render row('Primary button', 'accent-strong, on-accent', primary, t, ['selected'])}
          {@render row('Secondary button', 'control', secondary, t, ['selected'])}
          {@render row('Destructive button', 'danger-strong', destructive, t, ['selected'])}
          {@render row('Icon button', 'pressed = panel open', icon, t)}
          {@render row('Field', 'field, outline divider', field, t, ['selected'])}
          {@render row('Variable-bound field', 'chip colours', bound, t, ['selected'])}
          {@render row('Segmented control', 'selected = control-selected', segmented, t)}
          {@render row('Section toggle', 'selected = open', fold, t, ['disabled'])}
          {@render row('Tab', 'selected = hairline fill', tabs, t, ['disabled'])}
        </tbody>
      </table>
    </section>
  {/each}
</main>
</TooltipProvider>
