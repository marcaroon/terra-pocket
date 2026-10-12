//! Terra BLE protocol V1: payload codecs, fragmentation and reassembly, plus
//! the V2 album artwork messages (§4.8).
//!
//! The wire format is specified in `docs/ble-protocol.md`. This crate is the
//! companion-side implementation; the firmware has an independent C codec and
//! both are tested against `docs/protocol-test-vectors`. No transport code lives
//! here and nothing trusts input lengths.

use std::fmt;

pub const VERSION: u8 = 1;
pub const HEADER_LEN: usize = 4;
pub const MAX_MESSAGE: usize = 512;
pub const REASSEMBLY_TIMEOUT_MS: u32 = 1000;

pub const FLAG_START: u8 = 0x01;
pub const FLAG_END: u8 = 0x02;
pub const FLAG_ACK_REQUEST: u8 = 0x04;
const KNOWN_FLAGS: u8 = FLAG_START | FLAG_END | FLAG_ACK_REQUEST;

pub const SOURCE_MAX: usize = 31;
pub const TITLE_MAX: usize = 127;
pub const ARTIST_MAX: usize = 95;
pub const ALBUM_MAX: usize = 95;

pub const CAP_MEDIA_STATE: u16 = 0x0001;
pub const CAP_VOLUME_STATE: u16 = 0x0002;
pub const CAP_MEDIA_COMMAND: u16 = 0x0004;
pub const CAP_ARTWORK: u16 = 0x0008;
const CAP_ALL: u16 = CAP_MEDIA_STATE | CAP_VOLUME_STATE | CAP_MEDIA_COMMAND | CAP_ARTWORK;

pub const ARTWORK_JPEG: u8 = 1;
pub const ARTWORK_SIDE: u16 = 160;
pub const ARTWORK_MAX_BYTES: u32 = 32_768;
pub const ARTWORK_CHUNK_HEADER: usize = 8;

/// Message type byte (spec §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    Hello = 0x01,
    HelloAck = 0x02,
    MediaState = 0x10,
    VolumeState = 0x11,
    MediaCommand = 0x20,
    DeviceStatus = 0x30,
    Error = 0x40,
    ArtworkBegin = 0x50,
    ArtworkChunk = 0x51,
    ArtworkEnd = 0x52,
}

impl MessageType {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0x01 => Self::Hello,
            0x02 => Self::HelloAck,
            0x10 => Self::MediaState,
            0x11 => Self::VolumeState,
            0x20 => Self::MediaCommand,
            0x30 => Self::DeviceStatus,
            0x40 => Self::Error,
            0x50 => Self::ArtworkBegin,
            0x51 => Self::ArtworkChunk,
            0x52 => Self::ArtworkEnd,
            _ => return None,
        })
    }
}

/// A payload that violates the V1 specification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Malformed;

impl fmt::Display for Malformed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("malformed protocol payload")
    }
}

impl std::error::Error for Malformed {}

type Result<T> = std::result::Result<T, Malformed>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Playback {
    NoMedia = 0,
    Playing = 1,
    Paused = 2,
}

impl Playback {
    fn from_u8(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::NoMedia),
            1 => Ok(Self::Playing),
            2 => Ok(Self::Paused),
            _ => Err(Malformed),
        }
    }
}

