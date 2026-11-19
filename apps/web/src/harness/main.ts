// Entry of the canvas harness page (harness/canvas.html). Development only: `vite build` does not include it.
import { mount } from 'svelte';
import '../app.css';
import { applyTheme, loadTheme } from '@meldframe/web-tokens/theme';
import Harness from './Harness.svelte';

applyTheme(loadTheme());
mount(Harness, { target: document.getElementById('harness')! });
