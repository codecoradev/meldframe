const LOREM = 'Design tools should produce layouts that a browser can reproduce exactly, so that what you see is what you ship.';
const LONG = 'Auto layout is a small, closed vocabulary: direction, gap, padding, alignment, and hug or fill sizing. Every one of those has a direct CSS counterpart, which is why the export can be verified with a plain geometry comparison.';

const mk = (type) => (id, parentId, p = {}) => ({ id, name: id, type, parentId, ...p });
export const frame = mk('frame');
export const rect = mk('rect');
export const ell = mk('ellipse');
export const text = (id, parentId, t, p = {}) => ({ id, name: id, type: 'text', parentId, text: t, sw: 'hug', sh: 'hug', fill: '#111111', ...p });

const fixtures = [];
const add = (name, desc, nodes, extra = {}) => fixtures.push({ name, desc, nodes, ...extra });

// 1. row, fixed container, padding, gap, align center
add('f01-row-fixed', 'row frame, fixed size, padding 16, gap 12, align center, mixed fixed children + ellipse', [
  frame('r', null, { x: 20, y: 20, w: 600, h: 120, layout: 'row', padding: 16, gap: 12, align: 'center', fill: '#eeeeee' }),
  rect('a', 'r', { w: 80, h: 40, fill: '#f87171' }),
  rect('b', 'r', { w: 120, h: 80, fill: '#60a5fa', radius: 12 }),
  rect('c', 'r', { w: 60, h: 60, fill: '#34d399' }),
  ell('d', 'r', { w: 50, h: 50, fill: '#fbbf24' }),
]);

// 2. column hug/hug with text, nested hug row
add('f02-col-hug', 'column hug/hug, padding 24, gap 10, hug text and a nested hug row of buttons', [
  frame('c', null, { x: 30, y: 30, sw: 'hug', sh: 'hug', layout: 'column', padding: 24, gap: 10, fill: '#f4f4f5' }),
  text('h', 'c', 'Heading', { fontSize: 24 }),
  text('s', 'c', 'Subheading text goes here', { fontSize: 14 }),
  rect('r', 'c', { w: 200, h: 40, fill: '#a78bfa' }),
  frame('row', 'c', { sw: 'hug', sh: 'hug', layout: 'row', padding: 8, gap: 6, fill: '#d4d4d8' }),
  text('t1', 'row', 'Ok', { fontSize: 12 }),
  text('t2', 'row', 'Cancel', { fontSize: 12 }),
]);

// 3. fill behaviour
add('f03-fill', 'fill on main and cross axis, equal-padding fill siblings, fill height in a column', [
  frame('r', null, { x: 20, y: 20, w: 600, h: 100, layout: 'row', padding: 10, gap: 10, fill: '#eeeeee' }),
  rect('r1', 'r', { w: 100, h: 80, fill: '#f87171' }),
  rect('r2', 'r', { sw: 'fill', sh: 'fill', fill: '#60a5fa' }),
  rect('r3', 'r', { sw: 'fill', h: 60, fill: '#34d399' }),
  frame('r4', 'r', { sw: 'fill', sh: 'fill', layout: 'row', padding: 30, fill: '#fbbf24' }),
  frame('col', null, { x: 660, y: 20, w: 200, h: 300, layout: 'column', padding: 12, gap: 8, fill: '#eeeeee' }),
  rect('c1', 'col', { sw: 'fill', h: 40, fill: '#f87171' }),
  rect('c2', 'col', { sw: 'fill', sh: 'fill', fill: '#60a5fa' }),
  rect('c3', 'col', { w: 100, h: 40, fill: '#34d399' }),
  frame('r5', null, { x: 20, y: 160, w: 601, h: 80, layout: 'row', padding: 7, gap: 5, fill: '#eeeeee' }),
  rect('t1', 'r5', { sw: 'fill', sh: 'fill', fill: '#f87171' }),
  rect('t2', 'r5', { sw: 'fill', sh: 'fill', fill: '#60a5fa' }),
  rect('t3', 'r5', { sw: 'fill', sh: 'fill', fill: '#34d399' }),
]);

