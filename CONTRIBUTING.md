# Alur Pengembangan

## Branch

`main` selalu dalam keadaan bisa di-build. Semua perubahan lewat branch + Pull Request.

| Prefix | Untuk |
|---|---|
| `feat/<nama>` | Fitur baru |
| `fix/<nama>` | Perbaikan bug |
| `refactor/<nama>` | Perubahan struktur tanpa ubah perilaku |
| `docs/<nama>` | Dokumentasi |
| `chore/<nama>` | Tooling, CI, dependency |
| `release/vX.Y.Z` | Persiapan rilis (bump versi + changelog) |

Contoh: `feat/runtime-manager`, `fix/php-cgi-restart`.

## Commit

Format [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <ringkasan>

feat(shim): resolve versi dari .nvmrc
fix(service): php-cgi tidak restart setelah crash
docs: tambah contoh .devlock.toml
```

Type: `feat`, `fix`, `refactor`, `perf`, `docs`, `test`, `chore`, `ci`, `build`.
Tambahkan `!` atau footer `BREAKING CHANGE:` untuk perubahan yang tidak kompatibel.

## Versi & Rilis

[Semantic Versioning](https://semver.org/): `MAJOR.MINOR.PATCH`. Selama `0.x`, MINOR naik
untuk fitur/fase baru dan PATCH untuk perbaikan.

1. Merge PR fitur ke `main`.
2. Update `version` di `Cargo.toml` (workspace) dan `CHANGELOG.md`.
3. Tag: `git tag -a v0.2.0 -m "v0.2.0"` lalu `git push origin v0.2.0`.
4. Workflow `release` membangun binary Windows dan membuat GitHub Release otomatis.

## Sebelum PR

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI menjalankan perintah yang sama di `windows-latest`.
