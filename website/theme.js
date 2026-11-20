// Light or dark: follows your system unless you pick one with the button. The choice is kept
// in this browser only (localStorage), and forgotten again if it matches your system.
(function () {
  var root = document.documentElement;
  var media = window.matchMedia("(prefers-color-scheme: dark)");
  function saved() { try { return localStorage.getItem("theme"); } catch (e) { return null; } }
  function save(v) { try { v ? localStorage.setItem("theme", v) : localStorage.removeItem("theme"); } catch (e) {} }
  function system() { return media.matches ? "dark" : "light"; }
  function apply() { root.setAttribute("data-theme", saved() || system()); }
  apply();
  media.addEventListener("change", apply);
  document.addEventListener("DOMContentLoaded", function () {
    var button = document.querySelector(".toggle");
    if (!button) return;
    function label() {
      var next = root.getAttribute("data-theme") === "dark" ? "light" : "dark";
      button.setAttribute("aria-label", "Switch to " + next + " mode");
      button.setAttribute("title", "Switch to " + next + " mode");
    }
    label();
    button.addEventListener("click", function () {
      var next = root.getAttribute("data-theme") === "dark" ? "light" : "dark";
      save(next === system() ? null : next);
      apply();
      label();
    });
  });
})();
