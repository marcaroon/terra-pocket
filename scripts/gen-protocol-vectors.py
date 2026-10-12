"""Generate docs/protocol-test-vectors from the Python reference implementation.

usage: python scripts/gen-protocol-vectors.py docs/protocol-test-vectors
"""
import json
import pathlib
import struct
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import terra_protocol as p  # noqa: E402

out = pathlib.Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
for old in list(out.glob("*.bin")) + list(out.glob("*.json")):
    old.unlink()
rows = []


def hexs(data: bytes) -> str:
    return data.hex() if data else "-"


def emit(name, kind, expect, data, args=(), note=""):
    (out / f"{name}.bin").write_bytes(data)
    description = {"name": name, "kind": kind, "expect": expect, "note": note,
                   "bytes": data.hex(), "fields": list(args)}
    (out / f"{name}.json").write_text(json.dumps(description, indent=2, ensure_ascii=False) + "\n",
                                      encoding="utf-8", newline="\n")
    rows.append("\t".join([name, kind, expect, *map(str, args)]))


def media(name, state, note=""):
    data = state.encode()
    decoded = p.MediaState.decode(data)
    emit(name, "media_state", "ok", data,
         [decoded.playback, decoded.position_ms, decoded.duration_ms,
          *(hexs(getattr(decoded, f).encode()) for f in p.LIMITS)], note)


def bad(name, kind, data, decoder, note):
    try:
        decoder(data)
    except p.Malformed:
        emit(name, kind, "malformed", data, (), note)
        return
    raise AssertionError(f"{name} unexpectedly decoded")


basic = p.MediaState(p.PLAYING, 81200, 216000, "Spotify", "The Less I Know The Better",
                     "Tame Impala", "Currents")
media("media_state_basic", basic)
media("media_state_utf8_max", p.MediaState(p.PAUSED, 1, 2, "S" * 31, "é" * 63 + "a",
                                           "😀" * 23 + "abc", "ü" * 47 + "x"),
      "every string at its byte limit, multibyte UTF-8")
media("media_state_no_media", p.MediaState(p.NO_MEDIA, 0, 0), "empty strings, duration 0")
media("media_state_position_over_duration", p.MediaState(p.PLAYING, 5000, 3000, "Browser", "Clip"),
      "legal on the wire; receivers clamp for display")

raw = basic.encode()
header = lambda title_len: struct.pack("<BIIBBBB", p.PLAYING, 1, 2, 0, title_len, 0, 0)  # noqa: E731
decode_media = p.MediaState.decode
bad("media_state_title_128", "media_state", header(128) + b"a" * 128, decode_media, "title over 127")
bad("media_state_truncated", "media_state", raw[:-1], decode_media, "one byte short")
bad("media_state_trailing_byte", "media_state", raw + b"\x00", decode_media, "one extra byte")
bad("media_state_invalid_utf8", "media_state", header(2) + b"\xc3\x28", decode_media, "bad continuation")
bad("media_state_overlong_utf8", "media_state", header(2) + b"\xc0\xaf", decode_media, "overlong '/'")
bad("media_state_surrogate", "media_state", header(3) + b"\xed\xa0\x80", decode_media, "UTF-16 surrogate")
bad("media_state_above_unicode", "media_state", header(4) + b"\xf4\x90\x80\x80", decode_media,
    "code point above U+10FFFF")
bad("media_state_nul", "media_state", header(3) + b"A\x00B", decode_media, "NUL byte")
bad("media_state_bad_playback", "media_state", b"\x03" + raw[1:], decode_media, "playback 3")
bad("media_state_short", "media_state", raw[:12], decode_media, "shorter than fixed header")

for name, volume, muted in (("volume_72", 72, False), ("volume_100_muted", 100, True)):
    data = p.encode_volume(volume, muted)
    emit(name, "volume_state", "ok", data, p.decode_volume(data)[0:1] + (int(muted),))
bad("volume_101", "volume_state", bytes([101, 0]), p.decode_volume, "volume over 100")
bad("volume_muted_2", "volume_state", bytes([10, 2]), p.decode_volume, "muted not 0/1")
bad("volume_short", "volume_state", bytes([10]), p.decode_volume, "one byte")

for name, command, value in (("command_play_pause", p.PLAY_PAUSE, 0), ("command_next", p.NEXT, 0),
                             ("command_previous", p.PREVIOUS, 0),
                             ("command_volume_up", p.VOLUME_DELTA, 2),
                             ("command_volume_down", p.VOLUME_DELTA, -2),
                             ("command_mute_toggle", p.MUTE_TOGGLE, 0)):
    data = p.encode_command(command, value)
    emit(name, "media_command", "ok", data, p.decode_command(data))
bad("command_seek_reserved", "media_command", p.encode_command(p.SEEK_DELTA, 1000),
    p.decode_command, "SEEK_DELTA reserved in V1")
