// The two parts of the Layout that the React pilot can swap (docs/react-web-plan.md, section 7, slice S4): the editor and the
// layout page (`/?layout`) both hand them to the Layout, which draws the Svelte palette and shortcut sheet unless it is given
// others. Each switch file draws the Svelte copy, or the React copy with `/?react=palette` (`reactFor` in pilot.ts).
import PaletteSwitch from './PaletteSwitch.svelte';
import ShortcutHintSwitch from './ShortcutHintSwitch.svelte';

export const shellParts = { paletteView: PaletteSwitch, hintView: ShortcutHintSwitch };
