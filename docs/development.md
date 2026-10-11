# Development

Fase terkini: **M2 input + N0 USB diagnostics**. Mulai dari
[panduan build dan uji M2/N0](m2-n0-test.md). Profil normal menggunakan native USB;
profil USB Host menggunakan COM/UART. Bagian M1 di bawah dipertahankan untuk
verifikasi layar; startup kini berjudul `Terra - M2 input diagnostics, LCD 240x240`.

## Toolchain

Firmware dikunci pada ESP-IDF **6.0.3**, target `esp32s3`, LVGL **9.5.0**, dan
`esp_lvgl_port` **2.9.0**. Component Manager menghasilkan `dependencies.lock`;
simpan lockfile tersebut dalam Git. Jangan edit managed_components secara manual.

Gunakan [Espressif Installation Manager](https://docs.espressif.com/projects/idf-im-ui/en/latest/headless_usage.html)
untuk instalasi ESP-IDF. Contoh PowerShell setelah EIM tersedia:

```powershell
eim install -i v6.0.3 -t esp32s3
```

Jalankan activation script yang dilaporkan installer sebelum `idf.py`.
Panduan resmi [Windows setup](https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/get-started/windows-setup-update.html).
Untuk checkout saat ini, alat tambahan disimpan dalam `.tools/` (diabaikan Git);
`. .\scripts\activate-local.ps1` mengaktifkannya untuk sesi PowerShell sekarang.
Script membatasi `IDF_PY_BUILD_JOBS=4` bila belum disetel. Pada build awal,
kompilasi paralel default pernah memicu internal compiler error di driver RGB
bawaan ESP-IDF; file yang sama berhasil dikompilasi dengan empat proses tanpa
perubahan source. Penyebab crash belum dipastikan. Batas ini tidak mengubah
konfigurasi firmware atau fitur runtime.

Companion: Rust stable dengan target Windows. Pada komputer pengembangan biasa,
gunakan MSVC + Visual Studio Build Tools C++. Validasi lokal awal juga dapat
memakai GNU target + GCC/MinGW-w64; crate saat ini tidak memanggil Win32.
Validasi integrasi Windows nantinya tetap harus menggunakan API/hardware nyata.

Catatan toolchain lokal: Rust 1.98.1 GNU bersama w64devkit 2.10.0 gagal link
`-lgcc_eh` pada deteksi runtime otomatis. Library itu tersedia dalam distribusi
Rust. Validasi berhasil dengan `-C link-self-contained=yes`, sesuai
[opsi resmi rustc](https://doc.rust-lang.org/rustc/codegen-options/#link-self-contained).
Script aktivasi menerapkannya khusus target GNU lokal; build MSVC tidak diubah.

## Build dan cek

```powershell
cd firmware
idf.py set-target esp32s3
idf.py build
idf.py size
cd ../companion
cargo build --workspace --locked
cargo run -p companion-cli -- --help
cargo run -p companion-cli -- --version
```

Perubahan `sdkconfig.defaults` tidak menimpa nilai dalam `sdkconfig` yang sudah
ada. Gunakan `idf.py set-target esp32s3` untuk regenerasi (konfigurasi lama menjadi
sdkconfig.old), kemudian build ulang. Gunakan `menuconfig` untuk eksperimen lokal.

Tes C host untuk pemformat waktu, dari root dengan Clang/GCC tersedia:

```powershell
clang -std=c11 -Wall -Wextra -Werror -I firmware/main firmware/tests/test_time_utils.c firmware/main/util/time_utils.c -o .tools/test-time.exe
.\.tools\test-time.exe
```

CLI menerima `--help` dan `--version`. `media`, `devices`, `connect`, dan `monitor`
sengaja menghasilkan exit code 2 pada M0/M1 agar tidak mengesankan fitur selesai.

## Preview UI di komputer

`firmware/tests/preview` menggunakan **ui.c yang sama** dengan firmware dan
LVGL 9.5.0. Ini renderer tanpa jendela yang menghasilkan gambar PPM 240×240,
bukan simulator koneksi, driver panel, PSRAM, atau timing SPI. Jalankan sesudah
Component Manager mengunduh dependency firmware, dengan GCC/G++ dan Ninja tersedia:

```powershell
cmake -S firmware/tests/preview -B .tools/preview-build -G Ninja -DCMAKE_C_COMPILER=gcc -DCMAKE_CXX_COMPILER=g++ -DCMAKE_BUILD_TYPE=Release
cmake --build .tools/preview-build -j 6
.\.tools\preview-build\ui-preview.exe .tools/m1-preview.ppm
.\.tools\preview-build\ui-preview.exe .tools/long-text.ppm long-text
.\.tools\preview-build\ui-preview.exe .tools/zero-duration.ppm zero-duration
```

PPM dapat dibuka dengan viewer yang mendukungnya atau dikonversi ke PNG. Warna
hasil render tidak membuktikan RGB order/inversion panel fisik sudah cocok.
Harness membaca dimensi dari `board_config.h` dan gagal bila widget keluar dari
batas layar. Skenario tambahan memeriksa teks maksimum, muted/paused, dan durasi nol.

## Flash dan uji M1

1. Cocokkan wiring dan nilai di [hardware.md](hardware.md).
2. Sambungkan USB **native** dengan kabel data. Tentukan port:

```powershell
[System.IO.Ports.SerialPort]::GetPortNames()
```

3. Dari folder firmware, ganti `COMx` dengan port yang ditemukan:

```powershell
idf.py -p COMx flash monitor
```

Jika auto-download gagal, tahan BOOT, tekan-lepas RESET, lalu lepas BOOT dan ulangi.
Keluar monitor dengan `Ctrl+]`. Tidak ada perintah flash otomatis saat build.

Keluaran yang diharapkan (timestamp dan pesan driver dapat berbeda):

```text
I (APP) Terra 0.1.0 - M1 display bring-up
I (APP) PSRAM detected: 8388608 bytes
I (DISPLAY) ST7789 initialized (240x240, gap 0/0)
I (LVGL) Display registered
I (APP) Mock screen ready
```

Layar harus menunjukkan `DEMO`, status `OFF`, `Test Song`, `Test Artist`,
progress sepertiga, `01:00 / 03:00`, `Playing`, dan `VOL 72`. Posisi tetap selama
setidaknya 60 detik. Periksa semua teks terbaca, tidak terpotong, tidak berkedip,
background gelap, teks terang, dan bar biru muda. Restart board dan ulangi.

Laporkan log reset lengkap, foto layar, model board/LCD, serta perubahan konfigurasi.
PSRAM gagal berarti konfigurasi board perlu diperiksa dahulu. Layar kosong dengan
log sukses belum membuktikan driver cocok: cek suplai, backlight, CS/DC, gap,
inversion, dan orientasi. Turunkan SPI clock bila gambar rusak/intermiten.

## Referensi API

- [esp_lvgl_port 2.9.0](https://components.espressif.com/components/espressif/esp_lvgl_port/versions/2.9.0/readme)
- [LVGL 9.5 / ESP-IDF](https://lvgl.io/docs/open/9.5/integration/chip_vendors/espressif/add_lvgl_to_esp32_idf_project.html)
- API driver yang dipakai diverifikasi lagi terhadap header ESP-IDF hasil instalasi.
