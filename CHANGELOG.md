# Changelog

## Unreleased — 2026-10-11

- Uji hardware pertama: perbaikan stack overflow task main saat render LVGL
  (stack 8 KB), decoder encoder half-step (`BOARD_ENCODER_HALF_STEP`), pemindai
  wiring opsional `BOARD_DIAG_PIN_SCAN`. LCD dan semua input terverifikasi awal.
- Proyek berganti nama menjadi **Terra**: firmware `terra.bin`, CLI `terra-cli`,
  nama BLE `Terra`, menu/Kconfig `TERRA_USB_PROBE`, web console Terra Home Hub
  (kunci draf browser baru `terra.draft.v1`; draf lama tidak dibaca).
- LCD pengguna dikonfirmasi 7 pin tanpa CS: `BOARD_LCD_CS` = `GPIO_NUM_NC`, SPI mode 3,
  GPIO10 kosong; dokumen wiring, diagram, dan spec disesuaikan.
- `docs/wiring.md` dan `docs/wiring-diagram.svg`: daftar kabel W1–W25, mode daya
  A/B/C, pinout header, varian LCD tanpa CS, uji backlight, urutan perakitan.

- M2: input GPIO active-low, debounce 40 ms, decoder encoder full-step, queue 32
  event; log Previous/Play/Next/Volume ±2/Mute tanpa memalsukan state Windows.
- N0 opsional: USB MSC read-only sector probe, pemeriksaan ukuran sektor/rentang
  MBR, pengenalan signature NTFS/protective GPT; tanpa mount, format atau tulis.
- Profil build USB terpisah menggunakan UART0, dengan pemeriksaan konflik console.
- usb_host_msc 1.3.0 dan usb 1.6.0 ditambahkan melalui Component Manager/lockfile.
- Tes host input/disk, script build dua profil, CI matrix, dan panduan uji hardware.
- Identifikasi foto board YD V1.3, ORICO 2020U3, WD5000LPCX dan rencana suplai
  bercabang; kandidat Cloudflare Free ditambahkan ke rencana relay.

- LCD ST7789 diganti ke 240×240; footer/progress dirapatkan tanpa mengecilkan font.
- Gap awal menjadi (0,0), tetap membutuhkan kalibrasi modul fisik.
- Host preview memakai dimensi board yang sama serta memeriksa batas widget,
  teks panjang, dan durasi nol.
- Proposal NAS ESP32-only untuk HDD USB 500 GB NTFS, daya, dan relay/Vercel.
- Web console prototipe: navigasi file contoh, preview/download contoh, draf
  pengaturan lokal dan ekspor JSON. Tanpa akses HDD atau backend NAS.

## 0.1.0 — 2026-09-15

- Fondasi monorepo ESP-IDF / Rust dan salinan blueprint sebagai PROJECT_SPEC.
- Konfigurasi ESP32-S3 N16R8, partition factory tanpa OTA.
- Driver display ST7789 + LVGL dengan state contoh M1 yang statis.
- Pin, offset, warna, orientasi, dan backlight terpusat dalam board_config.h.
- Workspace Rust enam crate dan CLI versi/help.
- Dokumentasi arsitektur, wiring, protokol, state machine, dan validasi.
- Input, BLE, integrasi Windows, serta GUI tetap menunggu milestone berikutnya.

Status penerimaan hardware mengikuti docs/validation.md.
