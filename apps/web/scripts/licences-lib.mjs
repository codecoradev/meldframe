// Licence check for the npm packages of the web workspace (D2), the counterpart of
// `cargo xtask licences --check` (deny.toml). Commercial-friendly permissive licences only.

/** What a package that ends up in the shipped bundle may use. */
export const SHIPPED = ['MIT', 'Apache-2.0', 'ISC', 'BSD-2-Clause', 'BSD-3-Clause', 'Unicode-3.0', 'Zlib'];
/** What a development-only package (never in the bundle) may also use. */
export const DEV_ONLY = [...SHIPPED, '0BSD', 'BlueOak-1.0.0'];
/**
 * Dev-only build tools with a file-level copyleft licence that nothing links into the product: lightningcss
 * (a CSS minifier Vite calls as a separate native library) and its platform binaries.
 */
export const DEV_EXCEPTIONS = { 'MPL-2.0': /^lightningcss(?:-[a-z0-9-]+)?$/ };

function allowedBy(expression, allowed, name) {
  const options = expression.replace(/[()]/g, ' ').split(/\s+OR\s+/);
  return options.some((o) => {
    const id = o.trim();
    const exception = DEV_EXCEPTIONS[id];
    return allowed.includes(id) || (allowed === DEV_ONLY && exception?.test(name));
  });
}

/** Violations `{ name, license }` of a parsed package-lock.json, sorted by name. */
export function checkLock(lock) {
  const out = [];
  for (const [path, p] of Object.entries(lock.packages)) {
    if (path === '') continue;
    const name = path.replace(/^.*node_modules\//, '');
    const license = typeof p.license === 'string' ? p.license : '';
    const allowed = p.dev ? DEV_ONLY : SHIPPED;
    if (!license || !allowedBy(license, allowed, name)) out.push({ name, license: license || 'none' });
  }
  return out.sort((a, b) => (a.name < b.name ? -1 : 1));
}
