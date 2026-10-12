# Terra BLE protocol V1

Dokumen ini adalah **spesifikasi kanonik wire format V1**. Ia memperinci
PROJECT_SPEC.md bagian 11–17 dan menutup keputusan yang dulu terbuka (sequence,
reassembly, ACK, ERROR, HELLO/HELLO_ACK, DEVICE_STATUS). Firmware C
(`firmware/main/protocol/`), crate Rust `protocol`, dan referensi Python
(`scripts/terra_protocol.py`) wajib mengikuti dokumen ini dan lulus
[test vector bersama](protocol-test-vectors/README.md).

Semua bilangan multi-byte **little endian**. Semua panjang dihitung dalam byte.

## 1. GATT

| Item | UUID | Properti |
| --- | --- | --- |
| Service | `6F3A0001-9B21-4C71-9A8B-31A8B9F10001` | — |
| RX (PC → ESP32) | `6F3A0002-9B21-4C71-9A8B-31A8B9F10001` | write, write without response |
| TX (ESP32 → PC) | `6F3A0003-9B21-4C71-9A8B-31A8B9F10001` | notify |
| DEVICE STATUS | `6F3A0004-9B21-4C71-9A8B-31A8B9F10001` | read, notify |

Nama advertising `Terra`; UUID service ada di scan response. Satu koneksi
central dalam satu waktu. Tidak ada pairing/bonding di V1; karakteristik tidak
memerlukan enkripsi. Firmware meminta ATT MTU 247.

RX dan TX membawa **paket** (§2). DEVICE STATUS berisi payload DEVICE_STATUS
mentah (§4.6) **tanpa** header paket.

## 2. Paket dan fragmentasi

```text
byte 0  version   = 1
byte 1  type      (tabel §3)
byte 2  flags     START 0x01, END 0x02, ACK_REQUEST 0x04
byte 3  sequence  u8
byte 4+ potongan payload pesan
```

- Panjang paket maksimum = ATT MTU − 3 (MTU minimum 23 → 20 byte, yaitu 16 byte
  payload per paket). Panjang paket minimum 4 byte.
- Bit flag selain 0x07 harus 0; paket dengan bit lain ditolak.
- **ACK_REQUEST dicadangkan di V1**: pengirim tidak boleh men-set-nya, penerima
  mengabaikannya. Tidak ada pesan ACK di V1.
- **Sequence** adalah penghitung pesan per arah milik pengirim, mulai 0, naik satu
  per pesan dan kembali ke 0 setelah 255. Semua fragmen satu pesan memakai
  sequence dan type yang sama. Penerima tidak mewajibkan sequence berurutan antar
  pesan; sequence dipakai untuk mengikat fragmen dan untuk ERROR.
- Pesan muat satu paket: flags `START|END`. Pesan terfragmentasi: paket pertama
  `START`, tengah `0`, terakhir `END`. Payload kosong dikirim sebagai satu paket
  4 byte `START|END`.
- Ukuran payload pesan setelah reassembly maksimum **512 byte**.

### Aturan reassembly penerima

1. Paket < 4 byte, version ≠ 1, atau flag tak dikenal: paket dibuang
   (`BAD_PACKET` / `BAD_VERSION`). Pesan parsial yang sedang berjalan tetap.
2. Paket `START`: pesan parsial sebelumnya dibuang (`INCOMPLETE`), mulai pesan baru.
3. Paket tanpa `START` saat tidak ada pesan parsial: dibuang (`NO_START`).
4. Paket lanjutan dengan type atau sequence berbeda: pesan parsial dan paket
   dibuang (`MISMATCH`).
5. Total payload > 512 byte: pesan parsial dibuang (`TOO_LARGE`).
6. Jeda > **1000 ms** sejak fragmen terakhir: pesan parsial dibuang saat paket
   berikutnya tiba (`TIMEOUT`); paket baru lalu diproses sesuai aturan 2–5.
