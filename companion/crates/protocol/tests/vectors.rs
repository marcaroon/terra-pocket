//! Runs the shared vectors in docs/protocol-test-vectors (also used by the C firmware).

use protocol::*;
use std::fs;
use std::path::PathBuf;

fn vector_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../docs/protocol-test-vectors")
}

fn bin(name: &str) -> Vec<u8> {
    fs::read(vector_dir().join(format!("{name}.bin"))).expect("vector file")
}

fn unhex(hex: &str) -> Vec<u8> {
    if hex == "-" {
        return Vec::new();
    }
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect()
}

fn num<T: std::str::FromStr>(text: &str) -> T
where
    T::Err: std::fmt::Debug,
{
    text.parse().unwrap()
}

fn text(hex: &str) -> String {
    String::from_utf8(unhex(hex)).unwrap()
}

fn check_payload(name: &str, kind: &str, ok: bool, f: &[&str]) {
    let data = bin(name);
    let reencoded = match kind {
        "media_state" => MediaState::decode(&data).map(|s| {
            let playback = s.playback as u8;
            assert_eq!(
                (playback, s.position_ms, s.duration_ms),
                (num(f[0]), num(f[1]), num(f[2])),
                "{name}"
            );
            assert_eq!([s.source.clone(), s.title.clone(), s.artist.clone(), s.album.clone()],
                       [text(f[3]), text(f[4]), text(f[5]), text(f[6])], "{name}");
            s.encode().unwrap()
        }),
        "volume_state" => VolumeState::decode(&data).map(|v| {
            assert_eq!((v.volume, u8::from(v.muted)), (num(f[0]), num(f[1])), "{name}");
            v.encode().unwrap()
        }),
        "media_command" => MediaCommand::decode(&data).map(|c| {
            let (id, value): (u8, i16) = match c {
                MediaCommand::PlayPause => (1, 0),
                MediaCommand::Next => (2, 0),
                MediaCommand::Previous => (3, 0),
                MediaCommand::VolumeDelta(delta) => (4, delta),
                MediaCommand::MuteToggle => (5, 0),
            };
            assert_eq!((id, value), (num(f[0]), num(f[1])), "{name}");
            c.encode().unwrap()
        }),
        "hello" => Hello::decode(&data).map(|h| {
            let (a, b, c) = h.firmware;
            assert_eq!((h.protocol_version, a, b, c, h.capabilities),
                       (num(f[0]), num(f[1]), num(f[2]), num(f[3]), num(f[4])), "{name}");
            h.encode().unwrap()
        }),
        "hello_ack" => HelloAck::decode(&data).map(|h| {
            let (a, b, c) = h.companion;
            assert_eq!((h.protocol_version, a, b, c, u8::from(!h.accepted)),
                       (num(f[0]), num(f[1]), num(f[2]), num(f[3]), num(f[4])), "{name}");
            h.encode()
        }),
        "device_status" => DeviceStatus::decode(&data).map(|s| {
            let (a, b, c) = s.firmware;
            assert_eq!((s.protocol_version, a, b, c, s.link as u8, 0u8, s.rx_errors),
                       (num(f[0]), num(f[1]), num(f[2]), num(f[3]), num(f[4]), num(f[5]), num(f[6])),
                       "{name}");
            s.encode()
        }),
        "error" => ErrorReport::decode(&data).map(|e| {
            assert_eq!((e.code, e.related_type, e.related_seq), (num(f[0]), num(f[1]), num(f[2])), "{name}");
            e.encode().unwrap()
        }),
        other => panic!("unknown kind {other}"),
    };
    assert_eq!(reencoded.is_ok(), ok, "{name}");
    if let Ok(bytes) = reencoded {
        assert_eq!(bytes, data, "{name}: encoder must reproduce the vector");
    }
}

fn check_fragment(name: &str, f: &[&str]) {
    let blob = bin(name);
    let packets = fragment(num(f[0]), num(f[1]), &unhex(f[3]), num(f[2])).unwrap();
    let mut expected = Vec::new();
    for packet in packets {
        expected.extend_from_slice(&(packet.len() as u16).to_le_bytes());
        expected.extend_from_slice(&packet);
    }
    assert_eq!(expected, blob, "{name}");
}

fn drop_bits(text: &str) -> u8 {
    if text == "-" {
        return 0;
    }
    text.split('+')
        .map(|part| 1 << Drops::NAMES.iter().position(|n| *n == part).expect("drop name"))
        .fold(0, |bits, bit| bits | bit)
}

fn check_stream(name: &str, expected: &str) {
    let blob = bin(name);
    let mut reassembler = Reassembler::new();
    let mut offset = 0;
    for step in expected.split(';') {
        let len = u16::from_le_bytes([blob[offset], blob[offset + 1]]) as usize;
        let time = u32::from_le_bytes(blob[offset + 2..offset + 6].try_into().unwrap());
        let result = reassembler.push(&blob[offset + 6..offset + 6 + len], time);
        offset += 6 + len;
        let (drops, status) = step.split_once('/').unwrap();
        assert_eq!(result.drops.0, drop_bits(drops), "{name}: {step}");
        if status == "-" {
            assert!(result.message.is_none(), "{name}: {step}");
        } else {
            let mut parts = status.splitn(3, ':');
            let message_type = u8::from_str_radix(parts.next().unwrap(), 16).unwrap();
            let seq = u8::from_str_radix(parts.next().unwrap(), 16).unwrap();
            let payload = parts.next().map(|h| if h.is_empty() { Vec::new() } else { unhex(h) }).unwrap();
            assert_eq!(result.message, Some(Message { message_type, seq, payload }), "{name}: {step}");
        }
    }
    assert_eq!(offset, blob.len(), "{name}");
}

#[test]
fn shared_vectors() {
    let manifest = fs::read_to_string(vector_dir().join("manifest.tsv")).expect("manifest");
    let mut count = 0;
    for line in manifest.lines().map(|l| l.trim_end_matches('\r')) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let (name, kind, ok) = (fields[0], fields[1], fields[2] == "ok");
        match kind {
            "fragment" => check_fragment(name, &fields[3..]),
            "stream" => check_stream(name, fields[3]),
            _ => check_payload(name, kind, ok, &fields[3..]),
        }
        count += 1;
    }
    assert!(count >= 50, "only {count} vectors found");
}

#[test]
fn edge_cases() {
    assert!(fragment(0x10, 0, &[1], 7).is_err());
    assert!(fragment(0x10, 0, &[0; MAX_MESSAGE + 1], 247).is_err());
    let mut r = Reassembler::new();
    assert!(r.push(&[1, 0x11, FLAG_START, 7, 5], u32::MAX - 10).message.is_none());
    let wrapped = r.push(&[1, 0x11, FLAG_END, 7, 0], 20);
    assert_eq!(wrapped.drops, Drops(0));
    assert_eq!(wrapped.message.unwrap().payload, vec![5, 0]);
    assert_eq!(MessageType::from_u8(0x50), None);
}
