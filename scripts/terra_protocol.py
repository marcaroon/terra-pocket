"""Reference implementation of the Terra BLE protocol V1 (docs/ble-protocol.md).

Used to generate the shared test vectors and by the BLE test harness. The
firmware (C) and companion (Rust) implementations are independent and must
agree with the vectors this module produces.
"""
from __future__ import annotations

import struct
from dataclasses import dataclass, field

VERSION = 1
START, END, ACK_REQUEST = 0x01, 0x02, 0x04
HEADER_LEN = 4
MAX_MESSAGE = 512
REASSEMBLY_TIMEOUT_MS = 1000

HELLO, HELLO_ACK = 0x01, 0x02
MEDIA_STATE, VOLUME_STATE = 0x10, 0x11
MEDIA_COMMAND = 0x20
DEVICE_STATUS = 0x30
ERROR = 0x40

NO_MEDIA, PLAYING, PAUSED = 0, 1, 2
PLAY_PAUSE, NEXT, PREVIOUS, VOLUME_DELTA, MUTE_TOGGLE, SEEK_DELTA = range(1, 7)
LIMITS = {"source": 31, "title": 127, "artist": 95, "album": 95}

# Reassembly drop reasons (bit flags, shared names with C and Rust).
DROPS = ["BAD_PACKET", "BAD_VERSION", "INCOMPLETE", "NO_START", "MISMATCH",
         "TOO_LARGE", "TIMEOUT"]


class Malformed(ValueError):
    pass


def _text(value: bytes) -> str:
    if b"\x00" in value:
        raise Malformed("NUL in string")
    try:
        return value.decode("utf-8", errors="strict")
    except UnicodeDecodeError as error:
        raise Malformed("invalid UTF-8") from error


@dataclass
class MediaState:
    playback: int
    position_ms: int
    duration_ms: int
    source: str = ""
    title: str = ""
    artist: str = ""
    album: str = ""

    def encode(self) -> bytes:
        parts = [getattr(self, name).encode("utf-8") for name in LIMITS]
        for (name, limit), part in zip(LIMITS.items(), parts):
            if len(part) > limit:
                raise Malformed(f"{name} too long")
        if self.playback not in (NO_MEDIA, PLAYING, PAUSED):
            raise Malformed("playback")
        return (struct.pack("<BIIBBBB", self.playback, self.position_ms, self.duration_ms,
                            *map(len, parts)) + b"".join(parts))

    @classmethod
    def decode(cls, data: bytes) -> "MediaState":
        if len(data) < 13:
            raise Malformed("short")
        playback, position, duration, *lengths = struct.unpack_from("<BIIBBBB", data)
        if playback not in (NO_MEDIA, PLAYING, PAUSED):
            raise Malformed("playback")
        if any(n > limit for n, limit in zip(lengths, LIMITS.values())):
            raise Malformed("length")
        if len(data) != 13 + sum(lengths):
            raise Malformed("size")
        offset, texts = 13, []
        for n in lengths:
            texts.append(_text(data[offset:offset + n]))
            offset += n
        return cls(playback, position, duration, *texts)


def encode_volume(volume: int, muted: bool) -> bytes:
    return bytes([volume, int(muted)])


def decode_volume(data: bytes) -> tuple[int, bool]:
    if len(data) != 2 or data[0] > 100 or data[1] > 1:
        raise Malformed("volume")
    return data[0], bool(data[1])


def encode_command(command: int, value: int = 0) -> bytes:
    return struct.pack("<Bh", command, value)


def decode_command(data: bytes) -> tuple[int, int]:
    if len(data) != 3:
        raise Malformed("size")
    command, value = struct.unpack("<Bh", data)
    if command in (PLAY_PAUSE, NEXT, PREVIOUS, MUTE_TOGGLE):
        if value != 0:
            raise Malformed("value")
    elif command == VOLUME_DELTA:
        if value == 0 or not -100 <= value <= 100:
            raise Malformed("value")
    else:
        raise Malformed("command")
    return command, value


