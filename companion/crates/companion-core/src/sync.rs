//! Platform-neutral media synchronization rules (spec §14, §21).

use protocol::{ALBUM_MAX, ARTIST_MAX, MediaState, Playback, SOURCE_MAX, TITLE_MAX};

/// Playback as read from the operating system, before protocol mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaPlayback {
    Playing,
    Paused,
    /// Stopped, opened without playing, or closed.
    Stopped,
    /// Between tracks; transient and not worth sending.
    Changing,
}

/// One media session as the companion sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaInfo {
    pub source: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playback: MediaPlayback,
    pub position_ms: u64,
    pub duration_ms: u64,
}

/// Longest prefix of `text` that fits in `max` UTF-8 bytes without splitting a
/// character. NUL characters are removed because the wire format forbids them.
pub fn truncate_utf8(text: &str, max: usize) -> String {
    let mut out = String::new();
    for c in text.chars().filter(|&c| c != '\0') {
        if out.len() + c.len_utf8() > max {
            break;
        }
        out.push(c);
    }
    out
}

/// MEDIA_STATE for the current session; `None` (no session) and stopped
/// sessions become NO_MEDIA with empty strings. Returns `None` for transient
/// "changing" states so the previous state stays on the device.
pub fn media_state(info: Option<&MediaInfo>) -> Option<MediaState> {
    let Some(info) = info else { return Some(no_media()) };
    // Observed 12 Oct 2026: a closing Spotify briefly reports a paused session
    // with no title or artist. There is nothing to show; treat it as no media.
    if info.title.trim().is_empty() && info.artist.trim().is_empty() {
        return Some(no_media());
    }
    let playback = match info.playback {
        MediaPlayback::Playing => Playback::Playing,
        MediaPlayback::Paused => Playback::Paused,
        MediaPlayback::Stopped => return Some(no_media()),
        MediaPlayback::Changing => return None,
    };
    Some(MediaState {
        playback,
        position_ms: info.position_ms.min(u32::MAX as u64) as u32,
        duration_ms: info.duration_ms.min(u32::MAX as u64) as u32,
        source: truncate_utf8(&info.source, SOURCE_MAX),
        title: truncate_utf8(&info.title, TITLE_MAX),
        artist: truncate_utf8(&info.artist, ARTIST_MAX),
        album: truncate_utf8(&info.album, ALBUM_MAX),
    })
}

fn no_media() -> MediaState {
    MediaState {
        playback: Playback::NoMedia,
        position_ms: 0,
        duration_ms: 0,
        source: String::new(),
        title: String::new(),
        artist: String::new(),
        album: String::new(),
    }
}

/// How long a missing or stopped session must last before NO_MEDIA replaces a
/// track on the device. Observed 12 Oct 2026: Chrome reports no session for
/// 0.2–3.5 s between YouTube Music tracks.
pub const NO_MEDIA_GRACE_MS: u64 = 5_000;

/// Whether NO_MEDIA may be sent after `absent_ms` without a playing/paused
/// session, given the last state sent.
pub fn no_media_due(last: Option<&MediaState>, absent_ms: u64) -> bool {
    match last {
        Some(state) if state.playback != Playback::NoMedia => absent_ms >= NO_MEDIA_GRACE_MS,
        _ => true,
    }
}

/// Bounded exponential reconnect delay in seconds (spec §29): 1, 2, 4, 8, 15, 15…
#[derive(Debug, Default)]
pub struct Backoff {
    attempt: usize,
}

impl Backoff {
    const STEPS_S: [u64; 5] = [1, 2, 4, 8, 15];

    pub fn next_delay_s(&mut self) -> u64 {
        let delay = Self::STEPS_S[self.attempt.min(Self::STEPS_S.len() - 1)];
        self.attempt = self.attempt.saturating_add(1);
        delay
    }

    /// After a session that reached READY, the next outage starts at 1 s again.
    pub fn reset(&mut self) {
        self.attempt = 0;
    }
}

/// Position difference that counts as a seek rather than clock drift.
pub const SEEK_THRESHOLD_MS: u64 = 1500;

