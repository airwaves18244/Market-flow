import { mount } from "svelte";
import "dockview-core/dist/styles/dockview.css";
import "./app.css";
import App from "./App.svelte";

const target = document.getElementById("app");
if (!target) {
  throw new Error("корневой элемент #app не найден");
}

const app = mount(App, { target });

export default app;
