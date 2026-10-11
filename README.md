# Terra — Desktop Music Preview & Controller

**Terra** adalah perangkat meja ESP32-S3 N16R8 dengan layar ST7789 240×240, didampingi aplikasi
Windows. Spesifikasi lengkap: [PROJECT_SPEC.md](PROJECT_SPEC.md).

## Tahap saat ini: M2 input + N0 diagnostik USB

Revisi 11 Oktober 2026: LCD 240×240; tiga tombol dan encoder menghasilkan event
serial dengan debounce/antrean. Profil USB opsional membaca identitas, kapasitas,
dan metadata sektor disk tanpa mount atau tulis. Hardware yang dibahas: board YD
N16R8, ORICO 2020U3, WD5000LPCX 500 GB NTFS; belum ada uji fisik atau flash.
[Panduan M2/N0](docs/m2-n0-test.md) memuat build terpisah, wiring, dan log harapan.
[Proposal NAS](docs/nas-proposal.md) memuat tahap file server dan Cloudflare Free.
NTFS file access, server LAN, relay dan kontrol web perangkat belum tersedia.

**Build firmware dan Rust lulus. Render host diperiksa. Uji hardware masih menunggu.**

Implementasi awal sesuai bagian 38 blueprint:

- Proyek ESP-IDF C dengan konfigurasi flash 16 MB / Octal PSRAM 8 MB.
- ST7789 melalui `esp_lcd`, LVGL 9.5.0, `esp_lvgl_port` 2.9.0.
- Layar statis: **Test Song**, **Test Artist**, **01:00 / 03:00**, **VOL 72**.
- State contoh ditandai **DEMO** dan **OFF** untuk status Bluetooth.
- Workspace Rust enam crate; CLI hanya `--help` dan `--version`.

Bluetooth, pembacaan media Windows, dan Tauri belum diimplementasikan.
Input M2 hanya mencatat perintah; layar DEMO tidak mengklaim state Windows berubah.
Crate yang disediakan untuk tahap berikutnya adalah struktur kosong yang diberi
keterangan, bukan fitur yang dinyatakan selesai. Tidak ada Arduino atau API Spotify.

Preview dari renderer LVGL yang sama, dijalankan di komputer (bukan foto hardware):

![Preview UI M1](docs/m1-preview.png)

## Build

Dengan toolchain lokal checkout ini, dari root:

```powershell
.\scripts\test-host.ps1
.\scripts\build-firmware.ps1 -Profile normal
.\scripts\build-firmware.ps1 -Profile usb-probe
```

Profil USB memerlukan console **COM/UART**, suplai HDD terverifikasi, dan data
pada port **USB native**. Lihat panduan M2/N0 sebelum flash. Build tidak melakukan flash.

Aktifkan ESP-IDF **6.0.3** lalu:

```powershell
cd firmware
idf.py set-target esp32s3
idf.py build
```

Dengan Rust stable dan linker Windows yang sesuai:

```powershell
cd companion
cargo build --workspace --locked
cargo run -p companion-cli -- --version
```

Untuk toolchain lokal yang disiapkan dalam sesi ini, dari root proyek:

```powershell
. .\scripts\activate-local.ps1
```

Lihat [panduan development](docs/development.md) untuk setup pada komputer lain,
pengujian, dan flash. Jangan flash sebelum memeriksa [wiring](docs/wiring.md).

## Dokumen

- [Arsitektur dan batas milestone](docs/architecture.md)
- [Wiring dan asumsi panel](docs/hardware.md)
- [Detail wiring perakitan & daftar kabel](docs/wiring.md)
- [Protokol BLE yang direncanakan](docs/ble-protocol.md)
- [State machine](docs/state-machine.md)
- [Build dan pengujian hardware](docs/development.md)
- [Laporan validasi M0/M1](docs/validation.md)
- [Changelog](CHANGELOG.md)
- [Proposal NAS dan Vercel](docs/nas-proposal.md)
- [Uji input M2 dan diagnostik disk N0](docs/m2-n0-test.md)
- [Web console](web-console/README.md)

M1/M2/N0 baru diterima secara hardware setelah layar, log, input, dan daya/USB
disk diuji. Build firmware saja tidak membuktikan orientasi, warna, maupun wiring.
