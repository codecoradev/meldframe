<script lang="ts">
  import { Button, Switch } from 'bits-ui';
  import { applyTheme, loadTheme, saveTheme } from './theme';
  import { colorNames, colors, fontSize, space, type Theme } from './tokens';

  let theme = $state<Theme>(loadTheme());
  const light = $derived(theme === 'light');

  function setLight(on: boolean) {
    theme = on ? 'light' : 'dark';
    applyTheme(theme);
    saveTheme(theme);
  }

  const sampleText: Record<string, string> = {
    display: 'Board title',
    heading: 'Dialog headline',
    title: 'Panel title',
    body: 'Base text and selected layer',
    small: 'Dense rows and fields',
    caption: 'SECTION LABEL',
  };
  const weightFor: Record<string, number> = { display: 700, heading: 600, title: 600, body: 400, small: 400, caption: 600 };
</script>

<main class="mx-auto flex max-w-5xl flex-col gap-4 p-8 font-sans text-body text-text">
  <header class="flex items-start justify-between gap-4">
    <div>
      <h1 class="text-display font-bold tracking-tight text-text-strong">Foundations</h1>
      <p class="mt-2 text-text-muted">Design tokens of the editor, {theme} set.</p>
    </div>
    <label class="flex items-center gap-2 text-small">
      Light
      <Switch.Root
        checked={light}
        onCheckedChange={setLight}
        aria-label="Light theme"
        class="relative h-4.5 w-8 rounded-pill bg-text-muted focus-visible:ring-2 focus-visible:ring-selection data-[state=checked]:bg-accent"
      >
        <Switch.Thumb
          class="block size-3.5 translate-x-0.5 rounded-pill bg-canvas data-[state=checked]:translate-x-4"
        />
      </Switch.Root>
    </label>
  </header>

  <section class="flex flex-col gap-3 rounded-lg border border-border bg-surface p-4" aria-label="Colours">
    <h2 class="text-caption font-semibold uppercase tracking-wide text-text-muted">Colours</h2>
    <ul class="grid grid-cols-2 gap-2 sm:grid-cols-4 lg:grid-cols-6">
      {#each colorNames as name (name)}
        <li class="flex flex-col gap-1 text-caption">
          <i class="block h-6 rounded-md border border-border" style="background: var(--mf-{name})"></i>
          <span>{name}</span>
          <span class="font-mono text-text-muted">{colors[theme][name]}</span>
        </li>
      {/each}
    </ul>
  </section>

  <section class="flex flex-col gap-3 rounded-lg border border-border bg-surface p-4" aria-label="Type scale">
    <h2 class="text-caption font-semibold uppercase tracking-wide text-text-muted">Type scale</h2>
    {#each Object.entries(fontSize) as [key, px] (key)}
      <div class="flex items-baseline gap-3">
        <span class="w-8 font-mono text-text-muted">{px}</span>
        <span
          class="text-text-strong"
          style="font-size: var(--mf-font-size-{key}); font-weight: {weightFor[key]}"
        >{sampleText[key]}</span>
        <span class="text-small text-text-muted">{key}</span>
      </div>
    {/each}
    <p class="font-mono text-small">544 × 80 · seq 1,284</p>
  </section>

  <section class="flex flex-col gap-3 rounded-lg border border-border bg-surface p-4" aria-label="Components">
    <h2 class="text-caption font-semibold uppercase tracking-wide text-text-muted">Components</h2>
    <div class="flex flex-wrap items-center gap-2">
      <Button.Root class="h-(--mf-size-control-md) rounded-md bg-accent-strong px-3 text-small font-semibold text-on-accent focus-visible:ring-2 focus-visible:ring-selection">
        Export
      </Button.Root>
      <Button.Root class="h-(--mf-size-control-md) rounded-md bg-field px-3 text-small font-medium text-text focus-visible:ring-2 focus-visible:ring-selection">
        Cancel
      </Button.Root>
      <Button.Root class="h-(--mf-size-control-md) rounded-md bg-danger-strong px-3 text-small font-semibold text-on-accent focus-visible:ring-2 focus-visible:ring-selection">
        Stop agent
      </Button.Root>
    </div>
    <div class="grid max-w-sm grid-cols-2 gap-2">
      <label class="flex h-(--mf-size-control-md) items-center gap-2 rounded-md bg-field px-2 text-small focus-within:ring-2 focus-within:ring-selection">
        <b class="text-caption font-normal text-text-muted">X</b>
        <input class="w-full bg-transparent outline-none" value="28" />
      </label>
      <label class="flex h-(--mf-size-control-md) items-center gap-2 rounded-md bg-field px-2 text-small focus-within:ring-2 focus-within:ring-selection">
        <b class="text-caption font-normal text-text-muted">Y</b>
        <input class="w-full bg-transparent outline-none" value="104" />
      </label>
    </div>
    <ul class="flex items-end gap-3" aria-label="Spacing steps">
      {#each Object.entries(space) as [step, px] (step)}
        <li class="flex flex-col gap-1 text-caption">
          <i class="block h-5 bg-accent" style="width: var(--mf-space-{step})"></i>
          <span class="font-mono text-text-muted">{px}</span>
        </li>
      {/each}
    </ul>
    <div class="flex gap-3">
      <div class="flex h-10 w-24 items-center justify-center rounded-lg border border-border bg-surface text-caption shadow-float">
        float panel
      </div>
      <div class="flex h-10 w-24 items-center justify-center rounded-lg border border-border bg-surface-raised text-caption shadow-popover">
        popover
      </div>
    </div>
  </section>
</main>
