//! One synchronization session: scan, connect, handshake, then mirror the
//! Windows media session and master volume to Terra and route its buttons and
//! encoder (M6–M8). Reconnect with backoff is M9.

use std::fmt;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use audio_windows::{AudioError, MasterVolume, VolumeLevel};
use ble_windows::{BleError, Link, LinkEvent};
use media_windows::{ControlCommand, ControlOutcome, MediaError, MediaReader, MediaSnapshot, Playback as WinPlayback};
use protocol::{
    DeviceStatus, ErrorReport, Hello, HelloAck, MediaCommand, MediaState, MessageType, Reassembler, VERSION,
    VolumeState, fragment,
};

use crate::sync::{MediaInfo, MediaPlayback, media_state, needs_send};

const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
/// Events often arrive in bursts (metadata, then playback, then timeline).
const SETTLE: Duration = Duration::from_millis(150);
/// A dragged Windows slider fires many volume events; the encoder wants a quick echo.
const VOLUME_SETTLE: Duration = Duration::from_millis(50);
/// Windows reports no volume event when the default playback device changes.
const DEVICE_POLL: Duration = Duration::from_secs(1);
const IDLE: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub enum SessionError {
    Ble(BleError),
    Media(MediaError),
    Audio(AudioError),
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
            Self::Audio(error) => write!(f, "{error}"),
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

impl From<AudioError> for SessionError {
    fn from(error: AudioError) -> Self {
        Self::Audio(error)
    }
}

pub struct SessionOptions {
    pub address: Option<u64>,
    pub scan_timeout: Duration,
}

enum Event {
    Link(LinkEvent),
    MediaChanged,
    VolumeChanged,
}

/// Deadlines of coalesced Windows signals that still need a read.
#[derive(Default)]
struct Pending {
    media: Option<Instant>,
    volume: Option<Instant>,
}

impl Pending {
    fn take_due(&mut self, now: Instant) -> Option<Event> {
        if self.volume.is_some_and(|at| at <= now) {
            self.volume = None;
            return Some(Event::VolumeChanged);
        }
        if self.media.is_some_and(|at| at <= now) {
            self.media = None;
            return Some(Event::MediaChanged);
        }
        None
    }
}

// Coalesce only Windows signals. BLE packets and disconnects must keep their order.
fn next_event(rx: &mpsc::Receiver<Event>, pending: &mut Pending) -> Result<Event, RecvTimeoutError> {
    loop {
        let now = Instant::now();
        if let Some(event) = pending.take_due(now) {
            return Ok(event);
        }
        let wait = [pending.media, pending.volume].into_iter().flatten().min().map_or(IDLE, |at| at - now);
        match rx.recv_timeout(wait) {
            Ok(Event::MediaChanged) => {
                pending.media.get_or_insert_with(|| Instant::now() + SETTLE);
            }
            Ok(Event::VolumeChanged) => {
                pending.volume.get_or_insert_with(|| Instant::now() + VOLUME_SETTLE);
            }
            Err(RecvTimeoutError::Timeout) if pending.media.is_some() || pending.volume.is_some() => {}
            event => return event,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AudioRequest {
    Step(i16),
    ToggleMute,
}

fn describe_volume(level: VolumeLevel) -> String {
    if level.muted { format!("{} muted", level.percent) } else { level.percent.to_string() }
}

/// Executes one valid MEDIA_COMMAND exactly once and describes the result.
fn dispatch_command(
    payload: &[u8],
    mut media: impl FnMut(ControlCommand) -> Result<ControlOutcome, MediaError>,
    mut audio: impl FnMut(AudioRequest) -> Result<Option<VolumeLevel>, AudioError>,
) -> String {
    let command = match MediaCommand::decode(payload) {
        Ok(MediaCommand::PlayPause) => ControlCommand::PlayPause,
        Ok(MediaCommand::Next) => ControlCommand::Next,
        Ok(MediaCommand::Previous) => ControlCommand::Previous,
        Ok(MediaCommand::VolumeDelta(delta)) => {
            return audio_result(&format!("VolumeDelta({delta:+})"), audio(AudioRequest::Step(delta)));
        }
        Ok(MediaCommand::MuteToggle) => return audio_result("MuteToggle", audio(AudioRequest::ToggleMute)),
        Err(_) => return "[WARN] malformed MEDIA_COMMAND ignored".into(),
    };
    match media(command) {
        Ok(outcome) => format!("[COMMAND] {command:?}: {outcome:?}"),
        Err(error) => format!("[COMMAND] {command:?} failed: {error}"),
    }
}

fn audio_result(name: &str, result: Result<Option<VolumeLevel>, AudioError>) -> String {
    match result {
        Ok(Some(level)) => format!("[COMMAND] {name}: now {}", describe_volume(level)),
        Ok(None) => format!("[COMMAND] {name}: no playback device"),
        Err(error) => format!("[COMMAND] {name} failed: {error}"),
    }
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

/// Mirrors the Windows master volume; remembers what was last sent.
struct VolumeSync {
    audio: MasterVolume,
    last: Option<VolumeLevel>,
}

impl VolumeSync {
    /// Sends VOLUME_STATE when the level Windows reports differs from the last
    /// one sent. Audio read failures are logged; they never end the session.
    fn sync(&mut self, sender: &mut Sender, log: &mut impl FnMut(String)) -> Result<(), SessionError> {
        let level = match self.audio.refresh().and_then(|_| self.audio.level()) {
            Ok(level) => level,
            Err(error) => {
                log(format!("[WARN] volume read failed: {error}"));
                return Ok(());
            }
        };
        let Some(level) = level else {
            if self.last.take().is_some() {
                log("[VOLUME] no playback device; Terra keeps the last level".into());
            }
            return Ok(());
        };
        if self.last == Some(level) {
            return Ok(());
        }
        let payload = VolumeState { volume: level.percent, muted: level.muted }
            .encode()
            .map_err(|_| SessionError::Protocol("VOLUME_STATE encode"))?;
        sender.send(MessageType::VolumeState, &payload)?;
        log(format!("[VOLUME] {}", describe_volume(level)));
        self.last = Some(level);
        Ok(())
    }

    fn execute(&mut self, request: AudioRequest) -> Result<Option<VolumeLevel>, AudioError> {
        match request {
            AudioRequest::Step(delta) => self.audio.step(delta),
            AudioRequest::ToggleMute => self.audio.toggle_mute(),
        }
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
            let found = ble_windows::scan(options.scan_timeout, false)?;
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
        if remaining.is_zero() { return Err(SessionError::NoHello); }
        match rx.recv_timeout(remaining) {
            Ok(Event::Link(LinkEvent::Packet(packet))) => {
                if let Some(message) = reassembler.push(&packet, now_ms(start)).message {
                    if message.message_type == MessageType::Hello as u8 {
                        break Hello::decode(&message.payload).map_err(|_| SessionError::Protocol("bad HELLO"))?;
                    }
                }
            }
            Ok(Event::Link(LinkEvent::Disconnected)) => return Err(SessionError::Protocol("disconnected during handshake")),
            Ok(Event::MediaChanged | Event::VolumeChanged) => {}
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
    let audio = MasterVolume::new(move || {
        let _ = tx.send(Event::VolumeChanged);
    })?;

    let mut media = MediaSync { reader: &reader, watch, last: None };
    let mut volume = VolumeSync { audio, last: None };
    media.sync(&mut sender, &mut log)?;
    volume.sync(&mut sender, &mut log)?;

    let mut pending = Pending::default();
    let mut polled = Instant::now();
    loop {
        if polled.elapsed() >= DEVICE_POLL {
            polled = Instant::now();
            volume.sync(&mut sender, &mut log)?; // follows a changed default playback device
        }
        match next_event(&rx, &mut pending) {
            Ok(Event::MediaChanged) => {
                media.sync(&mut sender, &mut log)?;
            }
            Ok(Event::VolumeChanged) => {
                volume.sync(&mut sender, &mut log)?;
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
                        let hello = Hello::decode(&message.payload).map_err(|_| SessionError::Protocol("bad HELLO"))?;
                        let accepted = hello.protocol_version == VERSION;
                        let ack = HelloAck { protocol_version: VERSION, companion, accepted };
                        sender.send(MessageType::HelloAck, &ack.encode())?;
                        if !accepted {
                            return Err(SessionError::ProtocolMismatch { device_version: hello.protocol_version });
                        }
                        reassembler.reset();
                        media.last = None;
                        volume.last = None;
                        log("[PROTOCOL] HELLO again; acknowledged".into());
                        media.sync(&mut sender, &mut log)?;
                        volume.sync(&mut sender, &mut log)?;
                    }
                    Some(MessageType::MediaCommand) => {
                        let mut media_request = false;
                        let line = dispatch_command(
                            &message.payload,
                            |command| {
                                media_request = true;
                                reader.control(command)
                            },
                            |request| volume.execute(request),
                        );
                        log(line);
                        // Report what Windows actually has afterwards; never echo the request.
                        if media_request {
                            pending.media.get_or_insert_with(|| Instant::now() + SETTLE);
                        } else {
                            volume.sync(&mut sender, &mut log)?;
                        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(command: MediaCommand) -> Vec<u8> {
        fragment(MessageType::MediaCommand as u8, 1, &command.encode().unwrap(), 23).unwrap().remove(0)
    }

    #[test]
    fn windows_bursts_preserve_commands_errors_and_disconnect_order() {
        let (tx, rx) = mpsc::channel();
        let mut pending = Pending::default();
        tx.send(Event::MediaChanged).unwrap();
        tx.send(Event::VolumeChanged).unwrap();
        for command in [MediaCommand::PlayPause, MediaCommand::VolumeDelta(2), MediaCommand::MuteToggle] {
            let packet = packet(command);
            tx.send(Event::Link(LinkEvent::Packet(packet.clone()))).unwrap();
            tx.send(Event::MediaChanged).unwrap();
            tx.send(Event::VolumeChanged).unwrap();
            assert!(matches!(next_event(&rx, &mut pending).unwrap(), Event::Link(LinkEvent::Packet(bytes)) if bytes == packet));
        }
        tx.send(Event::Link(LinkEvent::Packet(vec![1, 0x40, 3, 2, 2, 0x10, 1]))).unwrap();
        tx.send(Event::Link(LinkEvent::Disconnected)).unwrap();
        assert!(matches!(next_event(&rx, &mut pending).unwrap(), Event::Link(LinkEvent::Packet(_))));
        assert!(matches!(next_event(&rx, &mut pending).unwrap(), Event::Link(LinkEvent::Disconnected)));
        // Each burst collapsed into one pending read per kind; volume settles first.
        assert!(pending.media.is_some() && pending.volume.is_some());
        assert!(matches!(next_event(&rx, &mut pending).unwrap(), Event::VolumeChanged));
        assert!(matches!(next_event(&rx, &mut pending).unwrap(), Event::MediaChanged));
        assert!(pending.media.is_none() && pending.volume.is_none());
    }

    #[test]
    fn only_valid_media_buttons_execute_once() {
        for (wire, expected) in [(MediaCommand::PlayPause, ControlCommand::PlayPause),
            (MediaCommand::Next, ControlCommand::Next), (MediaCommand::Previous, ControlCommand::Previous)] {
            for outcome in [ControlOutcome::Accepted, ControlOutcome::Rejected, ControlOutcome::Unsupported, ControlOutcome::NoSession] {
                let mut calls = 0;
                let log = dispatch_command(&wire.encode().unwrap(), |actual| {
                    assert_eq!(actual, expected);
                    calls += 1;
                    Ok(outcome)
                }, |_| panic!("media buttons must not touch the volume"));
                assert_eq!(calls, 1);
                assert!(log.contains(&format!("{outcome:?}")));
            }
        }
        for payload in [vec![], vec![1, 1, 0], vec![6, 0, 0], vec![2, 0, 0, 0], vec![4, 0, 0], vec![4, 101, 0],
            vec![5, 1, 0], vec![7, 0, 0]] {
            dispatch_command(&payload, |_| panic!("malformed command must not execute"),
                |_| panic!("malformed command must not execute"));
        }
    }

    #[test]
    fn encoder_commands_route_to_the_volume_once() {
        let level = VolumeLevel { percent: 74, muted: false };
        for (wire, expected) in [(MediaCommand::VolumeDelta(2), AudioRequest::Step(2)),
            (MediaCommand::VolumeDelta(-2), AudioRequest::Step(-2)), (MediaCommand::MuteToggle, AudioRequest::ToggleMute)] {
            let mut calls = 0;
            let log = dispatch_command(&wire.encode().unwrap(), |_| panic!("encoder must not control playback"),
                |request| {
                    assert_eq!(request, expected);
                    calls += 1;
                    Ok(Some(level))
                });
            assert_eq!(calls, 1);
            assert!(log.ends_with("now 74"), "{log}");
        }
        let log = dispatch_command(&MediaCommand::MuteToggle.encode().unwrap(), |_| unreachable!(), |_| Ok(None));
        assert!(log.contains("no playback device"));
        assert_eq!(describe_volume(VolumeLevel { percent: 0, muted: true }), "0 muted");
    }
}
