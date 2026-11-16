import { describe, expect, it } from 'vitest';
import { RELOAD_LIMIT, RELOAD_WINDOW_MS, describeSnapshotFailure, shouldReload } from './fatal';

const store = () => {
  const m = new Map<string, string>();
  return { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => void m.set(k, v) };
};

describe('shouldReload', () => {
  it('allows a few automatic reloads in a short time, then stops', () => {
    const s = store();
    const answers = Array.from({ length: RELOAD_LIMIT + 2 }, (_, i) => shouldReload(s, 1000 + i));
    expect(answers).toEqual([true, true, true, false, false]);
  });
  it('starts counting again after a quiet minute', () => {
    const s = store();
    for (let i = 0; i < RELOAD_LIMIT; i++) shouldReload(s, 1000);
    expect(shouldReload(s, 1000)).toBe(false);
    expect(shouldReload(s, 1000 + RELOAD_WINDOW_MS + 1)).toBe(true);
  });
  it('still answers when storage is unavailable', () => {
    const broken = { getItem: () => { throw new Error('denied'); }, setItem: () => { throw new Error('denied'); } };
    expect(shouldReload(broken, 1)).toBe(false);
  });
});

describe('describeSnapshotFailure', () => {
  it('names the status and what to check for a missing token', () => {
    expect(describeSnapshotFailure(401, '')).toContain('401');
    expect(describeSnapshotFailure(401, '')).toContain('MELDFRAME_WEB_TOKEN');
  });
  it('names the status and the server text otherwise', () => {
    expect(describeSnapshotFailure(503, 'busy')).toContain('503');
    expect(describeSnapshotFailure(503, 'busy')).toContain('busy');
  });
});