def encode_hello(major: int, minor: int, patch: int, caps: int, version: int = VERSION) -> bytes:
    return struct.pack("<BBBBH", version, major, minor, patch, caps)


def decode_hello(data: bytes) -> tuple[int, int, int, int, int]:
    if len(data) != 6:
        raise Malformed("size")
    fields = struct.unpack("<BBBBH", data)
    if fields[4] & ~0x7:
        raise Malformed("caps")
    return fields


def encode_hello_ack(major: int, minor: int, patch: int, status: int = 0,
                     version: int = VERSION) -> bytes:
    return bytes([version, major, minor, patch, status])


def decode_hello_ack(data: bytes) -> tuple[int, int, int, int, int]:
    if len(data) != 5 or data[4] > 1:
        raise Malformed("hello_ack")
    return tuple(data)


def encode_status(major: int, minor: int, patch: int, link: int, rx_errors: int,
                  flags: int = 0) -> bytes:
    return struct.pack("<BBBBBBH", VERSION, major, minor, patch, link, flags, rx_errors)


def decode_status(data: bytes) -> tuple[int, ...]:
    if len(data) != 8:
        raise Malformed("size")
    fields = struct.unpack("<BBBBBBH", data)
    if fields[4] > 3 or fields[5] != 0:
        raise Malformed("status")
    return fields


def encode_error(code: int, related_type: int, related_seq: int) -> bytes:
    return bytes([code, related_type, related_seq])


def decode_error(data: bytes) -> tuple[int, int, int]:
    if len(data) != 3 or not 1 <= data[0] <= 5:
        raise Malformed("error")
    return tuple(data)


def fragment(msg_type: int, seq: int, payload: bytes, mtu: int) -> list[bytes]:
    """Split one message into packets for an ATT MTU (packet = MTU - 3)."""
    if len(payload) > MAX_MESSAGE:
        raise Malformed("too large")
    chunk = mtu - 3 - HEADER_LEN
    if chunk < 1:
        raise ValueError("MTU too small")
    pieces = [payload[i:i + chunk] for i in range(0, len(payload), chunk)] or [b""]
    packets = []
    for index, piece in enumerate(pieces):
        flags = (START if index == 0 else 0) | (END if index == len(pieces) - 1 else 0)
        packets.append(bytes([VERSION, msg_type, flags, seq & 0xFF]) + piece)
    return packets


@dataclass
class Result:
    drops: list[str] = field(default_factory=list)
    message: tuple[int, int, bytes] | None = None  # (type, seq, payload)


class Reassembler:
    def __init__(self) -> None:
        self.active = False
        self.type = self.seq = 0
        self.buffer = bytearray()
        self.last_ms = 0

    def reset(self) -> None:
        self.active = False
        self.buffer.clear()

    def push(self, packet: bytes, now_ms: int) -> Result:
        result = Result()
        if self.active and now_ms - self.last_ms > REASSEMBLY_TIMEOUT_MS:
            result.drops.append("TIMEOUT")
            self.reset()
        if len(packet) < HEADER_LEN or packet[2] & ~0x07:
            result.drops.append("BAD_PACKET")
            return result
        if packet[0] != VERSION:
            result.drops.append("BAD_VERSION")
            return result
        msg_type, flags, seq, body = packet[1], packet[2], packet[3], packet[4:]
        if flags & START:
            if self.active:
                result.drops.append("INCOMPLETE")
            self.active, self.type, self.seq = True, msg_type, seq
            self.buffer = bytearray()
        elif not self.active:
            result.drops.append("NO_START")
            return result
        elif msg_type != self.type or seq != self.seq:
            result.drops.append("MISMATCH")
            self.reset()
            return result
        if len(self.buffer) + len(body) > MAX_MESSAGE:
            result.drops.append("TOO_LARGE")
            self.reset()
            return result
        self.buffer += body
        self.last_ms = now_ms
        if flags & END:
            result.message = (self.type, self.seq, bytes(self.buffer))
            self.reset()
        return result
