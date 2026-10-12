# Terra — Desktop Music Preview & Controller

## Amendment — 11 Oktober 2026

Instruksi terbaru pengguna mengganti LCD menjadi **ST7789 240×240**, controller
telah dikonfirmasi; model breakout belum diketahui. Gap default sekarang (0,0),
dapat dikalibrasi di board_config.h. Ukuran diagonal modul belum diketahui.

Pengguna juga meminta proposal NAS **ESP32 saja** (tanpa PC/server lokal) berbasis
HDD laptop **500 GB, NTFS, casing bertenaga USB** dan
prototipe web konfigurasi sebelum fase berikutnya. Lihat `docs/nas-proposal.md`.
Pengguna kemudian meminta melanjutkan fase berikutnya dengan seluruh perubahan
yang dibahas. Scope fase berjalan: **M2 input fisik + N0 diagnostik USB read-only**.
LCD tetap 240×240. Persetujuan ini tidak mencakup format atau penulisan HDD.
Wi-Fi, akses file NTFS, dan relay menunggu hasil USB serta keputusan filesystem.
Prototipe web tetap memakai data contoh dan draf lokal, bukan kontrol perangkat.

Hardware dari foto: board bergaya YD-ESP32-S3 bertanda `2022-V1.3` / `YD-ESP32-23`,
modul berlabel N16R8; casing ORICO 2020U3; WD Blue WD5000LPCX 500 GB NTFS.
Label HDD 5 V 0,55 A; datasheet WD menyatakan arus puncak 1 A. Rancangan suplai
5 V 3 A bercabang masih perlu verifikasi listrik, bukan jaminan port board cukup.
Cloudflare Workers + Durable Objects Free dibahas untuk relay dengan kuota;
Vercel tetap opsi frontend. Tidak ada layanan relay yang diaktifkan.

Amendment 12 Oktober 2026 (lanjutan): setelah MVP M0–M10, pengguna meminta
melanjutkan ke V2. Album artwork (§36) dipindahkan dari backlog §35 dan
diimplementasikan; wire format ARTWORK_BEGIN/CHUNK/END ditetapkan di
`docs/ble-protocol.md` §4.8. Butir backlog lain tetap tidak dikerjakan.

Amendment 12 Oktober 2026: M3/M4 diimplementasikan. Keputusan protokol yang
sebelumnya terbuka (sequence, reassembly, ERROR, HELLO_ACK, DEVICE_STATUS, waktu
HELLO) ditetapkan di `docs/ble-protocol.md`, yang kini menjadi spesifikasi wire
format kanonik dan memperinci bagian 11–17 tanpa mengubahnya.

## 1. Project Objective

Bangun sebuah perangkat desktop berbasis ESP32-S3 yang berfungsi sebagai:

- Now Playing display
- Media controller
- System volume controller
- Bluetooth LE peripheral
- Companion device untuk Windows

Perangkat harus dapat menampilkan media yang sedang diputar pada Windows dan memberikan kontrol fisik terhadap playback.

Target penggunaan:

```text
Spotify
YouTube
YouTube Music
Apple Music
Browser media
Windows-compatible media players
```

selama media tersebut terdaftar pada Windows System Media Transport Controls.

Target hardware utama:

```text
ESP32-S3 N16R8
16 MB Flash
8 MB PSRAM

ST7789 (model breakout modul pengganti perlu dikonfirmasi)
240x240 IPS

3 tactile buttons
1 rotary encoder + push switch
USB-C power
```

V1 adalah USB-powered desktop device. Battery tidak termasuk scope V1.

---

# 2. MVP Feature Scope

MVP harus memiliki fungsi berikut.

## Display

Menampilkan:

```text
Media source
Song title
Artist
Playback state
Progress bar
Current position
Duration
System volume
BLE connection status
```

Contoh:

```text
┌────────────────────────────┐
│ SPOTIFY                 ●  │
│                            │
│ THE LESS I KNOW            │
│ THE BETTER                 │
│                            │
│ Tame Impala                │
│                            │
│ ━━━━━━━━━●━━━━━━━━        │
│ 01:21              03:36   │
│                            │
│ Playing             VOL 72 │
└────────────────────────────┘
```

