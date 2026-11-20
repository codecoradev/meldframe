// The rules of where the layout puts things, in px, from the size tokens. Pure, so the no-overlap rule is a test.
import { size, space } from '@meldframe/web-tokens/tokens';

/** The two sides of the right panel card: the Design side is 288 wide, the Activity tab 360 (Activity board). */
export type RightTab = 'design' | 'activity';
export const rightWidth = (tab: RightTab): number => (tab === 'activity' ? size.activity : size.inspector);

/** Inset of an open panel card from the window edge (Panels board), and the gap kept between a card and the tool bar. */
export const PANEL_INSET = space[3];
export const BAR_GAP = space[4];

/** Where the open panels end: the left one at `left`, the right one at `right` px from the right edge. */
export function reserved(open: { layers: boolean; inspector: boolean; tab?: RightTab }): { left: number; right: number } {
  return {
    left: open.layers ? PANEL_INSET + size.layers : 0,
    right: open.inspector ? PANEL_INSET + rightWidth(open.tab ?? 'design') : 0,
  };
}

/** True when a tool bar of `barWidth`, centred in a window of `viewportWidth`, keeps `BAR_GAP` from both open panels. */
export function barClear(viewportWidth: number, barWidth: number, open: { layers: boolean; inspector: boolean; tab?: RightTab }): boolean {
  const { left, right } = reserved(open);
  const start = (viewportWidth - barWidth) / 2;
  return start >= left + BAR_GAP && viewportWidth - start - barWidth >= right + BAR_GAP;
}

/** A row of the file menu. `opensDialog`: the row opens a dialog, which takes the focus, so the menu does not give it back to its button. */
export interface FileItem {
  id: string;
  label: string;
  onSelect: () => void;
  disabled?: boolean;
  testId: string;
  opensDialog?: boolean;
}

/** The rows of the file menu: PNG export at 1x and 2x and a variant, each for the one selected frame. */
export function fileItems(o: { canExport: boolean; canVariant: boolean; onExport: (scale: 1 | 2) => void; onVariant: () => void }): FileItem[] {
  return [
    { id: 'png1', label: 'Export PNG 1x', onSelect: () => o.onExport(1), disabled: !o.canExport, testId: 'export-1x' },
    { id: 'png2', label: 'Export PNG 2x', onSelect: () => o.onExport(2), disabled: !o.canExport, testId: 'export-2x' },
    { id: 'variant', label: 'Make a variant', onSelect: o.onVariant, disabled: !o.canVariant, testId: 'make-variant' },
  ];
}
