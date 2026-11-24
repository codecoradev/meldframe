<!--
  The icon set: 16 px grid, 1.5 stroke, round caps and joins, drawn from the boards as path data (no icon library).
  An icon is decoration: the control that holds it carries the name. Colour follows the text colour.
-->
<script module lang="ts">
  const n = (v: number) => String(Math.round(v * 100) / 100);
  const rect = (x: number, y: number, w: number, h: number, r = 1) =>
    `M${n(x + r)} ${n(y)}h${n(w - 2 * r)}a${n(r)} ${n(r)} 0 0 1 ${n(r)} ${n(r)}v${n(h - 2 * r)}a${n(r)} ${n(r)} 0 0 1-${n(r)} ${n(r)}h-${n(w - 2 * r)}a${n(r)} ${n(r)} 0 0 1-${n(r)}-${n(r)}v-${n(h - 2 * r)}a${n(r)} ${n(r)} 0 0 1 ${n(r)}-${n(r)}z`;
  const circle = (cx: number, cy: number, r: number) => `M${n(cx - r)} ${n(cy)}a${n(r)} ${n(r)} 0 1 0 ${n(2 * r)} 0a${n(r)} ${n(r)} 0 1 0-${n(2 * r)} 0`;

  export const icons = {
    menu: 'M3 4.5h10M3 8h10M3 11.5h10',
    'panel-left': 'M3 3h10v10H3zM6 3v10',
    'panel-right': 'M3 3h10v10H3zM10 3v10',
    plus: 'M8 3v10M3 8h10',
    minus: 'M3 8h10',
    close: 'M4 4l8 8M12 4l-8 8',
    check: 'M3 8l3 3 7-7',
    search: `${circle(7, 7, 4)}M10 10l3 3`,
    eye: `M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8z${circle(8, 8, 2)}`,
    'chevron-down': 'M4 6l4 4 4-4',
    'chevron-right': 'M6 4l4 4-4 4',
    'chevron-left': 'M10 4L6 8l4 4',
    'chevrons-updown': 'M5 6l3-3 3 3M5 10l3 3 3-3',
    select: 'M3.5 2.5l9 4.5-4 1.3L7 12.5z',
    frame: 'M6 2.5v11M10 2.5v11M2.5 6h11M2.5 10h11',
    hand: 'M5.5 8.5v-4a1 1 0 012 0V7m0 0V3.5a1 1 0 012 0V7m0 0V4.5a1 1 0 012 0V10c0 2.5-1.5 4.5-4 4.5-2 0-3-1-4-2.8L3 9.5a1 1 0 011.4-1.2l1.1 1',
    rectangle: rect(2.5, 3.5, 11, 9, 1.5),
    ellipse: circle(8, 8, 5.5),
    text: 'M3 4V3h10v1M8 3v10M6 13h4',
    'text-left': 'M3 4h10M3 8h6M3 12h8',
    'text-center': 'M3 4h10M5 8h6M4 12h8',
    'text-right': 'M3 4h10M7 8h6M5 12h8',
    comment: 'M3 4.5A1.5 1.5 0 014.5 3h7A1.5 1.5 0 0113 4.5v5a1.5 1.5 0 01-1.5 1.5H7l-3 2.5V11A1.5 1.5 0 013 9.5z',
    'flow-free': `${circle(5, 5, 1.5)}${circle(11, 9, 1.5)}${circle(6, 12, 1.5)}`,
    'flow-row': `${rect(2.5, 4, 4, 8)}${rect(9.5, 4, 4, 8)}`,
    'flow-column': `${rect(4, 2.5, 8, 4)}${rect(4, 9.5, 8, 4)}`,
    'flow-grid': `${rect(2.5, 2.5, 4.5, 4.5)}${rect(9, 2.5, 4.5, 4.5)}${rect(2.5, 9, 4.5, 4.5)}${rect(9, 9, 4.5, 4.5)}`,
    'align-start': 'M3 3v10M6 5h6M6 9h4',
    'align-center': 'M8 3v10M4 5h8M5 9h6',
    'align-end': 'M13 3v10M4 5h6M6 9h4',
    variable: 'M8 2l5 3v6l-5 3-5-3V5z',
    padding: `${rect(3, 3, 10, 10)}M6 6h4v4H6z`,
    agent: `${circle(8, 8, 2.5)}M8 2v2M8 12v2M2 8h2M12 8h2`,
    undo: 'M6 3L3 6l3 3M3 6h6a4 4 0 010 8H6',
    redo: 'M10 3l3 3-3 3M13 6H7a4 4 0 000 8h3',
    help: `${circle(8, 8, 6)}M6.3 6.2a1.8 1.8 0 113 1.2C8.5 8 8 8.3 8 9.2M8 11.4v.1`,
    layers: 'M8 2.5l5.5 3L8 8.5l-5.5-3zM2.5 8.5L8 11.5l5.5-3M2.5 11L8 14l5.5-3',
  } as const;
  export type IconName = keyof typeof icons;
</script>

<script lang="ts">
  let { name }: { name: IconName } = $props();
</script>

<svg viewBox="0 0 16 16" class="size-4 shrink-0 fill-none stroke-current stroke-(length:--mf-stroke-icon) [stroke-linecap:round] [stroke-linejoin:round]" aria-hidden="true" focusable="false"><path d={icons[name]} /></svg>
