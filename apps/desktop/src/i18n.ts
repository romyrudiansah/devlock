/**
 * Terjemahan UI. Teks Indonesia (bahasa sumber) dipakai langsung sebagai kunci, jadi kode tetap
 * terbaca dan teks yang belum diterjemahkan tampil apa adanya. Placeholder: `{nama}`.
 * Pesan dari backend (error, hasil perintah) tetap berbahasa Indonesia.
 */
export type Lang = "id" | "en";

let current: Lang = "id";

export function setLang(lang: string | undefined) {
  current = lang === "en" ? "en" : "id";
  document.documentElement.lang = current;
}

export function getLang(): Lang {
  return current;
}

export function t(text: string, vars?: Record<string, string | number>): string {
  const base = current === "en" ? (EN[text] ?? text) : text;
  return vars ? base.replace(/\{(\w+)\}/g, (_, key: string) => String(vars[key] ?? `{${key}}`)) : base;
}

const EN: Record<string, string> = {
  // App
  Service: "Services",
  Project: "Projects",
  Runtime: "Runtimes",
  Beranda: "Home",
  Navigasi: "Navigation",
  Cari: "Search",
  "Palet perintah (Ctrl+K)": "Command palette (Ctrl+K)",

  // Home
  "DevLock berhenti": "DevLock is stopped",
  "{n} service bermasalah": "Services with problems: {n}",
  "Semua berjalan": "Everything is running",
  "{on} service · {sites} site · http://*.{tld}": "{on} services · {sites} sites · http://*.{tld}",
  "Nyalakan untuk menjalankan web server, PHP, database, dan dev server project Anda.":
    "Turn it on to run the web server, PHP, databases, and dev servers for your projects.",
  "Cari perintah, service, atau site…": "Search commands, services, or sites…",
  Detail: "Details",
  Kelola: "Manage",
  "Belum ada service untuk project saat ini.": "No services are needed by the current projects.",
  "Service muncul di sini setelah DevLock dinyalakan.": "Services appear here once DevLock is running.",
  "Belum ada project. Buat folder di": "No projects yet. Create a folder in",
  "atau klik Project baru.": "or click New project.",

  // Palette
  "Palet perintah": "Command palette",
  "Restart semua": "Restart all",
  "Reload konfigurasi": "Reload configuration",
  "Buka {site}": "Open {site}",
  "Editor: {site}": "Editor: {site}",
  "Terminal: {site}": "Terminal: {site}",
  "Folder: {site}": "Folder: {site}",
  "Install runtime…": "Install runtime…",
  "Ke {page}": "Go to {page}",
  Halaman: "Page",
  "Tidak ada yang cocok.": "No matches.",

  // Dashboard
  Berjalan: "Running",
  Sebagian: "Partial",
  Berhenti: "Stopped",
  "Restart…": "Restarting…",
  "Menyiapkan…": "Starting…",
  Gagal: "Failed",
  "{running} dari {total} service berjalan": "{running} of {total} services running",
  "Daemon belum berjalan": "Daemon is not running",
  "Menjalankan…": "Starting…",
  "Start semua": "Start all",
  "Stop semua": "Stop all",
  "Log daemon": "Daemon log",
  "Belum ada service yang berjalan.": "No services are running yet.",
  Klik: "Click",
  "untuk menyalakan nginx, PHP, database, dan dev server sesuai project di":
    "to start nginx, PHP, databases, and dev servers for the projects in",
  Status: "Status",
  "{running}/{total} worker": "{running}/{total} workers",
  "{h}j {m}m": "{h}h {m}m",
  "Percayai CA DevLock": "Trust DevLock CA",
  "Menunggu konfirmasi Windows…": "Waiting for Windows confirmation…",
  "Tutup notifikasi": "Dismiss notification",

  // LogModal
  "Memuat…": "Loading…",
  "(log masih kosong)": "(log is empty)",
  Ikuti: "Follow",
  Tutup: "Close",

  // Projects
  "Setiap folder di": "Every folder in",
  "otomatis menjadi": "automatically becomes",
  nama: "name",
  "Project baru": "New project",
  "Buka folder www": "Open www folder",
  "Muat ulang": "Refresh",
  "Service belum berjalan — buka tab Service lalu klik Start semua.":
    "Services are not running — open the Services tab and click Start all.",
  "Belum ada project.": "No projects yet.",
  "Buat folder baru di": "Create a new folder in",
  ", lalu klik Muat ulang. Atur versi lewat file": ", then click Refresh. Set versions with a",
  "di folder project.": "file in the project folder.",
  alias: "aliases",
  Buka: "Open",
  Folder: "Folder",
  "Buat database untuk {site} di {db}": "Create a database for {site} on {db}",
  "Buat DB": "Create DB",

  // Runtimes
  "Hapus {name} {version}?": "Remove {name} {version}?",
  "Banyak versi bisa terinstall sekaligus; tiap project memilih versinya sendiri.":
    "Many versions can be installed side by side; each project picks its own.",
  "Memuat daftar…": "Loading list…",
  "Install versi…": "Install version…",
  " — terinstall": " — installed",
  "Menginstall…": "Installing…",
  Batal: "Cancel",
  "Belum ada versi terinstall.": "No versions installed yet.",
  "Jadikan default": "Make default",
  Hapus: "Remove",

  // PHP
  "max_execution_time (detik)": "max_execution_time (seconds)",
  "date.timezone (mis. Asia/Jakarta)": "date.timezone (e.g. Asia/Jakarta)",
  "Belum ada PHP terinstall. Install dulu lewat tab Runtime.": "No PHP installed yet. Install one from the Runtimes tab.",
  "Ekstensi, Xdebug, dan php.ini per versi. Pool PHP yang berjalan di-restart otomatis.":
    "Extensions, Xdebug, and php.ini per version. Running PHP pools restart automatically.",
  "Buka php.ini": "Open php.ini",
  aktif: "enabled",
  "terpasang, nonaktif": "installed, disabled",
  "belum terpasang": "not installed",
  "Mode debug + develop, mulai saat ada trigger": "debug + develop mode, starts on the",
  ", port 9003 (VS Code / PhpStorm).": " trigger, port 9003 (VS Code / PhpStorm).",
  "Xdebug terpasang dari xdebug.org dan aktif.": "Xdebug installed from xdebug.org and enabled.",
  "Mengunduh…": "Downloading…",
  "Install & aktifkan": "Install & enable",
  "Xdebug dinonaktifkan.": "Xdebug disabled.",
  "Xdebug diaktifkan.": "Xdebug enabled.",
  Nonaktifkan: "Disable",
  Aktifkan: "Enable",
  "php.ini disimpan.": "php.ini saved.",
  "default PHP": "PHP default",
  "Menyimpan…": "Saving…",
  Simpan: "Save",
  Ekstensi: "Extensions",
  "Cari ekstensi…": "Search extensions…",
  "{name} diaktifkan.": "{name} enabled.",
  "{name} dinonaktifkan.": "{name} disabled.",

  // Settings
  "Disimpan ke devlock.toml (komentar tetap utuh) dan langsung diterapkan bila daemon berjalan.":
    "Saved to devlock.toml (comments are kept) and applied right away when the daemon is running.",
  "Folder www (tiap subfolder menjadi site)": "www folder (each subfolder becomes a site)",
  "TLD domain lokal": "Local domain TLD",
  "Web server": "Web server",
  "Port HTTP": "HTTP port",
  "HTTPS untuk semua site (butuh": "HTTPS for all sites (needs",
  "sekali)": "once)",
  "Alihkan HTTP ke HTTPS untuk site ber-HTTPS (per project:": "Redirect HTTP to HTTPS for HTTPS sites (per project:",
  "Port HTTPS": "HTTPS port",
  "Worker php-cgi per versi PHP (1–10)": "php-cgi workers per PHP version (1–10)",
  "Jalankan semua service saat DevLock dibuka": "Start all services when DevLock opens",
  "Buka DevLock (di tray) saat login Windows": "Open DevLock (in the tray) when signing in to Windows",
  "Editor (kosong = deteksi VS Code, Cursor, Windsurf, Zed, Sublime, PhpStorm)":
    "Editor (empty = detect VS Code, Cursor, Windsurf, Zed, Sublime, PhpStorm)",
  Tampilan: "Appearance",
  Bahasa: "Language",
  Tema: "Theme",
  "Ikuti Windows": "Follow Windows",
  Terang: "Light",
  Gelap: "Dark",
  "Tema langsung berubah, tanpa perlu Simpan.": "The theme changes right away, no need to Save.",
  "Bahasa berubah setelah disimpan (jendela dimuat ulang). Pesan dari CLI tetap berbahasa Indonesia.":
    "The language changes after saving (the window reloads). Messages from the CLI stay in Indonesian.",

  // NewProject
  "Project siap di {dir}": "Project ready in {dir}",
  "Nama (folder & domain): huruf kecil, angka, tanda minus": "Name (folder & domain): lowercase letters, digits, hyphens",
  "mis. toko-online": "e.g. online-shop",
  "Versi PHP": "PHP version",
  "Versi Node.js": "Node.js version",
  Default: "Default",
  "(wajib)": "(required)",
  "(opsional)": "(optional)",
  "Tanpa database": "No database",
  "Install {what} dulu lewat tab Runtime.": "Install {what} from the Runtimes tab first.",
  "Nama hanya huruf kecil, angka, dan tanda minus.": "Use only lowercase letters, digits, and hyphens.",
  "Membuat…": "Creating…",
  "Buat project": "Create project",
};
