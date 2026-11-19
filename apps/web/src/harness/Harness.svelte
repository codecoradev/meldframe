<!--
  Development harness for the canvas: not part of the product. It stands in for the editor: it holds a scene,
  applies the move and resize intents to it the way an editor would, and exposes `window.__harness` for the
  scripts in packages/web-e2e/scripts. Open /harness/canvas.html on the dev server.
  Query: scene=design|many, n=<count>, select=<id,id>, touch=1, rm=1 (reduced motion), tool=move|hand|frame|rect|text.
-->
<script lang="ts">
  import { onMount } from 'svelte';
  import Canvas from '../canvas/Canvas.svelte';
  import { HOME, fitRect, formatZoom, type Camera } from '@meldframe/web-canvas/camera';
  import type { MoveIntent, ResizeIntent, ToolMode } from '@meldframe/web-canvas/interaction';
  import type { Touch } from '../canvas/overlayview';
  import { contentBounds, resolveScene, type Scene, type SceneNode } from '@meldframe/web-canvas/scene';
  import { DESIGN_ZOOM, SELECTED_ID, designScene, manyScene } from './sample';
  import { pixiColors } from '../tokens';

  const q = new URLSearchParams(location.search);
  const kind = q.get('scene') ?? 'design';
  const count = Number(q.get('n') ?? 5000);

  let scene = $state<Scene>(kind === 'many' ? manyScene(count) : designScene());
  let selection = $state<string[]>((q.get('select') ?? '').split(',').filter(Boolean));
  let camera = $state<Camera>(kind === 'many' ? { ...HOME } : { x: 0, y: 0, zoom: DESIGN_ZOOM });
  let touches = $state<Touch[]>([]);
  let tool = $state<ToolMode>((q.get('tool') as ToolMode | null) ?? 'move');
  let canvas = $state<ReturnType<typeof Canvas>>();
  let mounted = $state(true);
  const events: unknown[] = [];

  const reduced = q.get('rm') === '1' ? true : undefined;
  const agent = pixiColors('dark')['agent-1'];

  // The editor's job, in miniature: the node as it was when the gesture began, plus the reported delta.
  let origin: Record<string, SceneNode> = {};
  const remember = (ids: string[]) => {
    if (Object.keys(origin).length) return;
    for (const n of scene.nodes) if (ids.includes(n.id)) origin[n.id] = n;
  };
  const edit = (f: (n: SceneNode) => SceneNode) => {
    scene = { ...scene, nodes: scene.nodes.map((n) => (origin[n.id] ? f(origin[n.id]!) : n)) };
  };

  function onMove(i: MoveIntent) {
    events.push({ type: 'move', ...i });
    remember(i.ids);
    if (i.cancelled) edit((n) => n);
    else edit((n) => ({ ...n, x: n.x + i.dx, y: n.y + i.dy }));
    if (i.final) origin = {};
  }

  function onResize(i: ResizeIntent) {
    events.push({ type: 'resize', ...i });
    remember(i.ids);
    if (i.cancelled) edit((n) => n);
    else {
      edit((n) => {
        let { x, y, w, h } = n;
        if (i.handle.includes('e')) w += i.dx;
        if (i.handle.includes('s')) h += i.dy;
        if (i.handle.includes('w')) { x += i.dx; w -= i.dx; }
        if (i.handle.includes('n')) { y += i.dy; h -= i.dy; }
        return { ...n, x, y, w: Math.max(1, w), h: Math.max(1, h) };
      });
    }
    if (i.final) origin = {};
  }

  function onSelect(ids: string[], additive: boolean) {
    events.push({ type: 'select', ids, additive });
    selection = additive ? [...new Set([...selection, ...ids])] : ids;
  }

  onMount(() => {
    if (q.get('touch') === '1') {
      const stamp = () => {
        touches = [{ id: 't1', color: agent, label: 'Claude Code', nodeIds: ['mobile-1'], at: performance.now() }];
      };
      stamp();
      const timer = setInterval(stamp, 300);
      return () => clearInterval(timer);
    }
  });

  /** For scripts: everything the page can be asked from outside. */
  (window as unknown as { __harness: unknown }).__harness = {
    events,
    camera: () => $state.snapshot(camera),
    selection: () => $state.snapshot(selection),
    scene: () => $state.snapshot(scene),
    stats: () => canvas?.stats(),
    setCamera: (c: Camera) => { camera = c; },
    fit: () => { camera = fitRect(contentBounds(resolveScene(scene).scene), { w: innerWidth, h: innerHeight }); },
    setTool: (t: ToolMode) => { tool = t; },
    remount: async (times: number) => {
      for (let i = 0; i < times; i++) {
        mounted = false;
        await new Promise((r) => setTimeout(r, 50));
        mounted = true;
        await new Promise((r) => setTimeout(r, 200));
      }
    },
    designSelectedId: SELECTED_ID,
    /** Moves one node by a world unit, the way a host applies a move: only that node changes in the scene. */
    moveOne: (id: string) => { scene = { ...scene, nodes: scene.nodes.map((n) => (n.id === id ? { ...n, x: n.x + 1 } : n)) }; },
    /** Replaces the scene by `n` generated nodes and resolves when the frame with them has been drawn. */
    swapScene: async (n: number) => {
      const next = manyScene(n);
      const t0 = performance.now();
      scene = next;
      await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
      return performance.now() - t0;
    },
    /** Moves the camera for `frames` animation frames with `step` and returns the frame intervals in ms. */
    animate: async (frames: number, step: (i: number, c: Camera) => Camera) => {
      const out: number[] & { timing?: unknown[] } = [];
      const timing: unknown[] = [];
      out.timing = timing;
      let last = 0;
      for (let i = 0; i < frames; i++) {
        const t = await new Promise<number>((r) => requestAnimationFrame(r));
        if (last) out.push(t - last);
        last = t;
        timing.push(canvas?.stats().timing);
        camera = step(i, camera);
      }
      return { intervals: out, timing };
    },
  };
</script>

<div class="fixed inset-0 bg-canvas font-sans text-body text-text">
  {#if mounted}
    <Canvas
      bind:this={canvas}
      {scene}
      {selection}
      bind:camera
      {touches}
      {tool}
      reducedMotion={reduced}
      {onSelect}
      {onMove}
      {onResize}
      onCameraChange={(c, final) => final && events.push({ type: 'camera', c })}
      onHover={(id) => events.push({ type: 'hover', id })}
    />
  {/if}
  <output class="pointer-events-none absolute right-5 bottom-7 font-mono text-small text-text-muted">{formatZoom(camera.zoom)}</output>
</div>
