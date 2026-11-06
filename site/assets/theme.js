// Tombol tema terang/gelap; pilihan disimpan per browser (bila localStorage tersedia).
(function () {
  var root = document.documentElement;
  function stored() {
    try {
      return localStorage.getItem("devlock-theme");
    } catch (e) {
      return null;
    }
  }
  function current() {
    return root.getAttribute("data-theme") ||
      (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
  }
  var saved = stored();
  if (saved === "light" || saved === "dark") root.setAttribute("data-theme", saved);

  document.addEventListener("DOMContentLoaded", function () {
    var button = document.querySelector(".theme-toggle");
    if (!button) return;
    function label() {
      var dark = current() === "dark";
      button.textContent = dark ? "☀" : "☾";
      // Label per bahasa dari data-to-light / data-to-dark di tombol (default Indonesia).
      button.setAttribute(
        "aria-label",
        dark ? button.dataset.toLight || "Ganti ke tema terang" : button.dataset.toDark || "Ganti ke tema gelap"
      );
    }
    label();
    button.addEventListener("click", function () {
      var next = current() === "dark" ? "light" : "dark";
      root.setAttribute("data-theme", next);
      try {
        localStorage.setItem("devlock-theme", next);
      } catch (e) {
        // Tanpa localStorage, pilihan hanya berlaku sampai halaman ditutup.
      }
      label();
    });
  });
})();
