# DevLock

Local development environment untuk Windows — alternatif Laragon / WAMP / XAMPP dengan
**multi-versi runtime** (PHP, Node, MySQL/MariaDB, PostgreSQL, Redis) yang bisa berjalan
**bersamaan** dan dipilih **per-project**.

> Status: Fase 2 (CLI + GUI dasar dengan tray). Lihat [docs/ROADMAP.md](docs/ROADMAP.md).

## Konsep singkat

```toml
# www/blog/.devlock.toml
php = "8.2"
node = "20"
database = "mysql@8.4"
```

- `php`, `node`, `composer`, `mysql`, … di folder `shims\` otomatis memakai versi milik project.
- Tiap versi PHP berjalan sebagai pool php-cgi sendiri, jadi `blog.test` (PHP 7.4) dan
  `shop.test` (PHP 8.3) aktif bersamaan.
- Tiap engine database + versi adalah instance terpisah (port & data dir sendiri).

## CLI

```
devlock init                       siapkan struktur folder + devlock.toml
devlock list php --available       lihat versi yang bisa diinstall
devlock install php 8.3            install runtime
devlock use php 8.2 [--global]     pilih versi (per project / global)
devlock which php                  versi & path yang dipakai di folder ini
devlock current                    versi php/node/composer/database di folder ini
devlock path add                   taruh folder shims di depan PATH user
devlock site list                  daftar project di www\ beserta domain .test
devlock start [service]            jalankan service (all, nginx, php@8.3, mysql@8.4, dev:<site>)
devlock stop [service]             hentikan service (tanpa argumen: semua + daemon)
devlock status                     status service
devlock logs <service> [-f]        log service
devlock hosts                      samakan file hosts dengan daftar site
devlock doctor                     diagnosa PATH, port, dependency
```

## GUI

`devlock-gui.exe` (satu folder dengan `devlock.exe`) membuka dashboard dan ikon tray:

- **Service**: start/stop/restart semua atau per grup (`php@8.3`, `mysql@8.4`, `dev:<site>`), status, dan log
- **Project**: daftar site di `www\`, buka di browser, folder, atau terminal (shim sudah di PATH)
- **Runtime**: install versi baru dengan progress, hapus, pilih default

Menutup jendela menyembunyikannya ke tray; service tetap diawasi daemon.

## Build dari source

Butuh Rust stable (toolchain MSVC) dan Node.js 20.19+.

```
cd apps\desktop
npm ci
npm run build
cd ..\..
cargo build --release
```

Binary ada di `target\release\`: `devlock.exe`, `devlock-shim.exe`, dan `devlock-gui.exe`.

Mode development GUI (hot reload): `cd apps\desktop` lalu `npx tauri dev`. Untuk mengutak-atik tampilan saja, `npm run dev` lalu buka `http://localhost:1420` di browser: tanpa backend Tauri, UI memakai data contoh (`src/devMock.ts`).

## Kontribusi

Lihat [CONTRIBUTING.md](CONTRIBUTING.md) untuk alur branch, format commit, dan rilis.
