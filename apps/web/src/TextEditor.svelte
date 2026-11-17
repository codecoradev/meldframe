<!--
  The field laid over a text node while its text is edited. It holds the draft and nothing else: a commit (blur or
  Ctrl/Cmd+Enter) reports the text, Escape reports a cancel. The editor page turns a commit into one editText action.
-->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import type { TextBoxStyle } from '@meldframe/web-document/textedit';

  interface Props {
    text: string;
    box: TextBoxStyle;
    onCommit: (text: string) => void;
    onCancel: () => void;
  }
  let { text, box, onCommit, onCancel }: Props = $props();

  let field: HTMLTextAreaElement;
  let draft = $state(untrack(() => text));
  let done = false; // a commit or a cancel happens once: the blur that follows must not run a second time

  // The field grows with its text (never under the node's own height), so a line that wraps is not hidden.
  function grow() {
    if (!field) return;
    field.style.height = 'auto';
    field.style.height = `${Math.max(box.height, field.scrollHeight)}px`;
  }
  $effect(() => {
    void [draft, box.width, box.fontSize, box.height];
    grow();
  });

  onMount(() => {
    field.focus();
    field.select();
  });

  function finish(commit: boolean) {
    if (done) return;
    done = true;
    if (commit) onCommit(draft);
    else onCancel();
  }

  function onKey(e: KeyboardEvent) {
    if (e.isComposing) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      finish(false);
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      finish(true);
    }
    e.stopPropagation(); // keys typed here are text, not commands of the editor
  }
</script>

<textarea
  bind:this={field}
  bind:value={draft}
  data-testid="text-editor"
  aria-label="Edit text"
  class="absolute resize-none overflow-hidden rounded-sm border border-accent bg-surface p-0 font-sans text-text outline-none"
  style:left="{box.left}px"
  style:top="{box.top}px"
  style:width="{box.width}px"
  style:min-height="{box.height}px"
  style:font-size="{box.fontSize}px"
  style:line-height="1.2"
  onkeydown={onKey}
  onblur={() => finish(true)}
></textarea>
