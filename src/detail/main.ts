import { mount } from 'svelte';
import Detail from './Detail.svelte';

const app = mount(Detail, { target: document.getElementById('app')! });

export default app;
