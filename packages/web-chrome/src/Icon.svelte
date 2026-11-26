<!--
  The icon set: 16 px grid, 1.5 stroke, round caps and joins, drawn from the boards as path data (no icon library).
  An icon is decoration: the control that holds it carries the name. Colour follows the text colour.
-->
<script module lang="ts">
  import { icons as paths, type IconName as Name } from './icons';

  // The path data lives in icons.ts, shared with the React icon; the Svelte tests and files still import it from here.
  export const icons = paths;
  export type IconName = Name;
</script>

<script lang="ts">
  let { name }: { name: IconName } = $props();
</script>

<svg viewBox="0 0 16 16" class="size-4 shrink-0 fill-none stroke-current stroke-(length:--mf-stroke-icon) [stroke-linecap:round] [stroke-linejoin:round]" aria-hidden="true" focusable="false"><path d={icons[name]} /></svg>
