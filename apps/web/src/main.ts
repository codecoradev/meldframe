import { mount } from 'svelte';
import './app.css';
import App from './App.svelte';
import { applyTheme, loadTheme } from './theme';

applyTheme(loadTheme());
mount(App, { target: document.getElementById('app')! });
