import { mount } from "svelte";
import Overlay from "./Overlay.svelte";

const app = document.getElementById("app")!;
mount(Overlay, { target: app });