Album artwork TIDAK termasuk MVP.

---

# 3. Physical Controls

Gunakan:

```text
Button 1 → Previous
Button 2 → Play / Pause
Button 3 → Next

Rotary CW  → Volume Up
Rotary CCW → Volume Down

Rotary Push → Mute / Unmute
```

Future behavior:

```text
Long press encoder
→ change screen

Double press
→ configurable action
```

Jangan implementasikan future behavior pada MVP kecuali foundation-nya diperlukan.

---

# 4. System Architecture

Arsitektur utama:

```text
┌─────────────────────────────────────┐
│              WINDOWS                │
│                                     │
│ Spotify / Browser / Music App       │
│                 │                   │
│                 ▼                   │
│ Windows Media Session               │
│                 │                   │
│          companion-core             │
│          ┌─────────────┐            │
│          │ Media       │            │
│          │ Audio       │            │
│          │ BLE         │            │
│          │ State Sync  │            │
│          └──────┬──────┘            │
│                 │                   │
└─────────────────┼───────────────────┘
                  │
              Bluetooth LE
                  │
                  ▼
┌─────────────────────────────────────┐
│            ESP32-S3 N16R8           │
│                                     │
│             BLE GATT                │
│                │                    │
│         Application State           │
│          /            \             │
│       Input           Display       │
│       Manager         Manager       │
│         │                │          │
│ Buttons/Encoder      ST7789         │
└─────────────────────────────────────┘
```

Windows bertindak sebagai:

```text
BLE Central
GATT Client
```

ESP32 bertindak sebagai:

```text
BLE Peripheral
GATT Server
```

---

# 5. Technology Stack

## Firmware

Use:

```text
ESP-IDF 6.0.x stable
C
FreeRTOS
NimBLE
esp_lcd
LVGL 9.5.x
esp_lvgl_port 2.9.x
```

Target:

```text
idf.py set-target esp32s3
```

Do not use Arduino framework for the main implementation.

Arduino examples may be consulted only for hardware references if needed.

---

## Windows Companion Core

Use:

```text
Rust
Tokio where asynchronous runtime is useful
windows-rs
```

Windows APIs:

```text
Windows.Media.Control

GlobalSystemMediaTransportControlsSessionManager
```

For Bluetooth:

```text
Windows.Devices.Bluetooth
Windows.Devices.Bluetooth.GenericAttributeProfile
Windows.Devices.Enumeration
```

For system volume:

```text
Windows Core Audio API
IAudioEndpointVolume
```

---

## Companion GUI

Only after companion-core works.

Use:

```text
Tauri 2
React
TypeScript
```

UI is a thin client.

Business logic MUST remain in Rust.

Do not move media/BLE/audio logic into React.

---

# 6. Repository Architecture

Create monorepo:

```text
terra-pocket/
│
├── README.md
├── PROJECT_SPEC.md
├── CHANGELOG.md
│
├── docs/
│   ├── architecture.md
│   ├── hardware.md
│   ├── ble-protocol.md
│   ├── state-machine.md
│   └── development.md
│
├── firmware/
│   ├── CMakeLists.txt
│   ├── sdkconfig.defaults
│   ├── partitions.csv
│   │
│   └── main/
│       ├── CMakeLists.txt
│       ├── idf_component.yml
│       │
│       ├── app_main.c
│       │
│       ├── board_config.h
│       │
│       ├── app_state.c
│       ├── app_state.h
│       │
│       ├── ble/
│       │   ├── ble_service.c
│       │   ├── ble_service.h
│       │   ├── ble_transport.c
│       │   └── ble_transport.h
│       │
│       ├── protocol/
│       │   ├── protocol.c
│       │   └── protocol.h
│       │
│       ├── display/
│       │   ├── display.c
│       │   ├── display.h
│       │   ├── ui.c
│       │   └── ui.h
│       │
│       ├── input/
│       │   ├── buttons.c
│       │   ├── buttons.h
│       │   ├── encoder.c
│       │   └── encoder.h
│       │
│       └── util/
│           ├── time_utils.c
│           └── time_utils.h
│
└── companion/
    ├── Cargo.toml
    │
    ├── crates/
    │   ├── protocol/
    │   ├── media-windows/
    │   ├── audio-windows/
    │   ├── ble-windows/
    │   ├── companion-core/
    │   └── companion-cli/
    │
    └── app/
        ├── src/
        ├── src-tauri/
        ├── package.json
        └── vite.config.ts
```

