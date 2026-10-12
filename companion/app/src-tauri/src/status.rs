//! What the window and tray show, folded from companion-core session updates.

use companion_core::session::{Phase, Playback, Update};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NowPlaying {
    pub source: String,
    pub title: String,
    pub artist: String,
    pub playing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Volume {
    pub percent: u8,
    pub muted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// stopped | scanning | connecting | ready | waiting | mismatch
    pub link: &'static str,
    /// Last Terra address seen, kept while disconnected.
    pub device: Option<String>,
    /// Last firmware/protocol reported by HELLO, kept while disconnected.
    pub firmware: Option<String>,
    pub protocol: Option<u8>,
    pub retry_in_s: Option<u64>,
    /// Only while ready: what Terra currently shows.
    pub media: Option<NowPlaying>,
    pub volume: Option<Volume>,
    pub last_message: Option<String>,
    pub companion: &'static str,
}

impl Default for Status {
    fn default() -> Self {
        Self {
            link: "stopped",
            device: None,
            firmware: None,
            protocol: None,
            retry_in_s: None,
            media: None,
            volume: None,
            last_message: None,
            companion: companion_core::VERSION,
        }
    }
}

pub fn address_text(address: u64) -> String {
    let a = address.to_be_bytes();
    format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", a[2], a[3], a[4], a[5], a[6], a[7])
}

impl Status {
    pub fn apply(&mut self, update: Update) {
        match update {
            Update::Log(line) => self.last_message = Some(line),
            Update::Phase(phase) => {
                self.retry_in_s = None;
                if !matches!(phase, Phase::Ready { .. }) {
                    // Terra clears both when the link is not READY; mirror it.
                    self.media = None;
                    self.volume = None;
                }
                self.link = match phase {
                    Phase::Scanning => "scanning",
                    Phase::Connecting { address } => {
                        self.device = Some(address_text(address));
                        "connecting"
                    }
                    Phase::Ready { address, firmware: (major, minor, patch), protocol } => {
                        self.device = Some(address_text(address));
                        self.firmware = Some(format!("{major}.{minor}.{patch}"));
                        self.protocol = Some(protocol);
                        "ready"
                    }
                    Phase::Waiting { retry_in_s } => {
                        self.retry_in_s = Some(retry_in_s);
                        "waiting"
                    }
                    Phase::Stopped => "stopped",
                    Phase::Mismatch { .. } => "mismatch",
                };
            }
            Update::Media(state) => {
                self.media = (state.playback != Playback::NoMedia).then(|| NowPlaying {
                    source: state.source,
                    title: state.title,
                    artist: state.artist,
                    playing: state.playback == Playback::Playing,
                });
            }
            Update::Volume(level) => {
                self.volume = level.map(|level| Volume { percent: level.percent, muted: level.muted });
            }
        }
    }

    /// The status line of the tray menu (spec §28).
    pub fn tray_text(&self) -> &'static str {
        match self.link {
            "ready" => "Connected",
            "scanning" | "connecting" => "Connecting…",
            "waiting" => "Disconnected",
            "mismatch" => "Protocol mismatch",
            _ => "Not connecting",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use companion_core::session::{MediaState, VolumeLevel};

    fn media(playback: Playback) -> MediaState {
        MediaState {
            playback,
            position_ms: 1_000,
            duration_ms: 216_000,
            source: "Spotify".into(),
            title: "The Less I Know The Better".into(),
            artist: "Tame Impala".into(),
            album: "Currents".into(),
        }
    }

    #[test]
    fn session_updates_fold_into_the_window_status() {
        let mut status = Status::default();
        assert_eq!(status.tray_text(), "Not connecting");
        status.apply(Update::Phase(Phase::Scanning));
        status.apply(Update::Phase(Phase::Connecting { address: 0x2884_8548_15BA }));
        assert_eq!((status.link, status.tray_text()), ("connecting", "Connecting…"));
        status.apply(Update::Phase(Phase::Ready { address: 0x2884_8548_15BA, firmware: (0, 4, 0), protocol: 1 }));
        status.apply(Update::Media(media(Playback::Paused)));
        status.apply(Update::Volume(Some(VolumeLevel { percent: 72, muted: false })));
        status.apply(Update::Log("[VOLUME] 72".into()));
        assert_eq!(status.tray_text(), "Connected");
        assert_eq!(status.device.as_deref(), Some("28:84:85:48:15:BA"));
        assert_eq!(status.firmware.as_deref(), Some("0.4.0"));
        assert_eq!(status.media.as_ref().map(|m| (m.title.as_str(), m.playing)), Some(("The Less I Know The Better", false)));
        assert_eq!(status.volume, Some(Volume { percent: 72, muted: false }));
        assert_eq!(status.last_message.as_deref(), Some("[VOLUME] 72"));

        status.apply(Update::Media(media(Playback::NoMedia)));
        assert_eq!(status.media, None);

        // A dropped link clears what Terra no longer shows, keeps device facts.
        status.apply(Update::Media(media(Playback::Playing)));
        status.apply(Update::Phase(Phase::Waiting { retry_in_s: 4 }));
        assert_eq!((status.link, status.retry_in_s, status.tray_text()), ("waiting", Some(4), "Disconnected"));
        assert_eq!((status.media.as_ref(), status.volume), (None, None));
        assert_eq!(status.firmware.as_deref(), Some("0.4.0"));
        status.apply(Update::Phase(Phase::Scanning));
        assert_eq!(status.retry_in_s, None);
        status.apply(Update::Phase(Phase::Mismatch { device_version: 2 }));
        assert_eq!(status.tray_text(), "Protocol mismatch");
    }

    #[test]
    fn status_serializes_for_the_frontend() {
        let json = serde_json::to_value(Status::default()).unwrap();
        for key in ["link", "device", "firmware", "protocol", "retryInS", "media", "volume", "lastMessage", "companion"] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
    }
}
