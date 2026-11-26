import { describe, expect, it } from 'vitest';
import { NO_NAME, nameOf } from './names';

describe('nameOf', () => {
  it('is the name the app layer gave, or one word for a name that shows as nothing, and cleans nothing', () => {
    expect(nameOf('dark')).toBe('dark');
    expect(nameOf('')).toBe(NO_NAME);
    expect(NO_NAME).toBe('(no name)');
    // The app layer made the name safe; this only words an empty one.
    expect(nameOf('a b')).toBe('a b');
  });
});
