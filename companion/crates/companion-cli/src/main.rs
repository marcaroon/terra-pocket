use std::process::ExitCode;

const USAGE: &str = "\
Usage: terra-cli <command>

Commands:
  media            Print the current Windows media session (M5)
  media --watch    Print a line whenever the track, playback or source changes
  --help, -h       Show this help
  --version, -V    Show the version

devices/connect/monitor arrive with the Windows BLE client (M6).";

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
        ["devices" | "connect" | "monitor", ..] => {
            eprintln!("Command not available yet: arrives with the Windows BLE client (M6).");
            ExitCode::from(2)
        }
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
        let watch = match reader.watch(tx) {
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
