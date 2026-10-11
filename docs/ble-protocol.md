# BLE protocol V1 — spesifikasi, belum diimplementasikan

Rujukan kanonik wire format adalah **PROJECT_SPEC.md bagian 11–17**. Dokumen ini
adalah indeks implementasi untuk M3/M4, bukan spesifikasi tandingan.

- Peripheral: ESP32-S3, nama `Terra`; central: Windows.
- Service: `6F3A0001-9B21-4C71-9A8B-31A8B9F10001`.
- RX `6F3A0002-9B21-4C71-9A8B-31A8B9F10001`: write / write without response.
- TX `6F3A0003-9B21-4C71-9A8B-31A8B9F10001`: notify.
- Status `6F3A0004-9B21-4C71-9A8B-31A8B9F10001`: read / notify.

Header empat byte: version, message type, flags, sequence. Flags START=1, END=2,
ACK_REQUEST=4. Payload biner little endian, direassembly sebelum parsing.
MEDIA_STATE: playback u8, position u32, duration u32, empat panjang u8, lalu UTF-8
source/title/artist/album. Batas byte 31/127/95/95; maksimum payload 361 byte.
VOLUME_STATE: volume 0–100 dan muted 0/1. MEDIA_COMMAND: command u8 + value i16.
Lihat spesifikasi untuk enum pesan dan command.

## Keputusan yang wajib ditutup sebelum M4

Blueprint belum menentukan semantik sequence antarfragmen, timeout reassembly,
ACK_REQUEST/ERROR payload, HELLO_ACK payload, atau format DEVICE_STATUS.
Selain itu HELLO tidak boleh hilang sebelum central berlangganan TX.
Tetapkan aturan tersebut secara eksplisit di spesifikasi kanonik sebelum membuat
codec; jangan membiarkan Rust dan C mengasumsikan format berbeda.

Saat M4, kedua implementasi wajib membaca fixture yang sama di
`protocol-test-vectors/`, meliputi malformed length, versi salah, batas UTF-8,
fragment tidak lengkap, overflow, dan disconnect. Tidak ada kode protokol atau
fixture pura-pura pada M0/M1.
