// React copy of BannerStack.svelte. Banners over the top of the canvas: the connection is down, the page and the server
// disagree, the canvas could not start. An error goes in a live region that interrupts (alert) and a warning in one that
// waits (status). Each may have one action; the page decides what its id does. The stack takes no pointer events, so the
// canvas under it still works. The text is the caller's, drawn as plain text.
import { Button } from '@meldframe/web-chrome/react/Button';
import type { Banner } from '../banner';

export interface BannerStackProps {
  banners: readonly Banner[];
  onAction: (banner: string, action: string) => void;
}

const tones = { warning: 'bg-warning-bg text-warning', danger: 'bg-danger-bg text-danger-text' } as const;

function Item({ banner: b, onAction }: { banner: Banner; onAction: BannerStackProps['onAction'] }) {
  const a = b.action;
  return (
    <div data-testid={`banner-${b.id}`} data-tone={b.tone} className={`pointer-events-auto mb-2 flex max-w-xl items-center gap-3 rounded-md py-2 pr-2 pl-4 text-small shadow-float ${tones[b.tone]}`}>
      <span className="min-w-0 flex-1 wrap-anywhere">{b.text}</span>
      {a ? (
        <Button size="sm" onClick={() => onAction(b.id, a.id)}>
          {a.label}
        </Button>
      ) : null}
    </div>
  );
}

// Both regions are always on the page and the banners are put into them: a region that appears together with its text is not announced. The alert is for what must interrupt (an error), the status for the rest.
export function BannerStack({ banners, onAction }: BannerStackProps) {
  return (
    <div className="pointer-events-none absolute inset-x-0 top-4 flex flex-col items-center px-4">
      <div role="alert" data-testid="banner-region-alert" className="flex flex-col items-center">
        {banners
          .filter((b) => b.tone === 'danger')
          .map((b) => (
            <Item key={b.id} banner={b} onAction={onAction} />
          ))}
      </div>
      <div role="status" data-testid="banner-region-status" className="flex flex-col items-center">
        {banners
          .filter((b) => b.tone !== 'danger')
          .map((b) => (
            <Item key={b.id} banner={b} onAction={onAction} />
          ))}
      </div>
    </div>
  );
}
