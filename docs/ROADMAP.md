# DevLock — Local Development Environment (Windows)

Alternatif Laragon / WAMP / XAMPP dengan fokus utama: **multi-versi runtime** (PHP, Node, MySQL, dll)
yang bisa berjalan **bersamaan** dan dipilih **per-project**.

---

## 1. Tujuan & Pembeda

| Laragon / WAMP | DevLock (target) |
|---|---|
| Satu versi PHP aktif global | Banyak versi PHP jalan bersamaan (FastCGI pool per versi), tiap project pilih sendiri |
| Ganti versi Node via PATH manual | Shim otomatis baca `.nvmrc` / `.node-version` / `.devlock.toml` |
| Download runtime manual | Katalog runtime + install 1 klik (checksum verified) |
| Hosts file per project | Wildcard `*.test` via DNS lokal + SSL otomatis |
| GUI saja | GUI (tray) + CLI `devlock` dengan fitur setara |

---

## 2. Rekomendasi Tech Stack

| Bagian | Pilihan | Alasan |
|---|---|---|
| Core / backend | **Rust** | Single binary kecil, akses Win32 API (Job Object, service, registry), cepat |
| Desktop UI | **Tauri 2** + React + TypeScript | Jauh lebih ringan dari Electron (~10MB), tray bawaan |
| CLI | Rust (`clap`), satu workspace dengan core | Logika dipakai bersama GUI & CLI |
| Shim | Rust binary kecil (`devlock-shim.exe`) | Disalin jadi `php.exe`, `node.exe`, dst |
| Elevated helper | Rust binary terpisah, dijalankan via UAC sekali | Edit hosts, trust CA, bind port 80/443 |

Crate penting: `tokio`, `reqwest`, `zip`, `sevenz-rust`, `sha2`, `rcgen` (SSL), `sysinfo`,
`windows` (Win32), `serde`/`toml`, `notify` (watch folder `www`), `hickory-dns` (DNS lokal).

Alternatif kalau tidak mau Rust: **Go + Wails** (struktur sama), atau **C# + WPF/.NET 8**.

---

## 3. Struktur Folder Instalasi

```
D:\DevLock\
├─ bin\              devlock.exe (CLI), devlock-gui.exe, devlock-helper.exe
├─ shims\            php.exe, node.exe, npm.cmd, npx.cmd, composer.bat, mysql.exe ...  ← masuk PATH
├─ runtimes\
│  ├─ php\8.3.12\  php\8.2.24\  php\7.4.33\
│  ├─ node\22.11.0\  node\20.18.0\
│  ├─ mysql\8.4.3\   mariadb\11.4.3\   postgresql\16.4\
│  ├─ nginx\1.27.2\  apache\2.4.62\
│  └─ redis\  mailpit\  composer\  ...
├─ etc\              config hasil generate (nginx sites, php.ini per versi, my.ini per versi)
│  └─ templates\     template mentah (vhost, php.ini, my.ini) — bisa di-override user
├─ data\             data dir database (dipisah per engine + versi major)
├─ www\              project user  →  www\blog  =  http(s)://blog.test
├─ ssl\              root CA + sertifikat per host
├─ logs\             log per service
├─ tmp\              download cache, session php, upload tmp
└─ devlock.toml      konfigurasi global
```

Prinsip: **portable** — semua relatif ke root, tidak menulis ke `Program Files`, registry seminimal mungkin.

---

## 4. Konsep Inti: Multi-Versi

### 4.1 Runtime Catalog
File manifest JSON (di-host di GitHub repo sendiri) berisi daftar versi + URL + SHA256:

```json
{
  "php": [
    { "version": "8.3.12", "variant": "nts-x64-vs16",
      "url": "https://windows.php.net/downloads/releases/php-8.3.12-nts-Win32-vs16-x64.zip",
      "sha256": "..." }
  ],
  "node": [
    { "version": "22.11.0", "url": "https://nodejs.org/dist/v22.11.0/node-v22.11.0-win-x64.zip", "sha256": "..." }
  ]
}
```

Sumber resmi: windows.php.net, nodejs.org/dist, nginx.org, apachelounge.com, mariadb.org,
dev.mysql.com, EDB (PostgreSQL binaries), github.com/redis-windows atau Memurai, Mailpit releases.

### 4.2 Shim (cara versi dipilih)
`shims\php.exe` adalah shim kecil. Saat dipanggil, ia:
1. Cari dari folder kerja ke atas: `.devlock.toml` → (php) `composer.json` `config.platform.php` / (node) `.nvmrc`, `.node-version`, `package.json#engines`/`volta`
2. Kalau tidak ketemu → versi default global di `devlock.toml`
3. Exec binary asli `runtimes\php\<ver>\php.exe` dengan argumen yang sama (exit code & stdio diteruskan)

