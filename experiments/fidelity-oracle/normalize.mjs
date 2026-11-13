// Contract normalization (candidate rule N1): `fill` on an axis where the parent is `hug` is not representable in CSS
// (the core resolves it to the node's own w/h, CSS flex/stretch resolves against max-content). The contract treats it as `fixed`.
export function normalize(nodes) {
  const by = Object.fromEntries(nodes.map((n) => [n.id, n]));
  return nodes.map((n) => {
    const p = n.parentId ? by[n.parentId] : null;
    if (!p) return n;
    const flex = p.type === 'frame' && p.layout !== 'none';
    let { sw, sh } = n;
    if (sw === 'fill' && (!flex || p.sw === 'hug')) sw = 'fixed';
    if (sh === 'fill' && (!flex || p.sh === 'hug')) sh = 'fixed';
    return sw === n.sw && sh === n.sh ? n : { ...n, sw, sh };
  });
}
