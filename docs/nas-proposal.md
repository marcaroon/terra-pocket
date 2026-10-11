# Proposal NAS ESP32-only & web console

**11 Oktober 2026 — implementasi awal diizinkan pengguna; NAS penuh belum siap.**

## Status setelah diskusi hardware

M2 input dan profil diagnostik N0 sekarang ada di source. N0 hanya membaca identitas,
kapasitas, LBA0, dan sektor awal entri MBR yang valid, tanpa mount/format/tulis.
GPT hanya dikenali, belum ditelusuri. Build dan tes tercatat di validation.md;
hasil hardware tetap menunggu. [Langkah uji M2/N0](m2-n0-test.md).

Foto mengidentifikasi board YD-style `2022-V1.3` N16R8, casing ORICO **2020U3**, HDD
WD Blue **WD5000LPCX** 500 GB NTFS. Label HDD 5 V 0,55 A; datasheet menyebut
arus puncak 1 A. Rencana adaptor 5 V 3 A bercabang memberi margin, bukan bukti
port ESP32 mampu menyalurkan daya. Detail sumber dan batas ada di hardware.md.
Chip NS1068X hanya dinyatakan listing casing, belum dibuktikan dari unit fisik.

Kandidat relay tanpa langganan: **Cloudflare Workers + Durable Objects SQLite Free**,
dengan WebSocket hibernation dan batas penggunaan. Free tidak berarti transfer
tak terbatas; operasi gagal saat kuota terkait habis. Belum ada akun/relay yang
diaktifkan. Vercel tetap opsi frontend; akses internet menunggu file service lokal.
[Harga/kuota resmi Cloudflare](https://developers.cloudflare.com/durable-objects/platform/pricing/).

Bagian di bawah adalah rencana tahapan lanjut; fitur yang tidak disebut selesai
di atas belum diimplementasikan. Persetujuan lanjut bukan persetujuan format HDD.

## Keputusan pengguna dan kesimpulan

- ESP32-S3 N16R8 menjadi satu-satunya perangkat komputasi server di rumah.
- LCD **ST7789 240×240**, controller dikonfirmasi; model breakout belum diketahui.
- HDD laptop **500 GB**, **NTFS**, casing USB dengan daya dari USB.
- Web untuk konfigurasi fitur dan akses file dari luar rumah; Vercel ditanyakan
  sebagai opsi. Belum ada persetujuan format/migrasi data atau layanan berbayar.

**Rekomendasi: mulai sebagai file server web pribadi ringan, read-only, lewat LAN
dan media uji FAT32 terpisah.** Ini masih eksperimen kelayakan. NTFS disk yang ada
belum didukung oleh stack firmware ini. Jangan menjanjikan NAS siap pakai hanya
karena USB enclosure bisa terdeteksi.

## 1. Apa yang membatasi ESP32?

| Batas | Dampak praktis |
| --- | --- |
| USB Full Speed 12 Mbit/s | Batas mentah 1,5 MB/s; payload nyata lebih rendah, ditambah Wi-Fi/TLS/filesystem. Belum ada benchmark perangkat ini. |
| USB MSC Bulk-Only Transport | Bridge USB–SATA pada casing harus mendukung BOT pada Full Speed. Label USB 3.x/UASP saja tidak membuktikan kompatibilitas. |
| FatFs pada ESP-IDF 6.0.3 | Mendukung FAT, bukan NTFS; source yang dipakai memiliki FF_FS_EXFAT=0. exFAT bukan pengganti instan tanpa pekerjaan tambahan. |
| RAM & radio bersama | Jangan memuat satu file penuh ke RAM. Satu transfer aktif dahulu; ukur RAM, respons UI dan BLE bersamaan dengan Wi-Fi. |
| PHY USB internal bersama | Native USB Serial/JTAG dan USB OTG Host tidak bekerja bersamaan melalui PHY yang sama. Debug NAS perlu UART bridge atau rancangan PHY eksternal. |
| HDD bertenaga USB | Suplai VBUS 5 V harus memenuhi lonjakan spin-up; pin 3V3/GPIO bukan sumber daya HDD. |

Dasar: [ESP32-S3 TRM, bab USB](https://documentation.espressif.com/esp32-s3_technical_reference_manual_en.pdf),
[USB Host ESP-IDF 6.0.3](https://docs.espressif.com/projects/esp-idf/en/v6.0.3/esp32s3/api-reference/peripherals/usb_host.html),
[FatFs ESP-IDF 6.0.3](https://docs.espressif.com/projects/esp-idf/en/v6.0.3/esp32s3/api-reference/storage/fatfs.html),
dan source lokal `components/fatfs/src/ffconf.h`.

500 GB adalah kapasitas media, bukan kebutuhan RAM. File dapat dibaca bertahap,
tetapi partisi MBR/GPT, ukuran sektor 512/4096, dan bridge casing harus diuji.
Target awal HTTP file listing/download; SMB kompatibel Windows Explorer tidak
termasuk proposal tahap pertama. Ini membatasi kompleksitas dan permukaan risiko.

## 2. Pilihan untuk NTFS

| Pilihan | Yang dipertahankan / dikorbankan | Penilaian |
| --- | --- | --- |
| Flashdisk FAT32 terpisah untuk uji | HDD NTFS tetap utuh; membuktikan USB → LAN terlebih dulu | Disarankan sebagai tahap pertama, bukan jawaban final penyimpanan HDD |
| Pertahankan HDD NTFS | Perlu memilih/audit/port driver NTFS, menguji RAM dan variasi fitur NTFS; mulai read-only | Eksperimen terpisah, belum ada driver yang dipilih atau hasil kelayakan |
| Migrasi HDD ke FAT32 setelah backup | Data harus dibackup/diverifikasi; file tunggal maksimum 4 GiB−1 byte | Hanya jika batas file cocok dan pengguna memberi persetujuan terpisah |
| exFAT | Bisa mengakomodasi file lebih besar, tetapi konfigurasi/driver pada stack ini belum siap | Riset tambahan; bukan asumsi dukungan default |

Jika **ESP32 saja + NTFS tetap + baca/tulis andal** semuanya wajib, belum ada
implementasi yang dapat saya jamin dari stack sekarang. Tahap riset bisa berakhir
tidak layak; tentukan ini sebelum membeli aksesori atau memindahkan data.
Perubahan filesystem tidak termasuk pekerjaan sekarang. Tidak ada tombol format
atau penulisan disk dalam prototipe.
[Perbandingan batas filesystem Microsoft](https://learn.microsoft.com/windows/win32/fileio/filesystem-functionality-comparison).

## 3. Hardware dan alur lokal

```text
Adaptor 5 V yang sesuai spin-up ── proteksi/saklar VBUS ── HDD dalam casing USB
                  └────────────── suplai board ESP32-S3
ESP32 USB Host (D-/D+, GPIO19/20) ──────────────────────── USB data HDD
                 │
                 ├─ USB MSC/BOT → block reads → filesystem → bounded stream
                 ├─ Wi-Fi 2,4 GHz → HTTP API/file service LAN
                 ├─ state/UI → ST7789 240×240
                 └─ BLE → companion Windows untuk musik (fase berikutnya)
```

Diagram bersifat konseptual. Jangan menjumper suplai 5 V dua sumber atau memakai
kabel Y yang backfeed ke PC. Pilih interface host berproteksi; powered hub hanya
opsi setelah dukungan hub dan bridge diuji. Rating adaptor ditentukan setelah
label HDD, spesifikasi casing, serta arus spin-up diketahui; kapasitas 500 GB tidak
cukup untuk menebak arus. Board DevKit biasa belum tentu menyediakan VBUS host.
Jangan hubungkan HDD ke USB native sambil menganggap konektor itu otomatis host.

Firmware M1 tetap memakai USB console sekarang. Peralihan UART dan pin debug
dilakukan dalam tahap NAS setelah model board jelas, tanpa eFuse permanen.

## 4. Relay cloud itu apa? Apakah Vercel bisa?

Relay adalah perantara internet. ESP32 di rumah membuka koneksi WSS (WebSocket
dengan TLS) ke relay, jadi tidak memerlukan koneksi masuk langsung ke IP rumah.
Browser login ke web, lalu permintaan yang sah dikirim lewat koneksi perangkat.

```text
Browser HP/laptop ── HTTPS/login ── Web dashboard (misalnya Vercel)
       │
       └── permintaan/transfer ── Relay cloud yang memverifikasi pemilik
                                         ⇅ WSS keluar dari rumah
                                    ESP32 → USB → HDD
```

Saat unduh: permintaan ID file → otorisasi → ESP membaca potongan file → relay →
browser. ID transfer, offset, ukuran potongan, checksum, batas antrean dan ACK
diperlukan; koneksi putus bisa dilanjutkan tanpa menggandakan operasi.
ESP32 hanya membaca chunk berikutnya saat penerima siap (backpressure).
Tidak ada nilai chunk atau klaim throughput final sebelum benchmark RAM/TLS.

**Vercel bisa dipakai.** Per dokumentasi yang diperiksa 11 Oktober 2026,
WebSockets tersedia **beta** pada Vercel Functions. Koneksi berakhir ketika
Function mencapai durasi maksimum; koneksi baru bisa masuk instance berbeda.
Karena itu relay perlu koordinasi eksternal antarfungsi (presence/routing/pub-sub),
bukan sekadar menyimpan socket ESP32 dalam variabel global dan berharap socket
browser masuk proses yang sama. Biaya runtime dan transfer juga perlu diukur.

- Opsi A: Vercel untuk UI/login, relay terpisah yang mendukung koneksi persisten.
  Kandidat: Cloudflare Worker + Durable Object untuk routing per perangkat atau
  service kecil persisten. Ini layanan cloud, bukan server tambahan di rumah.
- Opsi B: Vercel untuk UI dan relay beta, plus koordinasi eksternal. Lakukan uji
  timeout, multi-instance, reconnect dan transfer besar sebelum memilihnya.
- Opsi C: tanpa relay bila router punya VPN server yang cocok dan jaringan bisa
  dijangkau. Ini tetap memerlukan dukungan router/koneksi; CGNAT dapat menghalangi
  VPN inbound. Cloud relay lebih fleksibel untuk koneksi rumah biasa.

Hosting halaman statis di Vercel saja tidak membuka HDD. Vercel juga tidak
menghilangkan masalah NTFS atau batas USB ESP32. Browser jarak jauh tidak dapat
langsung menjangkau IP lokal rumah hanya karena UI-nya online.

Relay dapat meneruskan data tanpa menyimpan file permanen. Namun TLS per koneksi
**bukan enkripsi end-to-end**: operator relay berpotensi melihat isi transfer.
Jangan menyebutnya E2EE sebelum menambahkan enkripsi pada level aplikasi.
Quota/bandwidth/biaya cloud belum dihitung dan belum ada layanan berbayar diaktifkan.

Referensi: [WebSockets Vercel](https://vercel.com/docs/functions/websockets),
[batas Vercel Functions](https://vercel.com/docs/functions/limitations),
[Cloudflare Durable Objects/WebSockets](https://developers.cloudflare.com/durable-objects/best-practices/websockets/).

## 5. Kontrak fitur web yang diusulkan

| Area | Prototipe sekarang | Implementasi setelah disetujui |
| --- | --- | --- |
| Penyimpanan | Navigasi/filter file contoh, preview/unduh teks contoh | List/read/download HDD dengan pagination dan batas akses |
| Musik & layar | Render firmware 240×240, mapping kontrol | Status media/BLE; setting yang didukung firmware |
| Konfigurasi | Nama, orientasi, volume step, reconnect, storage, Wi-Fi label, rute remote | Validasi backend, simpan NVS, ACK dan state aktual |
| Akses remote | Diagram dan pilihan draf | Pairing perangkat, login pengguna, token terpisah, status offline |
| Keamanan | Tanpa input password/token dan tanpa koneksi HDD | TLS tervalidasi, session expiry, otorisasi per perangkat/file, rate limits |

Simpan draf di browser dan ekspor JSON **tidak menerapkan konfigurasi**. UI demo
tidak memanggil LAN atau relay. Login Sites yang melindungi preview bukan login NAS.
Browser-local state bukan database produksi dan tidak menyimpan rahasia.

Pada backend nanti: path traversal, symlink/reparse/volume escape harus ditolak;
operasi tidak boleh melewati root disk yang dipilih. Mulai satu transfer read-only.
Tahap write membutuhkan upload staging, pemeriksaan checksum, batas ruang kosong,
penanganan disk penuh/cabut mendadak dan flush sebelum unmount. Jurnal NTFS bukan
jaminan aman jika driver port-nya belum benar. Satu HDD bekas juga bukan backup.

## 6. Urutan implementasi & gerbang penerimaan

1. **N0 — inspeksi non-destruktif:** model board/casing/HDD, daya, USB BOT, partisi,
   ukuran sektor. Jangan auto-format bila mount gagal.
2. **N1 — media uji terpisah:** USB read-only → daftar file → unduh LAN. Verifikasi
   checksum file kecil/besar, teks UTF-8, long filename, cabut-pasang dan reboot.
3. **N2 — keputusan HDD utama:** NTFS port read-only harus lolos fixture dan media
   uji dulu, atau migrasi filesystem hanya setelah backup terverifikasi dan izin
   eksplisit. Bila keduanya tidak diterima, hentikan jalur NAS pada gerbang ini.
4. **N3 — integrasi dashboard lokal:** autentikasi, setting dengan ACK, status
   connected/offline/error, limit satu transfer; uji LCD/input/BLE saat transfer.
5. **N4 — relay:** uji isolasi dua akun/perangkat, expired token, reconnect,
   batas durasi provider, checksum/resume, sertifikat TLS dan biaya transfer.
6. **N5 — write (opsional kemudian):** setelah read-only stabil; uji disk penuh,
   listrik mati, cancel, restart, file conflict, flush/unmount. Bukan default awal.

### Usulan keputusan untuk Anda

Setujui **N0–N1 terlebih dahulu**, dengan media uji FAT32 dan HDD NTFS tetap utuh.
Vercel boleh menjadi target UI; pilih relay final setelah uji transport kecil.
Ini belum persetujuan tahap write/migrasi dan bukan janji NTFS akan berhasil.
Jika NTFS wajib dipertahankan, catat itu sebagai gerbang wajib N2.