bad("command_unknown", "media_command", p.encode_command(7, 0), p.decode_command, "command 7")
bad("command_play_with_value", "media_command", p.encode_command(p.PLAY_PAUSE, 5),
    p.decode_command, "value must be 0")
bad("command_volume_zero", "media_command", p.encode_command(p.VOLUME_DELTA, 0),
    p.decode_command, "zero delta")
bad("command_volume_101", "media_command", p.encode_command(p.VOLUME_DELTA, 101),
    p.decode_command, "delta over 100")

data = p.encode_hello(0, 2, 0, 0x0003)
emit("hello_basic", "hello", "ok", data, p.decode_hello(data))
data = p.encode_hello(0, 5, 0, p.CAPS_ALL)
emit("hello_artwork", "hello", "ok", data, p.decode_hello(data), "V2: capability bit3 renders artwork")
bad("hello_bad_caps", "hello", p.encode_hello(0, 2, 0, 0x0010), p.decode_hello, "unknown capability bit")
bad("hello_short", "hello", data[:5], p.decode_hello, "five bytes")

for name, payload in (("hello_ack_accepted", p.encode_hello_ack(0, 1, 0)),
                      ("hello_ack_rejected", p.encode_hello_ack(0, 1, 0, status=1, version=2))):
    emit(name, "hello_ack", "ok", payload, p.decode_hello_ack(payload))
bad("hello_ack_bad_status", "hello_ack", p.encode_hello_ack(0, 1, 0, status=2), p.decode_hello_ack,
    "status 2")

data = p.encode_status(0, 2, 0, 2, 3)
emit("device_status_ready", "device_status", "ok", data, p.decode_status(data))
bad("device_status_bad_link", "device_status", p.encode_status(0, 2, 0, 4, 0), p.decode_status,
    "link state 4")

data = p.encode_error(5, p.MEDIA_STATE, 7)
emit("error_not_ready", "error", "ok", data, p.decode_error(data))
bad("error_bad_code", "error", p.encode_error(0, 0x10, 1), p.decode_error, "code 0")


# V2 album artwork (§4.8).
for name, data in (("artwork_begin_jpeg", p.encode_artwork_begin(0x1234ABCD, 9876)),
                   ("artwork_begin_max", p.encode_artwork_begin(1, p.ARTWORK_MAX_BYTES)),
                   ("artwork_begin_clear", p.encode_artwork_clear())):
    emit(name, "artwork_begin", "ok", data, p.decode_artwork_begin(data))
for name, data, note in (
        ("artwork_begin_too_large", p.encode_artwork_begin(1, p.ARTWORK_MAX_BYTES + 1), "32769 bytes"),
        ("artwork_begin_png", p.encode_artwork_begin(1, 100, fmt=2), "format 2"),
        ("artwork_begin_161", p.encode_artwork_begin(1, 100, width=161), "not 160x160"),
        ("artwork_begin_zero_id", p.encode_artwork_begin(0, 100), "id 0 with data"),
        ("artwork_begin_clear_with_id", p.encode_artwork_begin(7, 0, 0, 0, 0), "clear must be all zero"),
        ("artwork_begin_reserved", p.encode_artwork_begin(1, 100, reserved=1), "reserved byte"),
        ("artwork_begin_short", p.encode_artwork_begin(1, 100)[:13], "13 bytes")):
    bad(name, "artwork_begin", data, p.decode_artwork_begin, note)

chunk_bytes = bytes(range(232))
for name, data in (("artwork_chunk_basic", p.encode_artwork_chunk(0x1234ABCD, 464, chunk_bytes)),
                   ("artwork_chunk_last_byte", p.encode_artwork_chunk(9, p.ARTWORK_MAX_BYTES - 1, b"\xd9"))):
    artwork_id, offset, chunk = p.decode_artwork_chunk(data)
    emit(name, "artwork_chunk", "ok", data, (artwork_id, offset, hexs(chunk)))
for name, data, note in (
        ("artwork_chunk_empty", p.encode_artwork_chunk(1, 0, b""), "no data bytes"),
        ("artwork_chunk_zero_id", p.encode_artwork_chunk(0, 0, b"x"), "id 0"),
        ("artwork_chunk_past_max", p.encode_artwork_chunk(1, p.ARTWORK_MAX_BYTES - 1, b"xy"),
         "ends past 32768")):
    bad(name, "artwork_chunk", data, p.decode_artwork_chunk, note)

data = p.encode_artwork_end(0x1234ABCD, 0xCBF43926)
emit("artwork_end_basic", "artwork_end", "ok", data, p.decode_artwork_end(data))
bad("artwork_end_zero_id", "artwork_end", p.encode_artwork_end(0, 1), p.decode_artwork_end, "id 0")
bad("artwork_end_short", "artwork_end", data[:7], p.decode_artwork_end, "7 bytes")
for name, data in (("crc32_check", b"123456789"), ("crc32_empty", b""), ("crc32_chunk", chunk_bytes)):
    emit(name, "crc32", "ok", data, (p.crc32(data),), "CRC-32/ISO-HDLC as in ARTWORK_END")


