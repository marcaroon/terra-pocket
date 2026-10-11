# State machine

## M1 yang berjalan

```text
reset → init state → init ST7789/LVGL → load mock → render statis
```

Mock: `connection=DISCONNECTED`, `demo=true`, `playback=PLAYING`, posisi 60 detik,
durasi 180 detik, volume 72, tidak mute. Status koneksi dan playback memakai enum
terpisah. Label DEMO/OFF tidak menyatakan BLE sudah berjalan.

## Rencana setelah M1

Firmware: BOOT → BLE_ADVERTISING → CONNECTED → READY. READY hanya setelah
handshake cocok. Disconnect mereset reassembly dan status koneksi; state playback
terpisah NO_MEDIA / PLAYING / PAUSED. Callback BLE hanya memperbarui state dan
memberi sinyal ke UI; tidak memodifikasi widget.

Companion: DISCONNECTED → SCANNING → CONNECTING → DISCOVERING → HANDSHAKE →
CONNECTED. Gagal/disconnect mengarah ke retry 1, 2, 4, 8, 15, 15 detik; tidak ada
busy-loop scan. Ketidakcocokan versi menghentikan sinkronisasi dan ditampilkan.

Timeline nantinya memakai posisi dasar + selisih jam monotonic saat playing,
dibatasi durasi; pause tidak menambah waktu. Sinkronisasi saat track, playback,
seek, atau timeline berubah. Belum ada animasi maupun sinkronisasi pada M1.
