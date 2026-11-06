// Statistik publik di website: jumlah unduhan & versi terbaru dari GitHub Releases, dan
// penghitung pengunjung GoatCounter (tanpa cookie) bila kodenya diisi.
(() => {
  // Kode situs GoatCounter, mis. "devlock" untuk https://devlock.goatcounter.com. Kosong = mati.
  const GOATCOUNTER = "romy";

  if (GOATCOUNTER && !/^(localhost|127\.0\.0\.1)$/.test(location.hostname)) {
    const script = document.createElement("script");
    script.async = true;
    script.src = "https://gc.zgo.at/count.js";
    script.dataset.goatcounter = `https://${GOATCOUNTER}.goatcounter.com/count`;
    document.head.appendChild(script);
  }

  const API = "https://api.github.com/repos/romyrudiansah/devlock/releases?per_page=100";
  const CACHE_KEY = "devlock-release-stats";
  const CACHE_MS = 60 * 60 * 1000;

  function render(stats) {
    const locale = document.documentElement.lang === "en" ? "en-US" : "id-ID";
    for (const el of document.querySelectorAll("[data-downloads]")) {
      el.textContent = stats.downloads.toLocaleString(locale);
    }
    for (const el of document.querySelectorAll("[data-version]")) {
      el.textContent = stats.version;
    }
    for (const el of document.querySelectorAll("[data-stats]")) {
      el.hidden = false;
    }
  }

  function cached() {
    try {
      const value = JSON.parse(localStorage.getItem(CACHE_KEY) ?? "null");
      return value && Date.now() - value.at < CACHE_MS ? value : null;
    } catch {
      return null;
    }
  }

  async function load() {
    if (!document.querySelector("[data-stats]")) return;
    const hit = cached();
    if (hit) return render(hit);
    try {
      const releases = await (await fetch(API)).json();
      if (!Array.isArray(releases) || releases.length === 0) return;
      // Hanya installer dan zip, bukan file .sha256.
      const downloads = releases
        .flatMap((r) => r.assets)
        .filter((a) => /\.(exe|zip)$/i.test(a.name))
        .reduce((sum, a) => sum + a.download_count, 0);
      const latest = releases.find((r) => !r.draft && !r.prerelease) ?? releases[0];
      const stats = { downloads, version: latest.tag_name, at: Date.now() };
      try {
        localStorage.setItem(CACHE_KEY, JSON.stringify(stats));
      } catch {
        // Tanpa cache: diambil ulang di kunjungan berikutnya.
      }
      render(stats);
    } catch {
      // API tidak terjangkau / kena batas: statistik tetap tersembunyi.
    }
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", load);
  else load();
})();
