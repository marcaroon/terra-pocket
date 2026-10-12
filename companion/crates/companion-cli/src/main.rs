use std::process::ExitCode;

const USAGE: &str = "\
Usage: terra-cli <command>

Commands:
  media            Print the current Windows media session (M5)
  media --watch    Print a line whenever the track, playback or source changes
  volume           Print the Windows master volume of the default playback device (M8)
  volume --watch   Print a line whenever that volume or mute changes
  devices          Scan for Terra devices over Bluetooth LE (M6)
  connect [ADDR]   Mirror Windows media and volume; handle Terra's buttons and encoder
                   ADDR is optional, e.g. 28:84:85:48:15:BA; default: the Terra last
                   connected to, otherwise the strongest found
  monitor [ADDR]   Alias of connect; includes live media, volume and command-result logs
  --help, -h       Show this help
  --version, -V    Show the version

connect/monitor keep running and reconnect after a disconnect, Bluetooth off/on,
a device restart or PC sleep, retrying after 1, 2, 4, 8, then every 15 s (M9).
They stop only on a protocol version mismatch (exit 3) or Ctrl+C.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        [] | ["--help" | "-h"] => {
            println!("Terra companion CLI {}\n\n{USAGE}", companion_core::VERSION);
            ExitCode::SUCCESS
        }
        ["--version" | "-V"] => {
            println!("terra-cli {}", companion_core::VERSION);
            ExitCode::SUCCESS
        }
        ["media"] => media::print_current(),
        ["media", "--watch"] => media::watch(),
        ["volume"] => volume::print_current(),
        ["volume", "--watch"] => volume::watch(),
        ["devices"] => ble::devices(),
        ["connect" | "monitor"] => ble::connect(None),
        ["connect" | "monitor", address] => match ble::parse_address(address) {
            Some(address) => ble::connect(Some(address)),
            None => {
                eprintln!("Invalid address {address}; expected AA:BB:CC:DD:EE:FF.");
                ExitCode::from(2)
            }
        },
        _ => {
            eprintln!("Unknown command. Use --help.");
            ExitCode::from(2)
        }
    }
}

#[cfg(windows)]
mod media {
    use media_windows::{MediaReader, MediaSnapshot, Playback};
    use std::process::ExitCode;
    use std::sync::mpsc;
    use std::time::Duration;

    fn seconds(ms: u64) -> String {
        format!("{:.1}s", ms as f64 / 1000.0)
    }

    fn print_snapshot(snapshot: &MediaSnapshot) {
        println!("Source   : {}", snapshot.source);
        println!("Title    : {}", snapshot.title);
        println!("Artist   : {}", snapshot.artist);
        println!("Album    : {}", snapshot.album);
        println!("State    : {}", snapshot.playback.label());
        println!("Position : {}", seconds(snapshot.position_ms));
        println!(
            "Duration : {}",
            if snapshot.duration_ms > 0 { seconds(snapshot.duration_ms) } else { "unknown".into() }
        );
        println!("App ID   : {}", snapshot.app_id);
    }

    fn reader() -> Result<MediaReader, ExitCode> {
        MediaReader::new().map_err(|error| {
            eprintln!("Cannot open the Windows media session manager: {error}");
            ExitCode::FAILURE
        })
    }

    pub fn print_current() -> ExitCode {
        let reader = match reader() {
            Ok(reader) => reader,
            Err(code) => return code,
        };
        match reader.current() {
            Ok(Some(snapshot)) => {
                print_snapshot(&snapshot);
                ExitCode::SUCCESS
            }
            Ok(None) => {
                println!("No active media session");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("Cannot read the current media session: {error}");
                ExitCode::FAILURE
            }
        }
    }

