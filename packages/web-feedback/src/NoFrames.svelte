<!--
  The empty state of a file with no frame: what to do about it, and a button that adds one. Shown once the canvas is
  ready (a canvas that has not started has nothing to say yet). The page decides what "add" does.
-->
<script lang="ts">
  import Button from '@meldframe/web-chrome/Button.svelte';
  import EmptyState from './EmptyState.svelte';
  import { hasFrames } from './empty';

  interface Props {
    nodes: readonly { type: string }[];
    ready: boolean;
    /** False while the page is out of step with the server: the app ignores edits then. */
    canAdd: boolean;
    onAdd: () => void;
  }
  let { nodes, ready, canAdd, onAdd }: Props = $props();
</script>

{#if ready && !hasFrames(nodes)}
  <EmptyState title="No frames yet" text="Press F and drag to draw a frame, or ask an agent connected over MCP to build a screen.">
    <Button variant="primary" disabled={!canAdd} onclick={onAdd}>Add a frame</Button>
  </EmptyState>
{/if}
