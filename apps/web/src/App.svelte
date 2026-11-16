<script lang="ts">
  import { Button, Switch } from 'bits-ui';
  import { applyTheme, loadTheme, saveTheme } from './theme';
  import { colorNames, colors, space, typeScale, type Theme } from './tokens';

  let theme = $state<Theme>(loadTheme());
  const light = $derived(theme === 'light');

  function setLight(on: boolean) {
    theme = on ? 'light' : 'dark';
    applyTheme(theme);
    saveTheme(theme);
  }

  const sampleText: Record<string, string> = {
    title: 'Board title',
    dialog: 'Dialog headline',
    panel: 'Panel title',
    base: 'Base text and selected layer',
    dense: 'Dense rows and fields',
    label: 'SECTION LABEL',
  };
</script>

<main class="mx-auto flex max-w-5xl flex-col gap-4 p-8 font-ui text-base text-text">
  <header class="flex items-start justify-between gap-4">
    <div>
      <h1 class="text-title font-bold tracking-tight text-text-strong">Foundations</h1>
      <p class="mt-2 text-text-muted">Design tokens of the editor, {theme} set.</p>
    </div>
    <label class="flex items-center gap-2 text-dense">
      Light
      <Switch.Root
        checked={light}
        onCheckedChange={setLight}
        aria-label="Light theme"
        class="relative h-4.5 w-8 rounded-pill bg-menu-border data-[state=checked]:bg-accent"
      >
        <Switch.Thumb
          class="block size-3.5 translate-x-0.5 rounded-pill bg-text-strong data-[state=checked]:translate-x-4"
        />
      </Switch.Root>
    </label>
  </header>

  <section class="flex flex-col gap-3 rounded-panel border border-hairline bg-panel p-4" aria-label="Colours">
    <h2 class="text-label font-semibold uppercase tracking-wide text-text-secondary">Colours</h2>
    <ul class="grid grid-cols-2 gap-2 sm:grid-cols-4 lg:grid-cols-6">
      {#each colorNames as name (name)}
        <li class="flex flex-col gap-1 text-label">
          <i class="block h-6 rounded-button border border-hairline" style="background: var(--mf-{name})"></i>
          <span>{name}</span>
          <span class="font-mono text-text-muted">{colors[theme][name]}</span>
        </li>
      {/each}
    </ul>
  </section>

  <section class="flex flex-col gap-3 rounded-panel border border-hairline bg-panel p-4" aria-label="Type scale">
    <h2 class="text-label font-semibold uppercase tracking-wide text-text-secondary">Type scale</h2>
    {#each Object.entries(typeScale) as [key, step] (key)}
      <div class="flex items-baseline gap-3">
        <span class="w-8 font-mono text-text-muted">{step.size}</span>
        <span
          class="text-text-strong"
          style="font-size: var(--mf-text-{key}); font-weight: {step.weights[step.weights.length - 1]}"
        >{sampleText[key]}</span>
        <span class="text-dense text-text-muted">{step.weights.join(' / ')}</span>
      </div>
    {/each}
    <p class="font-mono text-dense">544 × 80 · seq 1,284</p>
  </section>

  <section class="flex flex-col gap-3 rounded-panel border border-hairline bg-panel p-4" aria-label="Components">
    <h2 class="text-label font-semibold uppercase tracking-wide text-text-secondary">Components</h2>
    <div class="flex flex-wrap items-center gap-2">
      <Button.Root class="h-(--mf-height-button) rounded-button bg-primary px-3 text-dense font-semibold text-on-primary">
        Export
      </Button.Root>
      <Button.Root class="h-(--mf-height-button) rounded-button bg-hairline px-3 text-dense font-medium text-text">
        Cancel
      </Button.Root>
      <Button.Root class="h-(--mf-height-button) rounded-button bg-danger-solid px-3 text-dense font-semibold text-on-primary">
        Stop agent
      </Button.Root>
    </div>
    <div class="grid max-w-sm grid-cols-2 gap-2">
      <label class="flex h-(--mf-height-field) items-center gap-2 rounded-field bg-field px-2 text-dense">
        <b class="text-label font-normal text-text-muted">X</b>
        <input class="w-full bg-transparent outline-none" value="28" />
      </label>
      <label
        class="flex h-(--mf-height-field) items-center gap-2 rounded-field border border-selection bg-field px-2 text-dense"
      >
        <b class="text-label font-normal text-text-muted">Y</b>
        <input class="w-full bg-transparent outline-none" value="104" />
      </label>
    </div>
    <ul class="flex items-end gap-3" aria-label="Spacing steps">
      {#each space as s (s)}
        <li class="flex flex-col gap-1 text-label">
          <i class="block h-5 bg-accent" style="width: var(--mf-space-{s})"></i>
          <span class="font-mono text-text-muted">{s}</span>
        </li>
      {/each}
    </ul>
    <div class="flex gap-3">
      <div class="flex h-10 w-24 items-center justify-center rounded-panel border border-hairline bg-panel text-label shadow-float">
        float panel
      </div>
      <div class="flex h-10 w-24 items-center justify-center rounded-panel border border-menu-border bg-raised text-label shadow-popover">
        popover
      </div>
    </div>
  </section>
</main>
