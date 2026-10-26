# DevLock

Local development environment untuk Windows — alternatif Laragon / WAMP / XAMPP dengan
**multi-versi runtime** (PHP, Node, MySQL/MariaDB, PostgreSQL, Redis) yang bisa berjalan
**bersamaan** dan dipilih **per-project**.

> Status: Fase 1 (CLI). GUI menyusul di Fase 2. Lihat [docs/ROADMAP.md](docs/ROADMAP.md).

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

## Build dari source

Butuh Rust stable (toolchain MSVC).

```
cargo build --release
```

Binary ada di `target\release\`: `devlock.exe` dan `devlock-shim.exe`.

## Kontribusi

Lihat [CONTRIBUTING.md](CONTRIBUTING.md) untuk alur branch, format commit, dan rilis.
