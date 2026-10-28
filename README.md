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

**Code signing:** binary rilis DevLock **belum ditandatangani**. Pastikan zip berasal dari
halaman Releases di atas dan checksum-nya cocok. Windows bisa menampilkan peringatan:

- **SmartScreen** ("Windows protected your PC"): pilih *More info* → *Run anyway*, atau
  sebelum mengekstrak buka *Properties* zip → centang *Unblock*.
- **Smart App Control** bisa memblokir exe yang belum bertanda tangan sepenuhnya, tanpa
  tombol pengecualian. Bila terjadi, coba rilis lain atau build dari source
  ([Build dari source](#build-dari-source)).

Detail: [Code signing policy](#code-signing-policy).

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
- Project Node cukup diberi `dev = "pnpm dev"`: DevLock menjalankan dev server itu sebagai
  service, membaca port dari output-nya (Vite, Next.js, Nuxt, Astro, Angular, …), dan
  mengarahkan `<project>.test` ke port tersebut. `proxy_port = 5173` mengunci port secara manual.

## Apache (per project)

```
devlock install apache          # build Win64 dari Apache Lounge, diverifikasi SHA256
```

```toml
# www/legacy/.devlock.toml
webserver = "apache"
```

nginx tetap menerima semua request (port 80/443, HTTPS) dan meneruskan site
`webserver = "apache"` ke Apache di `127.0.0.1:8180` (`[apache] port`). Apache memproses
`.htaccess` (`AllowOverride All`, mod_rewrite, headers, expires, auth basic) dan menjalankan
PHP lewat pool php-cgi yang sama (mod_proxy_fcgi), jadi versi PHP per project tetap berlaku.
`webserver = "apache"` di `devlock.toml` menjadikan Apache default untuk semua site. Bila
Apache belum terinstall, site tetap dilayani nginx dengan peringatan. Build Apache Lounge
membutuhkan Visual C++ Redistributable terbaru.

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

## Pindah dari Laragon / XAMPP

```
devlock import --dry-run      # lihat dulu: C:\laragon\www dan C:\xampp\htdocs (juga D:)
devlock import                # tiap subfolder → www\<nama> sebagai junction
devlock import D:\proyek --copy
```

Junction membuat project tetap di folder asal (tanpa hak admin), jadi Laragon/XAMPP dan
DevLock melihat file yang sama; `--copy` menyalinnya. Nama site dibuat aman untuk domain
(`My_Shop` → `my-shop.test`). Matikan Laragon/XAMPP sebelum `devlock start` agar port 80 dan
3306 tidak bentrok.

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

## Go & Python

```
devlock install go 1.27        # zip resmi go.dev (SHA256 dari go.dev/dl)
devlock install python 3.13    # build portable python-build-standalone (SHA256 dari GitHub)
devlock use python 3.13        # atau `python = "3.13"` / `go = "1.27"` di .devlock.toml
```

Shim `go`, `gofmt`, `python`, dan `pip` (`python -m pip`) memakai versi project, sama seperti
PHP/Node. Selain `.devlock.toml`, versi dibaca dari `go.mod` (`toolchain`, lalu `go`) dan
`.python-version`.

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

### MongoDB

```
devlock install mongodb 8.0     # MongoDB Community resmi (SHA256 dari downloads.mongodb.org)
devlock install mongosh         # shell MongoDB, juga dipakai untuk shutdown yang rapi
devlock use database mongodb@8.0
mongosh                         # shim: langsung ke mongodb://127.0.0.1:27080
```

Instance per major.minor dengan port `27000 + major*10 + minor` (8.0 → 27080) dan data di
`data\mongodb\<versi>`, hanya di `127.0.0.1` tanpa autentikasi. `devlock db`, Adminer, dan
isian `.env` otomatis hanya untuk database SQL.

## Adminer (panel database)

```
devlock install adminer          # adminer.php resmi (vrana/adminer), SHA256 dari GitHub
```

Setelah terinstall, `http://adminer.test` langsung tersedia (memakai PHP default) untuk
MySQL, MariaDB, dan PostgreSQL. Field server menawarkan instance DevLock (mis.
`127.0.0.1:3384`), dan login tanpa password diizinkan karena user lokal DevLock (`root`,
`postgres`) memang tanpa password dan site ini hanya terjangkau dari `127.0.0.1`. Folder
`www\adminer` milik Anda sendiri menggantikan site bawaan ini.

## Mailpit (email lokal)

```
devlock install mailpit      # rilis resmi axllent/mailpit (SHA256 dari GitHub)
devlock start mail
```

Semua email dari project ditangkap [Mailpit](https://mailpit.axllent.org/) dan bisa dibaca di
`http://mail.test`. SMTP ada di `127.0.0.1:1025` (UI di `8025`; ubah lewat `[mail]` di
`devlock.toml`). Saat install, `mail()` PHP di semua versi diarahkan ke Mailpit (`SMTP` /
`smtp_port` di `php.ini`), dan project Laravel baru mendapat `MAIL_MAILER=smtp`,
`MAIL_HOST=127.0.0.1`, `MAIL_PORT=1025`. Pesan disimpan di `data\mailpit`.

## Bagikan site ke publik

```
devlock install cloudflared   # rilis resmi cloudflare/cloudflared (SHA256 dari GitHub)
devlock share blog            # → https://<acak>.trycloudflare.com, tutup dengan Ctrl+C
```

Memakai [Cloudflare Quick Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/do-more-with-tunnels/trycloudflare/)
tanpa akun: request dari URL publik diteruskan ke nginx DevLock dengan header `Host` site
itu. Siapa pun yang tahu URL-nya bisa membuka site selama tunnel aktif. Aplikasi yang
membuat URL absolut dari konfigurasi (mis. `APP_URL` Laravel) tetap menulis `blog.test`.

## Redis / Garnet (cache)

```
devlock install redis 8          # build Windows komunitas (redis-windows), SHA256 dari GitHub
devlock use cache redis@8        # per project (.devlock.toml), atau --global
redis-cli ping                   # shim: otomatis ke instance milik project
```

Redis resmi tidak merilis binary Windows, jadi DevLock memakai build komunitas
[redis-windows](https://github.com/redis-windows/redis-windows) (Redis asli, varian msys2).
Alternatifnya [Garnet](https://github.com/microsoft/garnet) dari Microsoft
(`devlock install garnet`, `cache = "garnet@2"`): server cache berprotokol Redis, berjalan di
memori saja. Garnet butuh runtime .NET; `devlock install garnet` ikut memasang runtime .NET
resmi Microsoft (portable, diverifikasi SHA512) di folder Garnet, tanpa install .NET ke sistem.

Tiap instance punya port sendiri: `redis@7` → 6379, `redis@8` → 6389, `garnet@2` → 6402 (ubah
lewat `[ports]`). Data Redis tersimpan di `data\redis\<major>` (snapshot RDB, juga saat stop).
Isi `REDIS_PORT` di `.env` project sesuai port instance; `devlock current` menampilkannya.

## HTTPS lokal

```
devlock ssl trust      # sekali: pasang CA DevLock ke Windows (ada dialog konfirmasi)
devlock ssl enable     # HTTPS untuk semua site, atau `https = true` di .devlock.toml
devlock ssl redirect on   # opsional: HTTP dialihkan (301) ke HTTPS, atau `https_redirect = true`
devlock reload
```

Sertifikat per site dibuat otomatis di `ssl\sites\`. Chrome/Edge memakai store Windows milik
user. Firefox hanya mengimpor root dari store **komputer**, jadi pengguna Firefox menjalankan
`devlock ssl trust --machine` (UAC sekali; cabut dengan `devlock ssl untrust --machine`).
`curl.exe` bawaan Windows perlu
`--ssl-no-revoke` karena sertifikat lokal tidak punya daftar pencabutan (CRL).

PHP memakai `ssl\ca-bundle.pem` (bundle CA Mozilla + CA DevLock) untuk `curl.cainfo` dan
`openssl.cafile`, jadi request HTTPS dari PHP dan Composer langsung jalan. Perbarui dengan
`devlock ssl bundle`.

## DNS wildcard `*.test`

```
devlock dns enable     # sekali (UAC): aturan DNS Windows .test → DNS lokal DevLock
devlock dns status
```

DevLock menjalankan service `dns` kecil di `127.0.0.1:53` yang menjawab semua `*.test`
(termasuk subdomain seperti `api.shop.test`) dengan `127.0.0.1`. Windows hanya mengarahkan
query `.test` ke sana lewat aturan NRPT; DNS lain tidak tersentuh. Setelah aktif, project baru
tidak perlu menulis file hosts (alias di luar `.test` tetap lewat hosts). Matikan dengan
`devlock dns disable`.

Port 53 harus bebas. Layanan Windows **Internet Connection Sharing** (SharedAccess, dipakai
Mobile Hotspot dan switch NAT Hyper-V) mengambil semua query ke `127.0.0.1:53`; `devlock dns
enable` dan `devlock dns status` mendeteksinya. Hentikan layanan itu atau tetap pakai file hosts.

## GUI

`devlock-gui.exe` (satu folder dengan `devlock.exe`) membuka dashboard dan ikon tray:

- **Service**: start/stop/restart semua atau per grup (`php@8.3`, `mysql@8.4`, `dev:<site>`), status, dan log
- **Project**: daftar site di `www\`, buka di browser, folder, editor (`[gui] editor`, atau otomatis VS Code/Cursor/Windsurf/Zed/Sublime/PhpStorm), atau terminal (shim sudah di PATH)
- **Runtime**: install versi baru dengan progress, hapus, pilih default

Menutup jendela menyembunyikannya ke tray; service tetap diawasi daemon.

## Windows Terminal

```
devlock terminal install   # profil "DevLock": PowerShell di folder www, shim di depan PATH
devlock terminal remove
```

Profil dipasang sebagai [JSON fragment](https://learn.microsoft.com/windows/terminal/json-fragment-extensions)
di `%LOCALAPPDATA%\Microsoft\Windows Terminal\Fragments\DevLock`, jadi `settings.json` Windows
Terminal tidak diubah. Setelah dipasang, profil diperbarui otomatis saat GUI dibuka atau
`devlock shims` dijalankan.

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
| CA "DevLock Local CA" di store sertifikat Root komputer (lewat UAC, untuk Firefox) | `devlock ssl trust --machine` | `devlock ssl untrust --machine` |
| Aturan DNS (NRPT) `.test` → `127.0.0.1` berkomentar "DevLock" (lewat UAC) | `devlock dns enable` | `devlock dns disable` |
| Profil Windows Terminal "DevLock" (fragment di `%LOCALAPPDATA%\Microsoft\Windows Terminal\Fragments\DevLock`) | `devlock terminal install` | `devlock terminal remove` |

## Uninstall

```
devlock stop
devlock dns disable    # bila DNS wildcard pernah diaktifkan
devlock hosts --clear
devlock ssl untrust
devlock ssl untrust --machine   # bila pernah `devlock ssl trust --machine`
devlock path remove
devlock terminal remove
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

**Status: rilis belum ditandatangani.** Pendaftaran ke
[SignPath Foundation](https://signpath.org/) (code signing gratis untuk open source) belum
disetujui karena proyek ini dinilai belum cukup dikenal publik; pendaftaran akan diajukan lagi
setelah DevLock lebih banyak dipakai. Workflow rilis sudah siap menandatangani lewat
[SignPath.io](https://about.signpath.io/) begitu sertifikat tersedia.

Kebijakan yang berlaku sekarang dan saat tanda tangan diaktifkan:

- Binary DevLock sendiri (`devlock.exe`, `devlock-shim.exe`, `devlock-gui.exe`) selalu
  dibangun otomatis oleh [GitHub Actions](.github/workflows/release.yml) dari source di
  repository ini, dengan checksum SHA256 di setiap rilis. Runtime yang diunduh DevLock (PHP,
  Node, dll.) tetap memakai tanda tangan atau checksum dari penerbit aslinya.
- Bila tanda tangan aktif, hanya binary DevLock sendiri yang ditandatangani, dan setiap rilis
  disetujui manual oleh approver di bawah. Catatan rilis menyebut apakah rilis itu bertanda
  tangan.

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
