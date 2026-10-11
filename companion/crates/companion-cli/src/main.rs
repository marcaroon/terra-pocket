use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [] => help(),
        [arg] if arg == "--help" || arg == "-h" => help(),
        [arg] if arg == "--version" || arg == "-V" => {
            println!("terra-cli {}", companion_core::VERSION);
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("Command unavailable in M0/M1. Use --help for current scope.");
            ExitCode::from(2)
        }
    }
}

fn help() -> ExitCode {
    println!("Terra — repository foundation (M0/M1)");
    println!("Usage: terra-cli [--help | --version]");
    println!("Media commands arrive in M5; devices/connect/monitor in M6.");
    ExitCode::SUCCESS
}