    /// A change worth a log line: everything except the continuously moving
    /// position (spec §32), plus seeks detected as a jump against the estimate.
    fn describe(previous: Option<&MediaSnapshot>, current: Option<&MediaSnapshot>, elapsed_ms: u64) -> Option<String> {
        match (previous, current) {
            (_, None) if previous.is_some() => Some("[MEDIA] no active session".into()),
            (_, None) => None,
            (Some(old), Some(new))
                if old.source == new.source
                    && old.title == new.title
                    && old.artist == new.artist
                    && old.album == new.album
                    && old.duration_ms == new.duration_ms =>
            {
                if old.playback != new.playback {
                    return Some(format!("[MEDIA] {} at {}", new.playback.label(), seconds(new.position_ms)));
                }
                let expected = if old.playback == Playback::Playing { old.position_ms + elapsed_ms } else { old.position_ms };
                (expected.abs_diff(new.position_ms) > 2_000)
                    .then(|| format!("[MEDIA] seek to {}", seconds(new.position_ms)))
            }
            (_, Some(new)) => Some(format!(
                "[MEDIA] {} - {} - {} [{}, {} / {}]",
                new.source,
                new.artist,
                new.title,
                new.playback.label(),
                seconds(new.position_ms),
                seconds(new.duration_ms)
            )),
        }
    }

    pub fn watch() -> ExitCode {
        let reader = match reader() {
            Ok(reader) => reader,
            Err(code) => return code,
        };
        let (tx, rx) = mpsc::channel();
        let tx = std::sync::Mutex::new(tx);
        let watch = match reader.watch(move || {
            let _ = tx.lock().map(|tx| tx.send(()));
        }) {
            Ok(watch) => watch,
            Err(error) => {
                eprintln!("Cannot subscribe to media changes: {error}");
                return ExitCode::FAILURE;
            }
        };
        println!("Watching Windows media sessions; Ctrl+C to stop.");
        let mut last: Option<MediaSnapshot> = None;
        let mut last_at = std::time::Instant::now();
        let mut first = true;
        loop {
            // Events can arrive in bursts; settle briefly, then read once.
            if !first {
                if rx.recv().is_err() {
                    return ExitCode::SUCCESS;
                }
                std::thread::sleep(Duration::from_millis(150));
                while rx.try_recv().is_ok() {}
            }
            if let Err(error) = watch.sync() {
                eprintln!("[WARN] resubscribe failed: {error}");
            }
            let current = match reader.current() {
                Ok(current) => current,
                Err(error) => {
                    eprintln!("[WARN] read failed: {error}");
                    continue;
                }
            };
            let elapsed = last_at.elapsed().as_millis() as u64;
            if first {
                match &current {
                    Some(snapshot) => println!("{}", describe(None, Some(snapshot), 0).unwrap_or_default()),
                    None => println!("[MEDIA] no active session"),
                }
                first = false;
            } else if let Some(line) = describe(last.as_ref(), current.as_ref(), elapsed) {
                println!("{line}");
            }
            last = current;
            last_at = std::time::Instant::now();
        }
    }
}

/// Parses AA:BB:CC:DD:EE:FF into the 48-bit Bluetooth address.
#[cfg_attr(not(windows), allow(dead_code))]
fn parse_address_text(text: &str) -> Option<u64> {
    let parts: Vec<&str> = text.split(':').collect();
    if parts.len() != 6 {
        return None;
    }
    parts.iter().try_fold(0u64, |acc, part| {
        (part.len() == 2).then_some(())?;
        u8::from_str_radix(part, 16).ok().map(|byte| (acc << 8) | byte as u64)
    })
}

#[cfg(windows)]
mod ble {
    use companion_core::session::{SessionOptions, run_forever};
    use std::process::ExitCode;
    use std::time::Duration;

    pub fn parse_address(text: &str) -> Option<u64> {
        super::parse_address_text(text)
    }

    pub fn devices() -> ExitCode {
        println!("Scanning for Terra (8 s)...");
        match ble_windows::scan(Duration::from_secs(8), false) {
            Ok(found) if found.is_empty() => {
                println!("No Terra device found.");
                ExitCode::FAILURE
            }
            Ok(found) => {
                for device in found {
                    println!("{}  {:<8} RSSI {} dBm", device.address_text(), device.name, device.rssi);
                }
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("Scan failed: {error}");
                ExitCode::FAILURE
            }
        }
    }

