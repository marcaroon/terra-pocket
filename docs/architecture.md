# Arsitektur

## Implementasi M0/M1

```text
app_main (inisialisasi)
  ├── app_state_init / app_state_load_mock
  └── display_init / display_render(snapshot)
        ├── esp_lcd → SPI DMA → ST7789
        └── ui_render → LVGL → task/timer esp_lvgl_port
```

`app_main.c` hanya mengoordinasikan startup. `app_state.c` memiliki state kanonik
dan mutex; pembaca memperoleh salinan, tanpa menyimpan pointer ke state internal.
`display.c` memiliki panel, SPI, dan lock LVGL. `ui.c` satu-satunya pembuat widget.
Tidak ada task buatan tambahan untuk layar statis, input, atau BLE pada M1.
Build menggunakan `MINIMAL_BUILD` ESP-IDF: hanya komponen utama dan dependency
yang dibutuhkan yang dimasukkan.

Rendering menggunakan dua buffer RGB565 masing-masing 240×40×2 = 19.200 byte
di RAM internal yang mendukung DMA. PSRAM tetap diinisialisasi dan diuji saat
boot untuk memastikan konfigurasi N16R8. M1 belum memerlukannya untuk framebuffer.
Kegagalan inisialisasi bersifat fatal dan dicatat; belum ada jalur parser BLE.

Layar menggunakan bidang persegi 240×240. Inversion, byte order, gap, dan mirror
adalah parameter board, bukan konstanta di renderer. Backlight diaktifkan setelah
render pertama. State contoh tidak menyimulasikan koneksi nyata atau progres waktu.

## Fondasi companion

Enam crate sudah terdaftar dalam workspace. Saat ini hanya `companion-core::VERSION`
dan CLI informasi versi yang berfungsi. Tidak ada dependency eksternal Rust.

| Crate | Tanggung jawab tahap berikutnya |
| --- | --- |
| protocol | Codec biner, fragmentasi, validasi, shared vectors (M4) |
| media-windows | GSMTC: snapshot dan langganan perubahan (M5, berjalan); perintah media (M7) |
| audio-windows | Core Audio volume/mute dan callback perubahan (M8) |
| ble-windows | Windows central: scan, GattSession, discovery, notify, write (M6, berjalan); reconnect (M9) |
| companion-core | Aturan sinkronisasi murni + sesi media→BLE (M6, berjalan); koordinator penuh di M7–M9 |
| companion-cli | Diagnostik sebelum GUI |

`windows-rs` dan Tokio akan ditambahkan saat dibutuhkan. Tauri 2, React, TypeScript
baru masuk M10. GUI adalah thin client Rust melalui command/event.

## M2 input dan N0 diagnostik USB

M2: GPIO active-low → task sampling 1 ms → filter tombol 40 ms / decoder encoder
half-step/full-step (board_config) → queue 32 event → loop aplikasi mencatat perintah serial. Input tidak
memanggil BLE atau LVGL. Queue penuh menghasilkan laporan kehilangan event.
Tidak ada long-press/double-click atau perubahan state media buatan.

N0 opsional: USB Host event task + MSC task → queue lifecycle → probe task dengan
satu handle disk. Hanya inquiry/read-capacity/read-sector; tanpa VFS/mount.
Parser metadata murni memvalidasi rentang MBR sebelum read pertama partisi.
Signature NTFS bukan implementasi filesystem. GPT hanya dikenali lewat protective
MBR. Overflow queue menghentikan probe sampai reboot; tidak meneruskan dengan
handle yang kehilangan event. Log memakai UART0 agar PHY native bebas untuk host.

Dependency terkunci: usb_host_msc 1.3.0 dan usb 1.6.0. Pada ESP-IDF **6.0.3**, driver
MSC ini masih memakai jalur diskio lama; BDL baru aktif mulai 6.0.4 menurut header
dan CMake driver, meskipun README driver menyebut 6.0+. Probe memakai
`scsi_cmd_read10` sesuai pengganti API read-sector yang deprecated, dengan versi
driver dikunci karena header berlabel `esp_private`. Tidak ada patch vendor.

## M3 BLE GATT dan M4 protokol

```text
NimBLE host task ── access/GAP callback ── salin paket/event ──► queue 16
                                                                   │
ble_transport task: reassembler → validasi payload → handshake ────┤
        │                       └─ ERROR / HELLO via fragmenter ──► notify TX
        └─ app_state_set_link / apply_media / apply_volume (mutex, revision)
                                                                   │
LVGL task: timer 200 ms → snapshot app_state → ui_update (hanya yang berubah)
```

`ble/ble_service.c` hanya NimBLE: advertising `Terra`, service dan tiga
karakteristik, MTU, subscribe. Callback tidak memanggil LVGL maupun parser; paket
RX disalin ke queue. `ble/ble_transport.c` adalah satu-satunya pemilik state link,
reassembler, sequence TX, dan penghitung `rx_errors`. `protocol/protocol.c` murni
C tanpa header ESP-IDF sehingga dites di host dengan vector bersama.

UI dibuat sekali (`ui_create`), lalu timer LVGL memperbarui label dari salinan
state. Posisi lagu dihitung lokal (`timeline_position_ms`) dari posisi dasar dan
waktu terima; BLE tidak mengirim posisi berkala. Status link dan playback adalah
enum terpisah. Link selain READY menghapus metadata agar reconnect tidak
menampilkan data basi. Input masih hanya dicatat; MEDIA_COMMAND dikirim di M7.

Sisi PC: crate Rust `protocol` mengimplementasikan codec yang sama. Untuk uji
M3/M4 sebelum companion Windows BLE (M6) ada harness dev `scripts/ble-test.py`
(bleak) yang memakai referensi Python `scripts/terra_protocol.py`.

## Urutan berikutnya

Pembaruan Oktober menambahkan `web-console/`, prototipe statis terpisah dengan
file contoh dan draf konfigurasi browser. Belum ada API ke firmware, NAS, relay,
atau perubahan scope companion Tauri M10. Arsitektur NAS ESP32-only masih proposal
di [nas-proposal.md](nas-proposal.md), menunggu keputusan pengguna.

M2 acceptance hardware → M3 GATT → M4 protokol → M5 media reader → M6 BLE Windows → M7 kontrol
media → M8 volume → M9 reliability → M10 GUI/tray. Setiap tahap harus berhasil
build sebelum berlanjut. M3/M4 sudah diimplementasikan; penerimaan hardware
menunggu uji `scripts/ble-test.py` pada perangkat (lihat validation.md).

Backlog V2: artwork, transport alternatif, OTA, baterai, dan fitur lain pada
PROJECT_SPEC bagian 35–36; tidak ada implementasinya di sini.
