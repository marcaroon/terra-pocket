# Laporan validasi

## M3/M4 — 12 Oktober 2026

**Compile verified:** build `normal` 878.821 byte dan `usb-probe` 945.197 byte, ESP-IDF
6.0.3, tanpa warning (`-Werror`). Partisi aplikasi 3 MiB.

**Software tested:** `scripts/test-host.ps1` lulus termasuk 57 vector protokol C;
`cargo test -p protocol` lulus pada vector yang sama; uji mutasi (satu byte vector
diubah) membuat tes C gagal sesuai harapan. Renderer host: enam layar (playing,
long-text, zero-duration, waiting, no-media, mismatch) dalam batas panel.

**Hardware tested** (board pengguna di COM11, Bluetooth laptop Intel, harness
`scripts/ble-test.py`, bleak 1.1.1):

- Advertising `Terra` dengan UUID service terlihat, RSSI −32 dBm di meja.
- Connect, MTU 247, service + RX/TX/STATUS dengan properti benar.
- STATUS CONNECTED sebelum handshake; subscribe TX → HELLO (protocol 1, firmware
  0.2.0, caps 0x0003); HELLO_ACK → READY.
- Tulis RX VOLUME_STATE + MEDIA_STATE diterima tanpa ERROR. MEDIA_STATE maksimum
  361 byte direassembly dari 2 paket (MTU 247) dan 23 paket (`--mtu 23`).
- Title 128 byte → ERROR MALFORMED; versi 2 → UNSUPPORTED_VERSION; fragmen tanpa
  START dibuang; MEDIA_COMMAND dari PC → UNSUPPORTED_TYPE. Perangkat tetap READY,
  `rx_errors` naik 4 per putaran, tidak ada reboot.
- Empat sesi connect/disconnect berturut-turut; setelah disconnect advertising lagi.
- Log UART build `usb-probe` cocok dengan development.md, termasuk metadata UTF-8
  multibyte dan emoji.

**Belum diverifikasi:** tampilan layar selama uji (menunggu konfirmasi visual
pengguna), log USB native build `normal`, nRF Connect/ponsel, jarak/interferensi,
dan durasi koneksi panjang. Companion Windows (M5/M6) belum ada; harness hanya
alat uji.

## Uji hardware pertama 11 Oktober 2026 — M1/M2 di perangkat

Board terdeteksi di COM11 (bridge CH343 `1A86:55D3`). esptool: ESP32-S3 (QFN56)
rev v0.2, PSRAM tertanam 8 MB, flash 16 MB quad. Uji memakai build `usb-probe`
(console UART) karena kabel pada port COM; kode LCD dan input sama dengan build
`normal`. USB-A/HDD tidak terpasang, jadi probe hanya menunggu.

- **Bug ditemukan:** stack overflow di task `main` saat render pertama LVGL
  (`lv_refr_now` di `display_render`), boot loop. Stack main 3.584 → 8.192 byte;
  sisa terukur 4.432–4.464 byte. Log sisa stack ditambahkan.
- Wiring awal salah dan dikoreksi pengguna dengan bantuan pemindai pin: tombol
  tersambung GND permanen, lalu GND tombol mengambang, dan encoder di GPIO8/9/10.
- Encoder terukur half-step (10 transisi untuk 5 klik); decoder half-step
  ditambahkan beserta tes host dari urutan yang terekam.
- **LCD:** setelah rewiring, layar menampilkan fixture DEMO (Test Song) dengan
  modul 7 pin tanpa CS, SPI mode 3, gap 0/0. Pengguna melaporkan warna normal dan
  tidak ada tepi terpotong; inversi warna dan offset (0,0) cocok untuk modul ini.
- **Input:** Previous 3/3, Play/Pause 3/3, Next 3/3, CW 5 klik → `VOLUME +2` ×5,
  knob 3/3 `MUTE`, tahan Play 3 detik → satu event tanpa repeat. CCW 5 klik →
  `VOLUME -2` ×6: event ke-6 berjarak 60 ms dari ke-5, kemungkinan knob melewati
  detent saat dilepas; perlu diulang sebelum M2 diterima.
- Belum diuji: build `normal` lewat USB native, 20× per tombol, putaran cepat,
  dua tombol bersamaan, boot sambil menahan tombol, N0 dengan HDD.

## Fase lanjutan 11 Oktober 2026 — M2 / N0

- Firmware normal M2: `idf.py -C firmware build size` lulus, exit 0;
  binary 579.472 byte, log `.tools/m2-build.log`. LCD tetap 240×240,
  USB probe default nonaktif.
