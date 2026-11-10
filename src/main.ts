import { mount } from 'svelte';
import App from './App.svelte';
import './themes.css';
import './styles.css';

mount(App, { target: document.getElementById('app')! });
