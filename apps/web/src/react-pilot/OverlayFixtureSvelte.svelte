<!--
  Development only, part of the React pilot (docs/react-web-plan.md, section 7): the Svelte reference of the overlay
  fixture at /?overlays=svelte. It draws the Svelte IconButton and Menu, with the tooltip they bring, and the same
  test ids as OverlayFixture.tsx (/?overlays=react), so that packages/web-chrome/scripts/overlays-e2e.mjs asks both the
  same questions. Not in production: main.ts loads it in development only.
-->
<script lang="ts">
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import Menu from '@meldframe/web-chrome/Menu.svelte';
  import TooltipProvider from '@meldframe/web-chrome/TooltipProvider.svelte';
  import { fixtureItems } from './fixture';

  let log = $state<string[]>([]);
  let dialog = $state(false);
  const items = fixtureItems((id) => {
    log.push(id);
    if (id === 'dialog') dialog = true;
  });
  // The stand-in for a dialog: it takes the focus when it opens, as the Export dialog does.
  const takeFocus = (el: HTMLElement) => el.focus();
</script>

<TooltipProvider>
  <main class="flex flex-col items-start gap-4 p-8 font-sans text-body text-text">
    <div class="mt-64 flex gap-2">
      <IconButton label="Layers panel" icon="panel-left" shortcut="Alt+1" data-testid="fx-icon" />
      <IconButton label="Inspector panel" icon="panel-right" shortcut="Alt+2" data-testid="fx-icon-2" />
      <Menu label="Fixture menu" icon="menu" {items} testId="fx-menu" />
      <Menu label="Zoom 100%" text="100%" {items} testId="fx-text-menu" side="top" align="end" />
    </div>
    <button type="button" data-testid="fx-outside" class="h-(--mf-size-control-lg) rounded-md bg-control px-4 text-small">Outside</button>
    <output data-testid="fx-log">{log.join(',')}</output>
    {#if dialog}<input data-testid="fx-dialog-input" aria-label="Dialog stand-in" {@attach takeFocus} class="h-(--mf-size-control-lg) rounded-md border border-divider bg-field px-3 text-small" />{/if}
  </main>
</TooltipProvider>
