<!--
  The Import variables dialog (D8): CSS text pasted into a field or read from a file the person chooses (in the browser,
  nothing is uploaded), sent to the app layer as `importVariables`, and the result drawn under it: what was added, what was
  skipped and why, or why nothing was imported. The app layer reads the text, checks the commands, and applies them as one
  undo step of the person's own, or applies nothing (`state.variablesIo.import`); this holds the text of the field and the size of its file,
  and words the result (io.ts) and the note about the file (the name is the app layer's, `importSource`). Open or closed is the app state (`state.variablesIo.importOpen`);
  closing sends `closeVariablesImport` and keeps the text. The focus goes into the field and back to the button that opened the dialog.
-->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { tick } from 'svelte';
  import type { Action, VariablesImportResult } from '@meldframe/protocol';
  import IconButton from '@meldframe/web-chrome/IconButton.svelte';
  import TooltipProvider from '@meldframe/web-chrome/TooltipProvider.svelte';
  import { FRESH, type ImportView, applied, edited, fileInto, latest, noteOf, stale, stranded } from './flow';
  import ImportVariablesBody from './ImportVariablesBody.svelte';
  import { sizeText } from './io';
  import { readFile } from './read';

  interface Props {
    /** `state.variablesIo.importOpen`. */
    open: boolean;
    /** `state.variablesIo.import` and `.importSource` (the name of the file the field was read from, made safe by the app layer). */
    result: VariablesImportResult | null;
    source: string | null;
    dispatch: (a: Action) => void;
    /** The button that opened the dialog, to give the focus back to. */
    opener: () => HTMLElement | null | undefined;
  }
  let { open, result, source, dispatch, opener }: Props = $props();

  // What the field holds, kept for the life of the page: closing the dialog (Escape can be hit by mistake) keeps the text; an applied import, or Clear, starts it again.
  let view = $state<ImportView>(FRESH);
  const guard = latest();
  // A primitive, so a new snapshot that carries the same result does not empty what the person has typed since.
  const done = $derived(result?.status === 'imported');
  $effect(() => {
    if (done) {
      view = applied();
      void keepFocus();
    }
  });
  // The field empties after an import, so Import and Clear become disabled; a focus left on one of them would leave the keyboard
  // with nothing to act on inside the modal. It goes to Close (the next thing to do), unless it is on a control that works.
  async function keepFocus() {
    await tick();
    if (stranded(document.activeElement as HTMLButtonElement | null, document.body)) document.querySelector<HTMLElement>('[data-testid=import-vars-done]')?.focus();
  }
  const change = (o: boolean) => {
    if (!o) {
      // A file that is still being read must not fill a field the person has left.
      guard.cancel();
      dispatch({ type: 'closeVariablesImport' });
    }
  };
  // The result, and the name of the file, belong to the text they were made from: editing the text puts them away. What is typed also ends a file
  // that is still being read, so the file's text never replaces it.
  const edit = (t: string) => {
    if (stale(result, view)) dispatch({ type: 'clearVariablesImport' });
    view = edited(guard, view, t);
  };
  const clear = () => {
    if (stale(result, view)) dispatch({ type: 'clearVariablesImport' });
    guard.cancel();
    view = FRESH;
  };
  const send = () => dispatch({ type: 'importVariables', text: view.text });
  async function chosen(file: File) {
    const next = await fileInto(guard, file, () => view, readFile, sizeText);
    if (!next) return;
    // The result of an earlier text is not about this one; the name of a file that was read goes to the app layer, which makes it safe to show.
    if (result) dispatch({ type: 'clearVariablesImport' });
    if (next.note === null) dispatch({ type: 'chooseVariablesImportSource', name: file.name });
    view = next;
  }
  // The first thing to focus is the field, not the Close button: a tooltip opens on that button and takes the first Escape.
  const focusStart = (e: Event) => {
    const el = document.querySelector<HTMLElement>('[data-testid=import-vars-text]');
    if (el) {
      e.preventDefault();
      el.focus();
    }
  };
  const giveBack = (e: Event) => {
    const el = opener();
    if (el?.isConnected) {
      e.preventDefault();
      el.focus();
    }
  };
</script>

<TooltipProvider>
  <Dialog.Root {open} onOpenChange={change}>
    <Dialog.Portal>
      <Dialog.Overlay class="fixed inset-0 z-40 bg-scrim" />
      <Dialog.Content data-owns-keys data-testid="import-vars-dialog" onOpenAutoFocus={focusStart} onCloseAutoFocus={giveBack} class="fixed inset-8 z-50 m-auto flex h-fit max-h-full w-full max-w-3xl flex-col overflow-hidden rounded-lg border border-divider bg-surface font-sans text-body text-text shadow-dialog">
        <header class="flex h-14 shrink-0 items-center justify-between border-b border-hairline pr-3 pl-5">
          <Dialog.Title class="m-0 truncate text-title font-semibold text-text-strong">Import variables</Dialog.Title>
          <Dialog.Close>{#snippet child({ props })}<IconButton label="Close" icon="close" triggerProps={props} data-testid="import-vars-close" />{/snippet}</Dialog.Close>
        </header>
        <Dialog.Description class="sr-only">Add the variables of CSS text to this file. Paste the text or choose a file; one undo takes the import back.</Dialog.Description>
        <ImportVariablesBody {result} text={view.text} onText={edit} note={noteOf(view, source)} onFile={chosen} onClear={clear} onImport={send} onClose={() => change(false)} />
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>
</TooltipProvider>
