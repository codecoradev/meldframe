// Entry of the canvas harness page (harness/canvas.html). Development only: `vite build` does not include it.
import { mount } from 'svelte';
import '../app.css';
import { applyTheme, loadTheme } from '@meldframe/web-tokens/theme';
import { loadWasm } from '@meldframe/web-link/engine';
import Harness from './Harness.svelte';

applyTheme(loadTheme());
// The canvas asks the app layer what is under a point (hit.ts), so the wasm module is loaded first.
loadWasm().then(() => mount(Harness, { target: document.getElementById('harness')! }));
