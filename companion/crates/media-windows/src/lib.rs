//! Windows media sessions through GSMTC (`Windows.Media.Control`), PROJECT_SPEC §18.
//!
//! Reads and observes the current session (M5), controls its playback (M7), and
//! prepares its thumbnail as Terra album artwork (V2).
#![cfg(windows)]

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use windows::ApplicationModel::AppInfo;
use windows::Foundation::{PropertyType, PropertyValue, TypedEventHandler};
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapBounds, BitmapDecoder, BitmapEncoder, BitmapInterpolationMode, BitmapPixelFormat,
    BitmapPropertySet, BitmapTransform, BitmapTypedValue, ColorManagementMode, ExifOrientationMode,
};
use windows::Storage::Streams::{DataReader, IRandomAccessStream, InMemoryRandomAccessStream};
use windows::Media::Control::{
    CurrentSessionChangedEventArgs, MediaPropertiesChangedEventArgs, PlaybackInfoChangedEventArgs,
    SessionsChangedEventArgs, TimelinePropertiesChangedEventArgs,
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::core::{HSTRING, Interface};

/// A Windows API call failed; carries the HRESULT message.
#[derive(Debug)]
pub struct MediaError(windows::core::Error);

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Windows media API error: {}", self.0.message())
    }
}

impl std::error::Error for MediaError {}

impl From<windows::core::Error> for MediaError {
    fn from(error: windows::core::Error) -> Self {
        Self(error)
    }
}

pub type Result<T> = std::result::Result<T, MediaError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlCommand { PlayPause, Next, Previous }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlOutcome { Accepted, Rejected, Unsupported, NoSession }

type Notify = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Playback {
    Closed,
    Opened,
    Changing,
    Stopped,
    Playing,
    Paused,
}

impl Playback {
    fn from_status(status: Status) -> Self {
        match status {
            Status::Opened => Self::Opened,
            Status::Changing => Self::Changing,
            Status::Stopped => Self::Stopped,
            Status::Playing => Self::Playing,
            Status::Paused => Self::Paused,
            _ => Self::Closed,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Closed => "Closed",
            Self::Opened => "Opened",
            Self::Changing => "Changing",
            Self::Stopped => "Stopped",
            Self::Playing => "Playing",
            Self::Paused => "Paused",
        }
    }
}

/// The current media session as Windows reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaSnapshot {
    /// Raw AppUserModelId, e.g. `Spotify.exe` or `MSEdge`.
    pub app_id: String,
    /// Human-readable source derived from the app id.
    pub source: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playback: Playback,
    /// Position estimated for the moment of the call (see `estimate_position`).
    pub position_ms: u64,
    /// 0 when the app publishes no timeline.
    pub duration_ms: u64,
}

/// Windows FILETIME ticks (100 ns since 1601-01-01) for "now".
fn now_ticks() -> i64 {
    const UNIX_TO_FILETIME: i64 = 116_444_736_000_000_000;
    let since_unix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    UNIX_TO_FILETIME + (since_unix.as_nanos() / 100) as i64
}

/// GSMTC reports the position as of `last_updated`. While playing, the current
/// position is that value plus the time elapsed since, clamped to the duration.
/// All arguments are in 100 ns ticks; a zero `last_updated` means unknown.
pub fn estimate_position(
    position: i64,
    start: i64,
    end: i64,
    last_updated: i64,
    now: i64,
    playing: bool,
) -> (u64, u64) {
    let duration = end.saturating_sub(start).max(0);
    let mut current = position.saturating_sub(start).max(0);
    if playing && last_updated > 0 && now > last_updated {
        current = current.saturating_add(now.saturating_sub(last_updated));
    }
    if duration > 0 {
        current = current.min(duration);
    }
    ((current / 10_000) as u64, (duration / 10_000) as u64)
}

