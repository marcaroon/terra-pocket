# Development

Fase terkini: **M3 BLE GATT + M4 protokol** di atas M2 input dan N0 diagnostik USB.
Wire format kanonik: [ble-protocol.md](ble-protocol.md). Uji M2/N0 tetap di
[m2-n0-test.md](m2-n0-test.md). Profil `normal` memakai console USB native;
profil `usb-probe` memakai COM/UART. Keduanya berisi BLE.

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

Dari root, setelah `. .\scripts\activate-local.ps1`:

```powershell
.\scripts\test-host.ps1                      # tes C host + 57 vector protokol bersama
.\scripts\build-firmware.ps1 -Profile normal
.\scripts\build-firmware.ps1 -Profile usb-probe
cd companion
cargo build --workspace --locked
cargo test --workspace --locked               # codec Rust pada vector yang sama
```

Perubahan `sdkconfig.defaults` tidak menimpa nilai dalam `sdkconfig` yang sudah
ada. `firmware/sdkconfig` dan `firmware/sdkconfig.usb-probe` adalah file turunan
yang diabaikan Git: hapus keduanya agar dibuat ulang dari defaults (wajib setelah
M3 menambahkan BLE). Gunakan `menuconfig` hanya untuk eksperimen lokal.
`build-firmware.ps1` aman di-redirect (`| Tee-Object log.txt`); kegagalan dicek
lewat exit code idf.py.

Vector protokol dibuat ulang dengan `python scripts/gen-protocol-vectors.py
docs/protocol-test-vectors`; CI membandingkan hasilnya dengan isi repo.

CLI: `terra-cli media` mencetak sesi media Windows saat ini (exit 0, termasuk
"No active media session"; exit 1 bila GSMTC tidak tersedia). `terra-cli media --watch`
mencetak satu baris per perubahan lagu/playback/seek sampai Ctrl+C. `devices`,
`connect`, dan `monitor` sengaja exit 2 sampai M6 agar tidak mengesankan fitur selesai.

## Preview UI di komputer

`firmware/tests/preview` menggunakan **ui.c yang sama** dengan firmware dan
LVGL 9.5.0. Ini renderer tanpa jendela yang menghasilkan gambar PPM 240×240,
bukan simulator koneksi, driver panel, PSRAM, atau timing SPI. Jalankan sesudah
build firmware (butuh `firmware/build/config`), dengan GCC/G++ dan Ninja tersedia:

```powershell
cmake -S firmware/tests/preview -B .tools/preview-build -G Ninja -DCMAKE_C_COMPILER=gcc -DCMAKE_CXX_COMPILER=g++ -DCMAKE_BUILD_TYPE=Release
cmake --build .tools/preview-build -j 6
.\.tools\preview-build\ui-preview.exe .tools/preview-playing.ppm
.\.tools\preview-build\ui-preview.exe .tools/preview-waiting.ppm waiting
```

Skenario: tanpa argumen (playing), `long-text`, `zero-duration`, `waiting`,
`no-media`, `mismatch`. Harness membaca dimensi dari `board_config.h` dan gagal
bila widget keluar dari batas layar. Folder build preview menyimpan path absolut;
hapus `.tools/preview-build` bila folder proyek dipindah atau diganti nama.
Warna render tidak membuktikan RGB order/inversion panel fisik.

![Layar M3/M4 dari renderer host](m4-screens.png)

## Flash dan uji M3/M4

1. Wiring sesuai [wiring.md](wiring.md). HDD/USB-A tidak diperlukan.
2. Flash. Build `normal` dapat di-flash lewat port COM maupun USB native; log
   aplikasinya keluar di USB native. Ganti `COMx` dengan port aktual:

```powershell
idf.py -C firmware -B firmware/build -p COMx flash
```

3. Layar harus menampilkan `BLUETOOTH`, `Waiting for computer...`, `Terra`, status `OFF`.
4. Siapkan harness BLE sekali (Python 3.12, Bluetooth laptop aktif):

```powershell
python -m venv .tools/ble-test-venv
.\.tools\ble-test-venv\Scripts\python -m pip install -r scripts/ble-test-requirements.txt
```

5. Jalankan uji penerimaan; `--mtu 23` memaksa fragmentasi kecil:

```powershell
.\.tools\ble-test-venv\Scripts\python scripts/ble-test.py scan
.\.tools\ble-test-venv\Scripts\python scripts/ble-test.py test --hold 20
.\.tools\ble-test-venv\Scripts\python scripts/ble-test.py --mtu 23 test
```

Keluaran yang diharapkan diakhiri `ALL CHECKS PASSED`, meliputi discover, connect,
service dan tiga karakteristik, HELLO → HELLO_ACK → READY, tulis RX, ERROR untuk
payload rusak/versi asing/tipe salah, perangkat tetap READY, dan MEDIA_STATE
maksimum 361 byte direassembly. Selama `--hold`, layar menampilkan `SPOTIFY`,
`The Less I Know The Better`, `Tame Impala`, `VOL 72`, timeline berjalan; separuh
waktu kemudian `Paused`, `02:00 / 03:36`, `MUTED`. Setelah disconnect layar kembali
ke `Waiting for computer...`.

Log aplikasi (USB native atau UART pada `usb-probe`) yang diharapkan saat uji:

```text
I (BLE) Advertising as Terra
I (BLE) Client connected
I (BLE) MTU 247
I (PROTOCOL) HELLO protocol=1 firmware=0.2.0
I (PROTOCOL) HELLO_ACK protocol=1 companion=0.1.0
I (STATE) Volume 72
I (STATE) Track changed: The Less I Know The Better - Tame Impala (Spotify)
W (PROTOCOL) Rejected type 0x10 seq 3 (error 2)
I (BLE) Client disconnected (reason=0x...)
```

`send` mengirim metadata pilihan sendiri, misalnya
`scripts/ble-test.py send --title "Judul" --artist "Artis" --paused --hold 60`.

## Referensi API

- [esp_lvgl_port 2.9.0](https://components.espressif.com/components/espressif/esp_lvgl_port/versions/2.9.0/readme)
- [LVGL 9.5 / ESP-IDF](https://lvgl.io/docs/open/9.5/integration/chip_vendors/espressif/add_lvgl_to_esp32_idf_project.html)
- API driver yang dipakai diverifikasi lagi terhadap header ESP-IDF hasil instalasi.
