// The pages the importer is known not to match (docs/import-html.md section 3: what it does not read). Each is listed in IMPORT_GAPS
// (known-gaps.mjs) with its cause. They are real pages of the kinds a project has, not made-up failures.
const RESET = '*{box-sizing:border-box}body{margin:0}p{margin:0}';
const LH = 'font-size:16px;line-height:20px';
const box = (name, w, h, bg, extra = '') => `<div data-name="${name}" style="width:${w}px;height:${h}px;background:${bg};${extra}"></div>`;
const col = (name, kids, extra = '') => `<div data-name="${name}" style="display:flex;flex-direction:column;gap:8px;width:400px;padding:10px;background:#eeeeee;${extra}">${kids}</div>`;
const row = (name, kids, extra = '') => `<div data-name="${name}" style="display:flex;gap:8px;width:400px;padding:10px;background:#eeeeee;${extra}">${kids}</div>`;

export default [
  { name: 'h13-gap-percent', html: `<style>${RESET}</style>${col('c', box('half', 0, 30, '#f87171', 'width:50%') + box('full', 0, 30, '#60a5fa', 'width:100%') + box('after', 100, 30, '#34d399'))}` },
  { name: 'h14-gap-grid', html: `<style>${RESET}</style><div data-name="g" style="display:grid;grid-template-columns:100px 100px;gap:10px;padding:10px;width:300px;background:#eeeeee">${box('a', 100, 30, '#f87171')}${box('b', 100, 30, '#60a5fa')}${box('c', 100, 30, '#34d399')}${box('d', 100, 30, '#fbbf24')}</div>` },
  { name: 'h15-gap-margin-collapse', html: `<style>${RESET}</style><div data-name="blk" style="width:400px;background:#eeeeee">${box('a', 100, 30, '#f87171', 'margin:20px 0')}${box('b', 100, 30, '#60a5fa', 'margin:20px 0')}</div>` },
  { name: 'h16-gap-before', html: `<style>${RESET}.m::before{content:"";display:block;width:50px;height:20px;background:#a78bfa}</style>${col('c', `<div data-name="m" class="m" style="background:#dbeafe"></div>` + box('after', 100, 30, '#34d399'))}` },
  { name: 'h17-gap-wrap', html: `<style>${RESET}</style>${row('r', box('a', 150, 30, '#f87171') + box('b', 150, 30, '#60a5fa') + box('c', 150, 30, '#34d399') + box('d', 60, 30, '#fbbf24'), 'flex-wrap:wrap;align-items:flex-start')}` },
  { name: 'h18-gap-max-width', html: `<style>${RESET}</style>${col('c', box('capped', 0, 30, '#f87171', 'width:100%;max-width:200px') + box('after', 100, 30, '#34d399'), 'align-items:flex-start')}` },
  { name: 'h19-gap-space-around', html: `<style>${RESET}</style>${row('r', box('a', 50, 30, '#f87171') + box('b', 50, 30, '#60a5fa') + box('c', 50, 30, '#34d399'), 'justify-content:space-around')}` },
  { name: 'h20-gap-margin-auto', html: `<style>${RESET}</style>${col('c', box('centred', 100, 30, '#f87171', 'margin:0 auto') + box('after', 100, 30, '#34d399'), 'align-items:flex-start')}` },
  { name: 'h21-gap-inline-flow', html: `<style>${RESET}</style><div data-name="blk" style="width:400px;padding:10px;background:#eeeeee"><div data-name="a" style="display:inline-block;width:100px;height:30px;background:#f87171;vertical-align:top"></div><div data-name="b" style="display:inline-block;width:100px;height:30px;background:#60a5fa;vertical-align:top"></div></div>` },
  { name: 'h22-gap-line-height-normal', html: `<style>${RESET}</style>${col('c', `<p data-name="a" style="font-size:16px">Hello world</p><p data-name="b" style="font-size:24px;font-weight:700">Hello world</p>` + box('after', 100, 30, '#34d399'))}` },
  { name: 'h23-gap-text-width', html: `<style>${RESET}</style>${row('r', `<p data-name="a" style="${LH};white-space:pre">Hello world</p><p data-name="b" style="${LH};white-space:pre;font-weight:700">Hello world</p>` + box('after', 50, 30, '#34d399'), 'align-items:flex-start')}` },
  { name: 'h24-gap-fill-padding', html: `<style>${RESET}</style><div data-name="r" style="display:flex;width:600px;height:100px;padding:10px;gap:10px;background:#eeeeee"><div data-name="a" style="flex:1;align-self:stretch;background:#60a5fa"></div><div data-name="b" style="flex:1;align-self:stretch;padding:30px;display:flex;background:#fbbf24"></div></div>` },
  { name: 'h25-gap-tailwind-scale', tailwind: true, css: `@import "tailwindcss/theme.css" layer(theme);\n@import "tailwindcss/utilities.css" layer(utilities);\n*,::before,::after{box-sizing:border-box}body{margin:0}`, html: `<div data-name="c" class="flex flex-col gap-2 p-4 w-64 bg-[color:#eeeeee]"><div data-name="a" class="h-8 w-20 bg-[color:#f87171]"></div><div data-name="b" class="h-[30px] w-[100px] bg-[color:#60a5fa]"></div></div>` },
  { name: 'h26-gap-reverse-and-order', html: `<style>${RESET}</style>${row('r', box('a', 50, 30, '#f87171') + box('b', 80, 30, '#60a5fa') + box('c', 30, 30, '#34d399'), 'flex-direction:row-reverse')}` },
  { name: 'h27-gap-media', html: `<style>${RESET}.b{width:100px;height:30px;background:#f87171}@media (min-width:600px){.b{width:300px}}</style>${col('c', `<div data-name="a" class="b"></div>`, 'align-items:flex-start')}` },
  { name: 'h28-gap-named-colour', pixels: true, html: `<style>${RESET}</style><div data-name="c" style="width:200px;height:60px;background:rebeccapurple"></div>` },
];
