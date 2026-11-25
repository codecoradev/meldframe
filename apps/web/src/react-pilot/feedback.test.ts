import { describe, expect, it } from 'vitest';
import { reactFeedback } from './feedback';

describe('the switch of the React web-feedback', () => {
  it('draws the React copy only when the page asks for it with `react=feedback`', () => {
    expect(reactFeedback('?react=feedback')).toBe(true);
    expect(reactFeedback('?canvaslayout=1&react=feedback')).toBe(true);
    expect(reactFeedback('')).toBe(false); // the default page stays Svelte
    expect(reactFeedback('?react')).toBe(false);
    expect(reactFeedback('?react=')).toBe(false);
    expect(reactFeedback('?react=export')).toBe(false);
    expect(reactFeedback('?feedback')).toBe(false);
  });
  it('shares the `react` parameter with the switches of other slices', () => {
    expect(reactFeedback('?react=export,feedback')).toBe(true);
    expect(reactFeedback('?react=feedback, export')).toBe(true);
    expect(reactFeedback('?react=feedbacks')).toBe(false);
  });
});
