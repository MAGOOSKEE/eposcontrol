const { invoke } = window.__TAURI__.core;
const box = document.getElementById("autostart");
const call = (c) => invoke("plugin:autostart|" + c);
call("is_enabled").then((v) => (box.checked = v)).catch(() => {});
box.addEventListener("change", () => call(box.checked ? "enable" : "disable"));
