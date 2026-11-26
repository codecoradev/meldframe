<!--
  The tab strip of the right panel (Design and Activity): a tab list with one selected tab, as the Activity board draws
  it. Arrow keys, Home and End move the selection; the selected tab alone is in the Tab order. `badge` is a count.
-->
<script lang="ts">
  import { nextIndex, requireLabel, ring } from './focus';

  interface Tab {
    id: string;
    label: string;
    /** A count; `null` is a count that is not known, drawn as `?` and never as nothing or zero. */
    badge?: number | null;
    /** What the badge counts, for a reader who does not see its colour: read after the number (after `?` it says unknown). */
    badgeLabel?: string;
    /** Not available yet: it cannot be selected by click or arrow, and it has no panel (no aria-controls). */
    disabled?: boolean;
  }
  interface Props {
    label: string;
    tabs: Tab[];
    selected: string;
    onSelect: (id: string) => void;
    /** The panel of a tab is `${idPrefix}-panel-${id}`, the tab itself `${idPrefix}-tab-${id}`. */
    idPrefix: string;
    [data: `data-${string}`]: string | undefined;
  }
  let { label, tabs, selected, onSelect, idPrefix, ...rest }: Props = $props();
  const name = $derived(requireLabel(label, 'Tabs'));
  const current = $derived(Math.max(0, tabs.findIndex((t) => t.id === selected)));
  let host: HTMLElement | undefined = $state();

  function onkeydown(e: KeyboardEvent, i: number) {
    const to = nextIndex(e.key, i, tabs.length, tabs.map((t) => !!t.disabled));
    if (to === undefined) return;
    e.preventDefault();
    onSelect(tabs[to]!.id);
    (host?.querySelectorAll<HTMLElement>('[role=tab]')[to])?.focus();
  }
</script>

<div bind:this={host} {...rest} role="tablist" aria-label={name} class="flex min-w-0 flex-1 gap-1">
  {#each tabs as t, i (t.id)}
    {@const on = t.id === selected}
    <button
      type="button"
      role="tab"
      id="{idPrefix}-tab-{t.id}"
      aria-selected={on}
      aria-controls={t.disabled ? undefined : `${idPrefix}-panel-${t.id}`}
      aria-disabled={t.disabled ? 'true' : undefined}
      tabindex={i === current ? 0 : -1}
      onclick={() => { if (!t.disabled) onSelect(t.id); }}
      onkeydown={(e) => onkeydown(e, i)}
      class="{ring} flex h-(--mf-size-control-md) items-center gap-2 rounded-md px-3 text-small {on ? 'bg-hairline font-semibold text-text-strong' : t.disabled ? 'font-medium text-text-muted opacity-(--mf-opacity-disabled)' : 'font-medium text-text-muted hov:text-text'}"
    >{t.label}{#if t.badge === null}<span class="rounded-pill bg-accent-strong px-2 text-caption font-bold text-on-accent" data-unknown="true">?{#if t.badgeLabel}<span class="sr-only">{` ${t.badgeLabel}: unknown`}</span>{/if}</span>{:else if t.badge}<span class="rounded-pill bg-accent-strong px-2 text-caption font-bold text-on-accent">{t.badge}{#if t.badgeLabel}<span class="sr-only">{` ${t.badgeLabel}`}</span>{/if}</span>{/if}</button>
  {/each}
</div>