    pub fn connect(address: Option<u64>) -> ExitCode {
        let options = SessionOptions { address, scan_timeout: Duration::from_secs(10) };
        // Returns only on a protocol mismatch; everything else is retried.
        let error = run_forever(&options, |line| println!("{line}"));
        eprintln!("{error}");
        ExitCode::from(3)
    }
}

#[cfg(not(windows))]
mod ble {
    use std::process::ExitCode;

    pub fn parse_address(text: &str) -> Option<u64> {
        super::parse_address_text(text)
    }

    pub fn devices() -> ExitCode {
        eprintln!("Bluetooth LE is only implemented for Windows.");
        ExitCode::FAILURE
    }

    pub fn connect(_: Option<u64>) -> ExitCode {
        devices()
    }
}

#[cfg(windows)]
mod volume {
    use audio_windows::{MasterVolume, VolumeLevel};
    use std::process::ExitCode;
    use std::sync::mpsc;
    use std::time::Duration;

    fn describe(level: Option<VolumeLevel>) -> String {
        match level {
            Some(VolumeLevel { percent, muted: true }) => format!("[VOLUME] {percent} muted"),
            Some(VolumeLevel { percent, muted: false }) => format!("[VOLUME] {percent}"),
            None => "[VOLUME] no playback device".into(),
        }
    }

    pub fn print_current() -> ExitCode {
        match MasterVolume::new(|| {}).and_then(|volume| volume.level()) {
            Ok(Some(level)) => {
                println!("Volume : {}%", level.percent);
                println!("Muted  : {}", if level.muted { "yes" } else { "no" });
                ExitCode::SUCCESS
            }
            Ok(None) => {
                println!("No playback device");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("Cannot read the Windows master volume: {error}");
                ExitCode::FAILURE
            }
        }
    }

    pub fn watch() -> ExitCode {
        let (tx, rx) = mpsc::channel();
        let mut volume = match MasterVolume::new(move || {
            let _ = tx.send(());
        }) {
            Ok(volume) => volume,
            Err(error) => {
                eprintln!("Cannot open the Windows master volume: {error}");
                return ExitCode::FAILURE;
            }
        };
        println!("Watching the Windows master volume; Ctrl+C to stop.");
        let mut last = None;
        let mut first = true;
        loop {
            // Wake on a change, or every second to follow a new default device.
            if !first && rx.recv_timeout(Duration::from_secs(1)).is_ok() {
                std::thread::sleep(Duration::from_millis(50));
                while rx.try_recv().is_ok() {}
            }
            let print = std::mem::replace(&mut first, false);
            let current = match volume.refresh().and_then(|_| volume.level()) {
                Ok(current) => current,
                Err(error) => {
                    eprintln!("[WARN] read failed: {error}");
                    continue;
                }
            };
            if print || current != last {
                println!("{}", describe(current));
            }
            last = current;
        }
    }
}

#[cfg(not(windows))]
mod volume {
    use std::process::ExitCode;

    pub fn print_current() -> ExitCode {
        eprintln!("The Windows master volume is only available on Windows.");
        ExitCode::FAILURE
    }

    pub fn watch() -> ExitCode {
        print_current()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn address_parsing() {
        assert_eq!(super::parse_address_text("28:84:85:48:15:BA"), Some(0x2884_8548_15BA));
        assert_eq!(super::parse_address_text("28:84:85:48:15"), None);
        assert_eq!(super::parse_address_text("28:84:85:48:15:GG"), None);
        assert_eq!(super::parse_address_text("288:4:85:48:15:BA"), None);
    }
}

#[cfg(not(windows))]
mod media {
    use std::process::ExitCode;

    pub fn print_current() -> ExitCode {
        eprintln!("Windows media sessions are only available on Windows.");
        ExitCode::FAILURE
    }

    pub fn watch() -> ExitCode {
        print_current()
    }
}
