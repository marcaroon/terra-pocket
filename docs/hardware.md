# Wiring dan asumsi hardware M2 / N0

Target: **ESP32-S3 N16R8, board bergaya YD-ESP32-S3 bertanda 2022-V1.3**,
dan breakout **ST7789 240×240**. Foto memperlihatkan tulisan `YD-ESP32-23`,
port COM dan USB, serta pad IN-OUT/USB-OTG. Keaslian dan jalur listrik belum diukur.
Controller/resolusi LCD dikonfirmasi pengguna pada 11 Oktober 2026; modul 7 pin
(GND, VCC, SCL, SDA, RES, DC, BLK) tanpa CS. Merek, ukuran
diagonal dan skema breakout LCD belum dikonfirmasi.
Seluruh pin dan parameter panel ada di `firmware/main/board_config.h`.
Daftar kabel, mode daya, dan urutan perakitan lengkap: [wiring.md](wiring.md).

## Wiring LCD

Putuskan daya saat memasang kabel. Semua sinyal menggunakan logika **3,3 V**.

| Pin LCD | ESP32-S3 | Catatan |
| --- | --- | --- |
| VCC | 3V3 | Asumsi breakout mendukung suplai 3,3 V; cek datasheet modul |
| GND | GND | Ground bersama |
| SCL / SCK / CLK | GPIO12 | SPI clock, bukan I2C |
| SDA / DIN / MOSI | GPIO11 | SPI data masuk LCD |
| CS | — | Tidak ada pada modul 7 pin pengguna; `BOARD_LCD_CS` = `GPIO_NUM_NC` |
| DC / A0 | GPIO9 | Data/command |
| RES / RST | GPIO8 | Reset |
| BL / BLK | GPIO7 | **Hanya jika ini input enable berimpedansi tinggi** |

GPIO7 bukan sumber arus LED. Jika BL terhubung langsung ke LED, gunakan driver
transistor/resistor sesuai spesifikasi modul, dengan GPIO7 sebagai enable.
Jangan menghubungkan beban backlight langsung tanpa mengetahui rangkaian breakout.
Power board melalui USB-C; baterai tidak termasuk V1.

## Parameter panel yang harus diuji

| Parameter | Nilai awal | Jika tidak cocok |
| --- | --- | --- |
| Resolusi | 240×240 | Dikonfirmasi pengguna |
| Controller RAM | Asumsi area 240×320 | Area aktif modul 240×240, posisi bergantung modul |
| Gap X/Y | 0/0 | Ubah bila gambar bergeser/terpotong; beberapa orientasi/modul perlu Y=80 |
| SPI | SPI2, mode 3, 20 MHz | Mode 3 untuk modul tanpa CS; coba 0 bila layar tetap hitam. Turunkan frekuensi bila kabel menimbulkan gangguan |
| Format | RGB565, RGB order | Ubah RGB/BGR jika kanal merah-biru tertukar |
| Byte swap | true | ST7789 menerima byte tinggi terlebih dahulu |
| Color inversion | true | Ubah bila warna tampak negatif |
| Swap XY / mirror X / mirror Y | false / false / false | Sesuaikan orientasi pemasangan |
| Backlight enable | active high | Balik level bila driver active low |

Offset (0,0), inversi warna aktif, dan SPI mode 3 terverifikasi visual pada modul
pengguna (11 Oktober 2026): warna normal, tepi tidak terpotong. Untuk modul lain
offset (0,0) tetap **asumsi kalibrasi**, bukan fakta untuk semua modul 240×240.
Perubahan resolusi tidak cukup untuk menentukan offset tanpa datasheet modul.
Font bawaan Montserrat pada M1 cukup untuk fixture ASCII; dukungan glyph Unicode
metadata lintas bahasa harus ditentukan saat media nyata masuk.

## Memori dan USB

Flash **Quad 16 MB**, QIO 80 MHz; PSRAM **Octal 8 MB**, 80 MHz. Konfigurasi
terpusat dalam `sdkconfig.defaults` karena merupakan parameter build ESP-IDF.
PSRAM memory test diaktifkan; kegagalan tidak disembunyikan dengan fallback.

Hindari GPIO35–37 untuk Octal memory; GPIO0/3/45/46 untuk strapping; GPIO19/20
disisakan untuk USB native. Console menggunakan **USB Serial/JTAG native**.
Port USB-to-UART terpisah pada beberapa board tidak menampilkan console default
ini; pilih konektor USB native atau ubah console via `idf.py menuconfig`.

