# State machine

## Firmware (M3/M4, berjalan)

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
re-subscribe mereset reassembler. Paket/pesan tidak sah dibuang, menaikkan
`rx_errors`, dan (untuk pesan utuh) dijawab ERROR; perangkat tidak reboot.

Layar mengikuti link: BOOT "TERRA / Starting...", ADVERTISING "BLUETOOTH /
Waiting for computer...", CONNECTED "Connecting...", MISMATCH "Protocol mismatch",
READY tanpa media "CONNECTED / Nothing Playing", READY dengan media menampilkan
source, judul, artis, progres lokal, Playing/Paused, dan volume.

Timeline: posisi tampil = posisi dasar + (sekarang − waktu terima) saat PLAYING,
di-clamp ke durasi bila durasi > 0; PAUSED/NO_MEDIA tidak bertambah. Durasi 0
ditampilkan `--:--`.

## Companion (rencana M6/M9)

DISCONNECTED → SCANNING → CONNECTING → DISCOVERING → HANDSHAKE → CONNECTED.
Gagal/disconnect mengarah ke retry 1, 2, 4, 8, 15, 15 detik; tidak ada
busy-loop scan. Ketidakcocokan versi menghentikan sinkronisasi dan ditampilkan.
Aturan handshake dan wire format: [ble-protocol.md](ble-protocol.md).
