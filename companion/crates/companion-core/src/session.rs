//! Synchronization sessions: scan, connect, handshake, then mirror the Windows
//! media session and master volume to Terra and route its buttons and encoder
//! (M6–M8). `run_forever` reconnects with bounded backoff after any loss (M9).
//! Terra firmware that offers capability bit3 also receives album artwork (V2).

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

use audio_windows::{AudioError, MasterVolume};
pub use audio_windows::VolumeLevel;
pub use protocol::{ARTWORK_MAX_BYTES, ARTWORK_SIDE, MediaState, Playback};
use ble_windows::{BleError, Link, LinkEvent};
use media_windows::{ControlCommand, ControlOutcome, MediaError, MediaReader, MediaSnapshot, Playback as WinPlayback};
use protocol::{
    ARTIST_MAX, ARTWORK_CHUNK_HEADER, ArtworkBegin, ArtworkChunk, ArtworkEnd, CAP_ARTWORK, DeviceStatus, ErrorReport,
    HEADER_LEN, Hello, HelloAck, MediaCommand, MessageType, Reassembler, TITLE_MAX, VERSION, VolumeState, crc32,
    fragment,
};

use crate::sync::{Backoff, MediaInfo, MediaPlayback, media_state, needs_send, no_media_due, truncate_utf8};

const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
/// Events often arrive in bursts (metadata, then playback, then timeline).
const SETTLE: Duration = Duration::from_millis(150);
/// A dragged Windows slider fires many volume events; the encoder wants a quick echo.
const VOLUME_SETTLE: Duration = Duration::from_millis(50);
/// Windows reports no volume event when the default playback device changes.
const DEVICE_POLL: Duration = Duration::from_secs(1);
const IDLE: Duration = Duration::from_secs(1);
/// Retry a failed media read; apps can vanish while their session is read.
const MEDIA_RETRY: Duration = Duration::from_secs(1);
const NO_MEDIA_GRACE: Duration = Duration::from_millis(crate::sync::NO_MEDIA_GRACE_MS);
/// Apps often publish the thumbnail shortly after the metadata: look again
/// every second for this long after a track change, then only on media events.
const ARTWORK_RETRY: Duration = Duration::from_secs(1);
const ARTWORK_EAGER: Duration = Duration::from_secs(10);
/// Largest chunk that still fits one notification at the firmware's MTU 247.
const ARTWORK_CHUNK_MAX: usize = 232;

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

/// Where the link stands, for frontends (spec §29 companion states).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Scanning,
    Connecting { address: u64 },
    Ready { address: u64, firmware: (u8, u8, u8), protocol: u8 },
    /// Disconnected; the next attempt starts after `retry_in_s`.
    Waiting { retry_in_s: u64 },
    Stopped,
    Mismatch { device_version: u8 },
}

/// Everything a session reports. The CLI prints `Log`; a GUI also uses the rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Update {
    Log(String),
    Phase(Phase),
    /// MEDIA_STATE as last sent to Terra.
    Media(MediaState),
    /// VOLUME_STATE as last sent; `None` when Windows has no playback device.
    Volume(Option<VolumeLevel>),
}

/// Wraps the frontend callback.
pub struct Reporter<F: FnMut(Update)>(pub F);

impl<F: FnMut(Update)> Reporter<F> {
    fn log(&mut self, line: String) {
        (self.0)(Update::Log(line));
    }

    fn update(&mut self, update: Update) {
        (self.0)(update);
    }
}

/// Requests from a frontend to a running `run_forever`, checked at least once
/// per second while connected and every 100 ms while waiting to reconnect.
#[derive(Debug, Default)]
pub struct Control {
    stop: AtomicBool,
    reconnect: AtomicBool,
}

