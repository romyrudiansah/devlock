# Changelog

Semua perubahan penting dicatat di sini. Format mengikuti
[Keep a Changelog](https://keepachangelog.com/id/1.1.0/) dan versi mengikuti
[Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.15.1] - 2026-10-29

### Added
- GUI: pilihan **Tema** di Settings › Tampilan (Ikuti Windows / Terang / Gelap), langsung
  berlaku dan ikut mengubah Mica serta title bar jendela.

### Changed
- GUI: tombol di kartu Project kini berupa ikon (Buka, Folder, Terminal, Editor, Buat DB) dengan
  tooltip, seperti di Beranda.
- GUI: halaman PHP memakai dua kolom sama lebar (Xdebug | php.ini).

## [0.15.0] - 2026-10-29

### Added
- `node = "system"` dan `composer = "system"` (di `.devlock.toml`, `[defaults]`, atau
  `devlock use node system --global`): pakai Node.js/Composer yang terpasang global di Windows
  (dicari di PATH, di luar folder DevLock). Berlaku juga untuk dev server dan tombol Terminal;
  Composer global dijalankan dengan PHP versi project. `npm install -g` tetap ke prefix bawaan npm.
- `.devlock.toml` kini bisa memilih versi Composer per project (`composer = "2.8"`).

### Fixed
- Service yang exe atau DLL-nya diblokir Smart App Control tidak lagi memunculkan dialog
  "Bad Image" berulang tiap restart; service langsung berhenti dengan pesan "diblokir Smart App
  Control Windows" di daftar service.
- GUI (mode gelap): daftar pilihan dropdown (mis. versi saat install runtime) tampil putih
  dengan teks tak terbaca; kini berlatar gelap.

## [0.14.1] - 2026-10-29

### Fixed
- `devlock update` menghapus zip rilis di `tmp\downloads` setelah dipasang atau gagal, agar tidak
  menumpuk tiap update.
- GUI (English): judul Beranda saat ada service gagal kini "Services with problems: n".

## [0.14.0] - 2026-10-29

### Changed
- GUI didesain ulang bergaya Windows 11 ("Fluent Compact"): rail navigasi ikon, latar Mica di
  Windows 11 (warna solid di Windows 10), tombol, toggle, dan InfoBar Fluent; tema mengikuti Windows.

### Added
- GUI: halaman Beranda berisi status ringkas, satu tombol Start/Stop semua, tile service dengan
  toggle on/off, dan daftar site dengan aksi Buka / Folder / Terminal / Editor.
- GUI: palet perintah <kbd>Ctrl</kbd>+<kbd>K</kbd> untuk start/stop/restart service, membuka site,
  editor, terminal, folder, project baru, dan berpindah halaman.
- GUI: toggle per service dan tombol ikon Restart/Log di halaman Service.

## [0.13.0] - 2026-10-28

### Changed
- Logo baru: dua kurung kode yang membentuk heksagon dengan garis miring di tengah (`</>`). Dipakai di ikon aplikasi, tray, installer (`icon.ico` 16–256 px), header GUI, README, dan website. Sumber di `brand/` (`logo-mark.svg` tanpa kotak, `logo-app.svg` ikon aplikasi, PNG 16–512 px).
- Website dwibahasa: versi Inggris di `/en/` (halaman utama dan panduan lengkap) dengan pemilih ID/EN, `hreflang`, dan screenshot GUI berbahasa Inggris.

## [0.12.3] - 2026-10-28

### Changed
- Notifikasi di halaman Service GUI bisa ditutup (×); peringatan yang ditutup tidak muncul lagi selama teksnya sama. Peringatan "CA DevLock belum dipercaya" tidak bisa ditutup, tetapi punya tombol **Percayai CA DevLock** yang memasang CA lewat dialog Windows dan memperbarui bundle CA PHP.

## [0.12.2] - 2026-10-28

### Fixed
- `dev = "npm run dev"` di project PHP (mis. Laravel + Vite) tidak lagi mengubah site menjadi reverse proxy ke dev server: domain tetap dilayani PHP dan dev server berjalan sebagai service di sampingnya (tanpa `PORT`). `proxy_port` tetap memaksa mode proxy.

## [0.12.1] - 2026-10-28

### Fixed
- `devlock update` kini juga memperbarui salinan shim di `shims\` (sebelumnya shim tetap versi lama dan shim baru, mis. `java`, baru muncul setelah `devlock shims`).

## [0.12.0] - 2026-10-28

### Added
- Runtime Java (Eclipse Temurin): `devlock install java <versi>` dari API Adoptium (JDK terbaru tiap versi LTS ditambah versi fitur terbaru, zip Windows x64 ber-SHA256). Versi per project lewat `java` di `.devlock.toml` atau `.java-version` (`21`, `temurin-21.0.5`, `1.8`); shim `java`, `javac`, `jar` mengisi `JAVA_HOME` untuk Maven/Gradle.
- Mirror fallback unduhan runtime: bila arsip gagal diunduh dari sumber asli, dicoba mirror dari `[mirrors]` lalu mirror bawaan (`windows.php.net` untuk PHP, `npmmirror.com` untuk Node; matikan dengan `[download] builtin_mirrors = false`). Checksum tetap dari sumber asli/katalog resmi.
- GUI dan menu tray dwibahasa Indonesia/Inggris: pilih di Settings → Tampilan (`[gui] language = "id"|"en"`). Pesan dari backend/CLI tetap berbahasa Indonesia.

## [0.11.0] - 2026-10-28

### Added
- Mailpit sebagai penangkap email lokal: `devlock install mailpit` (axllent/mailpit, SHA256 dari GitHub) menjalankan service `mailpit` (SMTP `127.0.0.1:1025`, UI `8025`, data di `data\mailpit`, ubah lewat `[mail]`) dan site bawaan `mail.<tld>`. `mail()` PHP semua versi diarahkan ke Mailpit (`SMTP`/`smtp_port` di `php.ini`), dan project Laravel baru mendapat `MAIL_MAILER=smtp`, `MAIL_HOST`, `MAIL_PORT`. Target `devlock start/stop mail`.
- `devlock share <site>`: membagikan site lewat Cloudflare Quick Tunnel (`devlock install cloudflared`, tanpa akun) ke URL acak `*.trycloudflare.com` sampai Ctrl+C. Site dengan redirect HTTPS memakai origin TLS yang diverifikasi dengan CA DevLock.
- Runtime Go (`devlock install go`, zip resmi go.dev) dan Python (`devlock install python`, build portable python-build-standalone) multi-versi: versi per project lewat `go`/`python` di `.devlock.toml`, `go.mod` (`toolchain`, lalu `go`), dan `.python-version`; shim `go`, `gofmt`, `python`, dan `pip` (`python -m pip`). Installer kini bisa mengekstrak `.tar.gz`.
- `devlock import [folder] [--copy] [--dry-run]`: impor project dari Laragon (`www`) atau XAMPP (`htdocs`) sebagai directory junction (tanpa admin) atau salinan; nama site dibuat aman untuk DNS.
- `devlock config export|import`: pindah komputer lewat satu file TOML berisi `devlock.toml` dan daftar runtime terinstall (impor mencadangkan `devlock.toml` lama dan menginstall runtime yang kurang). `devlock profile save|use|list|delete`: simpan grup service yang sedang berjalan ke `[profiles]` dan ganti cepat antar kombinasi.
- `devlock update [--check]`: update dari GitHub Releases dengan verifikasi SHA256; binary lama disimpan sebagai `.old` dan otomatis dipulihkan bila Smart App Control memblokir binary baru.
- Installer NSIS `devlock-vX.Y.Z-windows-x64-setup.exe` (per user di `%LOCALAPPDATA%\DevLock`, tanpa admin, shortcut Start Menu, uninstall dari *Apps & features* tanpa menghapus runtime, data, dan project), diterbitkan bersama zip portable.

## [0.10.0] - 2026-10-28

### Added
- MongoDB sebagai instance database: `devlock install mongodb 8.0` (Community resmi dari `downloads.mongodb.org`, SHA256) dan `database = "mongodb@8.0"` menjalankan `mongod` per major.minor di `127.0.0.1` (port `27000 + major*10 + minor`, 8.0 → 27080; data di `data\mongodb\<versi>`). `devlock install mongosh` (GitHub Releases) menyediakan shim `mongosh` yang langsung terhubung ke instance project dan dipakai untuk shutdown yang rapi. `devlock start db` mencakup MongoDB; `devlock db`, Adminer, dan isian `.env` scaffold tetap khusus database SQL.
- Port dev server Node dideteksi otomatis: cukup `dev = "pnpm dev"` tanpa `proxy_port`. Dev server menerima `PORT` tetap per site (3100–3899), daemon membaca URL loopback terakhir dari log-nya (Vite, Next.js, Nuxt, Astro, Angular, …) lalu nginx di-reload ke port itu tanpa me-restart dev server. `proxy_port` tetap mengunci port.
- Profil Windows Terminal "DevLock" (`devlock terminal install|remove|status`): PowerShell di folder www dengan shim di depan PATH, dipasang sebagai JSON fragment tanpa mengubah `settings.json`. Profil yang sudah dipasang diperbarui saat GUI dibuka, `devlock init`, dan `devlock shims`.
- Tombol **Editor** di halaman Projects dan menu tray per project: memakai `[gui] editor` (juga di Settings) atau mendeteksi VS Code, Cursor, Windsurf, Zed, Sublime Text, atau PhpStorm.
- `devlock ssl trust --machine` / `untrust --machine`: CA DevLock di store Root komputer lewat UAC, karena Firefox hanya mengimpor root dari store komputer. `devlock ssl status` menampilkan kedua store.
- Redirect HTTP→HTTPS opsional: `[https] redirect = true` (atau `devlock ssl redirect on`, atau toggle di Settings GUI) untuk semua site ber-HTTPS, dan `https_redirect = true/false` per project di `.devlock.toml`. Port HTTP site itu hanya membalas `301` ke `https://` (port ditulis bila bukan 443). `devlock ssl status` menandai site yang dialihkan.

## [0.9.2] - 2026-10-28

### Fixed
- Log Redis (`logs\redis@<major>.log`) selalu kosong: log service dibuka sebagai handle append-only (`FILE_APPEND_DATA` tanpa `FILE_WRITE_DATA`), dan program berbasis msys2/Cygwin diam-diam gagal menulis ke handle itu. Log kini dibuka dengan akses tulis biasa di posisi akhir file.

## [0.9.1] - 2026-10-28

### Fixed
- `adminer.test` selalu fatal error: override di `index.php` pembungkus memakai type hint, padahal method `Adminer\Adminer` di rilis resmi (minified) tidak bertipe sehingga PHP menolak deklarasinya.
- Redis tidak bisa start: build msys2 menganggap path config `D:/...` sebagai path relatif. Config kini diberikan sebagai nama file dengan folder kerja `etc\redis`.
- Garnet tidak bisa start karena rilis Windows-nya butuh runtime .NET (bukan self-contained seperti dinyatakan di v0.9.0). `devlock install garnet` kini ikut memasang runtime .NET resmi Microsoft sesuai versi yang dibutuhkan bundle (portable di `runtimes\garnet\<versi>\dotnet`, diverifikasi SHA512), dan service Garnet memakainya lewat `DOTNET_ROOT`. Garnet yang sudah terinstall dengan v0.9.0 cukup menjalankan `devlock install garnet` lagi untuk melengkapi runtime-nya.

## [0.9.0] - 2026-10-28

### Added
- Redis dan Garnet sebagai server cache per project: `devlock install redis` (build Windows komunitas redis-windows, varian msys2) dan `devlock install garnet` (Microsoft Garnet), keduanya dari GitHub Releases dengan SHA256 dari GitHub. Pilih dengan `cache = "redis@8"` / `"garnet@2"` di `.devlock.toml` atau `devlock use cache … [--global]`. Instance per major dengan port sendiri (`redis@7` → 6379, `redis@8` → 6389, `garnet@2` → 6402, bisa ditimpa `[ports]`); Redis memakai `etc\redis\<major>.conf` hasil generate, data di `data\redis\<major>`, dan berhenti lewat `SHUTDOWN` (snapshot tersimpan). Target `devlock start/stop cache`, shim `redis-cli` yang otomatis terhubung ke instance project, dan `devlock current` menampilkan cache.
- Nama file unduhan diberi awalan versi bila nama aset upstream tidak memuat versi (mis. Garnet), agar unduhan versi lain tidak saling menimpa atau dilanjutkan dari file yang salah.
- Adminer bawaan: `devlock install adminer` (rilis resmi vrana/adminer, SHA256 dari GitHub) langsung menyediakan `adminer.<tld>` dengan PHP default. `index.php` pembungkus hasil generate menawarkan instance MySQL/MariaDB/PostgreSQL DevLock di field server dan mengizinkan login tanpa password (user lokal DevLock memang tanpa password; site hanya di `127.0.0.1`). Folder `www\adminer` milik pengguna menggantikan site bawaan.

### Changed
- README dan CONTRIBUTING: rilis dinyatakan belum ditandatangani karena pendaftaran SignPath Foundation belum disetujui (bisa diajukan ulang nanti), disertai cara menangani peringatan SmartScreen / Smart App Control. Tanda tangan lewat SignPath di workflow rilis tetap opsional.

## [0.8.1] - 2026-10-27

### Fixed
- PHP di site `webserver = "apache"` selalu 500: handler balancer kini `proxy:balancer://phpXY/` (tanpa garis miring penutup mod_proxy_balancer tidak menemukan balancer-nya) dan `SCRIPT_FILENAME` dikirim sebagai path file Windows (`ProxyFCGISetEnvIf`), karena php-cgi tidak memahami format URL proxy ala PHP-FPM.
- Shared memory Apache (`slotmem-shm`) disimpan di `tmp\apache` (`DefaultRuntimeDir`), bukan di folder runtime; vhost default Apache punya `DocumentRoot` sendiri.

## [0.8.0] - 2026-10-27

### Added
- Apache per project: `devlock install apache` (build Win64 Apache Lounge, SHA256 dari file checksum resminya; dokumentasi `manual\` dilewati) dan `webserver = "apache"` di `.devlock.toml` (atau global di `devlock.toml`). nginx tetap di depan (port 80/443, HTTPS) dan meneruskan site itu ke Apache di `127.0.0.1:8180` (`[apache] port`); Apache memproses `.htaccess` dan menjalankan PHP lewat pool php-cgi yang sama (mod_proxy_fcgi + balancer, `ProxyFCGIBackendType GENERIC`), IP klien & HTTPS diteruskan dari header nginx. Service `apache` dijalankan sebelum nginx dan otomatis start saat site pindah ke Apache. Bila Apache belum terinstall, site tetap dilayani nginx dengan peringatan. `devlock site list` dan tab Project menandai site Apache; tab Runtime menampilkan Apache.

## [0.7.0] - 2026-10-27

### Added
- Menu tray menampilkan status service (mis. "Service: 6/6 berjalan", juga di tooltip) dan submenu **Project**: tiap site bisa dibuka di browser, dibuka foldernya, atau dibuka terminalnya (dengan shim di PATH) langsung dari tray. Menu diperbarui otomatis bila status atau daftar site berubah.
- DNS wildcard `*.<tld>`: `devlock dns enable|disable|status`. Service `dns` (`127.0.0.1:53`, UDP + TCP) menjawab semua nama di bawah TLD DevLock dengan `127.0.0.1` (AAAA kosong agar browser memakai IPv4), dan aturan NRPT Windows (dipasang lewat satu UAC) mengarahkan hanya query `.<tld>` ke sana. Selama aktif, domain `*.<tld>` tidak lagi ditulis ke file hosts dan tidak memicu peringatan hosts; alias di luar TLD tetap lewat hosts. Bila query ke `127.0.0.1:53` diambil program lain (mis. layanan Internet Connection Sharing), `enable`, `status`, dan service `dns` melaporkannya dengan jelas. `devlock doctor` ikut menguji resolusi saat DNS aktif.

### Fixed
- Teks bantuan `devlock db dump` menampilkan path default `data\backups\…` dengan benar (sebelumnya berisi karakter kontrol).

## [0.6.0] - 2026-10-27

### Added
- Unduhan runtime yang terputus dilanjutkan otomatis (HTTP `Range`) saat perintah install diulang; server tanpa dukungan resume diunduh ulang dari awal.
- Cache katalog versi di `tmp\catalog\<runtime>.json` (6 jam) untuk `devlock list --available`, `devlock install`, dan GUI; bila sumber resmi tidak bisa dihubungi, cache lama tetap dipakai. `devlock list <runtime> --available --refresh` memaksa ambil ulang.
- Bundle CA untuk PHP: `ssl\ca-bundle.pem` (bundle Mozilla dari curl.se, diverifikasi SHA256, ditambah CA DevLock) dipasang ke `curl.cainfo` dan `openssl.cafile` saat install PHP, `devlock ssl trust/enable`, dan perintah baru `devlock ssl bundle`. Request HTTPS dari PHP (curl, `file_get_contents`, Composer) tidak lagi gagal "unable to get local issuer certificate", termasuk ke site HTTPS lokal DevLock.
- Folder `dist\` ikut dideteksi sebagai document root (setelah `public`, `web`, `public_html`).
- Daemon memantau folder `www`, `.devlock.toml`, dan `devlock.toml` (tiap ±3 detik): site baru/dihapus/berubah diterapkan tanpa `devlock reload`, dan service yang dibutuhkan site baru atau berubah (pool PHP, database, dev server) langsung dijalankan bila service lain sedang berjalan.
- Perubahan yang hanya menyentuh `nginx.conf` memakai `nginx -s reload` (tanpa memutus koneksi; fallback ke restart bila gagal).
- Peringatan bila ada domain site yang belum ada di file hosts.
- Rotasi log: log service dan `daemon.log` di atas 10 MB diganti nama menjadi `.1` saat (re)start.
- Port HTTPS (443) ikut dicek bentrok sebelum nginx start bila ada site HTTPS.
- Alat PHP per versi: `devlock php ext list|enable|disable`, `devlock php ini get|set|path`, dan `devlock php xdebug status|install|on|off [--php 8.3]`. Xdebug diunduh dari xdebug.org sesuai versi PHP (NTS x64, compiler cocok) dengan verifikasi SHA256, lalu diaktifkan (mode debug+develop, trigger `XDEBUG_SESSION`, port 9003). Pool php-cgi yang berjalan di-restart otomatis setelah perubahan.
- GUI: tab **PHP** (pilih versi, centang ekstensi, Xdebug satu klik, nilai php.ini umum, buka php.ini) dan tab **Settings** (folder www, TLD, port HTTP/HTTPS, HTTPS global, worker php-cgi, autostart service saat GUI dibuka, buka DevLock di tray saat login Windows).
- `[gui] autostart_services` di `devlock.toml`; `devlock-gui.exe --minimized` langsung ke tray.
- Quick create project: `devlock new <template> <nama> [--php] [--node] [--db]` dan tombol **Project baru** di GUI (log composer/npm tampil langsung). Template: Laravel, Symfony, CodeIgniter 4 (`composer create-project --no-interaction` dengan PHP pilihan), WordPress (zip resmi wordpress.org, verifikasi SHA1, `wp-config.php` terisi dengan database & salt acak), Next.js, Nuxt, Vite + React, Vite + Vue (kerangka minimal + `npm install <paket>@latest` dengan Node pilihan, dev server & `proxy_port` diatur otomatis), dan HTML statis. Database dibuat otomatis dan `.env` Laravel diisi (`DB_CONNECTION`, port instance, `APP_URL`).
- DevLock kini open source di bawah **MIT OR Apache-2.0** (`LICENSE-MIT`, `LICENSE-APACHE`, ikut di zip rilis).
- Metadata versi Windows (ProductName "DevLock", versi, deskripsi, copyright, ikon) untuk `devlock.exe` dan `devlock-shim.exe`; GUI mendapat publisher & copyright yang sama.
- `devlock hosts --clear` menghapus blok DevLock dari file hosts (untuk uninstall).
- README: bagian "Perubahan pada sistem", "Uninstall", "Privasi", "Code signing policy", dan "Lisensi"; CONTRIBUTING: peran tim, kewajiban 2FA, dan setup SignPath.
- Workflow rilis siap menandatangani binary lewat SignPath Foundation (aktif setelah secret `SIGNPATH_API_TOKEN` diisi), lalu memverifikasi tanda tangan setiap exe sebelum membuat zip. Catatan rilis menautkan Code signing policy.

## [0.5.0] - 2026-10-27

### Added
- Aksi cepat database untuk MySQL/MariaDB/PostgreSQL: `devlock db list|create|drop|dump|restore [--instance engine@versi]`. Instance default diambil dari `database` di `.devlock.toml` atau default global. Nama database divalidasi (`[A-Za-z_][A-Za-z0-9_]*`), database sistem dilindungi, `drop` wajib `--yes`, dump default ke `data\backups\<instance>\<db>-<waktu>.sql`.
- Tombol "Buat DB" di kartu project GUI: membuat database bernama sesuai site (`my-shop` → `my_shop`) di instance project.
- HTTPS lokal: CA "DevLock Local CA" dibuat sekali di `ssl\`, sertifikat per site (domain + alias, ditandatangani CA, diperbarui otomatis saat domain berubah atau mendekati kedaluwarsa), dan nginx mendengarkan juga di `127.0.0.1:443`. Aktifkan global dengan `devlock ssl enable` (`[https] enabled`) atau per project dengan `https = true` di `.devlock.toml`.
- `devlock ssl status|trust|untrust|enable|disable`. `trust` memasang CA ke store Root milik user (Windows meminta konfirmasi), sehingga Chrome/Edge langsung percaya.
- Node (shim dan dev server) mendapat `NODE_EXTRA_CA_CERTS` agar percaya sertifikat site HTTPS.
- `devlock site list` dan GUI menampilkan URL `https://` untuk site HTTPS.
- Status service `starting`: service baru dianggap `running` setelah port-nya menerima koneksi. `devlock start` (dan tombol Start di GUI) menunggu hingga 20 detik sampai php-cgi, database, nginx, dan dev server siap, sehingga request pertama tidak lagi kena 502.

### Fixed
- Target `db` / `database` pada `devlock start/stop/restart` kini juga mencakup instance PostgreSQL.

## [0.4.2] - 2026-10-27

### Fixed
- `devlock start` (juga tombol Start di GUI dan menu tray) tidak lagi menggantung bila output-nya di-pipe atau di-redirect, mis. `devlock start 2>&1 | Out-String`. Daemon latar dulu mewarisi semua handle pemanggil (termasuk pipe stdout/stderr) sehingga pipe tidak pernah EOF selama daemon hidup; kini daemon hanya mewarisi handle log-nya, dan service yang dijalankan daemon tidak lagi ikut memegang stdout/stderr daemon.

## [0.4.1] - 2026-10-26

### Fixed
- File hosts hanya berisi baris `127.0.0.1` (sebelumnya juga `::1`). Browser mendahulukan IPv6, sehingga domain `.test` bisa mendarat di program lain yang memakai `[::]:80` (mis. Apache Laragon) padahal nginx DevLock hanya mendengarkan di `127.0.0.1`.
- Deteksi bentrok port memakai tabel port Windows (IPv4 & IPv6, lengkap dengan nama proses). `devlock start` gagal dengan pesan jelas bila program lain memakai alamat yang sama, dan memberi peringatan bila program lain memakai `0.0.0.0`/`[::]` di port yang sama. `devlock doctor` tidak lagi melaporkan "port bebas" saat Laragon/XAMPP berjalan.

## [0.4.0] - 2026-10-26

### Added
- PostgreSQL multi-versi (13+ yang masih didukung): `devlock install postgresql 16`, instance `postgresql@16` (port `5400 + major`, data dir `data\postgresql\16`), `initdb` otomatis, berhenti rapi lewat `pg_ctl stop -m fast`. Binary resmi dari EnterpriseDB; pgAdmin, StackBuilder, dokumentasi, dan header dilewati saat ekstrak (~150 MB, bukan ~900 MB).
- Shim `psql`, `pg_dump`, `pg_restore`, `createdb`, `dropdb` otomatis terhubung ke instance PostgreSQL milik project (`PGHOST`/`PGPORT`/`PGUSER`).
- Tab Runtime di GUI menampilkan PostgreSQL.

## [0.3.1] - 2026-10-26

### Fixed
- `devlock-gui.exe` di zip rilis menampilkan halaman error "localhost refused to connect" karena dibangun tanpa fitur `tauri/custom-protocol` sehingga mencari dev server. Build rilis kini menyalakan fitur `devlock-desktop/custom-protocol`.

## [0.3.0] - 2026-10-26

### Added
- GUI desktop (`devlock-gui.exe`, Tauri 2 + React): tab Service (start/stop/restart per grup, status, uptime, restart, log), tab Project (buka di browser, folder, terminal dengan shim di PATH), tab Runtime (install dengan progress, hapus, jadikan default).
- Ikon tray: buka dashboard, start/stop semua, buka folder www, keluar (dengan atau tanpa menghentikan service). Menutup jendela menyembunyikannya ke tray; hanya satu instance yang berjalan.

### Fixed
- File `.sha256` di GitHub Release memakai akhiran baris LF sehingga `sha256sum -c` langsung berhasil.

## [0.2.0] - 2026-10-26

### Added
- Runtime manager: katalog dari sumber resmi (PHP, Node, Composer, nginx, MySQL, MariaDB), `devlock list/install/uninstall`, verifikasi SHA256, `php.ini` otomatis.
- Shim per-project (`php`, `node`, `npm`, `npx`, `pnpm`, `yarn`, `composer`, `mysql`, ...) dengan resolusi versi dari `.devlock.toml`, `.nvmrc`, `.node-version`, `composer.json`, `package.json#volta`.
- `devlock use`, `which`, `current`, `shims`, `path add|remove`.
- Service manager: daemon latar dengan Job Object (seluruh pohon proses ikut berhenti), restart otomatis dengan backoff.
- nginx + vhost otomatis untuk setiap folder di `www\` (`<nama>.test`): PHP, static, dan reverse proxy dev server.
- Pool php-cgi per versi PHP yang berjalan bersamaan; instance MySQL/MariaDB per versi dengan port & data dir sendiri (inisialisasi otomatis, shutdown rapi).
- Dev server project Node dijalankan sebagai service dengan versi Node milik project.
- `devlock start/stop/restart/reload/status/logs`, `devlock site list`, `devlock hosts` (blok domain di file hosts, minta UAC bila perlu).

## [0.1.0] - 2026-10-26

### Added
- Roadmap dan breakdown fitur (`docs/ROADMAP.md`).
- Konvensi branch, commit, dan rilis (`CONTRIBUTING.md`).
- Workspace Rust: `devlock-core` (path, konfigurasi global `devlock.toml`, konfigurasi project `.devlock.toml`) dan CLI `devlock`.
- Perintah `devlock init`, `devlock config`, `devlock doctor`.
- CI (fmt, clippy, test di Windows) dan workflow rilis otomatis dari tag `v*`.

[Unreleased]: https://github.com/romyrudiansah/devlock/compare/v0.15.1...HEAD
[0.15.1]: https://github.com/romyrudiansah/devlock/compare/v0.15.0...v0.15.1
[0.15.0]: https://github.com/romyrudiansah/devlock/compare/v0.14.1...v0.15.0
[0.14.1]: https://github.com/romyrudiansah/devlock/compare/v0.14.0...v0.14.1
[0.14.0]: https://github.com/romyrudiansah/devlock/compare/v0.13.0...v0.14.0
[0.13.0]: https://github.com/romyrudiansah/devlock/compare/v0.12.3...v0.13.0
[0.12.3]: https://github.com/romyrudiansah/devlock/compare/v0.12.2...v0.12.3
[0.12.2]: https://github.com/romyrudiansah/devlock/compare/v0.12.1...v0.12.2
[0.12.1]: https://github.com/romyrudiansah/devlock/compare/v0.12.0...v0.12.1
[0.12.0]: https://github.com/romyrudiansah/devlock/compare/v0.11.0...v0.12.0
[0.11.0]: https://github.com/romyrudiansah/devlock/compare/v0.10.0...v0.11.0
[0.10.0]: https://github.com/romyrudiansah/devlock/compare/v0.9.2...v0.10.0
[0.9.2]: https://github.com/romyrudiansah/devlock/compare/v0.9.1...v0.9.2
[0.9.1]: https://github.com/romyrudiansah/devlock/compare/v0.9.0...v0.9.1
[0.9.0]: https://github.com/romyrudiansah/devlock/compare/v0.8.1...v0.9.0
[0.8.1]: https://github.com/romyrudiansah/devlock/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/romyrudiansah/devlock/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/romyrudiansah/devlock/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/romyrudiansah/devlock/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/romyrudiansah/devlock/compare/v0.4.2...v0.5.0
[0.4.2]: https://github.com/romyrudiansah/devlock/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/romyrudiansah/devlock/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/romyrudiansah/devlock/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/romyrudiansah/devlock/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/romyrudiansah/devlock/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/romyrudiansah/devlock/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/romyrudiansah/devlock/releases/tag/v0.1.0
