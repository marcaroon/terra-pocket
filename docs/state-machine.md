# State machine

## Firmware (M3/M4/M7/M8, berjalan)

```text
reset → NVS → app_state (link BOOT) → ST7789/LVGL → UI → input → BLE
BOOT ─► ADVERTISING ─connect─► CONNECTED ─subscribe TX─► HELLO terkirim
                                   │                          │
                                   │            HELLO_ACK ok ─┴─► READY
                                   │            HELLO_ACK lain ──► PROTOCOL_MISMATCH
disconnect dari mana pun ─► ADVERTISING; unsubscribe/re-subscribe ─► CONNECTED + HELLO baru
```

Playback terpisah: NO_MEDIA / PLAYING / PAUSED, hanya diubah oleh MEDIA_STATE
saat READY. Link selain READY menghapus metadata dan volume. Disconnect dan
unsubscribe/re-subscribe mereset reassembler. Paket/pesan tidak sah dibuang, menaikkan
`rx_errors`, dan (untuk pesan utuh) dijawab ERROR; perangkat tidak reboot.

Layar mengikuti link: BOOT "TERRA / Starting...", ADVERTISING "BLUETOOTH /
Waiting for computer...", CONNECTED "Connecting...", MISMATCH "Protocol mismatch",
READY tanpa media "CONNECTED / Nothing Playing", READY dengan media menampilkan
source, judul, artis, progres lokal, Playing/Paused, dan volume.

Timeline: posisi tampil = posisi dasar + (sekarang − waktu terima) saat PLAYING,
di-clamp ke durasi bila durasi > 0; PAUSED/NO_MEDIA tidak bertambah. Durasi 0
ditampilkan `--:--`.

Tombol media, encoder volume, dan tombol mute dikirim hanya di READY. Perintah antrean membawa identitas sesi
dan waktu: dibuang setelah disconnect/re-handshake atau usia >500 ms, tidak
diputar ulang saat tersambung kembali. LCD menunggu MEDIA_STATE/VOLUME_STATE dari Windows.

## Companion (M6–M8 berjalan; retry M9 belum)

DISCONNECTED → SCANNING → CONNECTING → DISCOVERING → HANDSHAKE → CONNECTED.
Gagal/disconnect mengakhiri sesi; pengguna menjalankan ulang CLI. Rencana M9:
retry 1, 2, 4, 8, 15, 15 detik tanpa busy-loop scan.
Ketidakcocokan versi menghentikan sinkronisasi dan ditampilkan.
Di CONNECTED, sinyal media digabung selama 150 ms; paket BLE tetap diproses
berurutan tanpa ikut dibuang. MEDIA_COMMAND divalidasi lalu diarahkan ke sesi
GSMTC yang aktif saat perintah diterima. Kegagalan perintah dicatat dan tidak
mematikan sesi. HELLO ulang juga diperiksa versinya sebelum ACK dan sinkronisasi.
Volume: event Core Audio digabung 50 ms; VOLUME_DELTA/MUTE_TOGGLE dijalankan
sekali lalu volume dibaca ulang; perangkat output default diperiksa tiap detik.
Kegagalan baca volume dicatat tanpa mengakhiri sesi.
Aturan handshake dan wire format: [ble-protocol.md](ble-protocol.md).
