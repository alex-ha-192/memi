import init, * as bindings from '/memi-web-81c1f87019502e19.js';
const wasm = await init({ module_or_path: '/memi-web-81c1f87019502e19_bg.wasm' });


window.wasmBindings = bindings;


dispatchEvent(new CustomEvent("TrunkApplicationStarted", {detail: {wasm}}));

if ("serviceWorker" in navigator) {
  const registerWorker = () => navigator.serviceWorker.register("/sw.js", { updateViaCache: "none" }).catch(() => {});
  if (document.readyState === "complete") registerWorker(); else addEventListener("load", registerWorker);
}
