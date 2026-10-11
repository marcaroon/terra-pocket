//! One synchronization session: scan, connect, handshake, then mirror the
//! Windows media session to Terra (M6). Reconnect with backoff is M9; commands
//! from the device are M7; volume is M8.

use std::fmt;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use ble_windows::{BleError, Link, LinkEvent};
use media_windows::{MediaError, MediaReader, MediaSnapshot, Playback as WinPlayback};
use protocol::{
    DeviceStatus, ErrorReport, Hello, HelloAck, MediaState, MessageType, Reassembler, VERSION, fragment,
};

use crate::sync::{MediaInfo, MediaPlayback, media_state, needs_send};

const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
/// Events often arrive in bursts (metadata, then playback, then timeline).
const SETTLE: Duration = Duration::from_millis(150);

#[derive(Debug)]
pub enum SessionError {
    Ble(BleError),
    Media(MediaError),
    NotFound,
    NoHello,
    ProtocolMismatch { device_version: u8 },
    Protocol(&'static str),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ble(error) => write!(f, "{error}"),
            Self::Media(error) => write!(f, "{error}"),
            Self::NotFound => f.write_str("Terra not found; is it powered and advertising?"),
            Self::NoHello => f.write_str("no HELLO from the device within 5 s"),
            Self::ProtocolMismatch { device_version } => write!(
                f,
                "protocol mismatch: device speaks version {device_version}, companion speaks {VERSION}"
            ),
            Self::Protocol(what) => write!(f, "protocol error: {what}"),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<BleError> for SessionError {
    fn from(error: BleError) -> Self {
        Self::Ble(error)
    }
}

impl From<MediaError> for SessionError {
    fn from(error: MediaError) -> Self {
        Self::Media(error)
    }
}

pub struct SessionOptions {
    pub address: Option<u64>,
    pub scan_timeout: Duration,
}

enum Event {
    Link(LinkEvent),
    MediaChanged,
}

fn media_info(snapshot: &MediaSnapshot) -> MediaInfo {
    let playback = match snapshot.playback {
        WinPlayback::Playing => MediaPlayback::Playing,
        WinPlayback::Paused => MediaPlayback::Paused,
        WinPlayback::Changing => MediaPlayback::Changing,
        WinPlayback::Closed | WinPlayback::Opened | WinPlayback::Stopped => MediaPlayback::Stopped,
    };
    MediaInfo {
        source: snapshot.source.clone(),
        title: snapshot.title.clone(),
        artist: snapshot.artist.clone(),
        album: snapshot.album.clone(),
        playback,
        position_ms: snapshot.position_ms,
        duration_ms: snapshot.duration_ms,
    }
}

fn describe(state: &MediaState) -> String {
    match state.playback {
        protocol::Playback::NoMedia => "nothing playing".into(),
        playback => format!(
            "{} - {} - {} ({:?}, {:.1}s / {:.1}s)",
            state.source,
            state.artist,
            state.title,
            playback,
            state.position_ms as f64 / 1000.0,
            state.duration_ms as f64 / 1000.0
        ),
    }
}

struct Sender<'a> {
    link: &'a Link,
    seq: u8,
}

impl Sender<'_> {
    fn send(&mut self, message_type: MessageType, payload: &[u8]) -> Result<(), SessionError> {
        let mtu = (self.link.max_packet() + 3) as u16;
        let packets =
            fragment(message_type as u8, self.seq, payload, mtu).map_err(|_| SessionError::Protocol("fragment"))?;
        self.seq = self.seq.wrapping_add(1);
        for packet in packets {
            self.link.write(&packet)?;
        }
        Ok(())
    }
}

/// Mirrors the Windows media session; remembers what was last sent.
struct MediaSync<'a> {
    reader: &'a MediaReader,
    watch: media_windows::MediaWatch,
    last: Option<(MediaState, Instant)>,
}

impl MediaSync<'_> {
    fn sync(&mut self, sender: &mut Sender, log: &mut impl FnMut(String)) -> Result<(), SessionError> {
        if let Err(error) = self.watch.sync() {
            log(format!("[WARN] media resubscribe failed: {error}"));
        }
        let snapshot = self.reader.current()?;
        let Some(state) = media_state(snapshot.as_ref().map(media_info).as_ref()) else { return Ok(()) };
        let elapsed = self.last.as_ref().map(|(_, at)| at.elapsed().as_millis() as u64).unwrap_or(0);
        if !needs_send(self.last.as_ref().map(|(s, _)| s), &state, elapsed) {
            return Ok(());
        }
        let payload = state.encode().map_err(|_| SessionError::Protocol("MEDIA_STATE encode"))?;
        sender.send(MessageType::MediaState, &payload)?;
        log(format!("[MEDIA] {}", describe(&state)));
        self.last = Some((state, Instant::now()));
        Ok(())
    }
}