// 4. justify / align / free-layout parent
add('f04-justify-align', 'justify start/center/end/between, column align end, free (layout none) container with absolute children', [
  frame('free', null, { x: 10, y: 10, w: 900, h: 560, layout: 'none', fill: '#fafafa' }),
  ...['start', 'center', 'end', 'between'].flatMap((j, i) => [
    frame(`j${i}`, 'free', { x: 20, y: 20 + i * 100, w: 400, h: 80, layout: 'row', padding: 8, gap: 8, justify: j, align: 'center', fill: '#e4e4e7' }),
    rect(`j${i}a`, `j${i}`, { w: 50, h: 30, fill: '#f87171' }),
    rect(`j${i}b`, `j${i}`, { w: 70, h: 50, fill: '#60a5fa' }),
    rect(`j${i}c`, `j${i}`, { w: 40, h: 20, fill: '#34d399' }),
  ]),
  frame('ce', 'free', { x: 460, y: 20, w: 200, h: 300, layout: 'column', padding: 10, gap: 6, align: 'end', justify: 'center', fill: '#e4e4e7' }),
  rect('cea', 'ce', { w: 60, h: 40, fill: '#f87171' }),
  rect('ceb', 'ce', { w: 120, h: 40, fill: '#60a5fa' }),
  frame('clip', 'free', { x: 700, y: 20, w: 150, h: 100, clip: true, fill: '#e4e4e7' }),
  rect('clipc', 'clip', { x: 100, y: 60, w: 120, h: 80, fill: '#f87171' }),
]);

// 5. text wrapping
add('f05-text-wrap', 'wrapping text: fill width, fixed width + hug height, explicit newline, long word, narrow column', [
  frame('c', null, { x: 20, y: 20, w: 320, sh: 'hug', layout: 'column', padding: 20, gap: 12, fill: '#f4f4f5' }),
  text('p1', 'c', LOREM, { sw: 'fill', fontSize: 16 }),
  text('p2', 'c', LONG, { sw: 'fill', fontSize: 14 }),
  text('p3', 'c', LOREM, { sw: 'fixed', w: 200, fontSize: 15 }),
  text('p4', 'c', 'Line one\nLine two is longer\nThree', { fontSize: 13 }),
  text('p5', 'c', 'Supercalifragilisticexpialidocious_and_more_without_spaces here', { sw: 'fill', fontSize: 16 }),
  frame('n', null, { x: 400, y: 20, w: 180, sh: 'hug', layout: 'column', padding: 12, gap: 8, fill: '#f4f4f5' }),
  text('n1', 'n', 'Narrow column with a rather large headline', { sw: 'fill', fontSize: 20 }),
  text('n2', 'n', 'Ünïcödé — “quotes”, dashes… and 日本語 fallback glyphs', { sw: 'fill', fontSize: 14 }),
]);

// 6. realistic card
add('f06-card', 'social card: nested row/column, avatar, wrapped body, footer with justify between and a padded button', [
  frame('card', null, { x: 20, y: 20, w: 360, sh: 'hug', layout: 'column', padding: 20, gap: 16, radius: 16, fill: '#ffffff' }),
  frame('head', 'card', { sw: 'fill', sh: 'hug', layout: 'row', padding: 0, gap: 12, align: 'center', fill: 'transparent' }),
  ell('av', 'head', { w: 48, h: 48, fill: '#a78bfa' }),
  frame('who', 'head', { sw: 'fill', sh: 'hug', layout: 'column', padding: 0, gap: 2, fill: 'transparent' }),
  text('name', 'who', 'Ada Lovelace', { fontSize: 16, fill: '#111111' }),
  text('handle', 'who', '@ada · 2h', { fontSize: 13, fill: '#71717a' }),
  text('body', 'card', 'Shipping the first auto layout export today. The geometry matches the canvas to the pixel, or it does not ship.', { sw: 'fill', fontSize: 15 }),
  frame('foot', 'card', { sw: 'fill', sh: 'hug', layout: 'row', padding: 0, gap: 8, justify: 'between', align: 'center', fill: 'transparent' }),
  text('likes', 'foot', '128 likes', { fontSize: 13, fill: '#71717a' }),
  frame('btn', 'foot', { sw: 'hug', sh: 'hug', layout: 'row', padding: 12, gap: 0, radius: 8, fill: '#111111', align: 'center' }),
  text('btnt', 'btn', 'Follow', { fontSize: 14, fill: '#ffffff' }),
]);

