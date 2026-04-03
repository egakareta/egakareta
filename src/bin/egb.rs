/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use std::env;
use std::fs;
use std::io::{self, Read as _};
use std::path::PathBuf;

use egakareta_lib::{convert_level_binary_to_json, convert_level_json_to_binary};

#[derive(Clone, Copy)]
enum Mode {
    Decode,
    Encode,
}

struct CliOptions {
    mode: Mode,
    input_path: Option<PathBuf>,
    output_path: Option<PathBuf>,
}

fn usage() -> String {
    [
        "egb - level metadata conversion helper",
        "",
        "Usage:",
        "  egb decode --input <path/to/metadata.egb>",
        "  egb encode --output <path/to/metadata.egb> [--input <path/to/metadata.json>]",
        "  cat metadata.json | egb encode --output <path/to/metadata.egb>",
        "",
        "Commands:",
        "  decode                     Read .egb and print JSON to stdout",
        "  encode                     Read JSON (stdin or --input) and write .egb to --output",
        "",
        "Options:",
        "  --input <path>             Input path (required for decode; optional for encode)",
        "  --output <path>            Output path (required for encode)",
        "  --help, -h                 Show this message",
    ]
    .join("\n")
}

fn parse_cli(mut args: impl Iterator<Item = String>) -> Result<CliOptions, String> {
    let command = args.next().ok_or_else(usage)?;

    let mode = match command.as_str() {
        "decode" => Mode::Decode,
        "encode" => Mode::Encode,
        _ => {
            return Err(format!("Unknown command: {command}\n\n{}", usage()));
        }
    };

    let mut input_path = None;
    let mut output_path = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--input requires a file path".to_string())?;
                input_path = Some(PathBuf::from(value));
            }
            "--output" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--output requires a file path".to_string())?;
                output_path = Some(PathBuf::from(value));
            }
            _ => {
                return Err(format!("Unknown argument: {arg}\n\n{}", usage()));
            }
        }
    }

    match mode {
        Mode::Decode => {
            if input_path.is_none() {
                return Err("decode requires --input <path/to/metadata.egb>".to_string());
            }
        }
        Mode::Encode => {
            if output_path.is_none() {
                return Err("encode requires --output <path/to/metadata.egb>".to_string());
            }
        }
    }

    Ok(CliOptions {
        mode,
        input_path,
        output_path,
    })
}

fn read_stdin_to_string() -> Result<String, String> {
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .map_err(|error| error.to_string())?;

    if buffer.trim().is_empty() {
        return Err("No JSON input provided on stdin".to_string());
    }

    Ok(buffer)
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", usage());
        return Ok(());
    }

    let options = parse_cli(args.into_iter())?;

    match options.mode {
        Mode::Decode => {
            let input_path = options
                .input_path
                .ok_or_else(|| "decode requires --input <path/to/metadata.egb>".to_string())?;
            let bytes = fs::read(&input_path).map_err(|error| {
                format!(
                    "Failed to read input file {}: {error}",
                    input_path.display()
                )
            })?;
            let json = convert_level_binary_to_json(&bytes)?;
            println!("{json}");
        }
        Mode::Encode => {
            let output_path = options
                .output_path
                .ok_or_else(|| "encode requires --output <path/to/metadata.egb>".to_string())?;
            let json = if let Some(input_path) = options.input_path {
                fs::read_to_string(&input_path).map_err(|error| {
                    format!(
                        "Failed to read input file {}: {error}",
                        input_path.display()
                    )
                })?
            } else {
                read_stdin_to_string()?
            };

            let encoded = convert_level_json_to_binary(&json)?;
            fs::write(&output_path, encoded).map_err(|error| {
                format!(
                    "Failed to write output file {}: {error}",
                    output_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("egb failed: {error}");
        std::process::exit(1);
    }
}
