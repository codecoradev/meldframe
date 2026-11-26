// The switch of the React copy of web-feedback (docs/react-web-plan.md, section 8, slice S3). Temporary and reversible: the
// last slice deletes this file, FeedbackHost.svelte and FeedbackLayerHost.svelte, and the three places that call them
// (Editor.svelte, main.ts, and nothing else). Nothing here changes what a page shows by default.
//   /?react=feedback    the banners, the toast, the empty state of a file with no frame and the error page are drawn by React
// `react` takes a comma separated list so that the switches of other slices (`/?react=export,feedback`) can go together.
import { mount } from 'svelte';
import type { ErrorPageView } from '@meldframe/web-feedback/failure';

/** True when the query string asks for the React copy of web-feedback. */
export function reactFeedback(search: string): boolean {
  return (new URLSearchParams(search).get('react') ?? '')
    .split(',')
    .map((name) => name.trim())
    .includes('feedback');
}

/** Draws the React error page on `target` (the Svelte one is mounted by main.ts as before). The React side loads on demand. */
export async function mountReactErrorPage(target: HTMLElement, page: ErrorPageView, onAction: () => void) {
  const host = (await import('./FeedbackHost.svelte')).default;
  const { ErrorPage } = await import('@meldframe/web-feedback/react/ErrorPage');
  return mount(host, { target, props: { component: ErrorPage, props: { page, onAction } } });
}