fn now_ms(start: Instant) -> u32 {
    start.elapsed().as_millis() as u32
}

/// Runs until the link drops (returns `Ok`) or an error stops the session.
pub fn run(options: &SessionOptions, mut log: impl FnMut(String)) -> Result<(), SessionError> {
    let reader = MediaReader::new()?;
    let address = match options.address {
        Some(address) => address,
        None => {
            log("[BLE] scanning for Terra".into());
            let found = ble_windows::scan(options.scan_timeout, true)?;
            let device = found.into_iter().max_by_key(|f| f.rssi).ok_or(SessionError::NotFound)?;
            log(format!("[BLE] found {} {} RSSI {} dBm", device.name, device.address_text(), device.rssi));
            device.address
        }
    };

    let (tx, rx) = mpsc::channel::<Event>();
    let link_tx = tx.clone();
    let link = Link::connect(address, move |event| {
        let _ = link_tx.send(Event::Link(event));
    })?;
    log(format!("[BLE] connected, packet size {} bytes", link.max_packet()));

    let start = Instant::now();
    let mut reassembler = Reassembler::new();
    let mut sender = Sender { link: &link, seq: 0 };

    // Handshake (spec §5): HELLO arrives once TX notifications are enabled.
    let deadline = Instant::now() + HELLO_TIMEOUT;
    let hello = loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(Event::Link(LinkEvent::Packet(packet))) => {
                if let Some(message) = reassembler.push(&packet, now_ms(start)).message {
                    if message.message_type == MessageType::Hello as u8 {
                        break Hello::decode(&message.payload).map_err(|_| SessionError::Protocol("bad HELLO"))?;
                    }
                }
            }
            Ok(Event::Link(LinkEvent::Disconnected)) => return Err(SessionError::Protocol("disconnected during handshake")),
            Ok(Event::MediaChanged) => {}
            Err(_) => return Err(SessionError::NoHello),
        }
    };
    let (major, minor, patch) = hello.firmware;
    log(format!("[PROTOCOL] HELLO protocol={} firmware={major}.{minor}.{patch}", hello.protocol_version));
    let companion = crate::version_triple();
    let accepted = hello.protocol_version == VERSION;
    let ack = HelloAck { protocol_version: VERSION, companion, accepted };
    sender.send(MessageType::HelloAck, &ack.encode())?;
    if !accepted {
        return Err(SessionError::ProtocolMismatch { device_version: hello.protocol_version });
    }
    if let Ok(status) = link.read_status().map(|bytes| DeviceStatus::decode(&bytes)) {
        if let Ok(status) = status {
            log(format!("[PROTOCOL] device link state {:?}, rx_errors {}", status.link, status.rx_errors));
        }
    }

    let media_tx = tx.clone();
    let watch = reader.watch(move || {
        let _ = media_tx.send(Event::MediaChanged);
    })?;
    drop(tx);

    let mut media = MediaSync { reader: &reader, watch, last: None };
    media.sync(&mut sender, &mut log)?;

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Event::MediaChanged) => {
                std::thread::sleep(SETTLE);
                let mut disconnected = false;
                while let Ok(event) = rx.try_recv() {
                    if let Event::Link(LinkEvent::Disconnected) = event {
                        disconnected = true;
                    }
                }
                if disconnected {
                    log("[BLE] disconnected".into());
                    return Ok(());
                }
                media.sync(&mut sender, &mut log)?;
            }
            Ok(Event::Link(LinkEvent::Packet(packet))) => {
                let result = reassembler.push(&packet, now_ms(start));
                if result.drops.0 != 0 {
                    log(format!("[WARN] notification dropped (reasons 0x{:02x})", result.drops.0));
                }
                let Some(message) = result.message else { continue };
                match MessageType::from_u8(message.message_type) {
                    Some(MessageType::Error) => match ErrorReport::decode(&message.payload) {
                        Ok(error) => log(format!(
                            "[DEVICE] rejected type 0x{:02x} seq {} (error {})",
                            error.related_type, error.related_seq, error.code
                        )),
                        Err(_) => log("[WARN] malformed ERROR from device".into()),
                    },
                    Some(MessageType::Hello) => {
                        // The device restarts the handshake after a TX re-subscribe.
                        sender.send(MessageType::HelloAck, &ack.encode())?;
                        media.last = None;
                        log("[PROTOCOL] HELLO again; acknowledged".into());
                        media.sync(&mut sender, &mut log)?;
                    }
                    other => log(format!("[WARN] unexpected message {other:?} from device")),
                }
            }
            Ok(Event::Link(LinkEvent::Disconnected)) => {
                log("[BLE] disconnected".into());
                return Ok(());
            }
            Err(RecvTimeoutError::Timeout) => {
                if !link.is_connected() {
                    log("[BLE] disconnected".into());
                    return Ok(());
                }
            }
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}
