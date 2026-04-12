/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

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
        "  egb decode --input <path/to/metadata.egb> [--output <path/to/metadata.json>]",
        "  egb encode --output <path/to/metadata.egb> [--input <path/to/metadata.json>]",
        "  cat metadata.json | egb encode --output <path/to/metadata.egb>",
        "",
        "Commands:",
        "  decode                     Read .egb and print JSON to stdout or file",
        "  encode                     Read JSON (stdin or --input) and write .egb to --output",
        "",
        "Options:",
        "  --input, -i <path>         Input path (required for decode; optional for encode)",
        "  --output, -o <path>        Output path (optional for decode; required for encode)",
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
            "--input" | "-i" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--input requires a file path".to_string())?;
                input_path = Some(PathBuf::from(value));
            }
            "--output" | "-o" => {
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
            if let Some(output_path) = options.output_path {
                fs::write(&output_path, json).map_err(|error| {
                    format!(
                        "Failed to write output file {}: {error}",
                        output_path.display()
                    )
                })?;
            } else {
                println!("{json}");
            }
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

#[cfg(test)]
mod tests {
    use super::{parse_cli, Mode};

    fn parse_error(values: &[&str]) -> String {
        match parse_cli(values.iter().map(|value| value.to_string())) {
            Ok(_) => panic!("expected parse_cli to fail"),
            Err(error) => error,
        }
    }

    #[test]
    fn parse_cli_rejects_unknown_command() {
        let error = parse_error(&["unknown"]);
        assert!(error.contains("Unknown command: unknown"));
        assert!(error.contains("Usage:"));
    }

    #[test]
    fn parse_cli_rejects_unknown_argument() {
        let error = parse_error(&["decode", "--input", "level.egb", "--bad"]);
        assert!(error.contains("Unknown argument: --bad"));
        assert!(error.contains("Usage:"));
    }

    #[test]
    fn parse_cli_requires_decode_input() {
        let error = parse_error(&["decode"]);
        assert_eq!(error, "decode requires --input <path/to/metadata.egb>");
    }

    #[test]
    fn parse_cli_requires_encode_output() {
        let error = parse_error(&["encode"]);
        assert_eq!(error, "encode requires --output <path/to/metadata.egb>");
    }

    #[test]
    fn parse_cli_requires_input_value() {
        let error = parse_error(&["decode", "--input"]);
        assert_eq!(error, "--input requires a file path");
    }

    #[test]
    fn parse_cli_requires_output_value() {
        let error = parse_error(&["encode", "--output"]);
        assert_eq!(error, "--output requires a file path");
    }

    #[test]
    fn parse_cli_accepts_decode_with_optional_output() {
        let options = parse_cli(
            ["decode", "--input", "level.egb", "--output", "level.json"]
                .into_iter()
                .map(|value| value.to_string()),
        )
        .expect("valid decode args should parse");

        assert!(matches!(options.mode, Mode::Decode));
        assert_eq!(
            options.input_path.as_deref(),
            Some(std::path::Path::new("level.egb"))
        );
        assert_eq!(
            options.output_path.as_deref(),
            Some(std::path::Path::new("level.json"))
        );
    }

    #[test]
    fn parse_cli_accepts_encode_with_input_and_output() {
        let options = parse_cli(
            ["encode", "-i", "level.json", "-o", "level.egb"]
                .into_iter()
                .map(|value| value.to_string()),
        )
        .expect("valid encode args should parse");

        assert!(matches!(options.mode, Mode::Encode));
        assert_eq!(
            options.input_path.as_deref(),
            Some(std::path::Path::new("level.json"))
        );
        assert_eq!(
            options.output_path.as_deref(),
            Some(std::path::Path::new("level.egb"))
        );
    }
}