// 7. decoration: border / shadow / radius, probed at fixed page coordinates by probes.mjs
const bd = (width, position, color = '#ff0000') => ({ width, color, position });
const decor = [
  frame('stage', null, { w: 1000, h: 520, layout: 'none', fill: '#ffffff', clip: false }),
  rect('bi', 'stage', { x: 40, y: 40, w: 120, h: 80, fill: '#00ff00', border: bd(8, 'inside') }),
  rect('bc', 'stage', { x: 240, y: 40, w: 120, h: 80, fill: '#00ff00', border: bd(8, 'center') }),
  rect('bo', 'stage', { x: 440, y: 40, w: 120, h: 80, fill: '#00ff00', border: bd(8, 'outside') }),
  rect('bl', 'stage', { x: 640, y: 40, w: 120, h: 80, fill: '#00ff00', radius: 40, border: bd(4, 'inside') }),
  rect('s1', 'stage', { x: 40, y: 200, w: 120, h: 80, fill: '#00ff00', shadows: [{ x: 20, y: 0, blur: 0, spread: 0, color: '#ff0000ff' }] }),
  rect('s2', 'stage', { x: 240, y: 200, w: 120, h: 80, fill: '#00ff00', shadows: [{ x: 0, y: 0, blur: 0, spread: 10, color: '#0000ffff' }] }),
  rect('s3', 'stage', { x: 440, y: 200, w: 120, h: 80, fill: '#00ff00', shadows: [{ x: 0, y: 0, blur: 20, spread: 0, color: '#000000ff' }] }),
  rect('s4', 'stage', { x: 640, y: 200, w: 120, h: 80, fill: '#00ff00', shadows: [{ x: 20, y: 0, blur: 0, spread: 0, color: '#ff0000ff' }, { x: 20, y: 0, blur: 0, spread: 0, color: '#0000ffff' }] }),
  ell('se', 'stage', { x: 840, y: 200, w: 100, h: 100, fill: '#00ff00', border: bd(6, 'inside', '#ff0000'), shadows: [{ x: 0, y: 8, blur: 12, spread: -2, color: '#00000066' }] }),
  frame('cf', 'stage', { x: 40, y: 340, w: 200, h: 120, layout: 'row', padding: 12, gap: 8, radius: 16, fill: '#e4e4e7', border: bd(2, 'inside', '#6366f1'), shadows: [{ x: 0, y: 4, blur: 16, spread: 0, color: '#00000033' }] }),
  rect('cf1', 'cf', { sw: 'fill', sh: 'fill', fill: '#f87171' }),
  rect('cf2', 'cf', { sw: 'fill', sh: 'fill', fill: '#60a5fa' }),
];
add('f07-decor', 'border inside/center/outside, hard and blurred shadows, shadow order, ellipse border+shadow, decorated auto layout frame', decor);

// 8. images (the export refers to assets/<id>; the harness serves a 200x100 test PNG there)
const AID = 'a'.repeat(64);
const img = (fit) => ({ assetId: AID, fit });
add('f08-image', 'image fills: cover / contain / fill in a square box, image on a rounded rect and an ellipse', [
  frame('stage', null, { w: 1000, h: 200, layout: 'none', fill: '#ffffff', clip: false }),
  rect('ic', 'stage', { x: 40, y: 40, w: 100, h: 100, fill: '#00ff00', image: img('cover') }),
  rect('in', 'stage', { x: 240, y: 40, w: 100, h: 100, fill: '#00ff00', image: img('contain') }),
  rect('if', 'stage', { x: 440, y: 40, w: 100, h: 100, fill: '#00ff00', image: img('fill') }),
  rect('ir', 'stage', { x: 640, y: 40, w: 160, h: 100, fill: '#00ff00', radius: 24, image: img('cover') }),
  ell('ie', 'stage', { x: 840, y: 40, w: 100, h: 100, fill: '#00ff00', image: img('cover') }),
]);