- Firmware profil USB: `scripts/build-firmware.ps1 -Profile usb-probe` lulus,
  exit 0, binary 644.720 byte; log `.tools/n0-build.log`. Konfigurasi hasil build
  menunjukkan MUSIC_USB_PROBE=y, console UART0 aktif, USB Serial/JTAG utama dan
  sekunder nonaktif. Kedua profil tidak menghasilkan compiler warning/error.
  Ada catatan Kconfig vendor FATFS_PRINT_FLOAT/default bool; tidak menghalangi
  build dan tidak diatasi dengan mengubah kode vendor.
- Tes C host: pemformat waktu, debounce/encoder, parser metadata disk lulus
  melalui `scripts/test-host.ps1`, memakai `-Wall -Wextra -Werror`.
  Meliputi bounce, held-at-boot, release, rollover timer, reversal/invalid encoder,
  NTFS signature, MBR/GPT marker, input pendek/null, dan overflow/rentang sektor.
- Web: lima tes model lulus; skenario browser desktop/mobile lama tetap lulus,
  plus penyimpanan pilihan Cloudflare Free setelah reload. Empat halaman tidak
  overflow horizontal pada 375 px. Screenshot proposal 1440/375 px diperiksa.
- Dashboard M2/N0 berhasil dipublikasikan privat pada URL yang sama; native
  deployment berstatus `succeeded`, source commit Site
  `e52a9c0b87b0f49bd11bd7f47e09f8d3c49096e9`. Tidak ada deployment Vercel/relay.
- Tidak ada flash, mount, atau akses ke HDD fisik selama pengembangan. Input queue,
  timing GPIO, daya HDD, deteksi USB, cabut-pasang dan NTFS hardware belum diuji.
- N0 tidak menyediakan mount/list/download file. Probe membaca maksimal lima
  sektor; deteksi signature tidak setara implementasi driver NTFS. GPT/EBR belum
  ditelusuri. Cloudflare relay, Wi-Fi dan BLE belum diimplementasikan.
- Rust tidak diubah atau diuji ulang pada fase ini. CI diperbarui untuk dua profil
  firmware dan tes C, belum dijalankan di GitHub.

Instruksi dan log yang harus diverifikasi pada perangkat: [m2-n0-test.md](m2-n0-test.md).

## Riwayat 11 Oktober 2026 — LCD dan prototipe web sebelum M2

- Build firmware `idf.py -C firmware build size`: lulus, exit 0. Binary
  `music_controller.bin` 568.048 byte; log lokal `.tools/lcd240-build.log`.
- Render host LVGL 240×240: normal, teks panjang, dan durasi nol lulus.
  Harness memeriksa batas framebuffer dan semua widget; normal/teks panjang
  diperiksa secara visual. Resolusi harness mengikuti `board_config.h`.
- Web: empat tes model lulus (validasi konfigurasi, batas UTF-8, filter/sort,
  konsistensi fixture). Pemeriksaan sintaks JavaScript lulus.
- Uji browser Edge melalui Playwright: desktop 1440 px dan ponsel 375 px,
  empat halaman tanpa overflow horizontal; folder, pencarian, filter, unduh teks,
  simpan/reload/reset dan ekspor JSON lulus. Tidak ada page error.
- Screenshot desktop dan ponsel diperiksa. Seluruh file web adalah contoh;
  konfigurasi tersimpan hanya sebagai draf browser, belum dikirim ke ESP32.
- Preview privat berhasil dipublikasikan melalui Sites:
  https://music-controller-hub-ammar.sappyquail1.chatgpt.site
  Status deployment `succeeded`; ini bukan deployment Vercel atau backend NAS.
  Source publikasi identik dengan `web-console/`, disinkronkan dari checkout
  terpisah `.tools/site-publish` agar tidak membuat repository Git bersarang.
- NAS, NTFS, USB Host, suplai HDD, relay, dan akses remote belum diimplementasikan
  atau diuji. Tidak ada flash, mount, format, maupun penulisan HDD.
- Rust tidak diubah atau diuji ulang pada pembaruan ini. M2 belum dimulai.

![Render host terkini 240×240](m1-preview.png)

![Prototipe web desktop](web-console-desktop.png)

Controller ST7789 dan resolusi 240×240 sudah dikonfirmasi pengguna. Model breakout
dan board fisik masih perlu diverifikasi. Gap awal sekarang (0,0); beberapa panel
memerlukan offset lain (misalnya 80) sesuai orientasi. Render host tidak menguji
warna, backlight, PSRAM, wiring, atau offset panel fisik.

## Riwayat M0/M1 — 15 September 2026

Tanggal: 15 September 2026. Scope mengikuti PROJECT_SPEC bagian 38: berhenti
setelah fondasi dan display bring-up. M2 tidak dimulai.

## Berkas yang dibuat