/// Whether `next` must be sent, given the last state sent `elapsed_ms` ago.
/// The device advances the position itself while playing, so only track,
/// playback, metadata changes and seeks are sent (spec §21).
pub fn needs_send(last: Option<&MediaState>, next: &MediaState, elapsed_ms: u64) -> bool {
    let Some(last) = last else { return true };
    if last.playback != next.playback
        || last.duration_ms != next.duration_ms
        || last.source != next.source
        || last.title != next.title
        || last.artist != next.artist
        || last.album != next.album
    {
        return true;
    }
    let mut expected = last.position_ms as u64;
    if last.playback == Playback::Playing {
        expected += elapsed_ms;
        if last.duration_ms > 0 {
            expected = expected.min(last.duration_ms as u64);
        }
    }
    expected.abs_diff(next.position_ms as u64) > SEEK_THRESHOLD_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(playback: MediaPlayback, position_ms: u64) -> MediaInfo {
        MediaInfo {
            source: "Spotify".into(),
            title: "The Less I Know The Better".into(),
            artist: "Tame Impala".into(),
            album: "Currents".into(),
            playback,
            position_ms,
            duration_ms: 216_000,
        }
    }

    #[test]
    fn truncation_respects_character_boundaries() {
        assert_eq!(truncate_utf8("abc", 3), "abc");
        assert_eq!(truncate_utf8("abcd", 3), "abc");
        assert_eq!(truncate_utf8("ééé", 5), "éé"); // 2-byte characters
        assert_eq!(truncate_utf8("a😀", 4), "a"); // emoji needs 4 bytes
        assert_eq!(truncate_utf8("a\0b", 31), "ab");
        let long = "é".repeat(100);
        assert!(truncate_utf8(&long, TITLE_MAX).len() <= TITLE_MAX);
    }

    #[test]
    fn mapping_and_encoding() {
        let state = media_state(Some(&info(MediaPlayback::Playing, 81_200))).unwrap();
        assert_eq!(state.playback, Playback::Playing);
        assert!(state.encode().is_ok());
        assert_eq!(media_state(None).unwrap().playback, Playback::NoMedia);
        assert_eq!(media_state(Some(&info(MediaPlayback::Stopped, 5))).unwrap().title, "");
        assert_eq!(media_state(Some(&info(MediaPlayback::Changing, 0))), None);
        let mut closing = info(MediaPlayback::Paused, 0);
        closing.title = " ".into();
        closing.artist.clear();
        assert_eq!(media_state(Some(&closing)).unwrap().playback, Playback::NoMedia);
        closing.artist = "Tame Impala".into(); // a title-less track is still media
        assert_eq!(media_state(Some(&closing)).unwrap().playback, Playback::Paused);
        let mut huge = info(MediaPlayback::Paused, u64::MAX);
        huge.title = "😀".repeat(200);
        huge.source = "S".repeat(200);
        let state = media_state(Some(&huge)).unwrap();
        assert_eq!(state.position_ms, u32::MAX);
        assert!(state.encode().is_ok(), "oversized metadata must still encode");
    }

    #[test]
    fn send_only_on_meaningful_changes() {
        let playing = media_state(Some(&info(MediaPlayback::Playing, 81_000))).unwrap();
        assert!(needs_send(None, &playing, 0));
        // Natural progress while playing: no resend.
        let later = media_state(Some(&info(MediaPlayback::Playing, 86_000))).unwrap();
        assert!(!needs_send(Some(&playing), &later, 5_000));
        // Seek forward.
        let seek = media_state(Some(&info(MediaPlayback::Playing, 150_000))).unwrap();
        assert!(needs_send(Some(&playing), &seek, 5_000));
        // Pause.
        let paused = media_state(Some(&info(MediaPlayback::Paused, 86_000))).unwrap();
        assert!(needs_send(Some(&playing), &paused, 5_000));
        // Paused position does not drift.
        assert!(!needs_send(Some(&paused), &paused, 60_000));
        // Track change.
        let mut other = info(MediaPlayback::Playing, 0);
        other.title = "Let It Happen".into();
        assert!(needs_send(Some(&playing), &media_state(Some(&other)).unwrap(), 1_000));
    }

    #[test]
    fn no_media_waits_out_track_changes() {
        let playing = media_state(Some(&info(MediaPlayback::Playing, 0))).unwrap();
        let paused = media_state(Some(&info(MediaPlayback::Paused, 0))).unwrap();
        let none = media_state(None).unwrap();
        // First state of a session, or already showing nothing: no reason to wait.
        assert!(no_media_due(None, 0));
        assert!(no_media_due(Some(&none), 0));
        // A gap between tracks keeps the old track on screen.
        assert!(!no_media_due(Some(&playing), 3_500));
        assert!(!no_media_due(Some(&paused), NO_MEDIA_GRACE_MS - 1));
        // A player that really closed is cleared after the grace period.
        assert!(no_media_due(Some(&playing), NO_MEDIA_GRACE_MS));
    }

    #[test]
    fn reconnect_backoff_is_bounded_and_resets() {
        let mut backoff = Backoff::default();
        let delays: Vec<u64> = (0..8).map(|_| backoff.next_delay_s()).collect();
        assert_eq!(delays, [1, 2, 4, 8, 15, 15, 15, 15]);
        backoff.reset();
        assert_eq!(backoff.next_delay_s(), 1);
        let mut long = Backoff { attempt: usize::MAX };
        assert_eq!(long.next_delay_s(), 15);
        assert_eq!(long.next_delay_s(), 15);
    }
}
