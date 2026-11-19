import { describe, expect, it } from 'vitest';
import { STORAGE_KEY, applyTheme, loadTheme, saveTheme } from './theme';

const throwing = {
  getItem: () => {
    throw new Error('blocked');
  },
  setItem: () => {
    throw new Error('blocked');
  },
};

describe('theme persistence', () => {
  it('is dark when nothing is stored or the value is unknown', () => {
    expect(loadTheme({ getItem: () => null })).toBe('dark');
    expect(loadTheme({ getItem: () => 'purple' })).toBe('dark');
    expect(loadTheme(undefined)).toBe('dark');
  });
  it('round-trips the stored choice', () => {
    const data = new Map<string, string>();
    const storage = { getItem: (k: string) => data.get(k) ?? null, setItem: (k: string, v: string) => void data.set(k, v) };
    saveTheme('light', storage);
    expect(data.get(STORAGE_KEY)).toBe('light');
    expect(loadTheme(storage)).toBe('light');
  });
  it('survives storage that throws', () => {
    expect(loadTheme(throwing)).toBe('dark');
    expect(() => saveTheme('light', throwing)).not.toThrow();
  });
  it('sets the data-theme attribute value', () => {
    const root = { dataset: {} as DOMStringMap };
    applyTheme('light', root);
    expect(root.dataset.theme).toBe('light');
  });
});