impl Control {
    /// Ends the current session and `run_forever`.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// Ends the current session or wait and starts a new attempt immediately.
    pub fn reconnect(&self) {
        self.reconnect.store(true, Ordering::SeqCst);
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    fn interrupted(&self) -> bool {
        self.stopped() || self.reconnect.load(Ordering::SeqCst)
    }

    fn take_reconnect(&self) -> bool {
        self.reconnect.swap(false, Ordering::SeqCst)
    }
}

/// Why `run_forever` returned.
#[derive(Debug)]
pub enum Exit {
    Stopped,
    /// Retrying cannot fix this without updating the firmware or companion.
    Mismatch(SessionError),
}

/// What one session leaves for the next reconnect attempt.
#[derive(Debug, Default)]
pub struct Memory {
    /// Last Terra connected to; preferred when several advertise.
    pub address: Option<u64>,
    /// The last session completed the handshake.
    pub ready: bool,
}

enum Event {
    Link(LinkEvent),
    MediaChanged,
    VolumeChanged,
    /// Not queued: returned when artwork work is due and nothing else is waiting.
    ArtworkDue,
}

/// Deadlines of coalesced Windows signals that still need a read.
#[derive(Default)]
struct Pending {
    media: Option<Instant>,
    volume: Option<Instant>,
    /// Lowest priority: only taken when no queued event is waiting.
    artwork: Option<Instant>,
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
        let wait = [pending.media, pending.volume, pending.artwork]
            .into_iter()
            .flatten()
            .min()
            .map_or(IDLE, |at| at.saturating_duration_since(now));
        // A Windows event always gets a read within its settle time, even when a
        // later recheck (e.g. the NO_MEDIA grace period) is already pending.
        match rx.recv_timeout(wait) {
            Ok(Event::MediaChanged) => schedule(&mut pending.media, Some(Instant::now() + SETTLE)),
            Ok(Event::VolumeChanged) => schedule(&mut pending.volume, Some(Instant::now() + VOLUME_SETTLE)),
            Err(RecvTimeoutError::Timeout) if pending.artwork.is_some_and(|at| at <= Instant::now()) => {
                pending.artwork = None;
                return Ok(Event::ArtworkDue);
            }
            Err(RecvTimeoutError::Timeout)
                if pending.media.is_some() || pending.volume.is_some() || pending.artwork.is_some() => {}
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

    /// Room for message payload in one packet at the current MTU.
    fn packet_payload(&self) -> usize {
        self.link.max_packet() - HEADER_LEN
    }
}

/// Mirrors the Windows media session; remembers what was last sent.
struct MediaSync<'a> {
    reader: &'a MediaReader,
    watch: media_windows::MediaWatch,
    last: Option<(MediaState, Instant)>,
    /// Since when Windows has reported no playing/paused session.
    absent_since: Option<Instant>,
}

impl MediaSync<'_> {
    /// Sends MEDIA_STATE when it changed meaningfully. Returns when to read
    /// again without a Windows event: after a failed read, or when a pending
    /// NO_MEDIA has waited out its grace period.
    fn sync(&mut self, sender: &mut Sender, out: &mut Reporter<impl FnMut(Update)>) -> Result<Option<Instant>, SessionError> {
        if let Err(error) = self.watch.sync() {
            out.log(format!("[WARN] media resubscribe failed: {error}"));
        }
        let snapshot = match self.reader.current() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                out.log(format!("[WARN] media read failed: {error}"));
                return Ok(Some(Instant::now() + MEDIA_RETRY));
            }
        };
        let Some(state) = media_state(snapshot.as_ref().map(media_info).as_ref()) else { return Ok(None) };
        if state.playback == protocol::Playback::NoMedia {
            let since = *self.absent_since.get_or_insert_with(Instant::now);
            if !no_media_due(self.last.as_ref().map(|(s, _)| s), since.elapsed().as_millis() as u64) {
                return Ok(Some(since + NO_MEDIA_GRACE));
            }
        } else {
            self.absent_since = None;
        }
        let elapsed = self.last.as_ref().map(|(_, at)| at.elapsed().as_millis() as u64).unwrap_or(0);
        if !needs_send(self.last.as_ref().map(|(s, _)| s), &state, elapsed) {
            return Ok(None);
        }
        let payload = state.encode().map_err(|_| SessionError::Protocol("MEDIA_STATE encode"))?;
        sender.send(MessageType::MediaState, &payload)?;
        out.log(format!("[MEDIA] {}", describe(&state)));
        out.update(Update::Media(state.clone()));
        self.last = Some((state, Instant::now()));
        Ok(None)
    }
}

/// Keeps the earlier of two deadlines.
fn schedule(slot: &mut Option<Instant>, at: Option<Instant>) {
    if let Some(at) = at {
        *slot = Some(slot.map_or(at, |current| current.min(at)));
    }
}

/// Identity of the track Terra shows: (source, title, artist, album) as sent.
type TrackKey = (String, String, String, String);

struct Upload {
    id: u32,
    jpeg: Vec<u8>,
    offset: usize,
    started: Instant,
    chunks: u32,
    /// Time spent inside acknowledged writes, to tell link speed from scheduling.
    writing: Duration,
    slowest: Duration,
}