7. Disconnect atau unsubscribe mereset reassembler.
8. Paket `END` menyelesaikan pesan; payload diberikan ke parser pesan.

Paket yang dibuang tidak memutus koneksi dan tidak boleh me-reboot perangkat.

## 3. Tipe pesan

| Type | Nama | Arah | Payload |
| --- | --- | --- | --- |
| 0x01 | HELLO | ESP → PC | 6 byte (§4.1) |
| 0x02 | HELLO_ACK | PC → ESP | 5 byte (§4.2) |
| 0x10 | MEDIA_STATE | PC → ESP | 13–361 byte (§4.3) |
| 0x11 | VOLUME_STATE | PC → ESP | 2 byte (§4.4) |
| 0x20 | MEDIA_COMMAND | ESP → PC | 3 byte (§4.5) |
| 0x30 | DEVICE_STATUS | ESP → PC | 8 byte (§4.6) |
| 0x40 | ERROR | dua arah | 3 byte (§4.7) |

Tipe 0x50–0x52, 0x60, 0x70 dicadangkan untuk masa depan dan **tidak** didukung di
V1. Payload harus tepat sepanjang yang didefinisikan; byte berlebih atau kurang
berarti pesan malformed.

## 4. Payload

### 4.1 HELLO

```text
u8  protocol_version   = 1
u8  firmware_major
u8  firmware_minor
u8  firmware_patch
u16 capabilities       bit0 render MEDIA_STATE, bit1 render VOLUME_STATE,
                       bit2 mengirim MEDIA_COMMAND; bit lain 0
```

### 4.2 HELLO_ACK

```text
u8  protocol_version   versi yang dipakai companion
u8  companion_major
u8  companion_minor
u8  companion_patch
u8  status             0 = accepted, 1 = rejected (versi tidak cocok)
```

### 4.3 MEDIA_STATE

```text
u8  playback       0 = NO_MEDIA, 1 = PLAYING, 2 = PAUSED
u32 position_ms
u32 duration_ms
u8  source_length  ≤ 31
u8  title_length   ≤ 127
u8  artist_length  ≤ 95
u8  album_length   ≤ 95
    source, title, artist, album: UTF-8 valid, tanpa byte 0x00
```

Panjang payload = 13 + jumlah keempat panjang. `duration_ms = 0` sah (durasi tidak
diketahui). `position_ms > duration_ms` sah di wire; penerima men-clamp untuk
tampilan. Nilai playback lain, UTF-8 tidak valid (termasuk overlong, surrogate,
atau > U+10FFFF), atau panjang melebihi batas → malformed.

### 4.4 VOLUME_STATE

```text
u8 volume  0–100
u8 muted   0 = false, 1 = true
```

### 4.5 MEDIA_COMMAND

```text
u8  command  0x01 PLAY_PAUSE, 0x02 NEXT, 0x03 PREVIOUS,
             0x04 VOLUME_DELTA, 0x05 MUTE_TOGGLE, 0x06 SEEK_DELTA (dicadangkan)
i16 value
```

`value` harus 0 untuk PLAY_PAUSE, NEXT, PREVIOUS, MUTE_TOGGLE. VOLUME_DELTA
memakai −100…100 kecuali 0 (encoder V1: +2 / −2). SEEK_DELTA tidak didukung di V1.

Implementasi M7 (firmware 0.3.0): mengirim PLAY_PAUSE/NEXT/PREVIOUS saja setelah
READY dan mengiklankan capability bit2. Encoder volume/mute masih log lokal (M8).

