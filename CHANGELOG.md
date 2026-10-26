# Changelog

Semua perubahan penting dicatat di sini. Format mengikuti
[Keep a Changelog](https://keepachangelog.com/id/1.1.0/) dan versi mengikuti
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/romyrudiansah/devlock/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/romyrudiansah/devlock/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/romyrudiansah/devlock/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/romyrudiansah/devlock/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/romyrudiansah/devlock/releases/tag/v0.1.0
