# Changelog

Semua perubahan penting dicatat di sini. Format mengikuti
[Keep a Changelog](https://keepachangelog.com/id/1.1.0/) dan versi mengikuti
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/romyrudiansah/devlock/compare/v0.4.2...HEAD
[0.4.2]: https://github.com/romyrudiansah/devlock/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/romyrudiansah/devlock/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/romyrudiansah/devlock/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/romyrudiansah/devlock/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/romyrudiansah/devlock/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/romyrudiansah/devlock/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/romyrudiansah/devlock/releases/tag/v0.1.0
