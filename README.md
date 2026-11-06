<p align="center"><img src="site/assets/logo.svg" width="96" alt=""></p>

# DevLock

Local development environment untuk Windows: alternatif Laragon / WAMP / XAMPP dengan
**multi-versi runtime** (PHP, Node, MySQL/MariaDB, PostgreSQL, MongoDB, Go, Python, Java) yang
bisa berjalan **bersamaan** dan dipilih **per project**.

**Website:** https://romyrudiansah.github.io/devlock/ ([English](https://romyrudiansah.github.io/devlock/en/)) ·
**Panduan:** https://romyrudiansah.github.io/devlock/panduan.html ([English guide](https://romyrudiansah.github.io/devlock/en/guide.html))

Repository ini berisi rilis, website, dan tempat diskusi DevLock. Kode sumbernya dikembangkan
di repository terpisah.

## Unduh

Dari halaman [**Releases**](https://github.com/romyrudiansah/devlock/releases) (Windows 10/11 x64),
cocokkan checksum dengan file `.sha256` di rilis yang sama, lalu pilih salah satu:

- **Installer** `devlock-vX.Y.Z-windows-x64-setup.exe`: dipasang per user di
  `%LOCALAPPDATA%\DevLock` (tanpa admin), dengan shortcut Start Menu dan entri di *Apps &
  features*. Setelahnya jalankan `devlock path add` bila ingin shim di PATH.
- **Portable** `devlock-vX.Y.Z-windows-x64.zip`: ekstrak ke mis. `D:\DevLock`, jalankan
  `D:\DevLock\bin\devlock.exe init`, lalu `devlock path add`.

Versi yang sudah terpasang diperbarui dengan `devlock update` (atau dari GUI).

Perubahan tiap versi: [CHANGELOG.md](CHANGELOG.md).

## Laporkan masalah & diskusi

- Bug atau permintaan fitur: [Issues](https://github.com/romyrudiansah/devlock/issues/new/choose)
- Pertanyaan, ide, dan cerita pemakaian: [Discussions](https://github.com/romyrudiansah/devlock/discussions)

Sertakan versi DevLock (`devlock --version`), versi Windows, dan isi log terkait
(`devlock logs <service>` atau folder `logs\`).

## Code signing

Binary rilis DevLock **belum ditandatangani**. Pastikan file berasal dari halaman Releases di
atas dan checksum-nya cocok. Windows bisa menampilkan peringatan:

- **SmartScreen** ("Windows protected your PC"): pilih *More info* → *Run anyway*, atau
  sebelum mengekstrak buka *Properties* zip → centang *Unblock*.
- **Smart App Control** bisa memblokir exe baru yang belum bertanda tangan sampai Windows
  mempercayainya. `devlock update` memulihkan versi lama bila binary baru diblokir.

### Code signing policy

- Binary DevLock (`devlock.exe`, `devlock-shim.exe`, `devlock-gui.exe`) selalu dibangun
  otomatis oleh GitHub Actions dari kode sumber DevLock, dengan checksum SHA256 di setiap
  rilis. Runtime yang diunduh DevLock (PHP, Node, dll.) tetap memakai tanda tangan atau
  checksum dari penerbit aslinya.
- Bila tanda tangan aktif, hanya binary DevLock sendiri yang ditandatangani, dan setiap rilis
  disetujui manual oleh [@romyrudiansah](https://github.com/romyrudiansah). Catatan rilis
  menyebut apakah rilis itu bertanda tangan.

## Privasi

DevLock tidak mengumpulkan atau mengirim data penggunaan (tanpa telemetri). Koneksi keluar
hanya dibuat saat Anda meminta daftar atau unduhan runtime, update DevLock, atau
`devlock share`, langsung ke sumber resminya (php.net, nodejs.org, nginx.org, mariadb.org,
mysql.com, postgresql.org, mongodb.org, go.dev, Adoptium, dan GitHub Releases). Daemon DevLock
dan semua service hanya mendengarkan di `127.0.0.1`.

Website DevLock menghitung kunjungan dengan [GoatCounter](https://www.goatcounter.com/) tanpa
cookie dan tanpa data pribadi.

## Lisensi

Mulai v0.18.0 DevLock adalah **freeware**: gratis dipakai untuk keperluan pribadi maupun
komersial, hanya boleh diunduh dari halaman Releases resmi, dan tidak boleh dijual, diubah,
atau disebarkan ulang. Ketentuan lengkap: [LICENSE.txt](LICENSE.txt).

Rilis hingga v0.17.2 tetap berada di bawah MIT OR Apache-2.0 yang menyertai rilis tersebut.