---

# 7. Separation of Responsibilities

Firmware components MUST be separated.

Do not create one giant `app_main.c`.

`app_main.c` only coordinates initialization.

Example:

```text
app_main
   │
   ├── app_state_init()
   ├── display_init()
   ├── input_init()
   ├── ble_init()
   │
   └── app_start()
```

---

# 8. Application State

Create one canonical device state.

Example:

```c
typedef enum {
    PLAYBACK_STOPPED,
    PLAYBACK_PLAYING,
    PLAYBACK_PAUSED
} playback_state_t;

typedef struct {
    bool connected;

    char source[32];
    char title[128];
    char artist[96];
    char album[96];

    uint32_t position_ms;
    uint32_t duration_ms;

    uint8_t volume;

    playback_state_t playback;
    bool muted;

    uint64_t last_sync_ms;
} media_state_t;
```

UI MUST read from this state.

BLE callbacks MUST NOT manipulate LVGL widgets directly.

Correct pattern:

```text
BLE
 ↓
update app_state
 ↓
UI task receives state changed event
 ↓
LVGL render
```

---

# 9. FreeRTOS Architecture

Recommended logical tasks:

```text
BLE Task
Input Task
UI Task
```

Do not create unnecessary tasks for every component.

Use:

```text
Queue
EventGroup
Mutex
```

where appropriate.

Example:

```text
Button
  ↓
Input Task
  ↓
Command Queue
  ↓
BLE Task
  ↓
Windows
```

State update:

```text
BLE RX
  ↓
Protocol Parser
  ↓
App State
  ↓
UI Event
  ↓
Render
```

---

# 10. Provisional GPIO Assignment

Assume ESP32-S3 DevKit-style N16R8 board.

Use configurable definitions in:

```text
board_config.h
```

Suggested starting mapping:

```text
ST7789

SCLK  → GPIO12
MOSI  → GPIO11
CS    → tidak dipakai (modul 7 pin, CS terikat GND di modul)
DC    → GPIO9
RST   → GPIO8
BL    → GPIO7
```

Buttons:

```text
Previous    → GPIO4
Play/Pause  → GPIO5
Next        → GPIO6
```

Encoder:

```text
ENC_A  → GPIO15
ENC_B  → GPIO16
ENC_SW → GPIO17
```

Do NOT use:

```text
GPIO35
GPIO36
GPIO37
```

because N16R8 uses Octal memory.

Avoid unless specifically required:

```text
GPIO0
GPIO3
GPIO45
GPIO46
```

because of boot/strapping considerations.

Avoid GPIO19/GPIO20 for now so native USB remains available.

All GPIO assignments MUST remain centralized in `board_config.h`.

Do not scatter GPIO numbers throughout the code.

---

# 11. BLE GATT Architecture

Device advertising name:

```text
Terra
```

Use custom 128-bit UUIDs.

Service:

```text
6F3A0001-9B21-4C71-9A8B-31A8B9F10001
```

Characteristics:

```text
RX
PC → ESP32

UUID:
6F3A0002-9B21-4C71-9A8B-31A8B9F10001

Properties:
WRITE
WRITE WITHOUT RESPONSE
```

```text
TX
ESP32 → PC

UUID:
6F3A0003-9B21-4C71-9A8B-31A8B9F10001

Properties:
NOTIFY
```

```text
DEVICE STATUS

UUID:
6F3A0004-9B21-4C71-9A8B-31A8B9F10001

Properties:
READ
NOTIFY
```

Keep the GATT layout intentionally simple.

Do NOT create one characteristic for every media field.

---

# 12. Transport Protocol

Create a transport layer independent from BLE.

Reason:

Future transports may include:

```text
USB CDC
Wi-Fi
WebSocket
```

without changing application protocol.

Each BLE packet uses:

```text
BYTE 0    protocol version
BYTE 1    message type
BYTE 2    flags
BYTE 3    sequence
BYTE 4+   payload
```

Flags:

