//! Windows media sessions through GSMTC (`Windows.Media.Control`), PROJECT_SPEC §18.
//!
//! M5 scope: read the current session (source, metadata, playback, timeline) and
//! observe changes. Media commands (TryTogglePlayPauseAsync etc.) arrive with M7.
#![cfg(windows)]

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use windows::ApplicationModel::AppInfo;
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    CurrentSessionChangedEventArgs, MediaPropertiesChangedEventArgs, PlaybackInfoChangedEventArgs,
    SessionsChangedEventArgs, TimelinePropertiesChangedEventArgs,
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::core::HSTRING;

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
    let duration = (end - start).max(0);
    let mut current = (position - start).max(0);
    if playing && last_updated > 0 && now > last_updated {
        current = current.saturating_add(now - last_updated);
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
        ))?;
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
        let current = self.manager.GetCurrentSession().ok();
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
        )?;
        let tx = self.changes.clone();
        let timeline = session.TimelinePropertiesChanged(
            &TypedEventHandler::<Session, TimelinePropertiesChangedEventArgs>::new(move |_, _| {
                tx();
                Ok(())
            }),
        )?;
        let app_id = session.SourceAppUserModelId()?.to_string();
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