/// Friendly name for an AppUserModelId: the registered display name when Windows
/// knows it, otherwise a cleaned-up id (`Spotify.exe` → `Spotify`).
pub fn friendly_source(app_id: &str) -> String {
    if let Ok(name) = AppInfo::GetFromAppUserModelId(&HSTRING::from(app_id))
        .and_then(|info| info.DisplayInfo())
        .and_then(|display| display.DisplayName())
    {
        let name = name.to_string();
        if !name.trim().is_empty() {
            return name;
        }
    }
    fallback_source(app_id)
}

/// Heuristic used when Windows has no display name for the id.
pub fn fallback_source(app_id: &str) -> String {
    let id = app_id.rsplit(['\\', '/']).next().unwrap_or(app_id);
    let id = id.rsplit('!').next().unwrap_or(id);
    let id = if id.to_ascii_lowercase().ends_with(".exe") { &id[..id.len() - 4] } else { id };
    match id.to_ascii_lowercase().as_str() {
        "msedge" => "Edge".to_owned(),
        "chrome" => "Chrome".to_owned(),
        "firefox" | "308046b0af4a39cb" => "Firefox".to_owned(),
        _ => {
            let mut chars = id.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => "Unknown".to_owned(),
            }
        }
    }
}

fn snapshot(session: &Session) -> Result<MediaSnapshot> {
    let app_id = session.SourceAppUserModelId()?.to_string();
    let properties = session.TryGetMediaPropertiesAsync()?.join()?;
    let playback = Playback::from_status(session.GetPlaybackInfo()?.PlaybackStatus()?);
    let timeline = session.GetTimelineProperties()?;
    let (position_ms, duration_ms) = estimate_position(
        timeline.Position()?.Duration,
        timeline.StartTime()?.Duration,
        timeline.EndTime()?.Duration,
        timeline.LastUpdatedTime()?.UniversalTime,
        now_ticks(),
        playback == Playback::Playing,
    );
    Ok(MediaSnapshot {
        source: friendly_source(&app_id),
        app_id,
        title: properties.Title()?.to_string(),
        artist: properties.Artist()?.to_string(),
        album: properties.AlbumTitle()?.to_string(),
        playback,
        position_ms,
        duration_ms,
    })
}

/// The current session's thumbnail, cropped to a centered square, scaled and
/// re-encoded as a baseline JPEG that Terra's decoder accepts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artwork {
    /// Track the thumbnail belongs to, to match it with the MEDIA_STATE sent.
    pub title: String,
    pub artist: String,
    pub jpeg: Vec<u8>,
}

/// JPEG qualities tried in order until the image fits the size limit.
const ARTWORK_QUALITIES: [f32; 4] = [0.85, 0.7, 0.55, 0.4];

/// Scaled size and centered crop that turn a `width` x `height` image into a
/// `side` x `side` square covering it (short side scaled to `side`).
pub fn cover_crop(width: u32, height: u32, side: u32) -> Option<((u32, u32), BitmapBounds)> {
    if width == 0 || height == 0 || side == 0 {
        return None;
    }
    let short = width.min(height) as u64;
    let scale = |n: u32| ((n as u64 * side as u64 + short / 2) / short).clamp(side as u64, u32::MAX as u64) as u32;
    let (scaled_width, scaled_height) = (scale(width), scale(height));
    let bounds = BitmapBounds { X: (scaled_width - side) / 2, Y: (scaled_height - side) / 2, Width: side, Height: side };
    Some(((scaled_width, scaled_height), bounds))
}

