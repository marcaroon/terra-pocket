# Penerimaan M7 — tombol fisik

Gunakan firmware 0.3.0 dan companion dari source M7 (atau firmware 0.4.0 M8;
tombol media tidak berubah). Reconnect otomatis (M9) belum aktif. HDD/NAS tidak diperlukan.

## Persiapan

1. Pertahankan [wiring yang sudah diuji](wiring.md).
2. Jalankan `scripts/test-host.ps1`, tes workspace Rust, dan build kedua profil.
3. Flash profil `usb-probe` bila kabel memakai COM/UART. Setelah aktivasi toolchain
   dan build, jalankan `idf.py -C firmware -B firmware/build-usb-probe -p COM11 flash`.
   COM11 adalah port board yang diuji; periksa ulang di komputer lain.
4. Putar playlist Spotify atau YouTube dengan lebih dari satu lagu.
5. Jalankan `companion\target\debug\terra-cli.exe monitor`.
   Tunggu HELLO firmware 0.3.0, READY, dan metadata yang cocok di LCD.

## Pemeriksaan

| Aksi | Hasil yang diharapkan |
| --- | --- |
| Play/Pause 6 kali, jeda 2 detik | Playback berubah sekali tiap tekanan; LCD mengikuti Windows |
| Tahan Play/Pause 3 detik | Satu perintah tanpa repeat |
| Next 3 kali dan Previous 3 kali | Setiap tekanan menjalankan aksi aplikasi sekali |
| Tekan tombol saat metadata berganti | Notifikasi tidak hilang akibat penggabungan event media |
| Tutup semua sesi media lalu tekan tombol | `NoSession`; companion tetap berjalan |
| Aplikasi tidak mendukung perintah | `Unsupported` atau `Rejected`, bukan laporan sukses |
| Ganti aplikasi aktif lalu tekan tombol | Perintah menuju sesi yang kini dipilih Windows |
| Putus BLE, tekan tombol, jalankan ulang monitor | Tekanan offline tidak diputar ulang |
| Putar/tekan encoder | Firmware 0.3.0: log lokal saja. Firmware 0.4.0: lihat [m8-test.md](m8-test.md) |

`Accepted` berarti API Windows menerima permintaan, bukan bukti suara/layar
sudah berubah. `Previous` dapat kembali ke awal lagu menurut kebijakan aplikasi.
Log `[COMMAND]` companion dan `MEDIA_COMMAND` firmware harus cocok satu per
tekanan; konfirmasi visual tetap diperlukan.

Jika Previous tampak tidak bekerja, bandingkan tombol di aplikasi pada antrean
yang sama. Catat apakah lagu kembali ke awal, berpindah, atau tidak berubah.
Empat command `Accepted` tidak sama dengan empat perpindahan lagu; bila tombol
aplikasi juga berperilaku serupa, jangan langsung menyimpulkan input Terra hilang
atau menambahkan pengiriman ulang. Hasil pembandingan YouTube Music pada
12 Oktober 2026 dicatat di laporan validasi.

Metode kontrol resmi: [GSMTC session](https://learn.microsoft.com/en-us/uwp/api/windows.media.control.globalsystemmediatransportcontrolssession?view=winrt-26100).

## Cakupan otomatis

- C menjalankan transport produksi dengan scheduler/radio stub: handshake,
  tiga ID tombol, antrean penuh, perintah kedaluwarsa/offline/sesi lama.
- Rust: urutan BLE di tengah burst media, routing command, payload rusak/reserved
  tidak dieksekusi, serta hasil API tidak disamakan dengan sukses.
- Codec: 57 vector bersama C/Rust/Python.

Stub bukan perangkat fisik. Hasil nyata: [validation.md](validation.md).

## Diagnosis klik yang hilang

Opsi development `CONFIG_TERRA_INPUT_DIAGNOSTICS` (default mati) menambah log
`BUTTON_EDGE` untuk setiap transisi GPIO. `LOW/press after N ms` menyatakan
durasi kondisi HIGH sebelumnya; `HIGH/release after N ms` menyatakan durasi LOW
sebelumnya. Bandingkan empat pasangan tekan/lepas dengan empat `MEDIA_COMMAND`
firmware dan empat hasil `[COMMAND]` companion. GPIO HIGH berarti lepas.

Logging UART dapat memengaruhi sampling kontak yang sangat berisik; gunakan
untuk diagnosis sementara, bukan bukti akurasi timing listrik/oscilloscope.