```text
0x01 START
0x02 END
0x04 ACK_REQUEST
```

A message that fits into one BLE packet:

```text
START | END
```

A fragmented message:

```text
packet 1 → START
packet 2 → 0
packet 3 → 0
packet 4 → END
```

Reassemble before parsing application payload.

Never assume BLE MTU is always large enough for an entire track title or state object.

---

# 13. Protocol Message Types

Reserve:

```text
0x01 HELLO
0x02 HELLO_ACK

0x10 MEDIA_STATE
0x11 VOLUME_STATE

0x20 MEDIA_COMMAND

0x30 DEVICE_STATUS

0x40 ERROR
```

Future:

```text
0x50 ARTWORK_BEGIN
0x51 ARTWORK_CHUNK
0x52 ARTWORK_END

0x60 CLOCK_SYNC
0x70 DEVICE_CONFIG
```

Do not implement future message types yet.

---

# 14. MEDIA_STATE Payload

Use binary serialization.

Do NOT use JSON over BLE for normal device communication.

Layout:

```text
uint8   playback_state
uint32  position_ms
uint32  duration_ms

uint8   source_length
uint8   title_length
uint8   artist_length
uint8   album_length

UTF-8 source
UTF-8 title
UTF-8 artist
UTF-8 album
```

Little endian.

Bounds:

```text
source <= 31 bytes
title  <= 127 bytes
artist <= 95 bytes
album  <= 95 bytes
```

Validate all lengths before writing to buffers.

Malformed packets MUST be rejected safely.

---

# 15. VOLUME_STATE Payload

```text
uint8 volume
uint8 muted
```

Volume:

```text
0–100
```

Muted:

```text
0 = false
1 = true
```

---

# 16. MEDIA_COMMAND Payload

Layout:

```text
uint8 command
int16 value
```

Commands:

```text
0x01 PLAY_PAUSE
0x02 NEXT
0x03 PREVIOUS

0x04 VOLUME_DELTA
0x05 MUTE_TOGGLE

0x06 SEEK_DELTA
```

For V1:

```text
Encoder CW

command = VOLUME_DELTA
value   = +2
```

```text
Encoder CCW

command = VOLUME_DELTA
value   = -2
```

`SEEK_DELTA` may remain reserved until later.

---

# 17. HELLO Handshake

Immediately after connection:

ESP32:

```text
HELLO
```

Payload:

```text
protocol_version
firmware_major
firmware_minor
firmware_patch
device_capability_flags
```

Companion responds:

```text
HELLO_ACK
```

If protocol versions are incompatible:

```text
do not start media synchronization
show protocol mismatch
```

This avoids silent incompatibility after future firmware upgrades.

---

# 18. Windows Media Manager

Implement module:

```text
media-windows
```

Responsibilities:

```text
initialize GSMTC Session Manager
identify current media session
read current media properties
read playback state
read timeline
subscribe to media changes
send media commands
```

Use:

```text
GlobalSystemMediaTransportControlsSessionManager
```

Subscribe to:

```text
CurrentSessionChanged
SessionsChanged

MediaPropertiesChanged
PlaybackInfoChanged
TimelinePropertiesChanged
```

When current session changes:

```text
unsubscribe old session
select new session
subscribe new events
send full MEDIA_STATE
```

---

# 19. Media Commands

Map ESP command to Windows:

```text
PLAY_PAUSE
→ TryTogglePlayPauseAsync()

NEXT
→ TrySkipNextAsync()

PREVIOUS
→ TrySkipPreviousAsync()
```

Do not emulate keyboard shortcuts unless Windows Media Control fails and a fallback feature is intentionally introduced later.

---

# 20. Windows Volume Manager

Create:

```text
audio-windows
```

Use Windows Core Audio:

```text
IAudioEndpointVolume
```

Responsibilities:

```text
get master volume
set master volume
step volume
mute
unmute
observe volume changes
```

ESP command:

```text
VOLUME_DELTA +2
```

should result conceptually in:

```text
current = get_volume()
target = clamp(current + 2, 0, 100)
set_volume(target)
```

Then send updated:

```text
VOLUME_STATE
```

back to ESP32.

Do not assume encoder command succeeded until updated state is available.

---

# 21. Timeline Synchronization

