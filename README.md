# DevLock

Local development environment untuk Windows — alternatif Laragon / WAMP / XAMPP dengan
**multi-versi runtime** (PHP, Node, MySQL/MariaDB, PostgreSQL) yang bisa berjalan
**bersamaan** dan dipilih **per-project**.

> Status: Fase 3 (CLI + GUI, PostgreSQL, HTTPS lokal, aksi database). Open source di bawah MIT OR Apache-2.0. Lihat [docs/ROADMAP.md](docs/ROADMAP.md).

## Unduh

Unduh `devlock-vX.Y.Z-windows-x64.zip` dari halaman
[**Releases**](https://github.com/romyrudiansah/devlock/releases) (Windows 10/11 x64), cocokkan
checksum-nya dengan file `.sha256` di rilis yang sama, lalu:

1. Ekstrak ke folder instalasi, mis. `D:\DevLock` (hasilnya `D:\DevLock\bin\devlock.exe`, dst.).
2. Jalankan `D:\DevLock\bin\devlock.exe init`, lalu `devlock path add` agar shim masuk PATH.
3. Buka `devlock-gui.exe` untuk dashboard dan ikon tray, atau lanjut lewat CLI di bawah.

**Code signing:** binary rilis DevLock ditandatangani lewat
[SignPath.io](https://about.signpath.io/) dengan sertifikat dari
[SignPath Foundation](https://signpath.org/) (free code signing for open source), setelah
pendaftaran proyek ini diterima. Rilis yang dibuat sebelumnya belum bertanda tangan sehingga
bisa diblokir Smart App Control / SmartScreen. Detail: [Code signing policy](#code-signing-policy).

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
- Tiap engine database + versi adalah instance terpisah (port & data dir sendiri):
  `mysql@8.4` (3384), `mariadb@11.4` (3424), `postgresql@16` (5416). Shim `mysql`/`psql`
  otomatis terhubung ke instance milik project.

## CLI

```
devlock init                       siapkan struktur folder + devlock.toml
devlock list php --available       lihat versi yang bisa diinstall (cache 6 jam; --refresh)
devlock install php 8.3            install runtime (php, node, composer, nginx, mysql, mariadb, postgresql)
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

## Project baru

```
devlock new --list                              # daftar template
devlock new laravel toko --php 8.4 --db mysql@8.4
devlock new wordpress blog --db mariadb@11.4
devlock new next dashboard --node 22
```

Template: Laravel, Symfony, CodeIgniter 4 (`composer create-project`), WordPress (zip resmi,
SHA1), Next.js, Nuxt, Vite + React, Vite + Vue (kerangka minimal + `npm install <paket>@latest`),
dan HTML statis. DevLock menulis `.devlock.toml`, membuat database, mengisi `.env` Laravel /
`wp-config.php` WordPress, dan mendaftarkan domain di file hosts. Di GUI: tombol **Project baru**
di tab Project.

## PHP

```
devlock php ext list                   # ekstensi PHP milik project ini
devlock php ext enable gmp bz2 --php 8.3
devlock php xdebug install             # unduh dari xdebug.org (SHA256) + aktifkan
devlock php xdebug off
devlock php ini set memory_limit 1G
```

Perubahan langsung diterapkan ke pool php-cgi yang sedang berjalan. Xdebug aktif dengan mode
`debug,develop`, mulai saat ada trigger `XDEBUG_SESSION`, dan terhubung ke `127.0.0.1:9003`.
Semua ini juga tersedia di tab **PHP** pada GUI.

## Database

```
devlock db create toko                 # di instance milik project (.devlock.toml)
devlock db list --instance mysql@8.4
devlock db dump toko                   # → data\backups\<instance>\toko-<waktu>.sql
devlock db restore toko_salinan backup.sql
devlock db drop toko --yes
```

Instance harus sedang berjalan (`devlock start db`). Login lokal: `root` (MySQL/MariaDB) atau
`postgres` (PostgreSQL) tanpa password, hanya dari `127.0.0.1`.

## HTTPS lokal

```
devlock ssl trust      # sekali: pasang CA DevLock ke Windows (ada dialog konfirmasi)
devlock ssl enable     # HTTPS untuk semua site, atau `https = true` di .devlock.toml
devlock reload
```

Sertifikat per site dibuat otomatis di `ssl\sites\`. Chrome/Edge memakai store Windows;
Firefox perlu mengimpor `ssl\devlock-ca.crt` sendiri. `curl.exe` bawaan Windows perlu
`--ssl-no-revoke` karena sertifikat lokal tidak punya daftar pencabutan (CRL).

PHP memakai `ssl\ca-bundle.pem` (bundle CA Mozilla + CA DevLock) untuk `curl.cainfo` dan
`openssl.cafile`, jadi request HTTPS dari PHP dan Composer langsung jalan. Perbarui dengan
`devlock ssl bundle`.

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
cargo build --release --workspace --features devlock-desktop/custom-protocol
```

Binary ada di `target\release\`: `devlock.exe`, `devlock-shim.exe`, dan `devlock-gui.exe`.

Mode development GUI (hot reload): `cd apps\desktop` lalu `npx tauri dev`. Untuk mengutak-atik tampilan saja, `npm run dev` lalu buka `http://localhost:1420` di browser: tanpa backend Tauri, UI memakai data contoh (`src/devMock.ts`).

## Kontribusi

Lihat [CONTRIBUTING.md](CONTRIBUTING.md) untuk alur branch, format commit, dan rilis.

## Perubahan pada sistem

DevLock bersifat portable: semua runtime, data, dan konfigurasi ada di folder instalasinya.
Di luar folder itu DevLock hanya mengubah hal berikut, dan **hanya saat Anda menjalankan
perintahnya**:

| Perubahan | Kapan | Membatalkan |
|---|---|---|
| Blok `# devlock-start … # devlock-end` di file hosts Windows (lewat UAC) | `devlock hosts`, `devlock start` | `devlock hosts --clear` |
| Folder `shims` di depan PATH user (registry `HKCU\Environment`) | `devlock path add` | `devlock path remove` |
| CA "DevLock Local CA" di store sertifikat Root milik user (Windows meminta konfirmasi) | `devlock ssl trust` | `devlock ssl untrust` |

## Uninstall

```
devlock stop
devlock hosts --clear
devlock ssl untrust
devlock path remove
```

Lalu hapus folder instalasi DevLock (berisi runtime, database di `data`, dan project di `www`;
cadangkan dulu yang masih diperlukan, mis. dengan `devlock db dump`).

## Privasi

DevLock tidak mengumpulkan atau mengirim data penggunaan (tanpa telemetri). Koneksi keluar
hanya dibuat saat Anda meminta daftar atau unduhan runtime, langsung ke sumber resminya:
`downloads.php.net`, `nodejs.org`, `getcomposer.org`, `nginx.org`, `downloads.mariadb.org`,
`cdn.mysql.com`, `www.postgresql.org`, dan `get.enterprisedb.com`. Daemon DevLock dan semua
service hanya mendengarkan di `127.0.0.1`.

## Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by
[SignPath Foundation](https://signpath.org/).

- Hanya binary DevLock sendiri (`devlock.exe`, `devlock-shim.exe`, `devlock-gui.exe`) yang
  ditandatangani, dan selalu dibangun otomatis oleh [GitHub Actions](.github/workflows/release.yml)
  dari source di repository ini. Runtime yang diunduh DevLock (PHP, Node, dll.) tetap memakai
  tanda tangan atau checksum dari penerbit aslinya.
- Setiap rilis yang ditandatangani disetujui manual oleh approver di bawah.
- Rilis yang dibuat sebelum pendaftaran SignPath diterima belum ditandatangani.

Peran tim:

| Peran | Anggota |
|---|---|
| Committers & reviewers | [@romyrudiansah](https://github.com/romyrudiansah) |
| Approvers | [@romyrudiansah](https://github.com/romyrudiansah) |

Kebijakan privasi: lihat bagian [Privasi](#privasi).

## Lisensi

DevLock dilisensikan di bawah salah satu dari, sesuai pilihan Anda:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

Kecuali dinyatakan lain, setiap kontribusi yang Anda kirim untuk dimasukkan ke DevLock
dilisensikan ganda seperti di atas, tanpa syarat atau ketentuan tambahan.
