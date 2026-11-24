import init, * as bindings from '/memi-web-d65625bc397e540b.js';
const wasm = await init({ module_or_path: '/memi-web-332a6a18c696f68_bg.wasm' });


window.wasmBindings = bindings;


dispatchEvent(new CustomEvent("TrunkApplicationStarted", {detail: {wasm}}));

if ("serviceWorker" in navigator) {
  const registerWorker = () => navigator.serviceWorker.register("/sw.js", { updateViaCache: "none" }).catch(() => {});
  if (document.readyState === "complete") registerWorker(); else addEventListener("load", registerWorker);
}