Do not transmit playback position every 100 ms.

When Windows sends state:

```text
position = 81000
duration = 216000
playback = PLAYING
sync_time = local monotonic time
```

ESP stores:

```text
base_position_ms
last_sync_ms
```

If playing:

```text
display_position =
base_position_ms +
(now - last_sync_ms)
```

Clamp:

```text
0 <= position <= duration
```

Synchronize again when:

```text
track changes
pause occurs
resume occurs
seek occurs
timeline update received
```

UI progress animation should run locally.

---

# 22. Display Architecture

Use:

```text
esp_lcd
ST7789 driver
LVGL
esp_lvgl_port
```

Create separate screens/states.

## BOOT

```text
TERRA

Starting...
```

## WAITING

```text
Bluetooth

Waiting for computer...
```

## CONNECTED / NO MEDIA

```text
Connected

Nothing Playing
```

## PLAYING

```text
SPOTIFY

The Less I Know
The Better

Tame Impala

━━━━━━━━●━━━━━━━

01:21      03:36

Playing       72%
```

## PAUSED

Same screen with:

```text
Paused
```

---

# 23. Display Rules

UI should have:

```text
black/dark background
high contrast
minimal visual noise
large song title
smaller artist name
thin progress bar
small source label
small connection indicator
```

Do not design UI to imitate Spotify.

Do not use unnecessary gradients or animations.

Prioritize readability at approximately arm's-length desktop viewing distance.

Long text behavior:

First implementation:

```text
truncate / ellipsis
```

Future:

```text
slow marquee
```

Do not implement marquee until basic UI is stable.

---

# 24. Input Handling

Buttons require debounce.

Recommended logical debounce:

```text
30–50 ms
```

Generate events:

```text
BUTTON_PREVIOUS
BUTTON_PLAY
BUTTON_NEXT
ENCODER_LEFT
ENCODER_RIGHT
ENCODER_PRESS
```

Input module must NOT know anything about BLE.

Correct:

```text
hardware
 ↓
input event
 ↓
application command
 ↓
transport
```

Incorrect:

```text
button ISR
 ↓
send BLE directly
```

Keep ISRs short.

---

# 25. Companion Core Architecture

Create:

```text
companion-core
```

Responsibilities:

```text
MediaManager
AudioManager
BleManager
StateCoordinator
```

Conceptually:

```text
                StateCoordinator
                 /      |      \
                /       |       \
          MediaManager BLE     AudioManager
```

`StateCoordinator` is the only component coordinating cross-domain state.

Avoid circular dependencies.

---

# 26. companion-cli

Before Tauri exists, create:

```text
companion-cli
```

Expected commands:

```text
terra-cli media
terra-cli devices
terra-cli connect
terra-cli monitor
```

Example:

```text
$ terra-cli media

Source   : Spotify
Title    : The Less I Know The Better
Artist   : Tame Impala
State    : Playing
Position : 81.2s
Duration : 216.0s
```

BLE monitor:

```text
$ terra-cli monitor

[BLE] connected
[MEDIA] Spotify — Tame Impala — The Less I Know The Better
[VOLUME] 72
[COMMAND] PLAY_PAUSE
```

This CLI is required before implementing GUI.

---

# 27. Companion GUI

After CLI is stable, create Tauri application.

Main window:

```text
┌──────────────────────────────────┐
│ Terra                            │
├──────────────────────────────────┤
│                                  │
│ Device                           │
│ ● Terra                          │
│ Connected                        │
│                                  │
│ Now Playing                      │
│ The Less I Know The Better       │
│ Tame Impala                      │
│                                  │
│ Volume                           │
│ 72%                              │
│                                  │
│ Auto Connect            ON       │
│ Start with Windows      ON       │
│                                  │
│ Firmware                 0.1.0   │
│ Protocol                 1       │
│                                  │
└──────────────────────────────────┘
```

Tauri frontend accesses Rust core through commands/events.

React should not communicate with Bluetooth directly.

---

# 28. System Tray

After GUI implementation:

```text
Terra
├── Connected
├── Open
├── Reconnect
└── Exit
```

Closing main window should minimize to tray.

Application can optionally start with Windows.

---

# 29. Reconnection Strategy

Companion state:

