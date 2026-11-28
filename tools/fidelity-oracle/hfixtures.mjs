// Hand-written pages for the importer oracle (import.mjs): a whole document each, the way an author writes it, in the subset that
// docs/import-html.md section 3 reads. The oracle loads the HTML as written in Chromium and compares each element's box with the layout
// core's rect of the node the importer made of it. Text is Inter (the oracle adds the font; the page names none).
const head = (css) => `<!doctype html><html><head><meta charset="utf-8"><title>t</title><style>*{box-sizing:border-box}body{margin:0}h1,h2,p{margin:0}${css}</style></head>`;
const hfixtures = [];
const add = (name, desc, css, body, extra = {}) => hfixtures.push({ name, desc, html: `${head(css)}<body>${body}</body></html>`, ...extra });
export default hfixtures;

add('h01-row', 'a flex row with fixed boxes, a gap and padding',
  '.row{display:flex;gap:12px;padding:16px;background:#eee;width:600px}.a{width:80px;height:40px;background:#f87171}.b{width:120px;height:80px;background:#60a5fa}.c{width:60px;height:60px;background:#34d399}',
  '<div class="row"><div class="a"></div><div class="b"></div><div class="c"></div></div>');

add('h02-column-align', 'a fixed-size column with padding, align-items center and justify space-between',
  '.col{display:flex;flex-direction:column;align-items:center;justify-content:space-between;width:300px;height:260px;padding:20px 10px;background:#f4f4f5}.a{width:100px;height:30px;background:#a78bfa}.b{width:180px;height:50px;background:#fbbf24}.c{width:60px;height:60px;background:#34d399}',
  '<div class="col"><div class="a"></div><div class="b"></div><div class="c"></div></div>');

add('h03-block-flow', 'block boxes stacked in a padded block: a block is a column with no gap',
  '.box{width:400px;padding:12px;background:#eee}.a{height:40px;background:#f87171}.b{height:60px;background:#60a5fa}.c{height:20px;background:#34d399}',
  '<div class="box"><div class="a"></div><div class="b"></div><div class="c"></div></div>');

add('h04-grow', 'flex-grow on a row: fill and fixed items share the free space',
  '.row{display:flex;gap:10px;padding:10px;width:600px;height:100px;background:#eee}.f{flex:1;background:#f87171}.g{flex:1;background:#60a5fa;height:60px}.x{width:100px;background:#34d399}',
  '<div class="row"><div class="x"></div><div class="f"></div><div class="g"></div></div>');

add('h05-content-box', 'width and height with content-box sizing, padding and a border (the box is bigger than its width)',
  '*{box-sizing:content-box}.p{width:200px;height:50px;padding:10px;border:3px solid #333;background:#fde68a;margin-bottom:16px}.q{width:120px;height:30px;padding:5px 15px;background:#bfdbfe}',
  '<div class="p"></div><div class="q"></div>');

add('h06-absolute', 'absolute boxes in a relative frame with left, top, right and bottom',
  '.stage{position:relative;width:400px;height:200px;background:#eee}.a{position:absolute;left:20px;top:30px;width:80px;height:50px;background:#f87171}.b{position:absolute;right:10px;bottom:10px;width:100px;height:40px;background:#60a5fa}',
  '<div class="stage"><div class="a"></div><div class="b"></div></div>');

add('h07-text', 'a heading and a paragraph in a fixed-width column: text boxes against the core and the Inter table',
  '.card{display:flex;flex-direction:column;gap:8px;width:320px;padding:16px;background:#fff}h1{font-size:24px;font-weight:700;color:#111}p{font-size:14px;color:#444}',
  '<div class="card"><h1>Heading</h1><p>Design tools should produce layouts that a browser can reproduce exactly.</p></div>');

add('h08-button-row', 'a row of buttons: padded text in a frame, a row that hugs',
  '.bar{display:inline-flex;gap:8px;padding:8px;background:#eee}.btn{display:inline-block;padding:8px 16px;background:#2563eb;color:#fff;font-size:14px}',
  '<div class="bar"><span class="btn">Save</span><span class="btn">Cancel</span></div>');

add('h09-percent', 'percentage widths and an auto margin: the model has no percentages, the importer lists them',
  '.wrap{width:500px;background:#eee}.half{width:50%;height:30px;background:#f87171}.mid{width:200px;height:30px;margin:10px auto;background:#60a5fa}',
  '<div class="wrap"><div class="half"></div><div class="mid"></div></div>');

add('h10-margins', 'margins between block siblings that collapse in a browser, and a margin in a flex column that does not',
  '.col{width:300px;background:#eee}.a{height:30px;margin-bottom:20px;background:#f87171}.b{height:30px;margin-top:10px;background:#60a5fa}.f{display:flex;flex-direction:column;width:300px;margin-top:20px;background:#eee}.f .a,.f .b{margin:0}.f .a{margin-bottom:20px}.f .b{margin-top:10px}',
  '<div class="col"><div class="a"></div><div class="b"></div></div><div class="f"><div class="a"></div><div class="b"></div></div>');

add('h11-decor', 'a border, a radius, a shadow, an opacity and a gradient on border-box boxes: paint changes no box',
  '.p{display:flex;gap:10px;padding:10px;width:420px;background:#fff}.a{width:100px;height:60px;border:4px solid #333;border-radius:12px;background:#fde68a}.b{width:100px;height:60px;box-shadow:4px 4px 8px #00000066;opacity:.5;background:#60a5fa}.c{width:100px;height:60px;background:linear-gradient(90deg,#f87171,#60a5fa);border-radius:50%}',
  '<div class="p"><div class="a"></div><div class="b"></div><div class="c"></div></div>');

add('h12-wrapper', 'wrapper divs that make no node, an inline style, a hidden element and nesting two levels deep',
  '.outer{width:500px;padding:10px;background:#eee}.gone{display:none}',
  '<div class="outer"><div><div style="display:flex;gap:6px"><div style="width:50px;height:50px;background:#f87171"></div><div class="gone"></div><div style="width:70px;height:30px;background:#60a5fa;margin-left:4px"></div></div></div><div style="margin-top:12px"><div style="width:200px;height:20px;background:#34d399"></div></div></div>');
