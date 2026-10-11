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
}