```toml
# www/blog/.devlock.toml
php = "8.2"
node = "20"
webserver = "nginx"
database = "mysql@8.4"
```

Versi parsial (`"8.2"`) di-resolve ke patch terbaru yang terinstall.

### 4.3 PHP Multi-Versi Bersamaan (fitur unggulan)
- Tiap versi PHP yang dipakai project dijalankan sebagai **php-cgi pool** di port sendiri
  (port `9000 + major*100 + minor*10 + worker`: 8.3 → `9830-9833`, 7.4 → `9740-9743`)
- Vhost nginx/apache tiap project diarahkan ke port PHP versinya
- Pool dijalankan *lazy* (hanya versi yang dipakai) dengan beberapa worker + auto-restart
  (php-cgi di Windows mati setelah `PHP_FCGI_MAX_REQUESTS`, jadi wajib disupervisi)

### 4.4 Node Multi-Versi Bersamaan
- CLI: `node`, `npm`, `npx`, `pnpm`, `yarn` lewat shim → otomatis pakai versi project saat `cd` ke foldernya
- Dev server: tiap project Node dijalankan DevLock sebagai service dengan **versi Node miliknya sendiri**
  (mis. `admin.test` → Node 18 di :5173, `web.test` → Node 22 di :3000), semuanya jalan bersamaan
