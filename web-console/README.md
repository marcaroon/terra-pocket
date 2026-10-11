# Terra Home Hub — prototipe konfigurasi & NAS

Web statis tanpa dependency aplikasi. Tidak mengakses HDD, Wi-Fi, BLE, atau relay.
Status M2/N0 diperbarui: input diagnostik dan probe sektor USB ada pada firmware,
belum terhubung ke halaman ini. Identitas YD V1.3, ORICO 2020U3, WD5000LPCX dan
rencana suplai bercabang serta Cloudflare Free ditampilkan dengan batasnya.
HDD 500 GB NTFS adalah informasi pengguna; angka kapasitas terpakai tidak dibuat-buat.
File contoh dan status offline diberi label. Seluruh pengaturan hanya draf lokal.

## Yang dapat dicoba

- Navigasi Penyimpanan, Musik & layar, Konfigurasi, Rencana integrasi.
- Buka folder contoh, cari/filter/urutkan, preview dan unduh teks contoh.
- Simpan/reset draf browser; ekspor JSON dengan penanda `appliedToDevice: false`.
- Lihat render firmware aktual 240×240 dan proposal ESP32-only/Vercel.

Upload sengaja nonaktif dan diberi keterangan backend belum tersedia. Tidak ada
format/delete/mount, kredensial, atau tombol dengan klaim sambungan berhasil.

## Menjalankan lokal

Dari root monorepo, dengan Python/Node tersedia:

```powershell
python -m http.server 4173 --bind 127.0.0.1 --directory web-console/dist
node --test web-console/tests/model.test.mjs
```

Buka `http://127.0.0.1:4173`. Gunakan HTTP server, karena module JavaScript tidak
dimuat dengan andal melalui `file://`. Tidak perlu npm install atau build bundler.

## Hosting

Preview privat memakai Sites; identitas tersimpan di `.openai/hosting.json`.
`vercel.json` juga disediakan agar web **statis** dapat dihosting di Vercel dengan
root directory `web-console`, framework Other, output `dist`, tanpa build command.
Belum ada deployment ke akun Vercel, relay, autentikasi NAS, atau akses data nyata.
Memindahkan hosting frontend tidak membuat ESP32 mendukung NTFS.

Proposal lengkap: [docs/nas-proposal.md](../docs/nas-proposal.md).
Nama perangkat/Wi-Fi tersimpan hanya di browser tempat draf dibuat. Ekspor hanya
schema yang diizinkan; tidak ada field rahasia. Preview NAS bukan UI Tauri M10.
