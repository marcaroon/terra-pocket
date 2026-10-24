# Changelog

## Unreleased — V2 album artwork (2026-10-12)

- Cover layar penuh (firmware 0.6.0): artwork menjadi 240×240 dan tampil sendirian
  memenuhi layar sebagai tampilan kedua. Tekan lama encoder (≥ 600 ms) berganti
  antara tampilan teks (seperti M4) dan cover; pilihan disimpan di NVS. Tekan
  singkat tetap mute, kini dikirim saat tombol dilepas. Tata letak cover 160×160
  di atas teks dihapus. 80 vector (BEGIN 160×160 kini ditolak).

- Protokol: ARTWORK_BEGIN/CHUNK/END (0x50–0x52) dan capability bit3, didokumentasikan
  di `ble-protocol.md` §4.8; `protocol_version` tetap 1, firmware/companion lama
  tetap kompatibel. 79 vector bersama Python/C/Rust (termasuk CRC-32).
- Firmware 0.5.0: penerima artwork berurutan dengan CRC ke PSRAM, decode JPEG
  baseline 160×160 dengan TJpgDec ROM (tanpa komponen baru), tata letak cover
  160×160 dengan judul/artis satu baris, progres, ikon play/pause dan volume.
  Tata letak teks tetap dipakai tanpa artwork. Stack task transport 6 KB.
- Companion: thumbnail sesi GSMTC dipotong persegi, diskalakan dan di-encode JPEG
  dengan `Windows.Graphics.Imaging` (tanpa crate baru); dikirim per chunk ≤ 232 byte
  di sela event, hanya ke perangkat dengan bit3, sekali per lagu, dicoba ulang saat
  thumbnail datang terlambat. `terra-cli artwork [FILE]`.
- Stabilitas MVP: clippy bersih (satu lint diperbaiki), 18 reset ESP berturut-turut
  pulih otomatis, soak aplikasi release tanpa kenaikan memori/handle.

## Unreleased — M10 aplikasi companion (2026-10-12)

- `companion/app`: Tauri 2 (crate 2.11) + React 19 + TypeScript. Jendela
  menampilkan perangkat, status link dan jeda retry, Now Playing, volume,
  Auto Connect, Start with Windows, firmware/protocol/companion; tombol Reconnect.
- Tray: status, Open, Reconnect, Exit; menutup jendela menyembunyikannya ke tray.
  Start with Windows memakai `tauri-plugin-autostart` (`--minimized`), satu
  instance lewat `tauri-plugin-single-instance`. Auto Connect disimpan di
  `settings.json` di folder konfigurasi aplikasi.
- `companion-core`: sesi melaporkan `Update` terstruktur (log, `Phase`, media,
  volume) dan menerima `Control` stop/reconnect; `run_forever` mengembalikan
  `Exit`. Keluaran CLI tidak berubah.
- CI: job `app` (npm ci, build Tauri debug, tes pemetaan status).

## Unreleased — M9 keandalan (2026-10-12)

- `connect`/`monitor` tidak lagi berhenti saat link putus: sesi diulang dengan
  backoff 1, 2, 4, 8, 15 s (reset setelah READY), alamat Terra terakhir
  diutamakan dan scan berhenti begitu terlihat. Hanya ketidakcocokan protokol
  yang mengakhiri program (exit 3).
- `ble-windows`: `scan_until`, dan watcher yang `Aborted` (Bluetooth mati)
  dilaporkan sebagai `RadioUnavailable`, bukan "tidak ditemukan".
- Kegagalan baca GSMTC dicatat dan diulang 1 s alih-alih memutus BLE.
- NO_MEDIA ditahan 5 s saat lagu sebelumnya masih tampil, menghilangkan kedipan
  "Nothing Playing" di antara lagu Chrome/YouTube Music.
- Event GSMTC selalu dibaca dalam 150 ms meski pembacaan ulang penundaan
  NO_MEDIA sudah terjadwal (sebelumnya judul baru tertunda 6–7 s setelah Next).
- Sesi tanpa judul dan artis (Spotify saat ditutup) diperlakukan sebagai NO_MEDIA.
- Firmware tidak berubah (0.4.0). Diuji di hardware: Bluetooth off/on, restart ESP,
  sleep/wake, Chrome ↔ Spotify, companion mati paksa dan dijalankan ulang.
  Panduan [uji M9](docs/m9-test.md).

## Unreleased — M8 volume (2026-10-12)

- Firmware 0.4.0: encoder mengirim VOLUME_DELTA +2/−2 per klik dan tombol encoder
  MUTE_TOGGLE saat READY, dengan aturan sesi/umur/antrean yang sama seperti tombol
  media. Layar volume hanya berubah dari VOLUME_STATE. Wire format tidak berubah.
- Crate `audio-windows`: Core Audio `IAudioEndpointVolume` untuk perangkat output
  default (render/console): baca persen dan mute, step dengan clamp 0–100, toggle
  mute, callback perubahan, dan rebind saat perangkat default berganti.
  `unsafe` COM dibatasi ke satu modul; workspace lain tetap `forbid(unsafe_code)`.
- Companion: perintah encoder dijalankan sekali, volume dibaca ulang lalu dikirim
  sebagai VOLUME_STATE; perubahan dari Windows digabung 50 ms dan dikirim hanya bila
  berubah; perangkat default diperiksa tiap detik. `terra-cli volume [--watch]`.
- Tes: transport C untuk volume/mute dan perintah tidak sah, routing encoder Rust,
  pembulatan/clamp volume, uji Core Audio nyata opsional (`--ignored`). CI menjalankan
  smoke test `terra-cli volume`. Panduan [uji M8](docs/m8-test.md).

