// The hand-written pages of the importer's oracle (E4.4; fixtures `h01` onward): HTML of the kinds docs/import-html.md section 3 supports.
// The browser loads each page as written; the importer reads the same text; every element with a `data-name` is compared (import.mjs). The
// text is pinned the way the export pins it, because the importer drops `line-height` and the layout core ceils what Pixi measured: a
// line is `line-height` px, the height Pixi measures for that size, and a hug text has `min-width` of the measured width (contract rule G4).
import gapPages from './pages-gaps.mjs';

export const RESET = '*{box-sizing:border-box}body{margin:0}p{margin:0}';
/** Pixi's line heights for Inter, by size: the height the core uses for one line. */
export const LINE = { 12: 15, 14: 18, 16: 20, 18: 23, 20: 25, 24: 30 };
const lh = (s) => `font-size:${s}px;line-height:${LINE[s]}px`;
const box = (name, w, h, bg, extra = '') => `<div data-name="${name}" style="width:${w}px;height:${h}px;background:${bg};${extra}"></div>`;

export const supported = [
  { name: 'h01-row', pixels: true, html: `<style>${RESET}.row{display:flex;flex-direction:row;align-items:center;gap:12px;padding:16px;width:600px;height:120px;background:#eeeeee}</style>
<div class="row" data-name="row">${box('a', 80, 40, '#f87171')}${box('b', 120, 80, '#60a5fa')}${box('c', 60, 60, '#34d399')}</div>` },
  { name: 'h02-column', pixels: true, html: `<style>${RESET}</style>
<div data-name="col" style="display:flex;flex-direction:column;justify-content:space-between;align-items:flex-end;gap:10px;padding:20px 10px;width:300px;height:400px;background:#f4f4f5">${box('a', 100, 50, '#f87171')}${box('b', 200, 60, '#60a5fa')}${box('c', 50, 70, '#34d399')}</div>` },
  { name: 'h03-spacing', pixels: true, html: `<style>${RESET}</style>
<div data-name="outer" style="display:flex;flex-direction:column;gap:8px;padding:10px 20px 30px 5px;width:400px;background:#eeeeee">
  <div data-name="row" style="display:flex;gap:14px;padding:4px;background:#d4d4d8">${box('a', 50, 30, '#f87171', 'margin:3px 0 0 7px')}${box('b', 60, 40, '#60a5fa', 'margin-right:9px')}${box('c', 40, 20, '#34d399')}</div>
  ${box('d', 100, 25, '#fbbf24', 'margin-top:12px;margin-left:30px')}
  <div data-name="nest" style="display:flex;flex-direction:column;padding:0 11px;background:#a78bfa">${box('e', 90, 33, '#ffffff')}</div>
</div>` },
  { name: 'h04-fill', pixels: true, html: `<style>${RESET}.r{display:flex;width:600px;height:100px;padding:10px;gap:10px;background:#eeeeee}.f{flex:1}</style>
<div data-name="r" class="r">${box('a', 100, 80, '#f87171')}<div data-name="b" class="f" style="align-self:stretch;background:#60a5fa"></div><div data-name="c" style="flex:1 1 0%;height:60px;background:#34d399"></div><div data-name="d" style="flex:1;padding:0;display:flex;align-self:stretch;background:#fbbf24"></div></div>
<div data-name="col" style="display:flex;flex-direction:column;width:200px;height:300px;padding:12px;gap:8px;background:#eeeeee"><div data-name="c1" style="height:40px;background:#f87171"></div><div data-name="c2" style="flex:1;background:#60a5fa"></div>${box('c3', 100, 40, '#34d399')}</div>` },
  { name: 'h05-text', html: `<style>${RESET}.card{display:flex;flex-direction:column;align-items:flex-start;gap:6px;width:360px;padding:16px;background:#f4f4f5}.row{display:flex;gap:10px;align-items:center}.pin{white-space:pre;width:auto}</style>
<div data-name="card" class="card">
  <p data-name="title" class="pin" style="${lh(24)};font-weight:700;color:#18181b;min-width:133px">Hello world</p>
  <p data-name="body" style="align-self:stretch;${lh(16)};color:#52525b">The quick brown fox jumps over the lazy dog and keeps running along the river bank until night</p>
  <p data-name="centre" style="align-self:stretch;${lh(14)};text-align:center;color:#3f3f46">Centred line</p>
  <p data-name="right" style="align-self:stretch;${lh(14)};text-align:right;font-weight:600">Right line</p>
  <div data-name="line" class="row"><p data-name="k" class="pin" style="${lh(14)};min-width:75px">Hello world</p><p data-name="v" class="pin" style="${lh(20)};font-weight:700;min-width:111px">Hello world</p></div>
</div>` },
];
export const more = [
  { name: 'h06-paint', pixels: true, html: `<style>${RESET}.s{display:flex;gap:20px;padding:20px;width:1000px;background:#ffffff}.b{width:100px;height:80px;flex-shrink:0}</style>
<div data-name="stage" class="s">
  <div data-name="bordered" class="b" style="background:#fde68a;border:4px solid #b45309"></div>
  <div data-name="round" class="b" style="background:#93c5fd;border-radius:16px"></div>
  <div data-name="shadow" class="b" style="background:#ffffff;box-shadow:4px 6px 0 2px #00000080,0 0 0 1px #ff0000"></div>
  <div data-name="grad" class="b" style="background:linear-gradient(90deg,#ff0000,#0000ff)"></div>
  <div data-name="radial" class="b" style="background:radial-gradient(circle farthest-corner,#ff0000,#0000ff)"></div>
  <div data-name="faded" class="b" style="background:#34d399;opacity:0.5"></div>
  <div data-name="clip" class="b" style="background:#e4e4e7;overflow:hidden;border-radius:12px"></div>
  <div data-name="ring" class="b" style="border-radius:50%;background:#f472b6"></div>
</div>` },
  { name: 'h07-cascade', pixels: true, css: '.pill{height:30px;background:#dbeafe}#top{padding:6px}.card .pill{width:120px}.card>.pill.hot{background:#fecaca !important}', html: `<style>${RESET}.card{display:flex;flex-direction:column;gap:6px;width:300px;background:#eeeeee}.pill{width:100px;height:20px;background:#bbf7d0}div.card .hot{height:34px}p.x,div.y{margin:0}</style>
<div data-name="card" id="top" class="card"><div data-name="p1" class="pill"></div><div data-name="p2" class="pill hot"></div><div data-name="p3" class="pill" style="width:222px"></div><div data-name="w" style="display:flex;gap:4px"><div data-name="p4" class="pill"></div><div data-name="p5" class="pill hot"></div></div></div>` },
  { name: 'h08-tailwind', pixels: true, tailwind: true, css: '@import "tailwindcss/utilities.css";\n*,::before,::after{box-sizing:border-box}body{margin:0}p{margin:0}', html: `<div data-name="card" class="flex flex-col gap-[12px] p-[16px] w-[400px] bg-[color:#eeeeee]">
  <div data-name="head" class="flex flex-row items-center justify-between gap-[8px] self-stretch h-[40px]"><div data-name="logo" class="w-[60px] h-[24px] bg-[color:#f87171] rounded-[6px] shrink-0"></div><div data-name="chip" class="w-[40px] h-[24px] bg-[color:#60a5fa] shrink-0"></div></div>
  <div data-name="body" class="flex flex-row gap-[10px] self-stretch"><div data-name="a" class="w-[100px] h-[60px] bg-[color:#34d399] shrink-0"></div><div data-name="b" class="flex-[1_1_0%] h-[60px] bg-[color:#fbbf24] min-w-[0px]"></div></div>
  <div data-name="note" class="absolute left-[420px] top-[10px] w-[50px] h-[50px] bg-[color:#a78bfa]"></div>
</div>` },
  { name: 'h09-no-reset', html: `<style>.c{display:flex;flex-direction:column;gap:10px;width:300px;padding:12px;border:3px solid #1d4ed8;background:#eff6ff}.k{width:120px;height:40px;padding:5px;border:2px solid #000000;background:#fecaca}</style>
<div data-name="c" class="c"><div data-name="k1" class="k"></div><div data-name="k2" class="k" style="margin-left:15px"></div><p data-name="p" style="${lh(16)}">Hello world</p><p data-name="q" style="${lh(14)};margin:4px 0 0">A second paragraph</p></div>` },
  { name: 'h10-absolute', pixels: true, html: `<style>${RESET}</style>
<div data-name="stage" style="position:relative;width:500px;height:300px;background:#eeeeee">${box('a', 100, 60, '#f87171', 'position:absolute;left:20px;top:30px')}${box('b', 80, 80, '#60a5fa', 'position:absolute;left:300px;top:100px')}
<div data-name="flow" style="position:absolute;left:150px;top:10px;display:flex;gap:6px;padding:6px;background:#d4d4d8">${box('c', 30, 30, '#34d399')}${box('d', 30, 30, '#fbbf24')}</div></div>
<div data-name="rowhost" style="position:relative;display:flex;gap:10px;padding:8px;width:300px;height:100px;background:#eeeeee">${box('e', 50, 50, '#a78bfa')}${box('f', 50, 50, '#f472b6', 'position:absolute;left:200px;top:40px')}${box('g', 50, 50, '#14b8a6')}</div>` },
  { name: 'h11-block-flow', html: `<style>${RESET}.blk{padding:10px;width:400px;background:#eeeeee}</style>
<div data-name="blk" class="blk"><div data-name="a" style="height:30px;background:#f87171"></div><div data-name="b" style="height:40px;background:#60a5fa;width:200px"></div><div data-name="c" style="height:20px;background:#34d399;padding:4px"></div>
<p data-name="t" style="${lh(16)}">Hello world and more words to wrap over a few lines inside this block of text</p></div>` },
  { name: 'h12-units-colours', pixels: true, html: `<style>${RESET}html{font-size:16px}</style>
<div data-name="u" style="display:flex;flex-direction:column;gap:0.5rem;padding:1rem 2rem;width:300pt;background:rgb(238,238,238)"><div data-name="a" style="width:10rem;height:3em;background:hsl(0,100%,50%)"></div><div data-name="b" style="width:7.5em;height:30px;font-size:20px;background:purple"></div><div data-name="c" style="width:90pt;height:1.5rem;background:rgba(0,0,255,0.5)"></div></div>` },
];
export default [...supported, ...more, ...gapPages];
