import { describe, expect, it } from 'vitest';
import { checkLock } from './licences-lib.mjs';

const lock = (packages) => ({ packages: { '': {}, ...packages } });

describe('npm licence check', () => {
  it('accepts the permissive set for shipped packages', () => {
    const l = lock({
      'node_modules/a': { license: 'MIT' },
      'node_modules/b': { license: 'Apache-2.0' },
      'node_modules/c': { license: 'ISC' },
      'node_modules/d': { license: 'BSD-3-Clause' },
    });
    expect(checkLock(l)).toEqual([]);
  });
  it('rejects a copyleft or unknown licence', () => {
    const l = lock({ 'node_modules/a': { license: 'GPL-3.0' }, 'node_modules/b': {} });
    expect(checkLock(l).map((v) => v.name)).toEqual(['a', 'b']);
  });
  it('lets a dev-only package use the wider permissive set but not copyleft', () => {
    const l = lock({
      'node_modules/a': { license: '0BSD', dev: true },
      'node_modules/b': { license: '0BSD' },
      'node_modules/c': { license: 'GPL-3.0', dev: true },
    });
    expect(checkLock(l).map((v) => v.name)).toEqual(['b', 'c']);
  });
  it('allows MPL-2.0 only for the named dev-only build tool exceptions', () => {
    const l = lock({
      'node_modules/lightningcss': { license: 'MPL-2.0', dev: true },
      'node_modules/lightningcss-linux-x64-gnu': { license: 'MPL-2.0', dev: true, optional: true },
      'node_modules/other': { license: 'MPL-2.0', dev: true },
    });
    expect(checkLock(l).map((v) => v.name)).toEqual(['other']);
  });
  it('allows MPL-2.0 only for the exact lightningcss packages, not look-alikes', () => {
    const dev = (n) => ({ [`node_modules/${n}`]: { license: 'MPL-2.0', dev: true } });
    const l = lock({ ...dev('lightningcss-darwin-arm64'), ...dev('lightningcss-linux-x64-musl'), ...dev('lightningcss-evil'), ...dev('lightningcss-linux'), ...dev('lightningcss-linux-x64-gnu-extra') });
    expect(checkLock(l).map((v) => v.name)).toEqual(['lightningcss-evil', 'lightningcss-linux', 'lightningcss-linux-x64-gnu-extra']);
  });
  it('does not allow MPL-2.0 for a shipped lightningcss', () => {
    expect(checkLock(lock({ 'node_modules/lightningcss': { license: 'MPL-2.0' } })).map((v) => v.name)).toEqual(['lightningcss']);
  });
  it('accepts OFL-1.1 only for the two named font packages', () => {
    const l = lock({
      'node_modules/@fontsource-variable/inter': { license: 'OFL-1.1' },
      'node_modules/@fontsource-variable/jetbrains-mono': { license: 'OFL-1.1' },
      'node_modules/@fontsource-variable/other': { license: 'OFL-1.1' },
      'node_modules/dev-font': { license: 'OFL-1.1', dev: true },
    });
    expect(checkLock(l).map((v) => v.name)).toEqual(['@fontsource-variable/other', 'dev-font']);
  });
  it('takes a declared licence only for the exact name and version', () => {
    const l = lock({
      'node_modules/svelte-toolbelt': { version: '0.10.7' },
      'node_modules/a/node_modules/svelte-toolbelt': { version: '0.10.8' },
      'node_modules/other': { version: '1.0.0' },
    });
    expect(checkLock(l).map((v) => v.name)).toEqual(['svelte-toolbelt', 'other'].sort());
    expect(checkLock(l).find((v) => v.name === 'svelte-toolbelt')?.license).toBe('none');
  });
  it('fails closed on AND and WITH expressions', () => {
    for (const license of ['MIT AND Apache-2.0', 'GPL-3.0 AND (MIT OR ISC)', 'MIT WITH Classpath-exception-2.0', '(MIT OR ISC) AND MIT']) {
      expect(checkLock(lock({ 'node_modules/a': { license } })).map((v) => v.name), license).toEqual(['a']);
    }
  });
  it('parses OR groups with parentheses', () => {
    const ok = ['(MIT OR GPL-3.0)', 'GPL-3.0 OR MIT', '(GPL-3.0 OR (ISC OR LGPL-2.1))', 'MIT OR (GPL-3.0 OR AGPL-3.0)'];
    for (const license of ok) expect(checkLock(lock({ 'node_modules/a': { license } })), license).toEqual([]);
    const bad = ['(GPL-3.0 OR AGPL-3.0)', 'GPL-3.0 OR (AGPL-3.0 OR LGPL-2.1)', '(MIT OR', 'MIT OR', '()', 'MIT ISC'];
    for (const license of bad) expect(checkLock(lock({ 'node_modules/a': { license } })).map((v) => v.name), license).toEqual(['a']);
  });
});