fn text(bytes: &[u8]) -> Result<String> {
    if bytes.contains(&0) {
        return Err(Malformed);
    }
    // std rejects overlong forms, surrogates and code points above U+10FFFF.
    std::str::from_utf8(bytes).map(str::to_owned).map_err(|_| Malformed)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaState {
    pub playback: Playback,
    pub position_ms: u32,
    pub duration_ms: u32,
    pub source: String,
    pub title: String,
    pub artist: String,
    pub album: String,
}

impl MediaState {
    pub fn encode(&self) -> Result<Vec<u8>> {
        let texts = [&self.source, &self.title, &self.artist, &self.album];
        let limits = [SOURCE_MAX, TITLE_MAX, ARTIST_MAX, ALBUM_MAX];
        let mut out = Vec::with_capacity(13 + texts.iter().map(|t| t.len()).sum::<usize>());
        out.push(self.playback as u8);
        out.extend_from_slice(&self.position_ms.to_le_bytes());
        out.extend_from_slice(&self.duration_ms.to_le_bytes());
        for (value, limit) in texts.iter().zip(limits) {
            if value.len() > limit || value.as_bytes().contains(&0) {
                return Err(Malformed);
            }
            out.push(value.len() as u8);
        }
        for value in texts {
            out.extend_from_slice(value.as_bytes());
        }
        Ok(out)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 13 {
            return Err(Malformed);
        }
        let playback = Playback::from_u8(data[0])?;
        let limits = [SOURCE_MAX, TITLE_MAX, ARTIST_MAX, ALBUM_MAX];
        let lengths: Vec<usize> = data[9..13].iter().map(|&n| n as usize).collect();
        if lengths.iter().zip(limits).any(|(&n, limit)| n > limit) {
            return Err(Malformed);
        }
        if data.len() != 13 + lengths.iter().sum::<usize>() {
            return Err(Malformed);
        }
        let mut offset = 13;
        let mut texts = Vec::with_capacity(4);
        for n in lengths {
            texts.push(text(&data[offset..offset + n])?);
            offset += n;
        }
        let mut texts = texts.into_iter();
        let mut next = || texts.next().unwrap_or_default();
        Ok(Self {
            playback,
            position_ms: u32::from_le_bytes(data[1..5].try_into().map_err(|_| Malformed)?),
            duration_ms: u32::from_le_bytes(data[5..9].try_into().map_err(|_| Malformed)?),
            source: next(),
            title: next(),
            artist: next(),
            album: next(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeState {
    pub volume: u8,
    pub muted: bool,
}

impl VolumeState {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.volume > 100 {
            return Err(Malformed);
        }
        Ok(vec![self.volume, u8::from(self.muted)])
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        match *data {
            [volume, muted] if volume <= 100 && muted <= 1 => Ok(Self { volume, muted: muted == 1 }),
            _ => Err(Malformed),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaCommand {
    PlayPause,
    Next,
    Previous,
    VolumeDelta(i16),
    MuteToggle,
}

impl MediaCommand {
    pub fn encode(&self) -> Result<Vec<u8>> {
        let (id, value) = match *self {
            Self::PlayPause => (0x01, 0),
            Self::Next => (0x02, 0),
            Self::Previous => (0x03, 0),
            Self::VolumeDelta(delta) if delta != 0 && (-100..=100).contains(&delta) => (0x04, delta),
            Self::VolumeDelta(_) => return Err(Malformed),
            Self::MuteToggle => (0x05, 0),
        };
        let mut out = vec![id];
        out.extend_from_slice(&value.to_le_bytes());
        Ok(out)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        let [id, low, high] = *data else { return Err(Malformed) };
        let value = i16::from_le_bytes([low, high]);
        match (id, value) {
            (0x01, 0) => Ok(Self::PlayPause),
            (0x02, 0) => Ok(Self::Next),
            (0x03, 0) => Ok(Self::Previous),
            (0x04, delta) if delta != 0 && (-100..=100).contains(&delta) => Ok(Self::VolumeDelta(delta)),
            (0x05, 0) => Ok(Self::MuteToggle),
            _ => Err(Malformed), // includes SEEK_DELTA, reserved in V1
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hello {
    pub protocol_version: u8,
    pub firmware: (u8, u8, u8),
    pub capabilities: u16,
}

impl Hello {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.capabilities & !CAP_ALL != 0 {
            return Err(Malformed);
        }
        let (major, minor, patch) = self.firmware;
        let mut out = vec![self.protocol_version, major, minor, patch];
        out.extend_from_slice(&self.capabilities.to_le_bytes());
        Ok(out)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        let [version, major, minor, patch, low, high] = *data else { return Err(Malformed) };
        let capabilities = u16::from_le_bytes([low, high]);
        if capabilities & !CAP_ALL != 0 {
            return Err(Malformed);
        }
        Ok(Self { protocol_version: version, firmware: (major, minor, patch), capabilities })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HelloAck {
    pub protocol_version: u8,
    pub companion: (u8, u8, u8),
    pub accepted: bool,
}

impl HelloAck {
    pub fn encode(&self) -> Vec<u8> {
        let (major, minor, patch) = self.companion;
        vec![self.protocol_version, major, minor, patch, u8::from(!self.accepted)]
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        match *data {
            [version, major, minor, patch, status] if status <= 1 => Ok(Self {
                protocol_version: version,
                companion: (major, minor, patch),
                accepted: status == 0,
            }),
            _ => Err(Malformed),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum LinkState {
    Advertising = 0,
    Connected = 1,
    Ready = 2,
    ProtocolMismatch = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceStatus {
    pub protocol_version: u8,
    pub firmware: (u8, u8, u8),
    pub link: LinkState,
    pub rx_errors: u16,
}

impl DeviceStatus {
    pub fn encode(&self) -> Vec<u8> {
        let (major, minor, patch) = self.firmware;
        let mut out = vec![self.protocol_version, major, minor, patch, self.link as u8, 0];
        out.extend_from_slice(&self.rx_errors.to_le_bytes());
        out
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        let [version, major, minor, patch, link, flags, low, high] = *data else { return Err(Malformed) };
        let link = match link {
            0 => LinkState::Advertising,
            1 => LinkState::Connected,
            2 => LinkState::Ready,
            3 => LinkState::ProtocolMismatch,
            _ => return Err(Malformed),
        };
        if flags != 0 {
            return Err(Malformed);
        }
        Ok(Self {
            protocol_version: version,
            firmware: (major, minor, patch),
            link,
            rx_errors: u16::from_le_bytes([low, high]),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ErrorReport {
    pub code: u8,
    pub related_type: u8,
    pub related_seq: u8,
}

impl ErrorReport {
    pub const UNSUPPORTED_VERSION: u8 = 1;
    pub const MALFORMED: u8 = 2;
    pub const UNSUPPORTED_TYPE: u8 = 3;
    pub const TOO_LARGE: u8 = 4;
    pub const NOT_READY: u8 = 5;

    pub fn encode(&self) -> Result<Vec<u8>> {
        if !(1..=5).contains(&self.code) {
            return Err(Malformed);
        }
        Ok(vec![self.code, self.related_type, self.related_seq])
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        match *data {
            [code, related_type, related_seq] if (1..=5).contains(&code) => {
                Ok(Self { code, related_type, related_seq })
            }
            _ => Err(Malformed),
        }
    }
}

/// ARTWORK_BEGIN (§4.8). `size == 0` with every other field zero means the
/// current track has no artwork.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtworkBegin {
    pub id: u32,
    pub format: u8,
    pub width: u16,
    pub height: u16,
    pub size: u32,
}

impl ArtworkBegin {
    pub const CLEAR: Self = Self { id: 0, format: 0, width: 0, height: 0, size: 0 };

    /// A 160x160 JPEG of `size` bytes.
    pub fn jpeg(id: u32, size: u32) -> Self {
        Self { id, format: ARTWORK_JPEG, width: ARTWORK_SIDE, height: ARTWORK_SIDE, size }
    }

    fn valid(&self) -> bool {
        if self.size == 0 {
            return *self == Self::CLEAR;
        }
        self.id != 0
            && self.format == ARTWORK_JPEG
            && (self.width, self.height) == (ARTWORK_SIDE, ARTWORK_SIDE)
            && self.size <= ARTWORK_MAX_BYTES
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        if !self.valid() {
            return Err(Malformed);
        }
        let mut out = self.id.to_le_bytes().to_vec();
        out.extend_from_slice(&[self.format, 0]);
        out.extend_from_slice(&self.width.to_le_bytes());
        out.extend_from_slice(&self.height.to_le_bytes());
        out.extend_from_slice(&self.size.to_le_bytes());
        Ok(out)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() != 14 || data[5] != 0 {
            return Err(Malformed);
        }
        let begin = Self {
            id: u32::from_le_bytes(data[0..4].try_into().unwrap()),
            format: data[4],
            width: u16::from_le_bytes([data[6], data[7]]),
            height: u16::from_le_bytes([data[8], data[9]]),
            size: u32::from_le_bytes(data[10..14].try_into().unwrap()),
        };
        if begin.valid() { Ok(begin) } else { Err(Malformed) }
    }
}

/// ARTWORK_CHUNK (§4.8): `data` lands at `offset` of the artwork `id`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtworkChunk {
    pub id: u32,
    pub offset: u32,
    pub data: Vec<u8>,
}

impl ArtworkChunk {
    fn valid(&self) -> bool {
        self.id != 0
            && !self.data.is_empty()
            && self.data.len() <= MAX_MESSAGE - ARTWORK_CHUNK_HEADER
            && u64::from(self.offset) + self.data.len() as u64 <= u64::from(ARTWORK_MAX_BYTES)
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        if !self.valid() {
            return Err(Malformed);
        }
        let mut out = self.id.to_le_bytes().to_vec();
        out.extend_from_slice(&self.offset.to_le_bytes());
        out.extend_from_slice(&self.data);
        Ok(out)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() <= ARTWORK_CHUNK_HEADER || data.len() > MAX_MESSAGE {
            return Err(Malformed);
        }
        let chunk = Self {
            id: u32::from_le_bytes(data[0..4].try_into().unwrap()),
            offset: u32::from_le_bytes(data[4..8].try_into().unwrap()),
            data: data[ARTWORK_CHUNK_HEADER..].to_vec(),
        };
        if chunk.valid() { Ok(chunk) } else { Err(Malformed) }
    }
}

/// ARTWORK_END (§4.8): the receiver checks the CRC of all received bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtworkEnd {
    pub id: u32,
    pub crc32: u32,
}

impl ArtworkEnd {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.id == 0 {
            return Err(Malformed);
        }
        let mut out = self.id.to_le_bytes().to_vec();
        out.extend_from_slice(&self.crc32.to_le_bytes());
        Ok(out)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() != 8 {
            return Err(Malformed);
        }
        let end = Self {
            id: u32::from_le_bytes(data[0..4].try_into().unwrap()),
            crc32: u32::from_le_bytes(data[4..8].try_into().unwrap()),
        };
        if end.id == 0 { Err(Malformed) } else { Ok(end) }
    }
}

/// CRC-32/ISO-HDLC (zlib), as carried by ARTWORK_END.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

/// Splits one message into packets for an ATT MTU (packet length = MTU - 3).
pub fn fragment(message_type: u8, seq: u8, payload: &[u8], mtu: u16) -> Result<Vec<Vec<u8>>> {
    let chunk = (mtu as usize).checked_sub(3 + HEADER_LEN).filter(|&c| c > 0).ok_or(Malformed)?;
    if payload.len() > MAX_MESSAGE {
        return Err(Malformed);
    }
    let pieces: Vec<&[u8]> = if payload.is_empty() { vec![&[]] } else { payload.chunks(chunk).collect() };
    let last = pieces.len() - 1;
    Ok(pieces
        .into_iter()
        .enumerate()
        .map(|(index, piece)| {
            let flags = if index == 0 { FLAG_START } else { 0 } | if index == last { FLAG_END } else { 0 };
            let mut packet = vec![VERSION, message_type, flags, seq];
            packet.extend_from_slice(piece);
            packet
        })
        .collect())
}

/// Reassembly drop reasons (spec §2); several may apply to one packet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Drops(pub u8);

impl Drops {
    pub const BAD_PACKET: u8 = 1 << 0;
    pub const BAD_VERSION: u8 = 1 << 1;
    pub const INCOMPLETE: u8 = 1 << 2;
    pub const NO_START: u8 = 1 << 3;
    pub const MISMATCH: u8 = 1 << 4;
    pub const TOO_LARGE: u8 = 1 << 5;
    pub const TIMEOUT: u8 = 1 << 6;
    pub const NAMES: [&'static str; 7] =
        ["BAD_PACKET", "BAD_VERSION", "INCOMPLETE", "NO_START", "MISMATCH", "TOO_LARGE", "TIMEOUT"];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub message_type: u8,
    pub seq: u8,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PushResult {
    pub drops: Drops,
    pub message: Option<Message>,
}

#[derive(Debug, Default)]
pub struct Reassembler {
    active: Option<(u8, u8)>,
    buffer: Vec<u8>,
    last_ms: u32,
}

impl Reassembler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.active = None;
        self.buffer.clear();
    }

    pub fn push(&mut self, packet: &[u8], now_ms: u32) -> PushResult {
        let mut drops = 0u8;
        let result = |drops: u8, message: Option<Message>| PushResult { drops: Drops(drops), message };
        if self.active.is_some() && now_ms.wrapping_sub(self.last_ms) > REASSEMBLY_TIMEOUT_MS {
            drops |= Drops::TIMEOUT;
            self.reset();
        }
        if packet.len() < HEADER_LEN || packet[2] & !KNOWN_FLAGS != 0 {
            return result(drops | Drops::BAD_PACKET, None);
        }
        if packet[0] != VERSION {
            return result(drops | Drops::BAD_VERSION, None);
        }
        let (message_type, flags, seq, body) = (packet[1], packet[2], packet[3], &packet[HEADER_LEN..]);
        if flags & FLAG_START != 0 {
            if self.active.is_some() {
                drops |= Drops::INCOMPLETE;
            }
            self.active = Some((message_type, seq));
            self.buffer.clear();
        } else {
            match self.active {
                None => return result(drops | Drops::NO_START, None),
                Some(active) if active != (message_type, seq) => {
                    self.reset();
                    return result(drops | Drops::MISMATCH, None);
                }
                Some(_) => {}
            }
        }
        if self.buffer.len() + body.len() > MAX_MESSAGE {
            self.reset();
            return result(drops | Drops::TOO_LARGE, None);
        }
        self.buffer.extend_from_slice(body);
        self.last_ms = now_ms;
        if flags & FLAG_END == 0 {
            return result(drops, None);
        }
        let (message_type, seq) = self.active.take().unwrap_or((message_type, seq));
        let payload = std::mem::take(&mut self.buffer);
        result(drops, Some(Message { message_type, seq, payload }))
    }
}
