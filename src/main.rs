use ariadne::{Label, Report, ReportKind, Source};
use clap::Parser;
use rayon::prelude::*;
use serde_json::Value;
use std::fs::File;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use which::which;

/// CLI wrapper for nixf-tidy with fancy diagnostic output
#[derive(Parser)]
#[command(
    name = "nixf-diagnose",
    version = "0.1.0",
    author = "Yingchi Long <longyingchi24s@ict.ac.cn>"
)]
struct Args {
    /// Path to the nixf-tidy executable
    #[arg(long)]
    nixf_tidy_path: Option<String>,

    /// Enable variable lookup analysis
    #[arg(long, default_value_t = true, default_missing_value="true", num_args=0..=1)]
    variable_lookup: bool,

    /// Ignore diagnostics with specific ids
    ///
    /// This can be ucsed multiple times
    #[arg(short, long, value_name = "ID")]
    ignore: Vec<String>,

    /// Input source files
    files: Vec<String>,
}

type NixfReport<'a> = (Report<(&'a str, std::ops::Range<usize>)>, &'a str, Source);

fn build_char_byte_table(s: &str) -> Vec<usize> {
    let mut table = Vec::new();
    let mut byte_pos = 0;
    for c in s.chars() {
        table.push(byte_pos);
        byte_pos += c.len_utf8();
    }
    table
}

fn byte_to_char_offset(table: &[usize], byte_pos: usize) -> usize {
    table.binary_search(&byte_pos).unwrap()
}

fn process_file<'a>(
    variable_lookup: bool,
    nixf_tidy_path: &str,
    ignore_rules: &[String],
    input_file: &'a str,
) -> Vec<NixfReport<'a>> {
    let mut cmd = Command::new(nixf_tidy_path);
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped());

    if variable_lookup {
        cmd.arg("--variable-lookup");
    }

    let mut input = String::new();
    File::open(input_file)
        .unwrap_or_else(|e| panic!("Failed to open {}: {}", input_file, e))
        .read_to_string(&mut input)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", input_file, e));

    let mut child = cmd
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to execute nixf-tidy: {}", e));
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();

    let char_byte_table = build_char_byte_table(&input);

    let output = child
        .wait_with_output()
        .unwrap_or_else(|e| panic!("Failed to read output: {}", e));

    if !output.status.success() {
        eprintln!("nixf-tidy failed on file '{}'", input_file);
        return vec![];
    }

    let stdout = String::from_utf8(output.stdout).unwrap_or_default();
    let diagnostics: Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to parse JSON from nixf-tidy output: {}", e);
            return vec![];
        }
    };

    let mut reports = vec![];

    if let Some(diags) = diagnostics.as_array() {
        for diag in diags {
            if let (
                Some(sname),
                Some(message),
                Some(spans),
                Some(severity),
                Some(args),
                Some(notes),
            ) = (
                diag.get("sname"),
                diag.get("message"),
                diag.get("range"),
                diag.get("severity"),
                diag.get("args"),
                diag.get("notes"),
            ) {
                if ignore_rules.iter().any(|rule| rule == sname) {
                    continue; // Ignore this diagnostic
                }
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
                    let start_char = byte_to_char_offset(&char_byte_table, start as usize);
                    let end_char = byte_to_char_offset(&char_byte_table, end as usize);
                    let mut report = Report::build(report_kind, input_file, start_char)
                        .with_message(&formatted_message)
                        .with_label(
                            Label::new((input_file, start_char..end_char))
                                .with_message(&formatted_message),
                        ).with_code(sname.as_str().unwrap());

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
                                    let start_char = byte_to_char_offset(&char_byte_table, note_start as usize);
                                    let end_char = byte_to_char_offset(&char_byte_table, note_end as usize);
                                    report = report.with_label(
                                        Label::new((
                                            input_file,
                                            start_char..end_char,
                                        ))
                                        .with_message(&formatted_note_message),
                                    );
                                }
                            }
                        }
                    }
                    reports.push((report.finish(), input_file, Source::from(&input)));
                }
            }
        }
    }

    reports
}

fn main() {
    let args = Args::parse();

    // Try to determine nixf-tidy path in order:
    // 1. Provided CLI argument
    // 2. Compile-time constant (from build script)
    // 3. Runtime discovery via `which`
    let nixf_tidy_path = args
        .nixf_tidy_path
        .or(option_env!("NIXF_TIDY_PATH").map(|s| s.to_string()))
        .or(which("nixf-tidy").ok().map(|p| p.display().to_string()))
        .expect("nixf-tidy executable not found in PATH or --nixf-tidy-path not provided");

    let files = args.files;
    let variable_lookup = args.variable_lookup;
    let ignore = args.ignore;

    let all_reports: Vec<_> = files
        .par_iter()
        .flat_map(|file| process_file(variable_lookup, &nixf_tidy_path, &ignore, file))
        .collect();

    if !all_reports.is_empty() {
        for (report, input_file, source) in all_reports {
            report.eprint((input_file, source)).unwrap();
        }
        std::process::exit(1);
    }
}
