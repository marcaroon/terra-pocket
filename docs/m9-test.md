# Penerimaan M9 — keandalan

Firmware tetap **0.4.0** (M9 tidak mengubah firmware). Companion dari source M9.
`terra-cli monitor` kini terus berjalan dan menyambung ulang sendiri; jangan
menjalankannya ulang di antara pemeriksaan.

Kriteria spesifikasi: perangkat bertahan dari Bluetooth PC mati/nyala, ESP
restart, restart Spotify, perpindahan browser → Spotify, serta sleep/wake PC,
tanpa perlu menjalankan ulang aplikasi bila Windows memungkinkan pemulihan.

## Persiapan

1. Build companion dan jalankan `companion\target\debug\terra-cli.exe monitor`.
2. Tunggu `[PROTOCOL] HELLO …`, `[MEDIA] …`, `[VOLUME] …`, dan LCD menampilkan lagu.

## Perilaku reconnect

Setelah link putus atau gagal, companion mencatat penyebab lalu
`[BLE] reconnecting in N s` dengan N = 1, 2, 4, 8, lalu 15 detik berulang
(spesifikasi §29). Setelah satu sesi READY, jeda kembali ke 1 detik. Saat
memindai, Terra yang terakhir tersambung diutamakan. Hanya ketidakcocokan versi
protokol yang menghentikan program (exit 3).

## Pemeriksaan

| Aksi | Hasil yang diharapkan |
| --- | --- |
| Reset ESP (tombol RST/EN) | `[BLE] disconnected`, lalu READY lagi otomatis; LCD kembali ke lagu |
| Matikan Bluetooth Windows 30 detik | LCD `Waiting for computer...`; companion `Terra not found` atau `Bluetooth is off…` (laptop uji 12 Oktober: "not found") dengan jeda naik sampai 15 s; tidak keluar |
| Nyalakan Bluetooth lagi | READY dalam ≤ 15 detik + waktu scan; lagu dan volume tersinkron |
| Sleep PC (Start → Power → Sleep) ±1 menit, lalu bangunkan | Setelah login, READY otomatis; LCD menampilkan lagu yang diputar. Pada laptop Modern Standby link BLE dapat tetap hidup selama sleep sehingga LCD tidak berubah (teramati 12 Oktober 2026) |
| Tutup Spotify saat diputar, buka dan putar lagi | LCD lagu lama sampai 5 detik, lalu `Nothing Playing`, lalu lagu Spotify baru |
| Putar di Chrome, lalu putar di Spotify | LCD berpindah mengikuti sesi yang dipilih Windows (aplikasi media yang terakhir difokus, walau sedang pause); tombol menuju aplikasi itu |
| Next/Previous di playlist Chrome/YouTube Music | Judul berganti langsung tanpa kedipan `Nothing Playing` |
| Tutup tab/aplikasi media terakhir | `Nothing Playing` setelah ±5 detik |
| Matikan ESP (cabut daya) 1 menit, lalu nyalakan | Companion tetap mencoba tiap 15 detik; tersambung setelah perangkat hidup |

Penundaan `Nothing Playing` 5 detik disengaja: Chrome melaporkan sesi kosong
0,2–3,5 detik di antara lagu (validasi 12 Oktober 2026). Timeline lagu lama
masih berjalan selama penundaan itu.

## Hasil yang perlu dilaporkan

- Log `monitor` dari seluruh rangkaian.
- Untuk tiap baris: apakah companion pulih sendiri, berapa lama, dan apa yang tampil di LCD.
- Kasus yang memerlukan menjalankan ulang `monitor` (sebutkan langkahnya).

## Cakupan otomatis

- Rust: backoff 1/2/4/8/15 dan reset setelah READY, keputusan penundaan NO_MEDIA,
  urutan event BLE di tengah burst Windows.
- Uji hardware oleh pengembang (12 Oktober 2026): restart ESP lewat EN, companion
  dimatikan paksa, dan companion dijalankan ulang segera; lihat validation.md.
