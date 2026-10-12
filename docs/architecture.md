# Arsitektur — M9

## Firmware ESP32-S3

```text
app_main: NVS → app_state → display/UI → input → BLE → USB probe opsional
  physical_input → debounce/encoder → antrean input → app_main
    → ble_transport_send_command → antrean transport → TX notify
  NimBLE GAP/GATT → antrean transport → handshake/reassembler/codec
    → app_state (mutex) → snapshot → timer LVGL → ST7789 240×240
```

`ble_service` menangani GAP/GATT; `ble_transport` memiliki handshake, sequence,
reassembly, dan pengiriman TX. Callbacks tidak menyentuh LVGL. Antrean transport
16 event; RX nonblocking, lifecycle dapat menunggu 50 ms. `app_main` meneruskan
tiga tombol media, encoder (VOLUME_DELTA ±2) dan tombol encoder (MUTE_TOGGLE)
tanpa menunggu BLE. Perintah lokal diberi identitas sesi READY serta batas umur
500 ms. Overflow dicatat. Layar volume hanya berubah oleh VOLUME_STATE.

`app_state` adalah sumber state layar dengan mutex. UI dibuat sekali lalu
diperbarui setiap 200 ms. Progres PLAYING diinterpolasi lokal. Metadata tidak
diubah secara optimistis setelah tombol ditekan. Disconnect menghapus media
lama. Buffer SPI DMA RGB565 berada di RAM internal; PSRAM octal 8 MB diverifikasi.

Profil `normal`: console USB native. Profil `usb-probe`: console COM/UART,
USB native menjadi host. N0 hanya READ(10) sektor metadata, tanpa mount,
NTFS driver, operasi tulis, atau file server. Power HDD belum terverifikasi.

## Companion Windows

| Crate | Implementasi |
| --- | --- |
| protocol | Codec V1/fragment/reassembly; 57 vector bersama C/Rust/Python |
| media-windows | GSMTC: sesi aktif, metadata/timeline/event dan perintah M7 |
| ble-windows | WinRT scan/connect/discovery/notify/write/status; cleanup saat gagal/drop |
| companion-core | Handshake, sinkronisasi bermakna, routing command; tidak memiliki UI |
| audio-windows | Core Audio `IAudioEndpointVolume`: perangkat output default, baca/step/mute, callback perubahan |
| companion-cli | media, media --watch, volume, volume --watch, devices, connect, monitor |

`connect`/`monitor`: scan atau alamat eksplisit → discovery → subscribe TX →
HELLO/ACK → sinkronisasi. Event GSMTC digabung 150 ms tanpa membuang paket BLE
yang menyela. Command divalidasi lalu dijalankan sekali pada sesi Windows aktif.
Hasil Accepted/Rejected/Unsupported/NoSession dicatat; kegagalan command tidak
memutus sinkronisasi. Setelah request, companion membaca ulang state Windows
dan tetap menerima event perubahan. Kode Windows API tetap di crate native.

Volume (M8): VOLUME_DELTA dihitung dari persen bulat saat ini, di-clamp 0–100
(spesifikasi §20), lalu dibaca ulang dan dikirim sebagai VOLUME_STATE. Mute
tidak diubah oleh delta. Callback Core Audio digabung 50 ms; VOLUME_STATE hanya
dikirim bila nilai/mute berubah. Windows tidak memberi event volume saat
perangkat output default berganti, jadi companion memeriksa perangkat default
setiap 1 detik. Tanpa perangkat output, perangkat Terra mempertahankan nilai
terakhir (V1 tidak punya status "volume tidak diketahui" selain sebelum READY).
`unsafe` (COM) hanya diizinkan di modul `native` crate audio-windows.

Tidak ada pairing/bonding atau ACK command tingkat aplikasi di BLE V1.
`session::run_forever` (M9) menjalankan sesi berulang: setiap akhir sesi selain
ketidakcocokan versi diikuti backoff 1–15 detik dan scan baru, dengan objek
WinRT (link, GSMTC, Core Audio) dibuat ulang per sesi agar pulih setelah
Bluetooth mati/nyala atau sleep/wake. Firmware tidak berubah di M9: setiap
disconnect, termasuk host reset NimBLE, kembali ke advertising. Tauri/tray tetap
M10 setelah CLI stabil.

## Website dan NAS

`web-console` adalah prototipe statis berisi file contoh/draf konfigurasi browser;
belum terhubung ke API perangkat. NTFS read-only, akses LAN, dan relay cloud masih
proposal di [nas-proposal.md](nas-proposal.md). Penyelesaian M7 tidak berarti NAS
sudah aktif. HDD tidak diformat atau ditulis.

## Urutan penerimaan

Bukti dan sisa pemeriksaan M0–M6 ada di [validation.md](validation.md).
M7 harus melewati [uji tombol](m7-test.md) sebelum diterima penuh.
M8 volume harus melewati [uji volume](m8-test.md), M9 [uji keandalan](m9-test.md).
Selanjutnya M10 GUI/tray. Wire format mengikuti
[ble-protocol.md](ble-protocol.md), bukan asumsi dari mock UI.
