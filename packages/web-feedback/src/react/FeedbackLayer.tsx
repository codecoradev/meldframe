// What the editor lays over the canvas, as one React root for the island helper: the empty state of a file with no frame,
// the toast and the banners, in the order the Svelte editor puts them (a later one is drawn over an earlier one). The
// props are those of the three Svelte components, side by side. The TooltipProvider gives the Dismiss button the tooltip
// delay that the Svelte layout gives it from above.
import { TooltipProvider } from '@meldframe/web-chrome/react/Tooltip';
import type { Banner } from '../banner';
import type { ToastView } from '../toaster';
import { BannerStack } from './BannerStack';
import { NoFrames } from './NoFrames';
import { Toast } from './Toast';

export interface FeedbackLayerProps {
  nodes: readonly { type: string }[];
  ready: boolean;
  canAdd: boolean;
  onAdd: () => void;
  toast: ToastView | null;
  onCloseToast: () => void;
  banners: readonly Banner[];
  onBanner: (banner: string, action: string) => void;
}

export function FeedbackLayer({ nodes, ready, canAdd, onAdd, toast, onCloseToast, banners, onBanner }: FeedbackLayerProps) {
  return (
    <TooltipProvider>
      <NoFrames nodes={nodes} ready={ready} canAdd={canAdd} onAdd={onAdd} />
      <Toast toast={toast} onClose={onCloseToast} />
      <BannerStack banners={banners} onAction={onBanner} />
    </TooltipProvider>
  );
}
