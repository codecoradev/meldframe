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
  it('checks an SPDX OR expression by any allowed alternative', () => {
    expect(checkLock(lock({ 'node_modules/a': { license: '(MIT OR GPL-3.0)' } }))).toEqual([]);
  });
});
