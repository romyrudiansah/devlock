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

### Tanda tangan kode (SignPath, opsional)

Workflow `release` menandatangani `devlock.exe`, `devlock-shim.exe`, dan `devlock-gui.exe`
lewat [SignPath](https://about.signpath.io/) bila secret `SIGNPATH_API_TOKEN` diisi; tanpa
secret itu rilis tetap jalan tanpa tanda tangan (kondisi saat ini). Pendaftaran SignPath
Foundation pertama belum disetujui (proyek dinilai belum cukup dikenal publik) dan bisa
diajukan ulang nanti; langganan SignPath berbayar memakai langkah yang sama. Setelah akses
SignPath tersedia:

1. Di SignPath: buat project `devlock` yang terhubung ke repository ini, signing policy
   `release-signing` (wajib persetujuan manual), dan artifact configuration dari
   [`.signpath/artifact-configuration.xml`](.signpath/artifact-configuration.xml). Aktifkan
   pembatasan metadata file: ProductName `DevLock`, versi sama dengan tag rilis.
2. Di GitHub (Settings > Secrets and variables > Actions):
   - secret `SIGNPATH_API_TOKEN` (token API user CI di SignPath)
   - variable `SIGNPATH_ORGANIZATION_ID`
   - opsional bila slug berbeda: `SIGNPATH_PROJECT_SLUG`, `SIGNPATH_SIGNING_POLICY_SLUG`,
     `SIGNPATH_ARTIFACT_CONFIGURATION_SLUG`
3. Setiap tag `v*` membuat permintaan tanda tangan; approver menyetujuinya di SignPath dalam
   1 jam, lalu workflow memverifikasi tanda tangan setiap exe sebelum membuat zip.

## Sebelum PR

```
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI menjalankan perintah yang sama di `windows-latest`.

## Peran & keamanan akun

Disiapkan mengikuti [syarat SignPath Foundation](https://signpath.org/terms) agar siap saat
pendaftaran diajukan ulang:

- **Committers**: boleh mengubah source tanpa review tambahan.
- **Reviewers**: meninjau setiap perubahan dari non-committer (pull request).
- **Approvers**: menyetujui setiap permintaan tanda tangan rilis di SignPath.

Daftar anggota ada di README, bagian "Code signing policy". Semua anggota tim **wajib memakai
autentikasi dua faktor (2FA)** di GitHub dan SignPath.

Dengan mengirim kontribusi, Anda setuju kontribusi itu dilisensikan ganda MIT OR Apache-2.0
seperti proyek ini.