Implementasi M8 (firmware 0.4.0): encoder mengirim VOLUME_DELTA +2/−2 per klik,
tombol encoder MUTE_TOGGLE, dengan aturan antrean yang sama. Companion membalas
dengan VOLUME_STATE berisi nilai yang dibaca ulang dari Windows (bukan nilai
yang diminta) dan juga mengirim VOLUME_STATE saat volume diubah dari Windows.
Wire format tidak berubah; tidak ada vector baru.
Perintah tidak mengubah layar secara optimistis: hasil playback berasal dari
MEDIA_STATE berikutnya. Tidak ada retry aplikasi/ACK command di V1; kegagalan
notify dapat kehilangan satu tekanan dan dicatat di firmware. Antrean tombol
dibatasi ke sesi READY saat tekanan diteruskan, kedaluwarsa setelah 500 ms,
dan dibuang saat disconnect atau re-handshake.

### 4.6 DEVICE_STATUS

```text
u8  protocol_version   = 1
u8  firmware_major
u8  firmware_minor
u8  firmware_patch
u8  link_state         0 ADVERTISING, 1 CONNECTED, 2 READY, 3 PROTOCOL_MISMATCH
u8  flags              0 di V1
u16 rx_errors          paket/pesan ditolak sejak boot, jenuh di 65535
```

Nilai karakteristik DEVICE STATUS selalu 8 byte ini. Firmware memberi notify
karakteristik saat link_state berubah. Pesan 0x30 didefinisikan untuk transport
lain; firmware V1 tidak mengirimnya lewat TX.

### 4.7 ERROR

```text
u8 code          1 UNSUPPORTED_VERSION, 2 MALFORMED, 3 UNSUPPORTED_TYPE,
                 4 TOO_LARGE, 5 NOT_READY
u8 related_type  type pesan yang ditolak
u8 related_seq   sequence pesan yang ditolak
```

Penerima ERROR hanya mencatatnya; tidak ada balasan ERROR untuk ERROR.

## 5. Handshake dan state link

```text
ESP32: ADVERTISING ─connect─► CONNECTED ─HELLO_ACK ok─► READY
                                   └──HELLO_ACK ditolak / versi beda──► PROTOCOL_MISMATCH
disconnect dari state mana pun ─► ADVERTISING (reassembler dan media direset)
```

1. Central terhubung dan (opsional) menukar MTU.
2. Central mengaktifkan notify TX. **Firmware mengirim HELLO saat notify TX aktif**,
   bukan saat connect, agar HELLO tidak hilang. Unsubscribe lalu subscribe ulang
   mengembalikan link ke CONNECTED dan mengirim HELLO baru.
3. Companion membalas HELLO_ACK. Jika `HELLO.protocol_version` ≠ 1 companion
   mengirim status 1 dan berhenti menyinkronkan media.
4. Firmware masuk READY hanya jika HELLO_ACK `protocol_version = 1` dan
   `status = 0`; selain itu PROTOCOL_MISMATCH dan layar menampilkan ketidakcocokan.
   HELLO_ACK hanya diterima pada CONNECTED setelah subscribe TX. ACK sebelum
   subscribe atau ACK duplikat saat READY mendapat ERROR `NOT_READY`.
5. MEDIA_STATE / VOLUME_STATE sebelum READY dijawab ERROR `NOT_READY`.
6. Firmware menjawab pesan yang di-reassembly tetapi tidak sah dengan ERROR:
   versi beda `UNSUPPORTED_VERSION` (hanya pada paket START), payload salah
   `MALFORMED`, tipe tidak didukung arah ini `UNSUPPORTED_TYPE`, terlalu besar
   `TOO_LARGE`. Pembuangan fragmen lainnya hanya menaikkan `rx_errors`.

## 6. Timeline

Firmware menyimpan `position_ms` sebagai posisi dasar dan waktu monotonic saat
MEDIA_STATE diterima. Saat PLAYING, posisi tampil = dasar + (sekarang − waktu
terima), di-clamp ke `duration_ms` bila durasi > 0. PAUSED/NO_MEDIA tidak
menambah posisi. Companion mengirim MEDIA_STATE saat track, playback, seek, atau
timeline berubah, bukan setiap 100 ms.
