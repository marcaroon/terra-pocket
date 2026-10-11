# Uji M2 input dan N0 USB

Scope: M2 menghasilkan event serial saja. N0 membaca metadata sektor saja.
Belum ada BLE, perubahan volume Windows, mount NTFS, server file LAN, atau relay.
Layar tetap fixture DEMO/OFF 240×240; menekan tombol tidak memalsukan state Windows.

## Build tanpa flash

Dari root proyek pada PowerShell:

```powershell
.\scripts\test-host.ps1
.\scripts\build-firmware.ps1 -Profile normal
.\scripts\build-firmware.ps1 -Profile usb-probe
```

Build terpisah: `firmware/build` dan `firmware/build-usb-probe`, masing-masing
memiliki sdkconfig sendiri. Profil USB menonaktifkan console native dan memakai
UART0 default; build normal tetap native USB. Script tidak mem-flash hardware.
Konfigurasi profil baru dibuat dari defaults. Jika file sdkconfig profil telah
diubah manual, defaults tidak menimpanya; periksa menuconfig dan hasil konfigurasi.

Linux/CI: dari `firmware`, untuk build probe yang pertama:

```sh
idf.py -B build-usb-probe -D SDKCONFIG=sdkconfig.usb-probe -D 'SDKCONFIG_DEFAULTS=sdkconfig.defaults;sdkconfig.usb-probe.defaults' build
```

## M2 — input

1. HDD dilepas. Rangkai tombol dan encoder mengikuti hardware.md.
2. Build `normal`. Sambungkan kabel data ke port USB native untuk flash dan log.
3. Cari port, lalu ganti `COMx` berikut dengan port aktual:

```powershell
. .\scripts\activate-local.ps1
[System.IO.Ports.SerialPort]::GetPortNames()
idf.py -C firmware -B firmware/build -p COMx flash monitor
```

Keluaran startup yang diharapkan:

```text
Terra - M2 input diagnostics, LCD 240x240
PSRAM detected: 8388608 bytes
ST7789 initialized (240x240, gap 0/0)
Mock screen ready
Main task stack free: ... bytes
Ready: active-low buttons, debounce 40 ms, encoder half-step
USB HDD probe disabled (normal input build)
Input events are diagnostic only; Windows/BLE not connected
```

| Tindakan | Log yang diharapkan |
| --- | --- |
| Previous sekali | PREVIOUS sekali |
| Play/Pause sekali | PLAY_PAUSE sekali |
| Next sekali | NEXT sekali |
| Encoder satu detent CW | VOLUME +2 sekali |
| Encoder satu detent CCW | VOLUME -2 sekali |
| Push encoder sekali | MUTE sekali |

Ulangi masing-masing tombol 20 kali, lalu tahan 3 detik: tetap satu event per
penekanan, tanpa repeat. Putar 20 detent per arah perlahan, lalu pada kecepatan
pemakaian normal; cocokkan jumlah dan arah. Balik `BOARD_ENCODER_DIRECTION` jika
arah mekanis berlawanan. Boot sambil menahan tombol tidak boleh mengirim event.
Tekan dua tombol bersamaan; keduanya harus tercatat. Layar tetap fixture statis.

Log `Queue full` berarti event dibuang dan pengujian gagal; laporkan jumlahnya.
Polling 1 ms bisa kehilangan transisi putaran sangat cepat; bukan klaim batas
kecepatan hardware. Full-step/half-step harus dicocokkan dengan encoder aktual.

## N0 — diagnostik disk, setelah daya diverifikasi

**Jangan menghubungkan HDD sebelum jalur VBUS dan suplai pada hardware.md benar.**
Tidak ada instruksi jumper solder otomatis atau asumsi board memasok arus HDD.

1. Build `usb-probe` dari perintah di atas, HDD masih dilepas.
2. Pindahkan kabel PC ke **COM/UART** (foto depan: kanan atas). Cari COM bridge.
3. Gunakan profil yang sama untuk flash/monitor:

```powershell
idf.py -C firmware -B firmware/build-usb-probe -D "SDKCONFIG=$PWD/firmware/sdkconfig.usb-probe" -D "SDKCONFIG_DEFAULTS=$PWD/firmware/sdkconfig.defaults;$PWD/firmware/sdkconfig.usb-probe.defaults" -p COMx flash monitor
```

4. Tunggu log `Read-only sector probe enabled; UART console; verify external HDD power`.
5. Setelah suplai HDD terverifikasi, sambungkan data casing ORICO ke **USB native**
   (foto depan: kanan bawah). PC tetap pada COM. Jangan memakai dua sumber 5 V
   yang saling tersambung tanpa isolasi daya yang sesuai.

Keluaran berikut adalah pola, bukan log hasil pengujian perangkat pengguna:

```text
MSC connected: address ...
VID=.... PID=.... sector=512 count=... bytes=...
MBR: ... non-empty primary entries
Partition type=07 start=... sectors=...
Volume LBA ...: NTFS signature (not mounted)
Probe finished: no filesystem mounted, no disk writes requested
```

Jika disk GPT: `Protective MBR detected; GPT traversal deferred, no mount` adalah
hasil yang diharapkan pada N0 ini. Disk tanpa partisi dengan signature NTFS di LBA0
juga dikenali. Signature saja tidak membuktikan NTFS sehat atau file dapat dibaca.
Tipe partisi 07 sendiri tidak dianggap NTFS. Extended/EBR dan GPT belum ditelusuri.

Pemeriksaan dibatasi LBA0 dan sektor awal maksimal empat entri MBR. Sektor harus
512/1024/2048/4096 byte; rentang partisi harus berada di dalam disk sebelum dibaca.
Kapasitas memakai perhitungan 64-bit. Driver ini memakai READ CAPACITY(10)/READ(10),
bukan dukungan umum disk >2 TiB. Target sekarang disk pengguna 500 GB.

Tidak memanggil VFS/mount, format, repair, write-sector, upload, atau penghapusan.
Tidak mencetak isi file/serial HDD, hanya identitas USB dan metadata kapasitas/partisi.
N0 bukan write blocker hardware: jaminan di sini sebatas jalur kode yang tidak
mengirim operasi tulis disk, belum dibuktikan dengan USB analyzer/perangkat fisik.

Uji cabut-pasang saat idle 5 kali: log `Disconnected; waiting for next device`
dan deteksi baru. Read error menghentikan pembacaan saat itu; reconnect untuk
mengulangi. Queue overflow menghentikan probe sampai restart, mencegah pemakaian
handle yang kehilangan event lifecycle. Device kedua diabaikan; hanya satu disk.
Saat driver gagal init/uninstall, firmware berhenti melalui pemeriksaan error.

Laporkan log startup sampai hasil probe, VID/PID, ukuran sektor/kapasitas, layout,
tegangan saat spin-up, apakah ESP32 reset, serta hasil tombol selama HDD terhubung.
Tidak perlu mengirim dump sektor atau daftar file pribadi.

## Gerbang berikutnya

Setelah hasil hardware M2/N0 tersedia: N1 media uji FAT32 read-only dan LAN,
atau eksperimen driver NTFS read-only jika mempertahankan NTFS menjadi syarat.
M3 BLE mengikuti blueprint setelah M2 diterima. Cloudflare Free adalah kandidat
relay, bukan layanan yang sudah berjalan; kuota dan autentikasi diuji belakangan.
