# Penerimaan V2 — album artwork

Firmware **0.6.0** dan companion dari source V2 (CLI atau aplikasi). Wire format
dan perilaku penerima: [ble-protocol.md §4.8](ble-protocol.md). Arah V2 di
spesifikasi proyek §36: thumbnail Windows → decode → resize → JPEG → transfer →
ESP32 → PSRAM → gambar LVGL; ukurannya 240×240 (layar penuh) atas permintaan
pengguna.

![Tampilan teks dan tampilan cover dari renderer host](v2-artwork-screens.png)

Dua tampilan untuk lagu yang punya cover: teks (kiri, sama dengan M4–M9) dan
cover murni layar penuh (kanan). **Tekan lama encoder ≥ 0,6 s** berganti
tampilan; pilihan disimpan di NVS dan bertahan setelah restart. Tekan singkat
tetap mute (dikirim saat tombol dilepas). Tanpa cover, belum tersambung, atau
tidak ada media, layar selalu teks. Gambar berasal dari `ui.c` yang dirender di
komputer (cover pengganti berupa gradien), bukan foto panel.

## Persiapan

1. `scripts/test-host.ps1`, `cargo test --workspace --locked`, build kedua profil.
2. Flash `usb-probe` ke COM11 (lihat [m7-test.md](m7-test.md)) atau `normal`.
3. Putar lagu yang punya cover (Spotify, YouTube Music di Chrome).
4. Opsional, tanpa perangkat: `companion\target\debug\terra-cli.exe artwork cover.jpg`
   menyimpan JPEG persis seperti yang akan dikirim (240×240, ±5–15 KB).
5. Jalankan `terra-cli monitor` (atau aplikasi). Log harus memuat
   `HELLO protocol=1 firmware=0.6.0` dan `[PROTOCOL] device shows album artwork`.

## Pemeriksaan

| Aksi | Hasil yang diharapkan |
| --- | --- |
| Lagu dengan cover diputar | Dalam ±1–3 s: `[ARTWORK] sent N bytes in X s`; UART `Artwork … shown` |
| Tekan lama encoder | UART `INPUT: VIEW artwork`; cover memenuhi layar tanpa teks |
| Tekan lama lagi | `VIEW text`; tampilan teks seperti M4 |
| Tekan singkat encoder di kedua tampilan | Mute/unmute seperti M8, tampilan tidak berganti |
| Reset ESP di tampilan cover | Setelah tersambung dan cover terkirim, kembali ke tampilan cover |
| Warna dan orientasi cover | Sama dengan cover di aplikasi/jendela media Windows (tidak terbalik, tidak tertukar merah/biru) |
| Next/Previous | Cover lama hilang saat judul berganti, cover baru muncul tanpa sisa gambar lama |
| Tekan tombol/encoder saat cover sedang dikirim | Perintah tetap dijalankan segera (chunk mengalah ke event) |
| Lagu/aplikasi tanpa thumbnail (tampilan cover dipilih) | Tampilan teks; cover muncul lagi untuk lagu yang punya cover |
| Video YouTube 16:9 | Cover persegi dari bagian tengah thumbnail |
| Pause/Play di lagu yang sama | Cover tetap, tidak dikirim ulang |
| Putus BLE / reset ESP | Setelah tersambung lagi, cover dikirim ulang otomatis |
| Companion lama (M10) + firmware 0.5.0 | Tetap tersambung; tanpa cover |
| Companion V2 + firmware 0.4.0 | Tidak ada pesan artwork; perilaku M10 |
| Companion V2 (240) + firmware 0.5.0 (160) | BEGIN ditolak sekali per lagu (`[DEVICE] rejected type 0x50`); tanpa cover |

## Hasil yang perlu dilaporkan

Log companion dan UART, foto LCD dengan cover, catatan kecocokan warna, serta
lagu/aplikasi yang tidak menampilkan cover padahal Windows menampilkannya.

## Cakupan otomatis

- 80 vector bersama Python/C/Rust, termasuk BEGIN/CHUNK/END, batas ukuran, CRC.
- C: penerima artwork (urutan, celah, id asing, overflow, CRC, restart) dan
  transport produksi (sukses, CRC rusak, JPEG tak ter-decode, clear, ganti lagu
  di tengah transfer, belum READY). Decoder JPEG ROM hanya berjalan di perangkat.
- Rust: crop persegi, chunk mengalah ke event BLE, artwork mengikuti lagu yang
  tampil, tidak dikirim ke firmware tanpa bit3.
- Renderer host: tampilan teks dan cover layar penuh di dalam batas layar.
- C: tekan singkat/lama (dilepas sebelum 0,6 s, sekali saat ditahan, tanpa mute
  setelah tekan lama, tombol ditahan saat boot, rollover timer).