/// Sends the shown track's artwork once (V2). Chunks go out one per loop pass so
/// buttons and Windows events are never queued behind a whole image.
struct ArtworkSync {
    enabled: bool,
    track: Option<TrackKey>,
    since: Instant,
    /// Artwork sent (or abandoned after a device error) for this track.
    done: bool,
    next_try: Option<Instant>,
    upload: Option<Upload>,
}

impl ArtworkSync {
    fn new(enabled: bool) -> Self {
        Self { enabled, track: None, since: Instant::now(), done: false, next_try: None, upload: None }
    }

    /// After every media sync, with the MEDIA_STATE Terra now shows. A track
    /// change makes Terra drop its artwork and any partial transfer.
    fn on_media(&mut self, shown: Option<&MediaState>) {
        let key = shown
            .filter(|state| state.playback != Playback::NoMedia)
            .map(|s| (s.source.clone(), s.title.clone(), s.artist.clone(), s.album.clone()));
        if !self.enabled || key == self.track {
            return;
        }
        self.track = key;
        self.since = Instant::now();
        self.done = false;
        self.upload = None;
        self.next_try = self.track.as_ref().map(|_| Instant::now());
    }

    /// A Windows media event: a late thumbnail may have arrived.
    fn on_media_event(&mut self) {
        if self.track.is_some() && !self.done && self.upload.is_none() {
            schedule(&mut self.next_try, Some(Instant::now() + ARTWORK_RETRY));
        }
    }

    /// After a new handshake Terra has no artwork; send it again.
    fn reset(&mut self) {
        let enabled = self.enabled;
        *self = Self::new(enabled);
    }

    /// The device rejected a transfer; do not retry it for this track.
    fn abandon(&mut self) {
        self.upload = None;
        self.done = true;
        self.next_try = None;
    }

    fn due(&self) -> Option<Instant> {
        if self.upload.is_some() { Some(Instant::now()) } else { self.next_try }
    }

