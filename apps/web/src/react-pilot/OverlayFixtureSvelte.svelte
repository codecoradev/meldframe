<!--
  Development only, part of the React pilot (docs/react-web-plan.md, section 7): the Svelte reference of the overlay
  fixture at /?overlays=svelte. It draws the Svelte IconButton and Menu, with the tooltip they bring, and the same
  test ids as OverlayFixture.tsx (/?overlays=react), so that packages/web-chrome/scripts/overlays-e2e.mjs asks both the
  same questions. The "Dialog…" row opens a real dialog (FixtureDialogSvelte.svelte). Not in production: main.ts loads it in development only.
-->
<script lang="ts">
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import Menu from '@meldframe/web-chrome/Menu.svelte';
  import TooltipProvider from '@meldframe/web-chrome/TooltipProvider.svelte';
  import { rememberFocus } from '@meldframe/web-chrome/focus';
  import FixtureDialogSvelte from './FixtureDialogSvelte.svelte';
  import { fixtureItems } from './fixture';

  let log = $state<string[]>([]);
  let dialog = $state(false);
  let outside = $state<HTMLElement | null>(null);
  const items = fixtureItems((id) => {
    log.push(id);
    if (id === 'dialog') dialog = true;
  });
</script>

<TooltipProvider>
  <main class="flex flex-col items-start gap-4 p-8 font-sans text-body text-text">
    <div class="mt-64 flex gap-2">
      <IconButton label="Layers panel" icon="panel-left" shortcut="Alt+1" data-testid="fx-icon" />
      <IconButton label="Inspector panel" icon="panel-right" shortcut="Alt+2" data-testid="fx-icon-2" />
      <Menu label="Fixture menu" icon="menu" {items} testId="fx-menu" />
      <Menu label="Zoom 100%" text="100%" {items} testId="fx-text-menu" side="top" align="end" />
    </div>
    <output data-testid="fx-log">{log.join(',')}</output>
    <div class="mt-16 flex gap-2">
      <button type="button" data-testid="fx-open-elsewhere" onclick={() => { rememberFocus(outside); dialog = true; }} class="h-(--mf-size-control-lg) rounded-md bg-control px-4 text-small">Open the dialog, focus to Outside</button>
      <button bind:this={outside} type="button" data-testid="fx-outside" class="h-(--mf-size-control-lg) rounded-md bg-control px-4 text-small">Outside</button>
    </div>
    <FixtureDialogSvelte open={dialog} onClose={() => (dialog = false)} />
  </main>
</TooltipProvider>
