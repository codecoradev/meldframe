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

// What the importer (docs/import-html.md) is known not to read as a browser does, for the hand-written pages of hfixtures.mjs. Pinned like
// the gaps above and checked in both directions by import-gate.mjs: while a page fails only in the way its entry says the gate passes; when it
// stops failing, or fails in another way (a node outside `nodes`, a font, no element for a source), the gate fails.
//   reason  what the gap is
//   codes   `code:property` of what the importer must still list as refused (or the property of an `approximated` entry) on some node of the page
//   nodes   the sources (`tag.class#n`) allowed to be more than 0.05 px off
export const IMPORT_GAPS = {
  'h06-absolute': {
    reason: 'right and bottom are not modelled (docs/import-html.md section 3.4): the box keeps the default left and top of 0 and the importer lists both as refused',
    codes: ['refused_not_modelled:right', 'refused_not_modelled:bottom'],
    nodes: ['div.b#9'],
  },
  'h07-text': {
    reason: 'authored text is hugged at the size the core measures (whole pixels, rounded up, Pixi on Inter) where the browser lays out `line-height: normal` at the fractional height of the font; the importer lists nothing, the contract writes px line heights for an export and promises nothing for one the author wrote',
    codes: [],
    nodes: ['div.card#7', 'h1#8', 'p#9'],
  },
  'h08-button-row': {
    reason: 'the same text measuring as h07-text, in an inline-flex bar of inline-block buttons: the core rounds the width and the height of a text up, the browser does not',
    codes: [],
    nodes: ['div.bar#7', 'span.btn#8', 'span.btn#9'],
  },
  'h09-percent': {
    reason: 'a percentage size and an auto margin have no value in the model: the importer lists them as refused and writes the model\'s fixed size and no margin',
    codes: ['refused_percent:width', 'refused_unreadable:margin-left', 'refused_unreadable:margin-right'],
    nodes: ['div.wrap#7', 'div.half#8', 'div.mid#9'],
  },
  'h10-margins': {
    reason: 'the margins of two block siblings collapse in a browser (the larger one counts) and a flow in the model adds them; the importer lists nothing; the boxes after the first pair move by the difference',
    codes: [],
    nodes: ['body', 'div.col#7', 'div.b#9', 'div.f#10', 'div.a#11', 'div.b#12'],
  },
};

// The losses of docs/import-html.md section 3.7 that the round trip of the oracle can see, as `node:from>to` of the kind of a node, by fixture. The
// others of that section (the ids, a stored size of a hug or a fill, a variable that comes back as its value, the radius of an ellipse, a
// shader) change no box and no kind of these files; the Rust round trip (crates/features/html-import/html-import/tests/import.rs) lists them. An entry
// fails the gate on the day the loss is gone, and a loss that is not listed fails it.
export const IMPORT_LOSSES = {
  'f22-no-fill': ['n3:frame>rect'],
};
