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
