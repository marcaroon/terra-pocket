//! Windows master volume through Core Audio (`IAudioEndpointVolume`), PROJECT_SPEC §20.
//!
//! Follows the default playback endpoint (render, console role), steps and mutes
//! it, and reports changes made anywhere in Windows (M8).

/// Master volume as Terra shows it: whole percent 0–100 and mute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeLevel {
    pub percent: u8,
    pub muted: bool,
}

/// Windows volume scalar (0.0–1.0) to the nearest whole percent, as the
/// Windows volume flyout rounds it. Out-of-range or NaN values are clamped.
pub fn percent_from_scalar(scalar: f32) -> u8 {
    if scalar.is_nan() {
        return 0;
    }
    (scalar.clamp(0.0, 1.0) * 100.0).round() as u8
}

/// Spec §20: `target = clamp(current + delta, 0, 100)`.
pub fn step_target(current: u8, delta: i16) -> u8 {
    (i16::from(current.min(100)) + delta.clamp(-100, 100)).clamp(0, 100) as u8
}

#[cfg(windows)]
pub use native::{AudioError, MasterVolume, Result};

#[cfg(windows)]
#[allow(unsafe_code)] // Every unsafe call is a COM method on an interface this module owns.
mod native {
    use std::fmt;
    use std::sync::Arc;