fn encode_jpeg(bgra: &[u8], side: u32, quality: f32) -> Result<Vec<u8>> {
    let stream = InMemoryRandomAccessStream::new()?;
    let options = BitmapPropertySet::new()?;
    let value = BitmapTypedValue::Create(&PropertyValue::CreateSingle(quality)?, PropertyType::Single)?;
    options.Insert(&HSTRING::from("ImageQuality"), &value)?;
    let encoder = BitmapEncoder::CreateWithEncodingOptionsAsync(BitmapEncoder::JpegEncoderId()?, &stream, &options)?.join()?;
    encoder.SetPixelData(BitmapPixelFormat::Bgra8, BitmapAlphaMode::Ignore, side, side, 96.0, 96.0, bgra)?;
    encoder.FlushAsync()?.join()?;
    let size = u32::try_from(stream.Size()?).unwrap_or(u32::MAX);
    let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0)?)?;
    reader.LoadAsync(size)?.join()?;
    let mut jpeg = vec![0u8; reader.UnconsumedBufferLength()? as usize];
    reader.ReadBytes(&mut jpeg)?;
    Ok(jpeg)
}

fn session_artwork(session: &Session, side: u32, max_bytes: usize) -> Result<Option<Artwork>> {
    let properties = session.TryGetMediaPropertiesAsync()?.join()?;
    let reference = match properties.Thumbnail() {
        Ok(reference) => reference,
        Err(error) if error.code().is_ok() => return Ok(None), // the app published none
        Err(error) => return Err(error.into()),
    };
    let stream: IRandomAccessStream = reference.OpenReadAsync()?.join()?.cast()?;
    let decoder = BitmapDecoder::CreateAsync(&stream)?.join()?;
    let Some(((width, height), bounds)) =
        cover_crop(decoder.OrientedPixelWidth()?, decoder.OrientedPixelHeight()?, side)
    else {
        return Ok(None);
    };
    let transform = BitmapTransform::new()?;
    transform.SetScaledWidth(width)?;
    transform.SetScaledHeight(height)?;
    transform.SetInterpolationMode(BitmapInterpolationMode::Fant)?;
    transform.SetBounds(bounds)?;
    let pixels = decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Ignore,
            &transform,
            ExifOrientationMode::RespectExifOrientation,
            ColorManagementMode::ColorManageToSRgb,
        )?
        .join()?
        .DetachPixelData()?;
    for quality in ARTWORK_QUALITIES {
        let jpeg = encode_jpeg(&pixels, side, quality)?;
        if jpeg.len() <= max_bytes {
            return Ok(Some(Artwork {
                title: properties.Title()?.to_string(),
                artist: properties.Artist()?.to_string(),
                jpeg,
            }));
        }
    }
    Ok(None) // even the lowest quality is too large; show no artwork
}

/// Entry point to the GSMTC session manager.
pub struct MediaReader {
    manager: Manager,
}

impl MediaReader {
    pub fn new() -> Result<Self> {
        Ok(Self { manager: Manager::RequestAsync()?.join()? })
    }

