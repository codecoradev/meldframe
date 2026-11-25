// The fixtures the product export is known not to match. Each entry pins the gap to its cause, and gate.mjs checks both
// directions: while the fixture fails only in the way the entry says, the gate passes; when it stops failing, or fails in any
// other way (a font, Tailwind against CSS, a node outside `nodes`, the export's warning gone), the gate fails.
//   reason   what the gap is
//   warning  a finding the export must still report for the file (`code:node` starts with it)
//   nodes    the only nodes allowed to be more than 0.05 px off (the cause and what it moves)
export const KNOWN_GAPS = {
  'f14-text-edge': {
    reason: 'break opportunities outside the contract (section 6.7: slash tokens, URLs, CJK, tabs): Pixi and CSS break the same text differently; also a trailing newline (Pixi keeps the empty last line, the browser does not), which the export does not report',
    warning: 'text_outside_contract:',
    nodes: ['c', 'd', 'w3', 'w4', 'w5', 'w6', 'w7', 'w8', 'w9', 'h8', 'h9'],
  },
  'f17-fill-under-hug': {
    reason: 'rule G1: the core gives a fill under a hug parent the size the parent hugs, the export keeps the own size and reports fill_under_hug; closes when the write-time validator makes it fixed',
    warning: 'fill_under_hug:',
    nodes: ['b'],
  },
};

/** Gaps of the importer (docs/fidelity-contract.md section 14), checked in both directions like KNOWN_GAPS. `kind` is 'page' (a hand-written page of
 *  pages.mjs: the importer's rects against the browser's boxes) or 'roundtrip' (a fixture imported back from its export).
 *    reason   what the importer does not read, with the row of docs/import-html.md section 3 that says so
 *    warning  a finding the import must still report (`property:code` of a refused property, or `report:<code>`; it starts with this)
 *    nodes    the only nodes (by name) allowed to be more than 0.05 px off */
export const IMPORT_GAPS = {
  'f17-fill-under-hug': {
    kind: 'roundtrip',
    reason: 'rule G1: the core gives a fill under a hug parent the size the parent hugs; the importer reads the export of it as a fixed size (the export keeps the own size), so the imported file lays out as the export does and not as the original did',
    warning: null,
    nodes: ['b'],
  },
  'h13-gap-percent': { kind: 'page', reason: 'a width in percent other than 100% is refused (section 3.3, `%`; 3.4, `width: 100%` is the one read)', warning: 'width:refused_value', nodes: ['half'] },
  'h14-gap-grid': { kind: 'page', reason: 'grid is refused: the frame is a column of its children in DOM order (section 3.4, `display: grid`)', warning: 'display:refused_grid', nodes: ['g', 'b', 'c', 'd'] },
  'h15-gap-margin-collapse': { kind: 'page', reason: 'margins between block siblings are not collapsed (section 3.4, "Collapsing margins ... approximated"); the importer does not list it under approximated', warning: null, nodes: ['blk', 'b'] },
  'h16-gap-before': { kind: 'page', reason: '`::before` with `content` makes no node (section 3.2, `::before`, `::after`): a box that hugs it is smaller and what follows moves up', warning: '::before:refused_generated_content', nodes: ['c', 'm', 'after'] },
  'h17-gap-wrap': { kind: 'page', reason: '`flex-wrap` is refused: no wrapping (section 3.4, `flex-wrap`)', warning: 'flex-wrap:refused_not_modelled', nodes: ['r', 'c', 'd'] },
  'h18-gap-max-width': { kind: 'page', reason: '`max-width` is dropped (section 3.4, `min-width`, `max-width`)', warning: 'max-width:refused_not_modelled', nodes: ['capped'] },
  'h19-gap-space-around': { kind: 'page', reason: '`justify-content: space-around` is refused: it is read as start (section 3.4)', warning: 'justify-content:refused_value', nodes: ['a', 'b', 'c'] },
  'h20-gap-margin-auto': { kind: 'page', reason: '`margin: auto` is dropped (section 3.4, "Negative margin; `margin: auto`")', warning: 'margin-left:refused_value', nodes: ['centred'] },
  'h21-gap-inline-flow': { kind: 'page', reason: 'inline formatting is not modelled: `inline-block` siblings of a block are a column (section 3.4, `display: inline-block` is approximated); the importer does not list it', warning: null, nodes: ['blk', 'b'] },
  'h22-gap-line-height-normal': { kind: 'page', reason: 'a text with no `line-height` in px: the browser makes its line `normal` (fractional), the core gives it the whole pixels Pixi measured (contract rule G4; section 3.5 drops `line-height`); the importer does not report it', warning: null, nodes: ['c', 'b', 'after'] },
  'h23-gap-text-width': { kind: 'page', reason: 'a hug text with no `min-width` pin: the browser keeps the fractional width of the glyphs, the core the width rounded up (contract rule G4, section 6.3); the importer does not report it', warning: null, nodes: ['a', 'b', 'after'] },
  'h24-gap-fill-padding': { kind: 'page', reason: '`flex: 1` siblings that differ in padding: the model shares the boxes equally, the browser shares the free space and adds each padding (section 3.4, `flex: 1` approximated; the export writes `flex: 1 1 <px>` for it)', warning: null, nodes: ['a', 'b'] },
  'h25-gap-tailwind-scale': { kind: 'page', reason: 'the utilities of Tailwind\'s own scale (`p-4`, `w-64`, `gap-2`) are not decoded: the theme is not the importer\'s (section 2.2, 3.3)', warning: 'report:class_unresolved', nodes: ['c', 'a', 'b'] },
  'h26-gap-reverse-and-order': { kind: 'page', reason: '`row-reverse` is read in DOM order (section 3.4, `flex-direction`)', warning: 'flex-direction:refused_reverse', nodes: ['a', 'b', 'c'] },
  'h27-gap-media': { kind: 'page', reason: '`@media` blocks are skipped whole: the base rules apply (section 3.3, `@media`)', warning: 'report:at_rule_skipped', nodes: ['a'] },
  'h28-gap-named-colour': { kind: 'page', reason: 'a named colour outside the importer\'s table (`rebeccapurple`) is refused: the fill is none (section 3.3, colours)', warning: 'background:background_shorthand', nodes: [], pixels: true },
};