```text
DISCONNECTED
    ↓
SCANNING
    ↓
CONNECTING
    ↓
DISCOVERING
    ↓
HANDSHAKE
    ↓
CONNECTED
```

On failure:

```text
CONNECTED
   ↓
DISCONNECTED
   ↓
wait
   ↓
SCAN
```

Use bounded exponential backoff.

Example:

```text
1s
2s
4s
8s
15s
15s...
```

Do not busy-loop scan.

---

# 30. ESP32 State Machine

Use:

```text
BOOT
 ↓
BLE_ADVERTISING
 ↓
CONNECTED
 ↓
READY
```

Media states are separate:

```text
NO_MEDIA
PLAYING
PAUSED
```

Connection state and playback state MUST NOT be represented by the same enum.

---

# 31. Error Handling

Firmware must safely handle:

```text
invalid packet
unsupported protocol version
oversized strings
incomplete fragments
BLE disconnect during packet
missing media
duration = 0
position > duration
Windows command rejection
```

Device must never reboot because malformed BLE data was received.

Companion should log errors but keep running wherever reasonable.

---

# 32. Logging

Firmware tags:

```text
APP
BLE
PROTOCOL
DISPLAY
INPUT
STATE
```

Example:

```text
I (BLE) Client connected
I (PROTOCOL) HELLO_ACK protocol=1
I (STATE) Track changed
I (INPUT) PLAY_PAUSE
W (PROTOCOL) Invalid packet ignored
```

Companion logging:

```text
INFO
WARN
ERROR
DEBUG
```

Do not log continuously changing playback position at INFO level.

---

# 33. Testing Strategy

## Protocol

Unit test:

```text
serialize MEDIA_STATE
deserialize MEDIA_STATE
fragment
reassemble
malformed length
missing END
invalid version
oversized string
```

The protocol implementation on Rust and ESP must use identical test vectors.

Store vectors:

```text
docs/protocol-test-vectors/
```

Example:

```text
media_state_001.bin
media_state_001.json
```

---

## Firmware

Test:

```text
boot
LCD init
button debounce
encoder direction
BLE advertise
BLE connect
receive media
disconnect/reconnect
```

---

## Companion

Test:

```text
media session detection
track changes
play/pause
next
previous
volume
mute
BLE reconnect
invalid packets
```

---

# 34. Definition of Done per Milestone

## M0 — Repository Foundation

Deliver:

```text
monorepo
README
docs
firmware builds
Rust workspace builds
```

Acceptance:

```text
idf.py build = success
cargo build = success
```

---

## M1 — Display Bring-up

Deliver:

```text
ST7789 initialized
LVGL initialized
static mock Now Playing UI
```

Acceptance:

Screen displays:

```text
Test Song
Test Artist
01:00 / 03:00
```

No BLE required.

---

## M2 — Physical Input

Deliver:

```text
3 buttons
encoder
debounce
event queue
```

Acceptance:

Serial log:

```text
PREVIOUS
PLAY_PAUSE
NEXT
VOLUME +2
VOLUME -2
MUTE
```

---

## M3 — BLE GATT Server

Deliver:

```text
advertising
RX characteristic
TX characteristic
STATUS characteristic
```

Acceptance:

Windows/nRF Connect can:

```text
discover device
connect
discover service
write RX
subscribe TX
```

---

## M4 — Protocol

Deliver:

```text
fragmentation
serialization
reassembly
HELLO handshake
```

Acceptance:

PC can send mock MEDIA_STATE and ESP renders it.

---

## M5 — Windows Media Reader

Deliver:

```text
companion-cli media
```

Acceptance:

Playing Spotify/browser media prints:

```text
source
title
artist
state
position
duration
```

---

## M6 — Windows BLE Client

Deliver:

```text
scan
connect
service discovery
HELLO handshake
send MEDIA_STATE
```

Acceptance:

Real Windows media metadata appears on ESP LCD.

This is the first major end-to-end milestone.

---

## M7 — Physical Media Control

Deliver:

```text
Previous
Play/Pause
Next
```

Acceptance:

Physical buttons control current Windows media session.

---

## M8 — Volume

Deliver:

```text
encoder volume
encoder mute
volume sync
```

Acceptance:

