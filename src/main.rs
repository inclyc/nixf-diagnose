use ariadne::{Label, Report, ReportKind, Source};
use clap::{App, Arg};
use serde_json::Value;
use std::fs::File;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use which::which;

fn main() {
    let matches = App::new("nixf-diagnose")
        .version("0.1.0")
        .author("Yingchi Long <longyingchi24s@ict.ac.cn>")
        .about("CLI wrapper for nixf-tidy with fancy diagnostic output")
        .arg(
            Arg::with_name("nixf-tidy-path")
                .long("nixf-tidy-path")
                .value_name("PATH")
                .help("Path to the nixf-tidy executable")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("input")
                .long("input")
                .short("i")
                .value_name("FILE")
                .help("Input source file")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("variable-lookup")
                .long("variable-lookup")
                .value_name("BOOL")
                .help("Enable variable lookup analysis")
                .default_value("true")
                .takes_value(true),
        )
        .get_matches();

    let nixf_tidy_path = matches
        .value_of("nixf-tidy-path")
        .map(|p| p.to_string())
        .or_else(|| which("nixf-tidy").ok().map(|p| p.display().to_string()))
        .or(
            option_env!("NIXF_TIDY_PATH")
                .map(|p| p.to_string())
        )
        .expect("nixf-tidy executable not found in PATH or --nixf-tidy-path not provided");

    let input_file = matches.value_of("input").unwrap();
    let variable_lookup = matches
        .value_of("variable-lookup")
        .unwrap()
        .parse::<bool>()
        .unwrap_or(true);

    let mut cmd = Command::new(nixf_tidy_path);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped());

    if variable_lookup {
        cmd.arg("--variable-lookup");
    }

    let mut input = String::new();
    File::open(input_file)
        .expect("Failed to open input file")
        .read_to_string(&mut input)
        .expect("Failed to read input file");

    let mut child = cmd.spawn().expect("Failed to execute nixf-tidy");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .expect("Failed to write to stdin");

    let output = child
        .wait_with_output()
        .expect("Failed to read nixf-tidy output");

    if !output.status.success() {
        eprintln!("nixf-tidy failed");
        std::process::exit(1);
    }

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 output");
    let diagnostics: Value = serde_json::from_str(&stdout).expect("Failed to parse JSON");

    if let Some(diags) = diagnostics.as_array() {
        for diag in diags {
            if let (Some(message), Some(spans), Some(severity), Some(args), Some(notes)) = (
                diag.get("message"),
                diag.get("range"),
                diag.get("severity"),
                diag.get("args"),
                diag.get("notes"),
            ) {
                let report_kind = match severity.as_i64().unwrap_or(1) {
                    0 => ReportKind::Error,
                    1 => ReportKind::Error,
                    2 => ReportKind::Warning,
                    3 => ReportKind::Advice,
                    4 => ReportKind::Advice,
                    _ => ReportKind::Error,
                };

                let mut formatted_message = message.as_str().unwrap_or("Unknown error").to_string();
                if let Some(args_array) = args.as_array() {
                    for arg in args_array {
                        if let Some(arg_str) = arg.as_str() {
                            formatted_message = formatted_message.replacen("{}", arg_str, 1);
                        }
                    }
                }

                if let (Some(start), Some(end)) = (
                    spans
                        .get("lCur")
                        .and_then(|s| s.get("offset").and_then(|o| o.as_u64())),
                    spans
                        .get("rCur")
                        .and_then(|e| e.get("offset").and_then(|o| o.as_u64())),
                ) {
                    let mut report = Report::build(report_kind, input_file, start as usize)
                        .with_message(&formatted_message)
                        .with_label(
                            Label::new((input_file, start as usize..end as usize))
                                .with_message(&formatted_message),
                        );

                    if let Some(notes_array) = notes.as_array() {
                        for note in notes_array {
                            if let (Some(note_message), Some(note_args), Some(note_spans)) =
                                (note.get("message"), note.get("args"), note.get("range"))
                            {
                                let mut formatted_note_message =
                                    note_message.as_str().unwrap_or("Unknown note").to_string();
                                if let Some(note_args_array) = note_args.as_array() {
                                    for arg in note_args_array {
                                        if let Some(arg_str) = arg.as_str() {
                                            formatted_note_message =
                                                formatted_note_message.replacen("{}", arg_str, 1);
                                        }
                                    }
                                }

                                if let (Some(note_start), Some(note_end)) = (
                                    note_spans
                                        .get("lCur")
                                        .and_then(|s| s.get("offset").and_then(|o| o.as_u64())),
                                    note_spans
                                        .get("rCur")
                                        .and_then(|e| e.get("offset").and_then(|o| o.as_u64())),
                                ) {
                                    report = report.with_label(
                                        Label::new((
                                            input_file,
                                            note_start as usize..note_end as usize,
                                        ))
                                        .with_message(&formatted_note_message),
                                    );
                                }
                            }
                        }
                    }

                    let source = Source::from(&input);
                    report.finish().print((input_file, source)).unwrap();
                }
            }
        }
    }
}