## M7 dan pemeriksaan M6 (2026-10-12)

- Tindak lanjut input: opsi diagnostik GPIO tekan/lepas (default mati). Laporan
  klik berulang hilang belum memiliki akar penyebab yang terbukti; setelah flash
  diagnostik, Play/Pause 8/8 diterima pada Apple Music. Debounce belum diubah.
- Previous: capability false tidak lagi menghentikan request sebelum API dicoba;
  status keberhasilan berasal dari hasil `Try*Async`. Uji Apple Music sebelumnya
  mencatat Next 3/3 Accepted, Previous Unsupported. Uji ulang Previous Chrome
  menghasilkan 4/4 command Accepted; pengguna mengonfirmasi tombol web YouTube
  Music juga tidak selalu bereaksi. Belum ada dasar untuk retry atau perubahan
  debounce; penerimaan M7 masih parsial (lihat validation.md).

- Firmware 0.3.0: tiga tombol media menjadi MEDIA_COMMAND pada link READY,
  capability bit2 aktif; perintah offline, kedaluwarsa, atau dari sesi lama dibuang.
- Companion memakai GSMTC untuk Play/Pause, Next, Previous; hasil ditampilkan
  sebagai Accepted/Rejected/Unsupported/NoSession. `monitor` adalah alias aktif
  `connect`. Encoder volume/mute tetap M8.
- Perbaikan M6: penggabungan event media tidak lagi membuang notifikasi BLE;
  HELLO ulang divalidasi; deadline handshake berlaku meski paket terus masuk;
  scan default selesai sebelum memilih RSSI terkuat.
- Cleanup langganan media dan koneksi BLE pada inisialisasi parsial yang gagal;
  aritmetika timeline tahan overflow. Unsubscribe firmware mereset reassembler,
  HELLO_ACK hanya diterima setelah subscribe dan sebelum READY.
- Tes transport produksi dengan radio/scheduler stub, regresi antrean Rust,
  routing command, dan pembaruan panduan/status fase.

## M6 Windows BLE client

- Crate `ble-windows`: scan aktif (nama/UUID service), koneksi via `GattSession`
  dengan MaintainConnection, discovery service/karakteristik, handler TX dipasang
  sebelum CCCD notify agar HELLO tidak hilang, tulis RX dengan response, baca STATUS,
  deteksi disconnect.
- `companion-core`: aturan sinkronisasi murni (pemetaan playback, potong UTF-8 di batas
  karakter, kirim hanya saat lagu/playback/metadata berubah atau seek > 1,5 s) dan
  sesi `run`: scan → connect → HELLO/HELLO_ACK → MEDIA_STATE setiap perubahan GSMTC.
- `terra-cli devices` dan `terra-cli connect [ALAMAT]`; `monitor` tetap exit 2 (M7).
- Perilaku Windows yang terukur: discovery *uncached* sebelum link aktif gagal
  (Unreachable / 0x80070016) dan sesi tidak pernah aktif tanpa permintaan GATT;
  karena itu discovery memakai cache default dengan retry, lalu menunggu sesi aktif.

## M5 Windows media reader

- Crate `media-windows` (windows-rs 0.62.2, `Windows.Media.Control`): sesi saat ini,
  source (nama tampilan Windows atau heuristik AppUserModelId), judul, artis, album,
  status playback, posisi yang diestimasi dari `LastUpdatedTime`, dan durasi.
- `terra-cli media` mencetak snapshot; `terra-cli media --watch` berlangganan
  CurrentSessionChanged/SessionsChanged dan event media/playback/timeline sesi aktif,
  memindahkan langganan saat sesi berganti, dan hanya mencetak perubahan lagu,
  playback, atau seek (bukan posisi berjalan).
- `devices`/`connect`/`monitor` tetap exit 2 sampai M6. Perintah media ke Windows
  menunggu M7. Tanpa kode `unsafe`.

## 0.2.0 — 2026-10-12 (M3 BLE GATT + M4 protokol)

- Firmware 0.2.0: NimBLE peripheral `Terra` dengan service 6F3A0001 dan karakteristik
  RX (write/write without response), TX (notify), DEVICE STATUS (read/notify); MTU 247.
- Protokol V1 dikunci di `docs/ble-protocol.md`: sequence per pesan, timeout
  reassembly 1000 ms, batas 512 byte, HELLO saat TX di-subscribe, payload HELLO_ACK,
  ERROR, DEVICE_STATUS; ACK_REQUEST dicadangkan.
- Codec C murni (`firmware/main/protocol/`), crate Rust `protocol`, dan referensi
  Python dengan 57 test vector bersama; CI menjalankan C, Rust, dan diff generator.
- Task `ble_transport` memiliki reassembler dan handshake; data rusak dibuang,
  dihitung di `rx_errors`, dan dijawab ERROR tanpa reboot.
- UI dibuat sekali dan diperbarui timer LVGL 200 ms dari salinan state: layar
  Waiting / Connecting / Nothing Playing / Now Playing / Protocol mismatch, timeline
  lokal, `--:--` untuk durasi 0. Fixture DEMO M1 dihapus dari boot.
- `scripts/ble-test.py` (bleak) untuk uji penerimaan M3/M4 dari laptop.
- `build-firmware.ps1` tidak lagi gagal saat output di-redirect; CI memakai
  `safe.directory` di container ESP-IDF dan `actions/checkout@v5`.

## 0.1.0 — 2026-10-11

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