def packets_blob(packets):
    return b"".join(struct.pack("<H", len(x)) + x for x in packets)


def frag(name, msg_type, seq, payload, mtu, note=""):
    packets = p.fragment(msg_type, seq, payload, mtu)
    emit(name, "fragment", "ok", packets_blob(packets), (msg_type, seq, mtu, hexs(payload)), note)
    return packets


basic_packets = frag("fragment_media_basic_mtu23", p.MEDIA_STATE, 5, raw, 23, "16 payload bytes per packet")
frag("fragment_media_max_mtu247", p.MEDIA_STATE, 255,
     p.MediaState(p.PAUSED, 1, 2, "S" * 31, "é" * 63 + "a", "😀" * 23 + "abc", "ü" * 47 + "x").encode(),
     247, "361-byte payload in two packets")
frag("fragment_volume_mtu23", p.VOLUME_STATE, 0, p.encode_volume(72, False), 23, "single packet")
frag("fragment_empty_payload", p.ERROR, 0, b"", 23, "header-only START|END packet")


def stream(name, timed_packets, note=""):
    """timed_packets: list of (time_ms, packet). Expected results come from the reference."""
    r = p.Reassembler()
    blob, results = b"", []
    for time_ms, packet in timed_packets:
        blob += struct.pack("<HI", len(packet), time_ms) + packet
        res = r.push(packet, time_ms)
        drops = "+".join(res.drops) or "-"
        if res.message:
            msg_type, seq, payload = res.message
            status = f"{msg_type:02x}:{seq:02x}:{payload.hex()}"
        else:
            status = "-"
        results.append(f"{drops}/{status}")
    emit(name, "stream", "ok", blob, (";".join(results),), note)


def pkt(msg_type, flags, seq, body=b"", version=p.VERSION):
    return bytes([version, msg_type, flags, seq]) + body


volume_single = p.fragment(p.VOLUME_STATE, 2, p.encode_volume(40, True), 23)[0]
stream("stream_fragmented_ok", [(i * 10, x) for i, x in enumerate(basic_packets)])
stream("stream_missing_end", [(0, basic_packets[0]), (10, basic_packets[1]), (20, volume_single)],
       "new START discards the unfinished message")
stream("stream_no_start", [(0, basic_packets[1])])
stream("stream_mismatch_seq", [(0, pkt(p.MEDIA_STATE, p.START, 1, b"ab")),
                               (5, pkt(p.MEDIA_STATE, p.END, 2, b"cd"))])
stream("stream_mismatch_type", [(0, pkt(p.MEDIA_STATE, p.START, 1, b"ab")),
                                (5, pkt(p.VOLUME_STATE, p.END, 1, b"cd"))])
stream("stream_bad_version_keeps_partial", [(0, basic_packets[0]),
                                            (5, pkt(p.VOLUME_STATE, p.START | p.END, 9, b"\x01\x00", 2))]
       + [(10 + i, x) for i, x in enumerate(basic_packets[1:])])
stream("stream_bad_packets", [(0, b"\x01\x10\x03"), (1, pkt(p.MEDIA_STATE, p.START | p.END | 0x08, 1)),
                              (2, volume_single)])
stream("stream_too_large", [(0, pkt(0x10, p.START, 9, b"x" * 200)), (1, pkt(0x10, 0, 9, b"x" * 200)),
                            (2, pkt(0x10, p.END, 9, b"x" * 200)), (3, volume_single)])
stream("stream_exactly_max", [(0, pkt(0x10, p.START, 9, b"y" * 256)), (1, pkt(0x10, p.END, 9, b"z" * 256))],
       "512 bytes is allowed")
stream("stream_timeout", [(0, basic_packets[0]), (1500, basic_packets[1]), (1600, volume_single)],
       "gap over 1000 ms drops the partial message")
stream("stream_timeout_boundary", [(0, pkt(p.VOLUME_STATE, p.START, 3, b"\x05")),
                                   (1000, pkt(p.VOLUME_STATE, p.END, 3, b"\x00"))], "exactly 1000 ms is kept")
stream("stream_ack_request_ignored", [(0, pkt(p.VOLUME_STATE, p.START | p.END | p.ACK_REQUEST, 4, b"\x10\x00"))])
stream("stream_seq_wrap", [(0, pkt(p.VOLUME_STATE, p.START | p.END, 255, b"\x01\x00")),
                           (1, pkt(p.VOLUME_STATE, p.START | p.END, 0, b"\x02\x00"))])

manifest = ["# name\tkind\texpect\tfields... (generated by scripts/gen-protocol-vectors.py)"] + rows
(out / "manifest.tsv").write_text("\n".join(manifest) + "\n", encoding="utf-8", newline="\n")
print(f"{len(rows)} vectors written to {out}")
