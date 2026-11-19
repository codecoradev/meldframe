// Two triggers on one button (a tooltip and a menu) each hand it props: ids, data-state and pointer handlers. A plain
// spread lets the later one replace the earlier, which loses the tooltip's pointerdown (it closes the tooltip). Handlers
// are chained in order instead; any other prop takes the later value. Symbol keys are kept too: Svelte attachments
// (how Bits UI hands a trigger its anchor, so that a menu is positioned) are props with symbol keys.

type Props = Record<string | symbol, unknown>;

export function composeProps(...sets: readonly Props[]): Props {
  const out: Props = {};
  for (const set of sets) {
    for (const key of Reflect.ownKeys(set)) {
      const value = set[key];
      const before = out[key];
      if (typeof key === 'string' && /^on/.test(key) && typeof before === 'function' && typeof value === 'function') {
        out[key] = (...args: unknown[]) => {
          (before as (...a: unknown[]) => void)(...args);
          (value as (...a: unknown[]) => void)(...args);
        };
      } else if (value !== undefined || !(key in out)) out[key] = value;
    }
  }
  return out;
}