    /// The session Windows currently considers active, if any.
    pub fn current(&self) -> Result<Option<MediaSnapshot>> {
        match self.manager.GetCurrentSession() {
            Ok(session) => snapshot(&session).map(Some),
            // GetCurrentSession fails with a null result when nothing is playing.
            Err(error) if error.code().is_ok() => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// The current session's thumbnail as a `side` x `side` baseline JPEG of at
    /// most `max_bytes`, or `None` when there is no session or thumbnail.
    pub fn artwork(&self, side: u32, max_bytes: usize) -> Result<Option<Artwork>> {
        match self.manager.GetCurrentSession() {
            Ok(session) => session_artwork(&session, side, max_bytes),
            Err(error) if error.code().is_ok() => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Resolve the current session for each press, including after app switches.
    /// A false WinRT result is a rejected request, not a successful state change.
    pub fn control(&self, command: ControlCommand) -> Result<ControlOutcome> {
        let session = match self.manager.GetCurrentSession() {
            Ok(session) => session,
            Err(error) if error.code().is_ok() => return Ok(ControlOutcome::NoSession),
            Err(error) => return Err(error.into()),
        };
        let controls = session.GetPlaybackInfo()?.Controls()?;
        let enabled = match command {
            ControlCommand::PlayPause => controls.IsPlayPauseToggleEnabled()?,
            ControlCommand::Next => controls.IsNextEnabled()?,
            ControlCommand::Previous => controls.IsPreviousEnabled()?,
        };
        // Try* reports success/failure; do not skip a request solely on a capability snapshot.
        let operation = match command {
            ControlCommand::PlayPause => session.TryTogglePlayPauseAsync()?,
            ControlCommand::Next => session.TrySkipNextAsync()?,
            ControlCommand::Previous => session.TrySkipPreviousAsync()?,
        };
        Ok(match (operation.join()?, enabled) {
            (true, _) => ControlOutcome::Accepted,
            (false, true) => ControlOutcome::Rejected,
            (false, false) => ControlOutcome::Unsupported,
        })
    }

    /// Calls `on_change` whenever the current session changes or the current
    /// session reports new media, playback or timeline data. It runs on WinRT
    /// threads and should only signal; call `MediaWatch::sync` after each
    /// notification so the subscriptions follow the newly selected session.
    pub fn watch(&self, on_change: impl Fn() + Send + Sync + 'static) -> Result<MediaWatch> {
        let changes: Notify = Arc::new(on_change);
        let tx = changes.clone();
        let current = self.manager.CurrentSessionChanged(&TypedEventHandler::<
            Manager,
            CurrentSessionChangedEventArgs,
        >::new(move |_, _| {
            tx();
            Ok(())
        }))?;
        let tx = changes.clone();
        let sessions = self.manager.SessionsChanged(&TypedEventHandler::<Manager, SessionsChangedEventArgs>::new(
            move |_, _| {
                tx();
                Ok(())
            },
        )).map_err(|error| {
            let _ = self.manager.RemoveCurrentSessionChanged(current);
            MediaError::from(error)
        })?;
        let watch = MediaWatch {
            manager: self.manager.clone(),
            manager_tokens: (current, sessions),
            session: Mutex::new(None),
            changes,
        };
        watch.sync()?;
        Ok(watch)
    }
}

struct Subscribed {
    session: Session,
    app_id: String,
    tokens: [i64; 3],
}

/// Live subscriptions; unsubscribes everything when dropped.
pub struct MediaWatch {
    manager: Manager,
    manager_tokens: (i64, i64),
    session: Mutex<Option<Subscribed>>,
    changes: Notify,
}

impl MediaWatch {
    /// Moves the per-session subscriptions to the current session if it changed
    /// (spec §18: unsubscribe old, select new, subscribe new).
    pub fn sync(&self) -> Result<()> {
        let current = match self.manager.GetCurrentSession() {
            Ok(session) => Some(session),
            Err(error) if error.code().is_ok() => None,
            Err(error) => return Err(error.into()),
        };
        let mut guard = self.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let same = match (&*guard, &current) {
            (Some(old), Some(new)) => {
                old.session == *new && new.SourceAppUserModelId().map(|id| id.to_string()).ok().as_deref()
                    == Some(old.app_id.as_str())
            }
            (None, None) => true,
            _ => false,
        };
        if same {
            return Ok(());
        }
        if let Some(old) = guard.take() {
            unsubscribe(&old);
        }
        if let Some(session) = current {
            *guard = Some(self.subscribe(session)?);
        }
        Ok(())
    }

    fn subscribe(&self, session: Session) -> Result<Subscribed> {
        let app_id = session.SourceAppUserModelId()?.to_string();
        let tx = self.changes.clone();
        let media = session.MediaPropertiesChanged(
            &TypedEventHandler::<Session, MediaPropertiesChangedEventArgs>::new(move |_, _| {
                tx();
                Ok(())
            }),
        )?;
        let tx = self.changes.clone();
        let playback = session.PlaybackInfoChanged(
            &TypedEventHandler::<Session, PlaybackInfoChangedEventArgs>::new(move |_, _| {
                tx();
                Ok(())
            }),
        ).map_err(|error| {
            let _ = session.RemoveMediaPropertiesChanged(media);
            MediaError::from(error)
        })?;
        let tx = self.changes.clone();
        let timeline = session.TimelinePropertiesChanged(
            &TypedEventHandler::<Session, TimelinePropertiesChangedEventArgs>::new(move |_, _| {
                tx();
                Ok(())
            }),
        ).map_err(|error| {
            let _ = session.RemoveMediaPropertiesChanged(media);
            let _ = session.RemovePlaybackInfoChanged(playback);
            MediaError::from(error)
        })?;
        Ok(Subscribed { session, app_id, tokens: [media, playback, timeline] })
    }
}

fn unsubscribe(old: &Subscribed) {
    let [media, playback, timeline] = old.tokens;
    let _ = old.session.RemoveMediaPropertiesChanged(media);
    let _ = old.session.RemovePlaybackInfoChanged(playback);
    let _ = old.session.RemoveTimelinePropertiesChanged(timeline);
}

impl Drop for MediaWatch {
    fn drop(&mut self) {
        if let Some(old) = self.session.get_mut().ok().and_then(Option::take) {
            unsubscribe(&old);
        }
        let _ = self.manager.RemoveCurrentSessionChanged(self.manager_tokens.0);
        let _ = self.manager.RemoveSessionsChanged(self.manager_tokens.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_estimate() {
        const MS: i64 = 10_000;
        // Paused: reported position, regardless of elapsed time.
        assert_eq!(estimate_position(81_200 * MS, 0, 216_000 * MS, 1, 1_000_000 * MS, false), (81_200, 216_000));
        // Playing: advances by the time since the last update.
        assert_eq!(estimate_position(81_200 * MS, 0, 216_000 * MS, 1_000 * MS, 3_500 * MS, true), (83_700, 216_000));
        // Clamped to the duration; a non-zero start is subtracted.
        assert_eq!(estimate_position(15_000 * MS, 5_000 * MS, 20_000 * MS, 1, 100_000 * MS, true), (15_000, 15_000));
        // No timeline published.
        assert_eq!(estimate_position(0, 0, 0, 0, 5, true), (0, 0));
        // Malformed/extreme app timelines cannot overflow subtraction.
        assert_eq!(estimate_position(i64::MAX, i64::MIN, i64::MAX, 1, i64::MAX, true),
            ((i64::MAX / MS) as u64, (i64::MAX / MS) as u64));
    }

    #[test]
    fn cover_crop_fills_the_square() {
        let crop = |w, h| cover_crop(w, h, 160).map(|((sw, sh), b)| (sw, sh, b.X, b.Y, b.Width, b.Height));
        assert_eq!(crop(300, 300), Some((160, 160, 0, 0, 160, 160)));
        // 16:9 video thumbnail: scale the height to 160 and cut the sides.
        assert_eq!(crop(1280, 720), Some((284, 160, 62, 0, 160, 160)));
        assert_eq!(crop(720, 1280), Some((160, 284, 0, 62, 160, 160)));
        // Small images are scaled up.
        assert_eq!(crop(64, 64), Some((160, 160, 0, 0, 160, 160)));
        assert_eq!(crop(161, 160), Some((161, 160, 0, 0, 160, 160)));
        assert_eq!(crop(0, 100), None);
        // Extreme aspect ratios cannot overflow.
        assert_eq!(crop(u32::MAX, 1), Some((u32::MAX, 160, (u32::MAX - 160) / 2, 0, 160, 160)));
    }

    #[test]
    fn fallback_names() {
        assert_eq!(fallback_source("Spotify.exe"), "Spotify");
        assert_eq!(fallback_source("MSEdge"), "Edge");
        assert_eq!(fallback_source("chrome"), "Chrome");
        assert_eq!(fallback_source("308046B0AF4A39CB"), "Firefox");
        assert_eq!(fallback_source("SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify"), "Spotify");
        assert_eq!(fallback_source("C:\\Apps\\vlc.exe"), "Vlc");
        assert_eq!(fallback_source(""), "Unknown");
    }
}