Changing encoder changes Windows master volume.

Changing volume from Windows also updates device display.

---

## M9 — Reliability

Deliver:

```text
automatic reconnect
session switching
BLE disconnect recovery
media app switching
```

Acceptance:

Device can survive:

```text
PC Bluetooth off/on
ESP restart
Spotify restart
browser → Spotify transition
PC sleep/wake
```

without requiring application restart wherever Windows permits recovery.

---

## M10 — Tauri Companion

Deliver:

```text
React UI
system tray
auto connect
startup option
status information
```

M10 marks MVP completion.

---

# 35. Non-MVP Backlog

Do NOT implement yet:

```text
Album artwork
Wi-Fi transport
OTA firmware
Battery
Battery gauge
Touch screen
Spotify Web API
Mobile app
iPhone companion
macOS
Lyrics
Weather
Calendar
Notifications
Custom PCB
USB media transport
```

Keep these only in backlog.

---

# 36. V2 Direction

After MVP is stable:

## Album Artwork

Windows:

```text
Media Thumbnail
 ↓
decode
 ↓
resize
 ↓
160x160
 ↓
JPEG/RGB565
 ↓
transfer
 ↓
ESP32
 ↓
PSRAM
 ↓
LVGL image
```

The N16R8 choice is specifically useful here because of the 8 MB PSRAM.

---

# 37. Engineering Rules

The coding agent MUST follow these rules.

1. Do not implement future milestones early.

2. Every milestone must compile before moving forward.

3. Do not create fake/stub functionality and mark it completed.

4. Hardware-specific values belong in `board_config.h`.

5. Protocol definitions must exist in one documented specification.

6. Rust and C protocol implementations must remain compatible.

7. Use bounded buffers.

8. Never trust BLE input lengths.

9. ISR routines must be minimal.

10. BLE callbacks must not manipulate LVGL directly.

11. React must not contain native Windows or BLE business logic.

12. Commit dependency lockfiles.

13. Avoid unnecessary dependencies.

14. Prefer official Espressif and Microsoft APIs.

15. If an API behaves differently from documentation, document the actual behavior before creating a workaround.

---

# 38. First Development Instruction

Start only with:

```text
M0
M1
```

Do NOT implement Bluetooth yet.

Tasks:

```text
1. Initialize repository.
2. Create documentation structure.
3. Initialize ESP-IDF project targeting esp32s3.
4. Configure N16R8 flash/PSRAM correctly.
5. Add LVGL and esp_lvgl_port.
6. Implement ST7789 display abstraction.
7. Implement static Now Playing UI.
8. Add mock media state.
9. Build firmware.
10. Document wiring.
```

Before completing M1, report:

```text
- files created
- architectural decisions
- dependencies added
- build result
- assumed display configuration
- remaining hardware assumptions
```

Do not proceed to M2 automatically.

---

# 39. Coding Agent Operating Instructions

You are acting as the lead embedded + Windows systems engineer for this project.

Before editing code:

1. Read `PROJECT_SPEC.md`.
2. Read existing repository structure.
3. Inspect existing implementation.
4. Identify the current milestone.
5. Do not overwrite working code without understanding it.

For every implementation:

```text
Understand
→ Plan
→ Implement
→ Build
→ Test
→ Document
```

Never claim a hardware test passed unless actual hardware output has been provided.

Distinguish clearly between:

```text
compile verified
software tested
hardware tested
```

When hardware testing requires user action, provide:

```text
exact wiring
exact command
expected output
what result the user should report
```

Do not provide vague instructions such as:

```text
"test whether it works"
```

Instead say:

```text
Run:

idf.py -p COMx flash monitor

Expected serial output:

I (DISPLAY) ST7789 initialized
I (LVGL) Display registered
I (APP) Mock screen ready

The LCD should show:
Test Song
Test Artist
01:00 / 03:00
```

When encountering uncertainty about a pin, board revision, display module, or dependency version:

do not silently guess.

Centralize the assumption, document it, and make it configurable.

The priority is:

```text
Correctness
→ Stability
→ Maintainability
→ UI polish
→ Additional features
```

Do not optimize prematurely.

The goal is not to produce the most code.

The goal is to produce a physical device that reliably works.