| Kelompok | Berkas / fungsi |
| --- | --- |
| Root | README, PROJECT_SPEC (salinan blueprint), CHANGELOG, gitignore/gitattributes |
| docs | architecture, hardware, ble-protocol, state-machine, development, validation |
| Firmware build | CMakeLists, sdkconfig.defaults, partitions.csv, dependencies.lock |
| Firmware main | CMakeLists, idf_component.yml, app_main, board_config, app_state |
| Display | display.c/h untuk SPI/panel/port; ui.c/h untuk widget |
| Utilities/tests | time_utils.c/h, test_time_utils.c, harness headless LVGL tests/preview |
| Companion | Cargo.toml/lock, manifest dan source enam crate, README GUI terencana |
| Tooling | scripts/activate-local.ps1, .github/workflows/build.yml |
| Vectors | README penanda lokasi fixture bersama M4, tanpa fixture palsu |

## Keputusan

- `app_main` hanya startup; state kanonik memiliki mutex dan API snapshot.
- LVGL hanya disentuh modul UI dengan lock port. Tidak ada callback BLE.
- Dua buffer DMA internal RGB565, masing-masing 19.200 byte; PSRAM diaktifkan
  dan diuji oleh boot IDF, belum digunakan untuk artwork/framebuffer.
- Fixture M1 statis dan diberi label DEMO/OFF; tidak mengklaim terhubung ke PC.
- Rust crate tahap mendatang hanya mendokumentasikan batas tanggung jawab.
- Tidak menambahkan dependency Windows/Tokio/React sebelum milestone terkait.

## Dependency

ESP-IDF 6.0.3, Xtensa GCC 15.2.0 (esp-15.2.0_20251204), LVGL 9.5.0,
esp_lvgl_port 2.9.0. Lockfile firmware dihasilkan Component Manager.
Rust 1.98.1, target x86_64-pc-windows-gnu; Cargo.lock dihasilkan Cargo.
Tidak ada dependency Rust eksternal.

## Hasil validasi

| Pemeriksaan | Hasil |
| --- | --- |
| Build Rust workspace --locked | Lulus, enam crate |
| CLI --version / --help | Lulus |
| CLI media (belum diimplementasikan) | Exit 2 sesuai desain |
| Tes C pemformat waktu | Lulus, 7 pemeriksaan termasuk buffer dan UINT32_MAX |
| Render LVGL host | Lulus, preview 240×280 diperiksa: teks utuh, progres 1/3 |
| Build firmware / idf.py build size | Lulus, ESP32-S3, exit 0 |
| CI GitHub | Workflow disiapkan; belum dijalankan di GitHub |
| Flash, boot, LCD, warna, orientasi | Belum diuji pada hardware |

Toolchain GNU lokal menggunakan runtime bawaan Rust melalui
`-C link-self-contained=yes`; alasan dan rujukannya tercatat di development.md.

Firmware binary: **568.048 byte** (`0x8aaf0`), sekitar 555 KiB. Partition aplikasi
3 MiB masih memiliki **82% ruang kosong**. Artefak lokal:

- `firmware/build/music_controller.bin`
- `firmware/build/bootloader/bootloader.bin`
- `firmware/build/partition_table/partition-table.bin`
- `firmware/build/flasher_args.json`

Validasi akhir dijalankan dengan aktivasi toolchain lokal dan empat proses build:

```powershell
. .\scripts\activate-local.ps1
idf.py -C firmware build size
cargo build --workspace --locked --manifest-path companion/Cargo.toml
```

Log akhir: `Project build complete`; Cargo: `Finished dev profile`.
Log lengkap lokal ada di `.tools/firmware-acceptance.log`. Satu crash internal
compiler pada build paralel awal pulih pada retry empat proses; penyebabnya belum
dipastikan, tidak ada perubahan driver vendor untuk melewati kegagalan tersebut.

**M0 compile verified. M1 compile verified dan render host verified; hardware
acceptance masih menunggu.** Pada tahap awal ini ada 50 berkas proyek; hasil build dan
toolchain tidak dimasukkan ke Git. Kedua dependency lockfile disertakan.

## Asumsi pada validasi awal (digantikan pembaruan di atas)

Board DevKit N16R8 dan panel ST7789 240×280 belum dikonfirmasi modelnya.
Gap (0,20), RGB order, inversion aktif, byte swap, orientasi, dan backlight
active-high adalah nilai awal yang bisa diubah di board_config.h.
Rangkaian backlight dan kebutuhan arus breakout harus dicocokkan dengan modul.
GPIO19/20 tetap tersedia untuk USB native. Detail lengkap ada di hardware.md.

**M1 belum diterima secara hardware.** Ikuti perintah flash di development.md dan
kirim log sejak boot beserta foto layar. Jangan melanjutkan ke M2 otomatis.
