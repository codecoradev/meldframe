// Licence check for the npm packages of the web workspace (D2), the counterpart of
// `cargo xtask licences --check` (deny.toml). Commercial-friendly permissive licences only.

/** What a package that ends up in the shipped bundle may use. */
export const SHIPPED = ['MIT', 'Apache-2.0', 'ISC', 'BSD-2-Clause', 'BSD-3-Clause', 'Unicode-3.0', 'Zlib'];
/** What a development-only package (never in the bundle) may also use. */
export const DEV_ONLY = [...SHIPPED, '0BSD', 'BlueOak-1.0.0'];

const FONTS = ['@fontsource-variable/inter', '@fontsource-variable/jetbrains-mono'];
/**
 * Dev-only build tools with a file-level copyleft licence that nothing links into the product: lightningcss
 * (a CSS minifier Vite calls as a separate native library) and its platform binaries.
 */
const LIGHTNINGCSS = [
  'lightningcss', 'lightningcss-android-arm64', 'lightningcss-darwin-arm64', 'lightningcss-darwin-x64',
  'lightningcss-freebsd-x64', 'lightningcss-linux-arm-gnueabihf', 'lightningcss-linux-arm64-gnu',
  'lightningcss-linux-arm64-musl', 'lightningcss-linux-x64-gnu', 'lightningcss-linux-x64-musl',
  'lightningcss-win32-arm64-msvc', 'lightningcss-win32-x64-msvc',
];
/**
 * Licences allowed for named packages only. OFL-1.1 is for the two self-hosted font packages: the licence lets
 * the font files ship in a closed product when the notice travels with them (see NOTICE). MPL-2.0 is for the
 * dev-only lightningcss packages above.
 */
export const EXCEPTIONS = {
  'OFL-1.1': { names: FONTS, devOnly: false },
  'MPL-2.0': { names: LIGHTNINGCSS, devOnly: true },
};

/**
 * Packages whose package.json has no licence field although the package carries a licence file, keyed by
 * `name@version` (checked against the LICENSE file of that exact version by hand).
 */
export const DECLARED = { 'svelte-toolbelt@0.10.7': 'MIT' };

/**
 * Whether an SPDX expression is allowed. Only `OR` and parentheses are understood: any `AND` or `WITH` (and any
 * syntax error) is refused, because they cannot be judged by one allowed list.
 */
function allowedBy(expression, isAllowed) {
  const tokens = expression.match(/\(|\)|[^\s()]+/g) ?? [];
  let pos = 0;
  const atom = () => {
    const t = tokens[pos++];
    if (t === '(') {
      const v = or();
      if (tokens[pos++] !== ')') throw new Error('unbalanced');
      return v;
    }
    if (t === undefined || t === ')' || /^(?:AND|OR|WITH)$/i.test(t)) throw new Error('unexpected');
    return isAllowed(t);
  };
  const or = () => {
    let v = atom();
    while (/^OR$/i.test(tokens[pos] ?? '')) {
      pos++;
      const next = atom();
      v = v || next;
    }
    return v;
  };
  try {
    const v = or();
    return pos === tokens.length && v;
  } catch {
    return false;
  }
}

/** Violations `{ name, license }` of a parsed package-lock.json, sorted by name. */
export function checkLock(lock) {
  const out = [];
  for (const [path, p] of Object.entries(lock.packages)) {
    if (path === '') continue;
    const name = path.replace(/^.*node_modules\//, '');
    const license = typeof p.license === 'string' ? p.license : (DECLARED[`${name}@${p.version}`] ?? '');
    const allowed = p.dev ? DEV_ONLY : SHIPPED;
    const ok =
      license !== '' &&
      allowedBy(license, (id) => {
        const exception = EXCEPTIONS[id];
        return allowed.includes(id) || (exception !== undefined && exception.names.includes(name) && (p.dev || !exception.devOnly));
      });
    if (!ok) out.push({ name, license: license || 'none' });
  }
  return out.sort((a, b) => (a.name < b.name ? -1 : 1));
}
