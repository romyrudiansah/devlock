// Tema: terang secara default (tidak mengikuti tema OS); gelap hanya bila dipilih lewat tombol.
// Pilihan disimpan per browser bila localStorage tersedia. Skrip ini dimuat di <head> tanpa defer
// agar atribut data-theme terpasang sebelum halaman digambar (tidak berkedip).
(function () {
  var root = document.documentElement;
  try {
    if (localStorage.getItem("devlock-theme") === "dark") root.setAttribute("data-theme", "dark");
  } catch (e) {
    // Tanpa localStorage tetap tema terang.
  }

  document.addEventListener("DOMContentLoaded", function () {
    var button = document.querySelector(".theme-toggle");
    if (!button) return;
    function sync() {
      var dark = root.getAttribute("data-theme") === "dark";
      // Label per bahasa dari data-to-light / data-to-dark di tombol.
      var label = dark ? button.dataset.toLight : button.dataset.toDark;
      button.setAttribute("aria-label", label);
      button.setAttribute("title", label);
    }
    sync();
    button.addEventListener("click", function () {
      var dark = root.getAttribute("data-theme") !== "dark";
      if (dark) root.setAttribute("data-theme", "dark");
      else root.removeAttribute("data-theme");
      try {
        localStorage.setItem("devlock-theme", dark ? "dark" : "light");
      } catch (e) {
        // Tanpa localStorage, pilihan hanya berlaku sampai halaman ditutup.
      }
      sync();
    });
  });
})();
