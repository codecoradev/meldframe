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

  function draw() {
    if (!pixi || !world) return;
    world.removeChildren().forEach((c) => c.destroy());
    items = sceneItems(nodes);
    for (const i of items) {
      const moving = drag && selection.includes(i.id);
      const ox = moving ? drag!.dx : 0;
      const oy = moving ? drag!.dy : 0;
      const g = new Graphics();
      if (i.kind === 'text') {
        const t = new Text({
          text: i.text,
          style: { fontSize: i.fontSize, fill: colour(i.fill) || tokens.colour.nodeText, fontFamily: tokens.font.canvas },
        });
        t.position.set(i.x + ox, i.y + oy);
        world.addChild(t);
      } else {
        g.roundRect(i.x + ox, i.y + oy, i.w, i.h, i.radius).fill(colour(i.fill));
        if (i.kind === 'frame') g.stroke({ width: tokens.size.frameStroke, color: tokens.colour.border });
        world.addChild(g);
      }
      if (selection.includes(i.id)) {
        const s = new Graphics();
        s.rect(i.x + ox, i.y + oy, i.w, i.h).stroke({ width: tokens.size.selectionStroke, color: tokens.colour.selection });
        world.addChild(s);
      }
    }
    (host as HTMLDivElement & { __items?: SceneItem[] }).__items = items;
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

  $effect(() => {
    void [nodes, selection, pixi];
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
    draw();
  }
  function up() {
    if (!drag) return;
    const { dx, dy } = drag;
    drag = null;
    if (dx || dy) dispatch({ type: 'moveBy', dx, dy });
    else draw();
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
