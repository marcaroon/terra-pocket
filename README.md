# Terra — Desktop Music Preview & Controller

**Terra** adalah perangkat meja ESP32-S3 N16R8 dengan layar ST7789 240×240, didampingi aplikasi
Windows. Spesifikasi lengkap: [PROJECT_SPEC.md](PROJECT_SPEC.md).

## Status per fase (12 Oktober 2026)

| Fase | Isi | Status |
| --- | --- | --- |
| M0 | Monorepo, docs, build firmware + Rust | Lulus, CI GitHub hijau |
| M1 | ST7789 240×240 + LVGL | Diuji di hardware: tampil, warna normal, tidak terpotong |
| M2 | 3 tombol + encoder half-step, debounce, queue | Diuji di hardware; sisa uji ketahanan di [validasi](docs/validation.md) |
| N0 | Probe sektor USB HDD read-only | Build lulus; uji fisik menunggu USB-A + cabang daya |
| M3 | BLE GATT `Terra`: RX/TX/STATUS | Diuji dari laptop: discover, connect, service, write RX, subscribe TX |
| M4 | Protokol V1: fragmentasi, reassembly, HELLO, MEDIA/VOLUME_STATE | 57 vector bersama C/Rust/Python; uji BLE hardware lulus; konfirmasi visual layar menunggu |
| M5 | `terra-cli media` / `media --watch` membaca sesi media Windows (GSMTC) | Diuji di laptop: metadata, status, timeline, pause/track/seek |
| M6+ | BLE companion Windows, kontrol media, volume, reliability, GUI | Belum dimulai |

Wire format: [ble-protocol.md](docs/ble-protocol.md). Uji BLE dari PC memakai harness
dev `scripts/ble-test.py` sampai companion Windows (M6) tersedia. Tombol fisik masih
hanya dicatat di log; perintah media ke Windows datang di M7.

Layar dari renderer LVGL yang sama, dijalankan di komputer (bukan foto hardware):

![Layar M3/M4](docs/m4-screens.png)

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
cargo test --workspace --locked
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
- [Protokol BLE V1 (kanonik)](docs/ble-protocol.md)
- [State machine](docs/state-machine.md)
- [Build dan pengujian hardware](docs/development.md)
- [Laporan validasi](docs/validation.md)
- [Changelog](CHANGELOG.md)
- [Proposal NAS dan Vercel](docs/nas-proposal.md)
- [Uji input M2 dan diagnostik disk N0](docs/m2-n0-test.md)
- [Web console](web-console/README.md)

Status "diuji di hardware" hanya ditulis setelah ada keluaran perangkat nyata;
build firmware saja tidak membuktikan orientasi, warna, wiring, maupun radio.