    use windows::Win32::Media::Audio::Endpoints::{
        IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
    };
    use windows::Win32::Media::Audio::{
        AUDIO_VOLUME_NOTIFICATION_DATA, IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
    };
    use windows::Win32::System::Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree};
    use windows::core::{HRESULT, implement};

    use super::{VolumeLevel, percent_from_scalar, step_target};

    /// E_NOTFOUND: Windows has no active playback device.
    const NO_DEVICE: HRESULT = HRESULT(0x8007_0490_u32 as i32);

    /// A Core Audio call failed; carries the HRESULT message.
    #[derive(Debug)]
    pub struct AudioError(windows::core::Error);

    impl fmt::Display for AudioError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Windows audio API error: {}", self.0.message())
        }
    }

    impl std::error::Error for AudioError {}

    impl From<windows::core::Error> for AudioError {
        fn from(error: windows::core::Error) -> Self {
            Self(error)
        }
    }

    pub type Result<T> = std::result::Result<T, AudioError>;

    type Notify = Arc<dyn Fn() + Send + Sync>;

    #[implement(IAudioEndpointVolumeCallback)]
    struct Callback(Notify);

    impl IAudioEndpointVolumeCallback_Impl for Callback_Impl {
        // Runs on a Core Audio thread: only signal, never call back into the endpoint.
        fn OnNotify(&self, _: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
            (self.0)();
            Ok(())
        }
    }

    /// The bound endpoint; unregisters its callback when dropped.
    struct Endpoint {
        id: String,
        volume: IAudioEndpointVolume,
        callback: IAudioEndpointVolumeCallback,
    }

    impl Drop for Endpoint {
        fn drop(&mut self) {
            let _ = unsafe { self.volume.UnregisterControlChangeNotify(&self.callback) };
        }
    }

    /// Master volume of the default playback device. Not thread-safe: use it
    /// from the thread that created it.
    pub struct MasterVolume {
        enumerator: IMMDeviceEnumerator,
        endpoint: Option<Endpoint>,
        notify: Notify,
    }

    impl MasterVolume {
        /// `on_change` runs on a Windows thread whenever the bound endpoint's
        /// volume or mute changes, including changes this process makes.
        pub fn new(on_change: impl Fn() + Send + Sync + 'static) -> Result<Self> {
            // S_FALSE (already initialized) and RPC_E_CHANGED_MODE (an STA thread)
            // both leave COM usable on this thread.
            let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            let enumerator = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)? };
            let mut volume = Self { enumerator, endpoint: None, notify: Arc::new(on_change) };
            volume.refresh()?;
            Ok(volume)
        }

        /// Follows the default playback device: rebinds when it changed, appeared
        /// or disappeared. Windows sends no volume event for such a switch, so
        /// callers poll this. Returns whether the binding changed.
        pub fn refresh(&mut self) -> Result<bool> {
            let device = match unsafe { self.enumerator.GetDefaultAudioEndpoint(eRender, eConsole) } {
                Ok(device) => device,
                Err(error) if error.code() == NO_DEVICE => {
                    return Ok(self.endpoint.take().is_some());
                }
                Err(error) => return Err(error.into()),
            };
            let raw = unsafe { device.GetId()? };
            let id = unsafe { raw.to_string() };
            unsafe { CoTaskMemFree(Some(raw.0 as *const _)) };
            let id = id.unwrap_or_default();
            if self.endpoint.as_ref().is_some_and(|endpoint| endpoint.id == id) {
                return Ok(false);
            }
            self.endpoint = None; // unregister from the previous device first
            let volume: IAudioEndpointVolume = unsafe { device.Activate(CLSCTX_ALL, None)? };
            let callback: IAudioEndpointVolumeCallback = Callback(self.notify.clone()).into();
            unsafe { volume.RegisterControlChangeNotify(&callback)? };
            self.endpoint = Some(Endpoint { id, volume, callback });
            Ok(true)
        }

        /// Current level, or `None` when Windows has no playback device.
        pub fn level(&self) -> Result<Option<VolumeLevel>> {
            let Some(endpoint) = &self.endpoint else { return Ok(None) };
            let scalar = unsafe { endpoint.volume.GetMasterVolumeLevelScalar()? };
            let muted = unsafe { endpoint.volume.GetMute()? }.as_bool();
            Ok(Some(VolumeLevel { percent: percent_from_scalar(scalar), muted }))
        }

        /// Steps the volume by `delta` percent from the current whole percent and
        /// returns the level read back afterwards. Mute is left unchanged.
        pub fn step(&mut self, delta: i16) -> Result<Option<VolumeLevel>> {
            self.refresh()?;
            let Some(current) = self.level()? else { return Ok(None) };
            let target = step_target(current.percent, delta);
            if let Some(endpoint) = &self.endpoint {
                unsafe { endpoint.volume.SetMasterVolumeLevelScalar(f32::from(target) / 100.0, std::ptr::null())? };
            }
            self.level()
        }

        /// Toggles mute and returns the level read back afterwards.
        pub fn toggle_mute(&mut self) -> Result<Option<VolumeLevel>> {
            self.refresh()?;
            let Some(current) = self.level()? else { return Ok(None) };
            if let Some(endpoint) = &self.endpoint {
                unsafe { endpoint.volume.SetMute(!current.muted, std::ptr::null())? };
            }
            self.level()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_rounds_like_the_windows_flyout() {
        assert_eq!(percent_from_scalar(0.0), 0);
        assert_eq!(percent_from_scalar(0.72), 72);
        assert_eq!(percent_from_scalar(0.724_9), 72);
        assert_eq!(percent_from_scalar(0.725_1), 73);
        assert_eq!(percent_from_scalar(1.0), 100);
        assert_eq!(percent_from_scalar(1.5), 100);
        assert_eq!(percent_from_scalar(-0.2), 0);
        assert_eq!(percent_from_scalar(f32::NAN), 0);
        for percent in 0..=100u8 {
            assert_eq!(percent_from_scalar(f32::from(percent) / 100.0), percent, "round trip {percent}");
        }
    }

    #[test]
    fn steps_clamp_to_the_volume_range() {
        assert_eq!(step_target(72, 2), 74);
        assert_eq!(step_target(72, -2), 70);
        assert_eq!(step_target(99, 2), 100);
        assert_eq!(step_target(100, 2), 100);
        assert_eq!(step_target(1, -2), 0);
        assert_eq!(step_target(0, -2), 0);
        assert_eq!(step_target(50, 100), 100);
        assert_eq!(step_target(50, -100), 0);
        assert_eq!(step_target(200, i16::MIN), 0);
        assert_eq!(step_target(0, i16::MAX), 100);
    }

    /// Changes the real master volume by one step and back, and mutes twice.
    /// Run manually: `cargo test -p audio-windows -- --ignored --nocapture`.
    #[cfg(windows)]
    #[test]
    #[ignore = "changes the Windows master volume briefly"]
    fn round_trip_on_this_pc() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::time::{Duration, Instant};

        let events = Arc::new(AtomicUsize::new(0));
        let counter = events.clone();
        let mut volume = MasterVolume::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
        let Some(start) = volume.level().unwrap() else {
            println!("no playback device; nothing to test");
            return;
        };
        let delta = if start.percent >= 98 { -2 } else { 2 };
        let stepped = volume.step(delta).unwrap().unwrap();
        let back = volume.step(-delta).unwrap().unwrap();
        let toggled = volume.toggle_mute().unwrap().unwrap();
        let restored = volume.toggle_mute().unwrap().unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while events.load(Ordering::SeqCst) < 4 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        println!("start {start:?}, stepped {stepped:?}, back {back:?}, toggled {toggled:?}, restored {restored:?}, events {}",
            events.load(Ordering::SeqCst));
        assert_eq!(stepped.percent, step_target(start.percent, delta));
        assert_eq!(back, start);
        assert_eq!(toggled.muted, !start.muted);
        assert_eq!(restored, start);
        assert!(events.load(Ordering::SeqCst) >= 4, "Core Audio change notifications");
    }
}
