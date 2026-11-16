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
  it('checks an SPDX OR expression by any allowed alternative', () => {
    expect(checkLock(lock({ 'node_modules/a': { license: '(MIT OR GPL-3.0)' } }))).toEqual([]);
  });
  it('accepts the font licence for shipped packages and a declared licence for a package without a field', () => {
    const l = lock({ 'node_modules/font': { license: 'OFL-1.1' }, 'node_modules/svelte-toolbelt': {}, 'node_modules/other': {} });
    expect(checkLock(l).map((v) => v.name)).toEqual(['other']);
  });
});