Permintaan NAS baru akan memakai USB Host dan GPIO19/20 yang sama. USB Host dan
Serial/JTAG berbagi PHY internal; rencana NAS perlu console UART/debug terpisah.
Build normal M2 memakai console USB native. Profil `usb-probe` memakai UART0
default (TX43/RX44, 115200) melalui bridge COM; console native sekunder dinonaktifkan.
Kode menolak build probe dengan console yang masih memakai PHY USB native.

Pada foto depan pengguna (konektor di kanan): COM di kanan atas untuk flash/log;
USB native di kanan bawah dekat 5V/LED RGB untuk data HDD. Ikuti label PCB jika
board diputar. Tidak ada kode yang mengubah pad solder, eFuse, atau suplai VBUS.

## Wiring input M2

| Kontrol | GPIO | Sambungan |
| --- | --- | --- |
| Previous | 4 | Sakelar antara GPIO dan GND |
| Play/Pause | 5 | Sakelar antara GPIO dan GND |
| Next | 6 | Sakelar antara GPIO dan GND |
| Encoder A / CLK | 15 | Kontak A, pull-up internal |
| Encoder B / DT | 16 | Kontak B, pull-up internal |
| Encoder common | GND | Common kedua kontak |
| Encoder push / SW | 17 | Sakelar antara GPIO dan GND |

Semua input active-low, pull-up internal 3,3 V. Jika modul encoder memiliki VCC,
gunakan 3V3, bukan 5 V. Encoder pengguna terukur **half-step** (modul bergaya
KY-040 dengan pull-up): detent di AB=11 dan AB=00, dua transisi per klik.
`BOARD_ENCODER_HALF_STEP` memilih decoder; set `false` untuk EC11 full-step.
Debounce sakelar 40 ms; polling encoder 1 ms; langkah volume 2. Arah bisa dibalik
dengan `BOARD_ENCODER_DIRECTION`. Tahan tombol tidak mengulang event; tombol yang
ditahan saat boot harus dilepas sebelum penekanan berikutnya dihitung.

## HDD dan suplai

- Casing **ORICO 2020U3**, Micro-B USB 3.0 ke kabel USB-A.
- HDD **WD Blue WD5000LPCX**, 500 GB, NTFS; label **5 V DC 0,55 A**.
- [Datasheet WD halaman 3](https://documents.westerndigital.com/content/dam/doc-library/en_ie/assets/public/western-digital/product/internal-drives/wd-blue-hdd/specification-sheet-wd-blue-mobile-sata-hdd.pdf)
  memberi arus **puncak 1 A**, belum termasuk bridge casing dan board.
- Chip NS1068X disebut oleh listing pengguna, belum diverifikasi pada casing fisik.
  [Brosur chip](https://www.norelsys.com/assets/resource/preview/pdf/694e3452644d70664668455a5670413937676a4363413d3d/NS1068_NS1068X_product_brochure_V1%202.pdf)
  menyatakan dukungan Full Speed/BOT; ini bukan hasil uji casing.

Rancangan awal: adaptor teratur **5 V 3 A** dibagi sebelum board ke suplai ESP32/LCD
dan VBUS HDD berproteksi. Angka 3 A adalah margin desain, belum hasil pengukuran.
Jangan melewatkan seluruh arus HDD melalui board tanpa verifikasi jalur. USB data
D-/D+ tetap ke native USB, ground bersama. Rangkaian harus mencegah suplai HDD
backfeed ke PC/port lain. Jangan solder pad USB-OTG/IN-OUT berdasarkan foto saja.
Powered hub hanya opsi setelah kompatibilitas hub/kecepatan/arus port diverifikasi.
Adaptor pasif USB-A female ke USB-C male tidak menyediakan suplai tambahan.

**Wiring VBUS belum diterima secara hardware.** Uji voltase di casing saat spin-up
dan kestabilan ESP32 dahulu. Prosedur firmware N0 ada di [m2-n0-test.md](m2-n0-test.md).

## Hasil yang perlu dilaporkan

Model/revisi board dan LCD, wiring BL, foto layar, serta log sejak reset hingga
`Mock screen ready`. Catat apakah tepi terpotong, orientasi terbalik, atau warna
tidak sesuai. Prosedur flash dan keluaran yang diharapkan ada di development.md.