// 9. typography
add('f09-typography', 'font weight changes hug width; text-align in fixed boxes incl. wrapped multi-line text', [
  frame('row', null, { x: 20, y: 20, sw: 'hug', sh: 'hug', layout: 'row', padding: 12, gap: 16, fill: '#f4f4f5' }),
  text('w4', 'row', 'Weight four hundred', { fontSize: 20, fontWeight: 400 }),
  text('w7', 'row', 'Weight seven hundred', { fontSize: 20, fontWeight: 700 }),
  text('w9', 'row', 'Weight nine hundred', { fontSize: 20, fontWeight: 900 }),
  frame('al', null, { x: 20, y: 120, w: 700, h: 260, layout: 'row', padding: 10, gap: 10, fill: '#f4f4f5' }),
  frame('al1', 'al', { sw: 'fill', sh: 'fill', layout: 'column', padding: 8, gap: 8, fill: '#e4e4e7' }),
  text('l1', 'al1', 'Left aligned wrapped paragraph of text that spans lines', { sw: 'fill', fontSize: 16, textAlign: 'left' }),
  frame('al2', 'al', { sw: 'fill', sh: 'fill', layout: 'column', padding: 8, gap: 8, fill: '#e4e4e7' }),
  text('l2', 'al2', 'Centered wrapped paragraph of text that spans lines', { sw: 'fill', fontSize: 16, textAlign: 'center' }),
  frame('al3', 'al', { sw: 'fill', sh: 'fill', layout: 'column', padding: 8, gap: 8, fill: '#e4e4e7' }),
  text('l3', 'al3', 'Right aligned wrapped paragraph of text that spans lines', { sw: 'fill', fontSize: 16, textAlign: 'right', fontWeight: 700 }),
]);

// 10. composition: 3 equal cards in a row with hug + fill mix and a free badge container
const cards = [];
for (let i = 0; i < 3; i++) {
  cards.push(
    frame(`k${i}`, 'deck', { sw: 'fill', sh: 'hug', layout: 'column', padding: 20, gap: 10, radius: 12, fill: '#f4f4f5' }),
    rect(`ki${i}`, `k${i}`, { sw: 'fill', h: 90 + i * 10, fill: ['#f87171', '#60a5fa', '#34d399'][i] }),
    text(`kt${i}`, `k${i}`, ['Fast', 'Faithful and predictable', 'Agent-friendly with structured errors'][i], { sw: 'fill', fontSize: 18 }),
    text(`kd${i}`, `k${i}`, LOREM.slice(0, 40 + i * 25), { sw: 'fill', fontSize: 13, fill: '#52525b' }),
  );
}
add('f10-deck', 'three fill cards of different content height in a hug-height row, nested fills', [
  frame('deck', null, { x: 20, y: 20, w: 1000, sh: 'hug', layout: 'row', padding: 24, gap: 24, fill: '#ffffff' }),
  ...cards,
]);

// 12. fractional geometry
add('f12-fractional', 'fractional sizes, gaps, paddings, font sizes and positions; three fill siblings with a fractional share', [
  frame('r', null, { x: 20.5, y: 20.25, w: 333.333, h: 90.7, layout: 'row', padding: 5.5, gap: 7.5, fill: '#eeeeee' }),
  rect('a', 'r', { sw: 'fill', sh: 'fill', fill: '#f87171' }),
  rect('b', 'r', { sw: 'fill', sh: 'fill', fill: '#60a5fa' }),
  rect('c', 'r', { sw: 'fill', sh: 'fill', fill: '#34d399' }),
  frame('h', null, { x: 400.4, y: 20.6, sw: 'hug', sh: 'hug', layout: 'column', padding: 3.3, gap: 2.2, fill: '#eeeeee' }),
  text('t1', 'h', 'Fractional 13.5px', { fontSize: 13.5 }),
  text('t2', 'h', 'Fractional 14.3px text', { fontSize: 14.3 }),
  text('t3', 'h', 'Fractional 11.7px text here', { fontSize: 11.7 }),
  frame('j', null, { x: 20, y: 160, w: 301, h: 50, layout: 'row', padding: 0, gap: 0, justify: 'between', fill: '#eeeeee' }),
  rect('j1', 'j', { w: 33.3, h: 20, fill: '#f87171' }),
  rect('j2', 'j', { w: 33.3, h: 20, fill: '#60a5fa' }),
  rect('j3', 'j', { w: 33.3, h: 20, fill: '#34d399' }),
  frame('k', null, { x: 20, y: 240, w: 301, h: 50, layout: 'row', padding: 0, gap: 0, justify: 'center', align: 'center', fill: '#eeeeee' }),
  rect('k1', 'k', { w: 33.3, h: 21, fill: '#f87171' }),
  rect('k2', 'k', { w: 33.3, h: 20, fill: '#60a5fa' }),
]);

