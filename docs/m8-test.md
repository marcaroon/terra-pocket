# Penerimaan M8 — volume

Gunakan firmware **0.4.0** dan companion dari source M8. Wiring tetap
[seperti yang sudah diuji](wiring.md); encoder A/B di GPIO15/16, tombol encoder
di GPIO17. Reconnect otomatis (M9) belum aktif. HDD/NAS tidak diperlukan.

Kriteria spesifikasi: memutar encoder mengubah master volume Windows, dan
perubahan volume dari Windows memperbarui layar perangkat.

## Persiapan

1. Dari root: `.\scripts\test-host.ps1`, lalu di `companion`:
   `cargo build --workspace --locked` dan `cargo test --workspace --locked`.
2. Build dan flash. Profil `usb-probe` (console COM/UART) pada board yang diuji:

   ```powershell
   . .\scripts\activate-local.ps1
   .\scripts\build-firmware.ps1 -Profile usb-probe
   idf.py -C firmware -B firmware/build-usb-probe -p COM11 flash
   ```

   COM11 adalah port board yang diuji; periksa ulang di komputer lain.
   Build lokal ini memakai `CONFIG_TERRA_INPUT_DIAGNOSTICS` **mati**, sehingga
   rangkaian ini juga menjadi uji tombol M7 tanpa logging diagnostik.
3. Pastikan ada perangkat output audio aktif di Windows dan catat volumenya:
   `companion\target\debug\terra-cli.exe volume`.
4. Jalankan `companion\target\debug\terra-cli.exe monitor`. Tunggu
   `HELLO protocol=1 firmware=0.4.0`, lalu baris `[VOLUME] NN` yang sama dengan
   flyout volume Windows. LCD kanan bawah menampilkan `VOL NN`.

## Pemeriksaan

| Aksi | Hasil yang diharapkan |
| --- | --- |
| Putar encoder searah jarum jam 5 klik | Volume Windows naik 10 (2 per klik); companion `VolumeDelta(+2): now …` lima kali; LCD mengikuti |
| Putar berlawanan 5 klik | Volume turun 10; LCD mengikuti |
| Putar cepat satu putaran penuh | Tidak ada crash/putus; nilai akhir LCD = flyout Windows (bila antrean penuh firmware mencatat `ESP_ERR_TIMEOUT`; catat jumlahnya) |
| Volume 100 lalu putar searah jarum jam | Tetap 100, tidak melewati batas |
| Volume 0 lalu putar berlawanan | Tetap 0 |
| Tekan tombol encoder | Windows mute; companion `MuteToggle: now NN muted`; LCD `MUTED` |
| Tekan lagi | Unmute; LCD kembali `VOL NN` |
| Putar encoder saat mute | Volume berubah, mute tetap (mengikuti spesifikasi §20); LCD tetap `MUTED` |
| Ubah volume lewat flyout/tombol keyboard Windows | `[VOLUME] NN` di companion dan LCD berubah dalam < 1 detik |
| Mute lewat flyout Windows | LCD `MUTED` |
| Ganti perangkat output default (mis. speaker ↔ headset) | Dalam ±1 detik `[VOLUME]` mengikuti perangkat baru |
| Putar encoder saat BLE belum READY | Tidak ada perubahan Windows; firmware `ESP_ERR_INVALID_STATE` |
| Ulangi tabel tombol di [m7-test.md](m7-test.md) | Play/Pause, Next, Previous tetap sekali per tekanan |

Layar tidak berubah secara optimistis: nilai di LCD selalu dari VOLUME_STATE yang
dibaca ulang dari Windows setelah perintah.

Log firmware per klik encoder:

```text
I (INPUT) MEDIA_COMMAND 4 2: ESP_OK
I (STATE) Volume 74
```

`MEDIA_COMMAND 4 -2` untuk arah sebaliknya dan `MEDIA_COMMAND 5 0` untuk tombol
encoder. Log companion:

```text
[COMMAND] VolumeDelta(+2): now 74
[VOLUME] 74
[COMMAND] MuteToggle: now 74 muted
[VOLUME] 74 muted
```

Jika arah putar terbalik, ubah `BOARD_ENCODER_DIRECTION` di `board_config.h`;
jangan menukar logika companion.

## Hasil yang perlu dilaporkan

- Log `monitor` companion dan UART dari satu rangkaian tabel di atas.
- Untuk setiap baris: hasil yang terlihat di flyout Windows dan di LCD.
- Jumlah klik encoder vs jumlah `VolumeDelta` saat putaran lambat dan cepat.
- Hasil ulang tombol M7 tanpa diagnostik (Previous di aplikasi yang dipakai).

## Cakupan otomatis

- C: transport produksi meneruskan `VOLUME_DELTA ±2` dan `MUTE_TOGGLE`, menolak
  delta 0/di luar −100…100, mute dengan nilai, SEEK_DELTA, dan ID tak dikenal.
- Rust: pembulatan skalar Windows ke persen, clamp 0–100, routing encoder ke
  volume tepat sekali, tombol media tidak menyentuh volume, payload rusak tidak
  dieksekusi, penggabungan event volume/media tidak mengubah urutan paket BLE.
- Uji nyata Core Audio di laptop pengembangan (opsional, mengubah volume sebentar):
  `cargo test -p audio-windows -- --ignored --nocapture`.