    fn poll(&mut self, reader: &MediaReader, sender: &mut Sender, out: &mut Reporter<impl FnMut(Update)>)
        -> Result<(), SessionError> {
        if let Some(upload) = &mut self.upload {
            if upload.offset < upload.jpeg.len() {
                let room = sender.packet_payload().saturating_sub(ARTWORK_CHUNK_HEADER).clamp(1, ARTWORK_CHUNK_MAX);
                let end = (upload.offset + room).min(upload.jpeg.len());
                let chunk = ArtworkChunk { id: upload.id, offset: upload.offset as u32, data: upload.jpeg[upload.offset..end].to_vec() };
                let write = Instant::now();
                sender.send(MessageType::ArtworkChunk, &chunk.encode().map_err(|_| SessionError::Protocol("ARTWORK_CHUNK"))?)?;
                let took = write.elapsed();
                upload.writing += took;
                upload.slowest = upload.slowest.max(took);
                upload.chunks += 1;
                upload.offset = end;
            } else {
                let end = ArtworkEnd { id: upload.id, crc32: crc32(&upload.jpeg) };
                sender.send(MessageType::ArtworkEnd, &end.encode().map_err(|_| SessionError::Protocol("ARTWORK_END"))?)?;
                out.log(format!(
                    "[ARTWORK] sent {} bytes in {:.1} s ({} chunks, writes {:.1} s, slowest {} ms)",
                    upload.jpeg.len(),
                    upload.started.elapsed().as_secs_f32(),
                    upload.chunks,
                    upload.writing.as_secs_f32(),
                    upload.slowest.as_millis()
                ));
                self.upload = None;
                self.done = true;
            }
            return Ok(());
        }
        if self.done || !self.next_try.is_some_and(|at| at <= Instant::now()) {
            return Ok(());
        }
        self.next_try = None;
        let Some((_, title, artist, _)) = &self.track else { return Ok(()) };
        let artwork = match reader.artwork(u32::from(protocol::ARTWORK_SIDE), protocol::ARTWORK_MAX_BYTES as usize) {
            Ok(artwork) => artwork,
            Err(error) => {
                out.log(format!("[WARN] artwork read failed: {error}"));
                None
            }
        };
        // Only the thumbnail of the track Terra shows; the session may have moved on.
        let matching = artwork.filter(|a| {
            truncate_utf8(&a.title, TITLE_MAX) == *title && truncate_utf8(&a.artist, ARTIST_MAX) == *artist
        });
        let Some(artwork) = matching else {
            if self.since.elapsed() < ARTWORK_EAGER {
                self.next_try = Some(Instant::now() + ARTWORK_RETRY);
            }
            return Ok(());
        };
        let id = crc32(&artwork.jpeg).max(1);
        let begin = ArtworkBegin::jpeg(id, artwork.jpeg.len() as u32);
        sender.send(MessageType::ArtworkBegin, &begin.encode().map_err(|_| SessionError::Protocol("ARTWORK_BEGIN"))?)?;
        self.upload = Some(Upload {
            id,
            jpeg: artwork.jpeg,
            offset: 0,
            started: Instant::now(),
            chunks: 0,
            writing: Duration::ZERO,
            slowest: Duration::ZERO,
        });
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
    fn sync(&mut self, sender: &mut Sender, out: &mut Reporter<impl FnMut(Update)>) -> Result<(), SessionError> {
        let level = match self.audio.refresh().and_then(|_| self.audio.level()) {
            Ok(level) => level,
            Err(error) => {
                out.log(format!("[WARN] volume read failed: {error}"));
                return Ok(());
            }
        };
        let Some(level) = level else {
            if self.last.take().is_some() {
                out.log("[VOLUME] no playback device; Terra keeps the last level".into());
                out.update(Update::Volume(None));
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
        out.log(format!("[VOLUME] {}", describe_volume(level)));
        out.update(Update::Volume(Some(level)));
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

/// Runs sessions until stopped or a protocol mismatch. Every other end of a
/// session (disconnect, Bluetooth off, device gone, Windows API failure) is
/// logged and followed by a reconnect after a bounded backoff (spec §29).
pub fn run_forever(options: &SessionOptions, control: &Control, report: impl FnMut(Update)) -> Exit {
    let mut out = Reporter(report);
    let mut memory = Memory::default();
    let mut backoff = Backoff::default();
    loop {
        if control.stopped() {
            out.update(Update::Phase(Phase::Stopped));
            return Exit::Stopped;
        }
        control.take_reconnect();
        match run(options, &mut memory, control, &mut out) {
            Err(SessionError::ProtocolMismatch { device_version }) => {
                out.update(Update::Phase(Phase::Mismatch { device_version }));
                return Exit::Mismatch(SessionError::ProtocolMismatch { device_version });
            }
            Err(error) => out.log(format!("[ERROR] {error}")),
            Ok(()) => {}
        }
        if control.stopped() {
            continue;
        }
        if memory.ready || control.take_reconnect() {
            backoff.reset();
        }
        let delay = backoff.next_delay_s();
        out.log(format!("[BLE] reconnecting in {delay} s"));
        out.update(Update::Phase(Phase::Waiting { retry_in_s: delay }));
        let until = Instant::now() + Duration::from_secs(delay);
        while Instant::now() < until && !control.interrupted() {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Runs one session until the link drops or `control` interrupts it (returns
/// `Ok`), or an error stops it.
fn run(
    options: &SessionOptions,
    memory: &mut Memory,
    control: &Control,
    out: &mut Reporter<impl FnMut(Update)>,
) -> Result<(), SessionError> {
    memory.ready = false;
    let reader = MediaReader::new()?;
    let address = match options.address {
        Some(address) => address,
        None => {
            out.log("[BLE] scanning for Terra".into());
            out.update(Update::Phase(Phase::Scanning));
            let preferred = memory.address;
            let found = ble_windows::scan_until(options.scan_timeout, |found| {
                preferred.is_some_and(|address| found.iter().any(|f| f.address == address))
            })?;
            let device = found
                .iter()
                .find(|f| Some(f.address) == preferred)
                .or_else(|| found.iter().max_by_key(|f| f.rssi))
                .ok_or(SessionError::NotFound)?;
            out.log(format!("[BLE] found {} {} RSSI {} dBm", device.name, device.address_text(), device.rssi));
            device.address
        }
    };
    if control.interrupted() {
        return Ok(());
    }
    out.update(Update::Phase(Phase::Connecting { address }));

    let (tx, rx) = mpsc::channel::<Event>();
    let link_tx = tx.clone();
    let link = Link::connect(address, move |event| {
        let _ = link_tx.send(Event::Link(event));
    })?;
    memory.address = Some(address);
    out.log(format!("[BLE] connected, packet size {} bytes", link.max_packet()));

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
            Ok(Event::MediaChanged | Event::VolumeChanged | Event::ArtworkDue) => {}
            Err(_) => return Err(SessionError::NoHello),
        }
    };
    let (major, minor, patch) = hello.firmware;
    out.log(format!("[PROTOCOL] HELLO protocol={} firmware={major}.{minor}.{patch}", hello.protocol_version));
    let companion = crate::version_triple();
    let accepted = hello.protocol_version == VERSION;
    let ack = HelloAck { protocol_version: VERSION, companion, accepted };
    sender.send(MessageType::HelloAck, &ack.encode())?;
    if !accepted {
        return Err(SessionError::ProtocolMismatch { device_version: hello.protocol_version });
    }
    memory.ready = true;
    out.update(Update::Phase(Phase::Ready { address, firmware: hello.firmware, protocol: hello.protocol_version }));
    if let Ok(Ok(status)) = link.read_status().map(|bytes| DeviceStatus::decode(&bytes)) {
        out.log(format!("[PROTOCOL] device link state {:?}, rx_errors {}", status.link, status.rx_errors));
    }

    let media_tx = tx.clone();
    let watch = reader.watch(move || {
        let _ = media_tx.send(Event::MediaChanged);
    })?;
    let audio = MasterVolume::new(move || {
        let _ = tx.send(Event::VolumeChanged);
    })?;

    let mut media = MediaSync { reader: &reader, watch, last: None, absent_since: None };
    let mut volume = VolumeSync { audio, last: None };
    let artwork_enabled = hello.capabilities & CAP_ARTWORK != 0;
    if artwork_enabled {
        out.log("[PROTOCOL] device shows album artwork".into());
    }
    let mut artwork = ArtworkSync::new(artwork_enabled);
    let mut pending = Pending::default();
    schedule(&mut pending.media, media.sync(&mut sender, out)?);
    artwork.on_media(media.last.as_ref().map(|(state, _)| state));
    volume.sync(&mut sender, out)?;

    let mut polled = Instant::now();
    loop {
        if control.interrupted() {
            out.log("[BLE] disconnecting on request".into());
            return Ok(());
        }
        if polled.elapsed() >= DEVICE_POLL {
            polled = Instant::now();
            volume.sync(&mut sender, out)?; // follows a changed default playback device
        }
        pending.artwork = artwork.due();
        match next_event(&rx, &mut pending) {
            Ok(Event::MediaChanged) => {
                artwork.on_media_event();
                schedule(&mut pending.media, media.sync(&mut sender, out)?);
                artwork.on_media(media.last.as_ref().map(|(state, _)| state));
            }
            Ok(Event::ArtworkDue) => artwork.poll(&reader, &mut sender, out)?,
            Ok(Event::VolumeChanged) => {
                volume.sync(&mut sender, out)?;
            }
            Ok(Event::Link(LinkEvent::Packet(packet))) => {
                let result = reassembler.push(&packet, now_ms(start));
                if result.drops.0 != 0 {
                    out.log(format!("[WARN] notification dropped (reasons 0x{:02x})", result.drops.0));
                }
                let Some(message) = result.message else { continue };
                match MessageType::from_u8(message.message_type) {
                    Some(MessageType::Error) => match ErrorReport::decode(&message.payload) {
                        Ok(error) => {
                            out.log(format!(
                                "[DEVICE] rejected type 0x{:02x} seq {} (error {})",
                                error.related_type, error.related_seq, error.code
                            ));
                            if (0x50..=0x52).contains(&error.related_type) {
                                artwork.abandon();
                            }
                        }
                        Err(_) => out.log("[WARN] malformed ERROR from device".into()),
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
                        artwork.reset();
                        out.log("[PROTOCOL] HELLO again; acknowledged".into());
                        schedule(&mut pending.media, media.sync(&mut sender, out)?);
                        artwork.on_media(media.last.as_ref().map(|(state, _)| state));
                        volume.sync(&mut sender, out)?;
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
                        out.log(line);
                        // Report what Windows actually has afterwards; never echo the request.
                        if media_request {
                            pending.media.get_or_insert_with(|| Instant::now() + SETTLE);
                        } else {
                            volume.sync(&mut sender, out)?;
                        }
                    }
                    other => out.log(format!("[WARN] unexpected message {other:?} from device")),
                }
            }
            Ok(Event::Link(LinkEvent::Disconnected)) => {
                out.log("[BLE] disconnected".into());
                return Ok(());
            }
            Err(RecvTimeoutError::Timeout) => {
                if !link.is_connected() {
                    out.log("[BLE] disconnected".into());
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
    fn control_requests() {
        let control = Control::default();
        assert!(!control.interrupted());
        control.reconnect();
        assert!(control.interrupted() && !control.stopped());
        assert!(control.take_reconnect());
        assert!(!control.take_reconnect(), "a reconnect request is consumed once");
        assert!(!control.interrupted());
        control.stop();
        assert!(control.interrupted() && control.stopped());
        let mut updates = Vec::new();
        let options = SessionOptions { address: None, scan_timeout: Duration::from_millis(1) };
        // A stopped control returns before touching Bluetooth or Windows media.
        assert!(matches!(run_forever(&options, &control, |u| updates.push(u)), Exit::Stopped));
        assert_eq!(updates, [Update::Phase(Phase::Stopped)]);
    }

    #[test]
    fn artwork_chunks_yield_to_queued_events() {
        let (tx, rx) = mpsc::channel();
        let packet = packet(MediaCommand::Next);
        let mut pending = Pending { artwork: Some(Instant::now()), ..Pending::default() };
        tx.send(Event::Link(LinkEvent::Packet(packet.clone()))).unwrap();
        // A button press waiting in the queue goes before the next chunk ...
        assert!(matches!(next_event(&rx, &mut pending).unwrap(), Event::Link(LinkEvent::Packet(bytes)) if bytes == packet));
        // ... then, with nothing queued, the chunk is due at once.
        let start = Instant::now();
        assert!(matches!(next_event(&rx, &mut pending).unwrap(), Event::ArtworkDue));
        assert!(start.elapsed() < Duration::from_millis(100) && pending.artwork.is_none());
        // A future artwork retry does not block an earlier event.
        pending.artwork = Some(Instant::now() + Duration::from_secs(5));
        tx.send(Event::Link(LinkEvent::Disconnected)).unwrap();
        assert!(matches!(next_event(&rx, &mut pending).unwrap(), Event::Link(LinkEvent::Disconnected)));
    }

    #[test]
    fn artwork_follows_the_shown_track() {
        let state = |title: &str, playback| MediaState {
            playback,
            position_ms: 0,
            duration_ms: 1000,
            source: "Chrome".into(),
            title: title.into(),
            artist: "Olivia Dean".into(),
            album: String::new(),
        };
        let mut artwork = ArtworkSync::new(true);
        assert_eq!(artwork.due(), None);
        artwork.on_media(Some(&state("Lady Lady", Playback::Playing)));
        assert!(artwork.due().is_some(), "a new track is looked up at once");
        artwork.next_try = None;
        artwork.on_media(Some(&state("Lady Lady", Playback::Paused)));
        assert_eq!(artwork.due(), None, "pausing is not a new track");
        artwork.on_media_event();
        assert!(artwork.due().is_some(), "a media event retries a missing thumbnail");
        artwork.abandon();
        artwork.on_media_event();
        assert_eq!(artwork.due(), None, "no retry after the device rejected it");
        artwork.on_media(Some(&state("Baby Steps", Playback::Playing)));
        assert!(artwork.due().is_some() && !artwork.done);
        artwork.on_media(Some(&state("", Playback::NoMedia)));
        assert_eq!((artwork.track.as_ref(), artwork.due()), (None, None));
        let mut disabled = ArtworkSync::new(false);
        disabled.on_media(Some(&state("Lady Lady", Playback::Playing)));
        assert_eq!(disabled.due(), None, "firmware without capability bit3 gets no artwork");
    }

    #[test]
    fn media_event_is_read_before_a_pending_grace_recheck() {
        // Regression (12 Oct 2026): a track change during the NO_MEDIA grace
        // period was only read when the 5 s grace recheck came due.
        let (tx, rx) = mpsc::channel();
        let mut pending = Pending { media: Some(Instant::now() + NO_MEDIA_GRACE), ..Pending::default() };
        tx.send(Event::MediaChanged).unwrap();
        let start = Instant::now();
        assert!(matches!(next_event(&rx, &mut pending).unwrap(), Event::MediaChanged));
        assert!(start.elapsed() < Duration::from_secs(1), "read after {:?}", start.elapsed());
        // The read consumed the slot; the next sync schedules a new recheck itself.
        assert!(pending.media.is_none());
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