// 13. overflow behaviour (content larger than container)
add('f13-overflow', 'children larger than the container: fixed overflow, justify center/end/between with negative free space, fill with no space', [
  frame('a', null, { x: 20, y: 20, w: 200, h: 60, layout: 'row', padding: 10, gap: 10, justify: 'center', fill: '#eeeeee', clip: false }),
  rect('a1', 'a', { w: 120, h: 30, fill: '#f87171' }),
  rect('a2', 'a', { w: 120, h: 30, fill: '#60a5fa' }),
  frame('b', null, { x: 20, y: 120, w: 200, h: 60, layout: 'row', padding: 10, gap: 10, justify: 'end', fill: '#eeeeee', clip: false }),
  rect('b1', 'b', { w: 120, h: 30, fill: '#f87171' }),
  rect('b2', 'b', { w: 120, h: 30, fill: '#60a5fa' }),
  frame('c', null, { x: 20, y: 220, w: 200, h: 60, layout: 'row', padding: 10, gap: 10, justify: 'between', fill: '#eeeeee', clip: false }),
  rect('c1', 'c', { w: 120, h: 30, fill: '#f87171' }),
  rect('c2', 'c', { w: 120, h: 30, fill: '#60a5fa' }),
  frame('d', null, { x: 260, y: 20, w: 200, h: 60, layout: 'row', padding: 10, gap: 10, fill: '#eeeeee', clip: false }),
  rect('d1', 'd', { w: 120, h: 30, fill: '#f87171' }),
  rect('d2', 'd', { sw: 'fill', h: 30, fill: '#60a5fa' }),
  rect('d3', 'd', { w: 120, h: 30, fill: '#34d399' }),
  frame('e', null, { x: 260, y: 120, w: 100, h: 100, layout: 'column', padding: 10, gap: 10, align: 'center', fill: '#eeeeee', clip: false }),
  rect('e1', 'e', { w: 160, h: 50, fill: '#f87171' }),
  rect('e2', 'e', { w: 60, h: 80, fill: '#60a5fa' }),
  frame('f', null, { x: 400, y: 120, w: 100, h: 100, layout: 'column', padding: 10, gap: 10, align: 'end', fill: '#eeeeee', clip: false }),
  rect('f1', 'f', { w: 160, h: 50, fill: '#f87171' }),
  frame('g', null, { x: 20, y: 320, sw: 'hug', sh: 'hug', layout: 'row', padding: 10, gap: 10, fill: '#eeeeee' }),
  rect('g1', 'g', { sw: 'fill', sh: 'fill', w: 90, h: 70, fill: '#f87171' }),
  rect('g2', 'g', { w: 50, h: 30, fill: '#60a5fa' }),
  frame('h', null, { x: 260, y: 320, sw: 'hug', sh: 'hug', layout: 'column', padding: 0, gap: 0, fill: '#eeeeee' }),
]);

