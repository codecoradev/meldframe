<script lang="ts">
  import type { Action, Node, Tool } from '@meldframe/protocol';
  import { Application, Container, Graphics, Text } from 'pixi.js';
  import { onMount } from 'svelte';
  import { tokens } from '../tokens';
  import { hitTest, parentAt, sceneItems, type SceneItem } from './scene';

  let { nodes, selection, tool, dispatch }: { nodes: Node[]; selection: string[]; tool: Tool; dispatch: (a: Action) => unknown } = $props();

  let host: HTMLDivElement;
  let pixi: Application | undefined = $state();
  let world: Container | undefined;
  let items: SceneItem[] = [];
  let drag: { x: number; y: number; dx: number; dy: number } | null = null;

  const colour = (s: string) => s || tokens.colour.nodeFrame;
  /** One display container per node, so a drag moves containers and rebuilds nothing. */
  let views = new Map<string, Container>();

  /** Rebuild the scene. Runs when the nodes or the selection change, not on a tick or a pointer move. */
  function draw() {
    if (!pixi || !world) return;
    world.removeChildren().forEach((c) => c.destroy({ children: true }));
    views = new Map();
    items = sceneItems(nodes);
    for (const i of items) {
      const view = new Container();
      if (i.kind === 'text') {
        const t = new Text({
          text: i.text,
          style: { fontSize: i.fontSize, fill: colour(i.fill) || tokens.colour.nodeText, fontFamily: tokens.font.canvas },
        });
        t.position.set(i.x, i.y);
        view.addChild(t);
      } else {
        const g = new Graphics();
        g.roundRect(i.x, i.y, i.w, i.h, i.radius).fill(colour(i.fill));
        if (i.kind === 'frame') g.stroke({ width: tokens.size.frameStroke, color: tokens.colour.border });
        view.addChild(g);
      }
      if (selection.includes(i.id)) {
        const s = new Graphics();
        s.rect(i.x, i.y, i.w, i.h).stroke({ width: tokens.size.selectionStroke, color: tokens.colour.selection });
        view.addChild(s);
      }
      world.addChild(view);
      views.set(i.id, view);
    }
  }

  /** Show the drag by moving the selected containers. */
  function preview(dx: number, dy: number) {
    for (const id of selection) views.get(id)?.position.set(dx, dy);
  }

  onMount(() => {
    const app = new Application();
    let gone = false;
    app.init({ background: tokens.colour.canvas, resizeTo: host, antialias: true }).then(() => {
      if (gone) return app.destroy(true);
      host.appendChild(app.canvas);
      world = new Container();
      app.stage.addChild(world);
      pixi = app;
    });
    return () => {
      gone = true;
      if (pixi) pixi.destroy(true);
    };
  });

  // The selection array is new on every state read; what matters is which ids it holds.
  const selectionKey = $derived(selection.join('\0'));
  $effect(() => {
    void [nodes, selectionKey, pixi];
    draw();
  });

  const point = (e: PointerEvent) => {
    const r = host.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };

  function down(e: PointerEvent) {
    const p = point(e);
    if (tool !== 'select') {
      const at = parentAt(items, p.x, p.y);
      const kind = tool === 'frame' ? 'frame' : tool === 'rectangle' ? 'rectangle' : 'text';
      dispatch({ type: 'createNode', kind, x: at.x, y: at.y, parent: at.parent });
      return;
    }
    const id = hitTest(items, p.x, p.y);
    if (!id) return void dispatch({ type: 'clearSelection' });
    if (e.shiftKey) dispatch({ type: 'toggleSelect', id });
    else if (!selection.includes(id)) dispatch({ type: 'select', ids: [id] });
    drag = { x: p.x, y: p.y, dx: 0, dy: 0 };
    host.setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (!drag) return;
    const p = point(e);
    drag = { ...drag, dx: p.x - drag.x, dy: p.y - drag.y };
    preview(drag.dx, drag.dy);
  }
  function up() {
    if (!drag) return;
    const { dx, dy } = drag;
    drag = null;
    preview(0, 0);
    if (dx || dy) dispatch({ type: 'moveBy', dx, dy });
  }
</script>

<div class="canvas" data-testid="canvas" bind:this={host} onpointerdown={down} onpointermove={move} onpointerup={up} role="application"></div>

<style>
  .canvas {
    flex: 1;
    min-width: 0;
    position: relative;
    overflow: hidden;
  }
</style>
