// React copy of Icon.svelte: an icon is decoration, the control that holds it carries the name. The path data is shared
// with the Svelte icon (../icons.ts); colour follows the text colour.
import { type IconName, icons } from '../icons';

export function Icon({ name }: { name: IconName }) {
  return (
    <svg viewBox="0 0 16 16" className="size-4 shrink-0 fill-none stroke-current stroke-(length:--mf-stroke-icon) [stroke-linecap:round] [stroke-linejoin:round]" aria-hidden="true" focusable="false">
      <path d={icons[name]} />
    </svg>
  );
}