// 14. text edge cases: whitespace, break opportunities, scripts
const edge = ['two  spaces   inside and trailing   ', '   leading spaces', 'well-known state-of-the-art self-hosted', 'path/to/some/very/long/file/name.txt and https://example.com/a/very/long/url?x=1&y=2',
  '日本語のテキストは単語の間にスペースがありません。折り返しの位置が違う可能性があります。', 'Emoji 🙂🚀 and café naïve résumé', 'a b c d e f g h i j k l m n o p q r s t u v w x y z', 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789', 'tab\tseparated\tvalues', 'Trailing newline\n'];
add('f14-text-edge', 'text edge cases at fill width 200 (wrap) and hug: repeated spaces, hyphens, slashes, URL, CJK, emoji, tabs', [
  frame('c', null, { x: 20, y: 20, w: 240, sh: 'hug', layout: 'column', padding: 20, gap: 8, fill: '#f4f4f5' }),
  ...edge.map((t, i) => text(`w${i}`, 'c', t, { sw: 'fill', fontSize: 14 })),
  frame('d', null, { x: 320, y: 20, sw: 'hug', sh: 'hug', layout: 'column', padding: 20, gap: 8, fill: '#f4f4f5' }),
  ...edge.map((t, i) => text(`h${i}`, 'd', t, { fontSize: 14 })),
]);

// 15. font sizes
add('f15-font-sizes', 'hug text at font sizes 8..96 and wrapped text at the extremes (line-height rounding)', [
  frame('c', null, { x: 20, y: 20, sw: 'hug', sh: 'hug', layout: 'column', padding: 10, gap: 4, fill: '#f4f4f5' }),
  ...[8, 9, 10, 11, 12, 13, 14, 16, 18, 20, 24, 32, 48, 72, 96].map((s) => text(`s${s}`, 'c', `Size ${s} Hamburgefonstiv`, { fontSize: s })),
  frame('d', null, { x: 700, y: 20, w: 260, sh: 'hug', layout: 'column', padding: 10, gap: 4, fill: '#f4f4f5' }),
  ...[8, 11, 13, 17, 29, 41].map((s) => text(`ws${s}`, 'd', LOREM, { sw: 'fill', fontSize: s })),
]);

// 16. the box properties of M3: padding per side, margins, an absolute child in an auto layout frame, opacity
add('f16-box-props', 'padding per side, margins, an absolute child of an auto layout frame, opacity, margin on text', [
  frame('r', null, { x: 20, y: 20, w: 500, h: 160, layout: 'row', padT: 4, padR: 20, padB: 12, padL: 8, gap: 10, align: 'center', fill: '#eeeeee' }),
  rect('a', 'r', { w: 60, h: 40, marL: 6, marR: 3, marT: 2, marB: 9, fill: '#f87171' }),
  rect('b', 'r', { sw: 'fill', h: 50, marL: 5, fill: '#60a5fa', opacity: 0.5 }),
  rect('c', 'r', { w: 40, h: 40, absolute: true, x: 400, y: 100, fill: '#34d399' }),
  frame('col', null, { x: 20, y: 220, sw: 'hug', sh: 'hug', layout: 'column', padT: 10, padR: 2, padB: 6, padL: 14, gap: 4, fill: '#eeeeee' }),
  text('t', 'col', 'Margin text', { fontSize: 14, marT: 3, marL: 7 }),
  rect('d', 'col', { w: 50, h: 20, marB: 5, marT: 1 }),
]);

// 17. rule G1: fill on an axis where the parent hugs. The canvas gives the node what the parent hugs; the export keeps its own size.
add('f17-fill-under-hug', 'fill width and height under a hug parent: the core takes the size the parent hugs, the export keeps the node\'s own (rule G1; known gap)', [
  frame('r', null, { x: 20, y: 20, sw: 'hug', sh: 'hug', layout: 'column', padding: 12, gap: 0, fill: '#eeeeee' }),
  rect('a', 'r', { w: 81, h: 83, fill: '#f87171' }),
  rect('b', 'r', { w: 49, h: 62, sw: 'fill', fill: '#60a5fa' }),
]);

// 18. opacity of a node and of what is inside it, probed on pixels (probes.mjs)
add('f18-opacity', 'opacity 0.5 on a rect, and on a frame whose child is painted with the frame', [
  frame('stage', null, { w: 300, h: 100, layout: 'none', fill: '#ffffff', clip: false }),
  rect('o1', 'stage', { x: 20, y: 20, w: 100, h: 60, fill: '#ff0000', opacity: 0.5 }),
  frame('o2', 'stage', { x: 160, y: 20, w: 100, h: 60, layout: 'none', fill: '#00ff00', opacity: 0.5, clip: false }),
  rect('o3', 'o2', { x: 20, y: 10, w: 40, h: 40, fill: '#0000ff' }),
]);

// 19. gradients, probed on pixels (probes.mjs): hard-edged stops make the colours exact, so the angle convention (0 points up, 90
// right, 180 down), the stop positions, the circle of a radial gradient, the layering (over the fill, under the image) and the
// clipping by a radius and an ellipse can be read at fixed points
const hard = (a, b, angle, extra = {}) => ({ kind: 'linear', ...(angle === undefined ? {} : { angle }), stops: [{ pos: 0, color: a }, { pos: 0.5, color: a }, { pos: 0.5, color: b }, { pos: 1, color: b }], ...extra });
const hardRadial = (a, b) => ({ kind: 'radial', stops: [{ pos: 0, color: a }, { pos: 0.5, color: a }, { pos: 0.5, color: b }, { pos: 1, color: b }] });
const R = '#ff0000', G = '#00ff00', B = '#0000ff';
const gx = (i) => 40 + 180 * i;
add('f19-gradient', 'linear gradients at 90, 180 (the default), 0 and 45 degrees, stop positions, radial on a square and a wide box, a gradient over a fill, on an ellipse and a rounded rect, and over a fill under an image', [
  frame('stage', null, { w: 1160, h: 300, layout: 'none', fill: '#ffffff', clip: false }),
  rect('g90', 'stage', { x: gx(0), y: 40, w: 120, h: 80, gradient: hard(R, B, 90) }),
  rect('g180', 'stage', { x: gx(1), y: 40, w: 120, h: 80, gradient: hard(R, B) }),
  rect('g0', 'stage', { x: gx(2), y: 40, w: 120, h: 80, gradient: hard(R, B, 0) }),
  rect('g45', 'stage', { x: gx(3), y: 40, w: 120, h: 80, gradient: hard(R, B, 45) }),
  rect('gs', 'stage', { x: gx(4), y: 40, w: 120, h: 80, gradient: { kind: 'linear', angle: 90, stops: [{ pos: 0, color: '#000000' }, { pos: 1, color: '#ffffff' }] } }),
  rect('gp', 'stage', { x: gx(5), y: 40, w: 120, h: 80, gradient: { kind: 'linear', angle: 90, stops: [{ pos: 0, color: R }, { pos: 0.25, color: R }, { pos: 0.75, color: B }, { pos: 1, color: B }] } }),
  rect('rs', 'stage', { x: gx(0), y: 160, w: 100, h: 100, gradient: hardRadial(R, B) }),
  rect('rw', 'stage', { x: gx(1), y: 160, w: 160, h: 80, gradient: hardRadial(R, B) }),
  rect('go', 'stage', { x: gx(2), y: 160, w: 120, h: 80, fill: G, gradient: hard(B, '#0000ff00', 90) }),
  ell('ge', 'stage', { x: gx(3), y: 160, w: 100, h: 100, gradient: hard(R, B, 90) }),
  rect('gr', 'stage', { x: gx(4), y: 160, w: 120, h: 80, radius: 30, gradient: hard(R, B, 90) }),
  rect('gi', 'stage', { x: gx(5), y: 160, w: 100, h: 100, fill: '#ffffff', gradient: hard(G, '#ffff00'), image: img('contain') }),
]);

// 20 and 21. fills bound to a variable (ADR-0017, ADR-0019): a colour variable of a collection with two modes, an alias, an alias
// across collections, a binding that does not resolve, and a bound fill under a gradient and under an image; also the two other
// fields that bind a number variable, the radius and the opacity (the second mode has no radius and is opaque). f20 shows the
// default mode, f21 the same file with the second mode picked for the collection `brand`: the export says it as an attribute of <html>
const VARS = {
  collections: [{ id: 'brand', name: 'Brand', modes: [{ id: 'light', name: 'Light' }, { id: 'dark', name: 'Dark' }] }, { id: 'canvas', name: 'Canvas', modes: [{ id: 'base', name: 'Base' }] }],
  variables: [
    { id: 'v1', collection: 'brand', name: 'color/primary', type: 'color', values: { light: '#ff0000', dark: '#0000ff' } },
    { id: 'v2', collection: 'brand', name: 'color/surface', type: 'color', values: { light: '#00ff00', dark: '#ffff00' } },
    { id: 'v4', collection: 'brand', name: 'radius/round', type: 'number', values: { light: 40, dark: 0 } },
    { id: 'v5', collection: 'brand', name: 'opacity/muted', type: 'number', values: { light: 0.5, dark: 1 } },
    { id: 'v3', collection: 'brand', name: 'color/accent', type: 'color', values: { light: { var: 'v1' }, dark: { var: 'v1' } } },
    { id: 'vp', collection: 'canvas', name: 'bg/page', type: 'color', values: { base: '#ffffff' } },
    { id: 'vx', collection: 'canvas', name: 'bg/brand', type: 'color', values: { base: { var: 'v1' } } },
  ],
};
const bound = (id) => ({ var: id });
const vx = (i) => 40 + 180 * (i % 5), vy = (i) => [40, 170, 300][Math.floor(i / 5)];
const variableNodes = () => [
  frame('stage', null, { w: 1000, h: 400, layout: 'none', fill: bound('vp'), clip: false }),
  rect('a', 'stage', { x: vx(0), y: vy(0), w: 100, h: 80, fill: bound('v1') }),
  rect('b', 'stage', { x: vx(1), y: vy(1), w: 100, h: 80, fill: bound('v3') }),
  rect('c', 'stage', { x: vx(2), y: vy(2), w: 100, h: 80, radius: 24, fill: bound('v2') }),
  ell('d', 'stage', { x: vx(3), y: vy(3), w: 80, h: 80, fill: bound('v1') }),
  frame('e', 'stage', { x: vx(4), y: vy(4), w: 100, h: 80, layout: 'row', padding: 10, fill: bound('v2'), clip: false }),
  rect('e1', 'e', { w: 40, h: 40, fill: bound('v1') }),
  rect('f', 'stage', { x: vx(5), y: vy(5), w: 100, h: 80, fill: bound('vx') }),
  rect('g', 'stage', { x: vx(6), y: vy(6), w: 100, h: 80, fill: bound('missing') }),
  rect('h', 'stage', { x: vx(7), y: vy(7), w: 100, h: 80, fill: bound('v2'), gradient: hard(B, '#0000ff00', 90) }),
  rect('i', 'stage', { x: vx(8), y: vy(8), w: 100, h: 80, fill: bound('v2'), image: img('contain') }),
  rect('j', 'stage', { x: vx(9), y: vy(9), w: 100, h: 80, fill: G, radius: bound('v4') }),
  rect('k', 'stage', { x: vx(10), y: vy(10), w: 100, h: 80, fill: R, opacity: bound('v5') }),
];
add('f20-variable-fill', 'fills bound to a variable in the default mode: direct, alias, alias across collections, unresolved, under a gradient and under an image; a bound radius and a bound opacity', variableNodes(), { variables: VARS });
add('f21-variable-modes', 'the same fills with the dark mode of the collection Brand picked', variableNodes(), { variables: VARS, modes: { brand: 'dark' } });

// 22. no fill (`none`): nothing is painted for it, what is behind shows; a node with no `fill` at all takes the default fill
add('f22-no-fill', 'fill none on a rect, a frame over a coloured box (with a border), a layout frame around a child, an ellipse ring, a gradient and an image; a rect with no fill at all', [
  frame('stage', null, { w: 1000, h: 300, layout: 'none', fill: '#ffffff', clip: false }),
  rect('n1', 'stage', { x: 40, y: 40, w: 100, h: 80, fill: 'none' }),
  rect('bg', 'stage', { x: 200, y: 40, w: 200, h: 100, fill: G }),
  frame('n3', 'stage', { x: 240, y: 60, w: 120, h: 60, layout: 'none', fill: 'none', border: bd(6, 'inside', R), clip: false }),
  frame('n4', 'stage', { x: 440, y: 40, w: 160, h: 100, layout: 'row', padding: 10, fill: 'none', clip: false }),
  rect('n4c', 'n4', { w: 60, h: 60, fill: B }),
  rect('n7', 'stage', { x: 640, y: 40, w: 100, h: 80 }),
  rect('n8', 'stage', { x: 780, y: 40, w: 160, h: 80, fill: 'none', gradient: hard(R, '#ff000000', 90) }),
  ell('n9', 'stage', { x: 40, y: 170, w: 100, h: 100, fill: 'none', border: bd(8, 'inside', R) }),
  rect('n10', 'stage', { x: 200, y: 170, w: 100, h: 100, fill: 'none', image: img('contain') }),
]);

export default fixtures;
