# Penerimaan M10 — aplikasi companion

Firmware tetap **0.4.0**. Build aplikasi sesuai [companion/app/README.md](../companion/app/README.md).
Tutup `terra-cli connect/monitor` lebih dulu: Terra hanya menerima satu koneksi.

Kriteria spesifikasi: React UI, system tray, auto connect, opsi startup, dan
informasi status (§27, §28). M10 menandai MVP selesai.

## Pemeriksaan

| Aksi | Hasil yang diharapkan |
| --- | --- |
| Jalankan `terra-companion.exe` | Jendela Terra tampil; Device `Searching…` → `Connected`; LCD menampilkan lagu |
| Lihat jendela saat lagu diputar | Now Playing = judul/artis/sumber di LCD; Playing/Paused mengikuti Windows |
| Putar encoder / ubah volume Windows | Volume di jendela dan LCD sama (`72%` atau `Muted`) |
| Lihat Firmware / Protocol | `0.4.0` / `1`; Companion `0.1.0` |
| Klik X pada jendela | Jendela hilang, ikon Terra tetap di tray, LCD tetap tersambung |
| Klik kiri ikon tray | Jendela muncul lagi |
| Klik kanan ikon tray | Menu: status (`Connected`), Open, Reconnect, Exit |
| Tray → Reconnect | Status sempat `Searching…`/`Disconnected`, lalu `Connected` lagi |
| Matikan Auto Connect | `Not connecting`; LCD `Waiting for computer...`; tidak mencoba lagi |
| Nyalakan Auto Connect | Tersambung lagi tanpa restart aplikasi |
| Tutup aplikasi (Exit), buka lagi | Auto Connect tetap sesuai pilihan terakhir |
| Jalankan aplikasi kedua kali | Tidak ada instance kedua; jendela yang ada muncul |
| Nyalakan Start with Windows, sign out/restart Windows | Aplikasi berjalan di tray tanpa jendela dan tersambung otomatis |
| Matikan Start with Windows | Entri startup hilang (Task Manager → Startup apps) |
| Matikan Bluetooth / reset ESP | Status `Disconnected · retry in N s`, lalu pulih seperti M9 |

## Hasil yang perlu dilaporkan

Untuk tiap baris: apa yang tampil di jendela, tray, dan LCD; serta hal yang
tidak sesuai atau membingungkan. Tangkapan layar jendela bila memungkinkan.

## Cakupan otomatis

- Rust (`src-tauri`): pemetaan update sesi → status jendela/tray (fase, media,
  volume, pembersihan saat putus) dan nama field JSON yang dibaca React.
- `companion-core`: `Control` stop/reconnect; `run_forever` berhenti sebelum
  menyentuh Bluetooth bila sudah diminta stop.
- Frontend: `tsc --noEmit` (strict) dan `vite build`.
- Uji pengembang 12 Oktober 2026: aplikasi debug tersambung otomatis ke Terra
  saat start (UART `HELLO_ACK companion=0.1.0`, lagu dan volume terkirim);
  instance kedua keluar dengan kode 0 dan instance pertama tetap berjalan.