- Global package (`npm i -g`) dipisah per versi Node (`runtimes\node\<ver>\` sebagai prefix) agar tidak bentrok
- Cache npm/pnpm dibagi bersama antar versi untuk hemat disk

### 4.5 Database Multi-Versi Bersamaan
Setiap engine + versi = **instance terpisah** dengan port, data dir, config, dan log sendiri:

| Instance | Port | Data dir |
|---|---|---|
| `mysql@5.7` | 3357 | `data\mysql\5.7\` |
| `mysql@8.4` | 3384 | `data\mysql\8.4\` |
| `mariadb@11.4` | 3424 | `data\mariadb\11.4\` |
| `postgresql@14` | 5414 | `data\postgresql\14\` |
| `postgresql@16` | 5416 | `data\postgresql\16\` |
| `redis@7` | 6379 | `data\redis\7\` |

- Satu instance bisa dijadikan **default** dan juga mendengarkan port standar (3306 / 5432) — sisanya tetap di port versinya
- Project menunjuk instance lewat `.devlock.toml` → DevLock menyalakan instance itu otomatis saat project dibuka
  dan bisa mengisi `.env` (`DB_PORT`) sesuai instance
- Shim client (`mysql`, `mysqldump`, `psql`) otomatis konek ke port instance milik project
- Migrasi antar versi: `devlock db migrate blog --from mysql@5.7 --to mysql@8.4` (dump → restore)

### 4.6 Contoh: tiga project jalan bersamaan

```toml
# www/legacy-shop/.devlock.toml      # www/api/.devlock.toml         # www/dashboard/.devlock.toml
php = "7.4"                          php = "8.3"                      node = "22"
node = "16"                          database = "postgresql@16"       dev = "pnpm dev"
database = "mysql@5.7"               cache = "redis@7"                proxy_port = 3000
```

Hasilnya `legacy-shop.test` (PHP 7.4 + MySQL 5.7), `api.test` (PHP 8.3 + Postgres 16 + Redis),
dan `dashboard.test` (Node 22 → reverse proxy) aktif **bersamaan** tanpa ganti versi apa pun.

---

## 5. Breakdown Fitur per Modul

### A. Runtime Manager — **P0**
- [x] Ambil & cache katalog versi _(cache 6 jam di `tmp\catalog`, fallback ke cache lama; v0.6.0)_
- [x] Install / uninstall versi (download → verifikasi SHA256 → extract → post-install hook)
- [x] Post-install: generate `php.ini` dari template, aktifkan ekstensi umum, set `extension_dir`, CA bundle (`curl.cainfo`) _(CA bundle v0.6.0)_
- [x] Deteksi & peringatan dependency (Visual C++ Redistributable untuk PHP/Apache) _(lewat `devlock doctor`)_
- [x] Set versi default global
- [x] Progress download + resume + mirror fallback _(progress & resume v0.6.0; mirror v0.12.0)_

### B. Version Resolver & Shim — **P0**
- [x] Binary shim generik (`php`, `php-cgi`, `node`, `npm`, `npx`, `pnpm`, `yarn`, `composer`, `mysql`, `mysqldump`, `psql`, `redis-cli`) _(19 shim termasuk `pg_dump`/`createdb` dan `redis-cli`)_
- [x] Resolusi per-project (urutan file seperti di 4.2)
- [x] Tambah `shims\` ke PATH user (dengan opsi on/off) + deteksi konflik dengan PHP/Node lain di PATH
- [x] `devlock which php` / `devlock current` untuk debug

### C. Service Manager — **P0**
- [x] Start / stop / restart semua service atau satu-satu
- [x] Proses anak dimasukkan ke **Windows Job Object** → ikut mati kalau DevLock ditutup/crash
- [x] Cek port bentrok sebelum start (tampilkan proses yang memakai port) _(IPv4 & IPv6, v0.4.1; port 443 v0.6.0)_
- [x] Health check (TCP/HTTP) + auto-restart dengan backoff _(cek TCP & status `starting`, v0.5.0)_
- [x] Log stdout/stderr per service ke `logs\` + rotasi _(rotasi di atas 10 MB, v0.6.0)_
- [x] Autostart service saat app dibuka _(`[gui] autostart_services`, v0.6.0; belum bisa pilih per service)_

### D. Web Server & Virtual Host — **P0**
- [x] Nginx (default) dan Apache (opsional), bisa pilih per project _(Apache di belakang nginx, `webserver = "apache"`)_
- [x] Auto vhost: scan `www\` (watch perubahan folder) → `<folder>.test` _(watcher daemon, v0.6.0)_
- [x] Deteksi document root otomatis: `public\` (Laravel), `web\` (Symfony/Craft), `dist\`, root _(juga `public_html`)_
- [x] Template vhost per tipe: PHP generic, Laravel, WordPress, static, **reverse proxy** (Node/Vite/Next) _(satu template PHP melayani Laravel/WordPress/generic)_
- [x] Custom domain & alias tambahan per project
- [x] Regenerate config + reload graceful _(`nginx -s reload`, v0.6.0)_

### E. DNS & Hosts — **P0 (hosts) / P1 (DNS)**
- [x] P0: update `C:\Windows\System32\drivers\etc\hosts` via elevated helper (blok bertanda `# devlock-start/end`) _(elevasi UAC menjalankan `devlock hosts --elevated`; hanya IPv4 sejak v0.4.1)_
- [x] P1: DNS resolver lokal untuk wildcard `*.test` (tanpa edit hosts tiap project) _(`devlock dns enable`: service `dns` di 127.0.0.1:53 + aturan NRPT; butuh port 53 bebas dari ICS)_

### F. SSL Lokal — **P1**
- [x] Generate root CA sekali, trust ke Windows Cert Store (helper + UAC) — juga NSS Firefox (opsional) _(store user v0.5.0; store komputer untuk Firefox `--machine`, v0.10.0)_
- [x] Sertifikat per host / wildcard otomatis saat vhost dibuat _(per site + alias, v0.5.0)_
- [x] Toggle HTTPS per project, redirect HTTP→HTTPS opsional _(`https_redirect` / `[https] redirect`)_
- [x] Set `NODE_EXTRA_CA_CERTS` dan `curl.cainfo` agar runtime percaya CA lokal _(v0.5.0 / v0.6.0)_

### G. Database — **P0 (MySQL/MariaDB) / P1 (lainnya)**
- [x] MySQL & MariaDB multi-versi berjalan bersamaan, port per versi, data dir terpisah (lihat 4.5)
- [x] Inisialisasi data dir otomatis (`mysqld --initialize-insecure` / `mariadb-install-db`)
- [x] PostgreSQL (`initdb`) _(v0.4.0)_
- [x] Redis, MongoDB (P2) _(Redis & Garnet sebagai `cache` (v0.9.x); MongoDB + mongosh (v0.10.0))_
- [x] Quick action: buat database untuk project _(`devlock db create`, tombol "Buat DB", v0.5.0; user khusus belum)_
- [x] Backup / restore (dump) per database _(`devlock db dump/restore`, v0.5.0)_
- [x] Panel DB web bawaan (Adminer — 1 file PHP, ringan) di `adminer.test` _(`devlock install adminer`)_

### H. PHP Tools — **P1**
- [x] Toggle ekstensi per versi (checkbox UI → edit `php.ini`) _(v0.6.0)_
- [x] Toggle Xdebug 1 klik (install DLL yang cocok dengan versi/TS/VS) _(v0.6.0)_
- [x] Editor `php.ini` + preset (memory_limit, upload_max_filesize, max_execution_time) _(v0.6.0)_
- [x] Composer terkelola (global, dijalankan dengan PHP sesuai project)

### I. Node Tools — **P1**
- [x] Corepack (pnpm/yarn) per versi Node _(shim `corepack`/`pnpm`/`yarn` memakai Node milik project)_
- [x] Jalankan dev server project (`npm run dev`) sebagai service + reverse proxy ke `<project>.test`
- [x] Deteksi port dev server otomatis dari output _(v0.10.0)_

### J. Quick Create Project — **P1**
- [x] Template: Laravel, WordPress, Symfony, CodeIgniter, Next.js, Nuxt, Vite (React/Vue), static HTML _(v0.6.0)_
- [x] Wizard: nama → template → versi PHP/Node → buat DB → vhost + SSL langsung jadi _(GUI "Project baru", v0.6.0)_
- [x] Auto-isi `.env` (DB_HOST, DB_PORT, DB_DATABASE, APP_URL) _(Laravel; WordPress lewat `wp-config.php`)_

### K. Terminal Terintegrasi — **P1**
- [x] "Open Terminal" di folder project dengan env yang sudah benar (Windows Terminal / PowerShell / cmd) _(tombol di GUI; belum diuji manual)_
- [x] Profil Windows Terminal "DevLock" otomatis _(`devlock terminal install`, v0.10.0)_

### L. Tools Tambahan — **P2**
- [x] Mail catcher (Mailpit) — SMTP `127.0.0.1:1025`, UI `mail.test` _(v0.11.0)_
- [x] Share/tunnel project ke publik (cloudflared / ngrok) _(`devlock share`, Cloudflare Quick Tunnel, v0.11.0)_
- [x] Python / Go / Java runtime (pola shim yang sama) _(Go & Python v0.11.0; Java/Temurin v0.12.0)_
- [x] Profil/stack: simpan kombinasi service, ganti cepat _(`devlock profile`, v0.11.0)_
- [x] Export/import konfigurasi ke mesin lain _(`devlock config export/import`, v0.11.0)_

### M. GUI (Tauri) — **P0 dasar / P1 lengkap**
- [x] Tray icon: status service, Start All / Stop All, buka `www`, menu cepat per project _(status & menu per project (buka, folder, terminal), v0.7.0)_
- [x] Dashboard: daftar service (status, port, versi, tombol aksi)
- [x] Halaman Runtimes: daftar versi tersedia vs terinstall, install/uninstall, set default
- [x] Halaman Projects: daftar project, versi yang dipakai, buka di browser/editor/terminal _(editor v0.10.0)_
- [x] Log viewer realtime per service
- [x] Settings: lokasi `www`, TLD (`.test`), port, autostart Windows, bahasa (ID/EN) _(bahasa v0.12.0)_

### N. CLI — **P0**
```
devlock start [service]          devlock stop [service]        devlock status
devlock install php 8.3          devlock uninstall node 18     devlock list php [--available]
devlock use php 8.2 [--global]   devlock which node            devlock current
devlock site list                devlock site link <path> [name]   devlock site secure <name>
devlock db create <name> [--engine mysql@8.4]   devlock db dump <name>
devlock new laravel <name> --php 8.3            devlock logs nginx -f
devlock doctor                   (cek PATH, port, VC++ redist, CA trust)
```

### O. Sistem & Distribusi — **P1**
- [x] Installer (Tauri bundler → MSI/NSIS) + versi portable zip _(installer NSIS per user + zip portable, v0.11.0)_
- [x] Auto-update aplikasi & katalog _(`devlock update`, v0.11.0; katalog runtime di-cache 6 jam)_
- [x] `devlock doctor` untuk diagnosa
- [x] Import project dari Laragon/XAMPP (baca folder `www`/`htdocs`) _(`devlock import`, v0.11.0)_

---

## 6. Port Default

| Service | Port |
|---|---|
| HTTP / HTTPS | 80 / 443 |
| PHP-CGI pool | `9000 + major*100 + minor*10 + worker` (8.3 → 9830-9833) |
| MySQL | `3300 + major*10 + minor` (8.4 → 3384, 5.7 → 3357) |
| MariaDB | `3400 + (major-10)*20 + minor` (11.4 → 3424, 10.11 → 3411) |
| Override | `[ports]` di `devlock.toml`, mis. `"mysql@8.4" = 3306` |
| PostgreSQL | `5400 + major` (16 → 5416, 18 → 5418) |
| Redis | `6309 + major*10` (7 → 6379, 8 → 6389) |
| MongoDB | `27000 + major*10 + minor` (8.0 → 27080) |
| Dev server Node (port otomatis) | `PORT` 3100–3899 per site, port sebenarnya dibaca dari output |
| Garnet | `6400 + major` (2 → 6402) |
| Apache (di belakang nginx) | 8180 |
| Mailpit SMTP / UI | 1025 / 8025 (`[mail]`) |
| DNS lokal | 127.0.0.1:53 |
| DevLock API internal (GUI↔core) | 127.0.0.1:7433 atau named pipe |

---

## 7. Arsitektur

```
 ┌──────────────┐   ┌──────────────┐
 │  Tauri GUI   │   │  devlock CLI │
 └──────┬───────┘   └──────┬───────┘
        │   named pipe / IPC│
        ▼                   ▼
 ┌─────────────────────────────────────┐
 │           devlock-core (Rust)        │
 │  catalog · installer · resolver      │
 │  config gen (template) · supervisor  │
 │  site scanner · ssl · db ops         │
 └──────┬───────────────────┬──────────┘
        │ spawn (Job Object) │ UAC (sekali)
        ▼                    ▼
  nginx/apache, php-cgi    devlock … --elevated
  mysql, pg, redis, node   (hosts, NRPT DNS)
```

Workspace Rust yang disarankan:
```
crates/
  core/      logika utama (library)
  cli/       binary devlock
  shim/      binary shim (sekecil mungkin, tanpa tokio)
apps/
  desktop/   Tauri + React
catalog/     manifest JSON runtime
templates/   nginx, apache, php.ini, my.ini
```

---

## 8. Roadmap Bertahap

Status per item ada di checklist bagian 5 (`[x]` = selesai; catatan _sebagian_ = baru sebagian). Riwayat rilis: [CHANGELOG.md](../CHANGELOG.md).

| Fase | Isi | Hasil | Status |
|---|---|---|---|
| **0. Fondasi** | Workspace Rust, config `devlock.toml`, struktur folder, logging | Kerangka jalan | ✅ v0.1.0 |
| **1. MVP CLI** | Runtime Manager (PHP, Node, Nginx, MariaDB), shim + resolver, service manager, auto vhost + hosts | Bisa dev Laravel & Node via CLI dengan multi-versi | ✅ v0.2.0 |
| **2. GUI dasar** | Tray + dashboard service + halaman runtimes & projects | Pengalaman setara Laragon | ✅ v0.3.0–v0.3.1 |
| **3. Kenyamanan** | SSL, DNS wildcard, Apache, MySQL/Postgres/Redis, PHP ext/Xdebug toggle, quick create, terminal | Melebihi Laragon | ✅ PostgreSQL (v0.4.0), DB actions + SSL (v0.5.0), PHP tools + quick create (v0.6.0), tray per project + DNS wildcard (v0.7.0), Apache (v0.8.x), Redis/Garnet + Adminer (v0.9.x), redirect HTTPS + Windows Terminal + editor + port dev otomatis + Firefox + MongoDB (v0.10.0) |
| **4. Ekstra** | Mailpit, tunnel, Python/Go, profil stack, auto-update, installer | Siap dirilis | ✅ v0.11.0; opsional (Java, mirror, GUI EN) v0.12.0 |

---

## 9. Risiko & Catatan Teknis

- **Admin rights**: edit hosts, trust CA, dan bind port 80/443 butuh elevasi → pisahkan ke helper, minta UAC sekali per aksi, jangan jalankan seluruh app sebagai admin.
- **Port 80 bentrok**: IIS, Skype lama, `System` (http.sys), Laragon/XAMPP yang masih jalan → `doctor` harus mendeteksi & menyarankan.
- **php-cgi di Windows** tidak punya FPM → wajib supervisor + beberapa proses per versi.
- **Varian PHP**: pakai NTS untuk FastCGI; TS hanya untuk Apache mod_php (hindari, pakai fcgid/proxy_fcgi).
- **Antivirus/Defender** bisa memperlambat extract & menghapus shim → tanda tangani binary (code signing) saat rilis.
- **Smart App Control / SmartScreen**: rilis saat ini belum ditandatangani (pendaftaran SignPath Foundation ditolak), sehingga Smart App Control kadang memblokir exe rilis tertentu (mis. GUI v0.7.0, shim v0.9.2) secara acak per build. Solusi permanen: code signing; sementara itu pakai build sebelumnya yang diizinkan.
- **PATH**: shim harus di depan PATH user; beri peringatan jika ada PHP/Node lain sebelum shim.
- **Data DB antar versi major** tidak kompatibel → data dir dipisah per versi major + fitur dump/restore saat migrasi.
- **Lisensi**: redistribusi binary MySQL (GPL) / Redis — lebih aman download dari sumber resmi saat install, bukan dibundel.
