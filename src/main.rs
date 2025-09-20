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
    version = "0.1.4",
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
    /// This can be used multiple times
    #[arg(short, long, value_name = "ID")]
    ignore: Vec<String>,

    /// Only run a single diagnostic
    #[arg(short, long)]
    only: Option<String>,

    /// Automatically apply fixes to source files
    #[arg(long)]
    auto_fix: bool,

    /// Input source files
    files: Vec<String>,
}

type NixfReport<'a> = (Report<(&'a str, std::ops::Range<usize>)>, &'a str, Source);

#[derive(Debug, Clone)]
struct Edit {
    range: std::ops::Range<usize>,
    new_text: String,
}

fn apply_fixes_to_content(content: &str, edits: &[Edit]) -> String {
    if edits.is_empty() {
        return content.to_string();
    }

    // Sort fixes by start position in reverse order to apply from end to beginning.
    // This is to avoid the location markers from getting out of sync once the first
    // edit is done.
    let mut sorted_fixes = edits.to_vec();
    sorted_fixes.sort_by(|a, b| b.range.start.cmp(&a.range.start));

    let mut result = content.to_string();
    for fix in sorted_fixes {
        if fix.range.end <= result.len() {
            result.replace_range(fix.range.clone(), &fix.new_text);
        }
    }

    result
}

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
    only: &Option<String>,
    auto_fix: bool,
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
    let mut all_edits = vec![];

    if let Some(diags) = diagnostics.as_array() {
        for diag in diags {
            if let (
                Some(sname),
                Some(message),
                Some(spans),
                Some(severity),
                Some(args),
                Some(notes),
                Some(fixes), // Vec<Fix>, // Fix = { edits, message }
            ) = (
                diag.get("sname"),
                diag.get("message"),
                diag.get("range"),
                diag.get("severity"),
                diag.get("args"),
                diag.get("notes"),
                diag.get("fixes"),
            ) {
                if let Some(rule) = only {
                    if rule != sname {
                        continue; // Ignore all except --only
                    }
                }

                if ignore_rules.iter().any(|rule| rule == sname) {
                    continue; // Ignore this diagnostic
                }

                // Collect fixes for auto-fix functionality
                // TODO: We currently limit this to one edit per file per run, until
                // https://github.com/inclyc/nixf-diagnose/issues/13
                // is sorted out.
                if auto_fix && all_edits.len() == 0 {
                    if let Some(fixes_array) = fixes.as_array() {
                        if fixes_array.len() > 0 {
                            if fixes_array.len() > 1 {
                                eprintln!(
                                    "Warning: Multiple fixes found for a single diagnostic. Only the first fix will be applied to '{input_file}'."
                                );
                            }
                            let first_fix = fixes_array.first().unwrap();
                            if let Some(edits) = first_fix.get("edits").and_then(|e| e.as_array()) {
                                for edit in edits {
                                    if let (Some(new_text), Some(range)) = (
                                        edit.get("newText").and_then(|t| t.as_str()),
                                        edit.get("range"),
                                    ) {
                                        if let (Some(start), Some(end)) = (
                                            range
                                                .get("lCur")
                                                .and_then(|s| s.get("offset").and_then(|o| o.as_u64())),
                                            range
                                                .get("rCur")
                                                .and_then(|e| e.get("offset").and_then(|o| o.as_u64())),
                                        ) {
                                            all_edits.push(Edit {
                                                range: (start as usize)..(end as usize),
                                                new_text: new_text.to_string(),
                                            });
                                        }
                                    }
                                }
                                continue
                            }
                        }
                    }
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
                        )
                        .with_code(sname.as_str().unwrap());

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
                                    let start_char =
                                        byte_to_char_offset(&char_byte_table, note_start as usize);
                                    let end_char =
                                        byte_to_char_offset(&char_byte_table, note_end as usize);
                                    report = report.with_label(
                                        Label::new((input_file, start_char..end_char))
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

    // Apply edits if auto_fix is enabled
    if auto_fix && !all_edits.is_empty() {
        let fixed_content = apply_fixes_to_content(&input, &all_edits);
        if let Err(e) = std::fs::write(input_file, fixed_content) {
            eprintln!("Failed to write fixed content to {input_file}: {e}");
        } else {
            eprintln!("Applied {} edits to {}", all_edits.len(), input_file);
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
    let auto_fix = args.auto_fix;
    let ignore = args.ignore;
    let only = args.only;

    let all_reports: Vec<_> = files
        .par_iter()
        .flat_map(|file| process_file(variable_lookup, &nixf_tidy_path, &ignore, &only, auto_fix, file))
        .collect();

    if !all_reports.is_empty() {
        for (report, input_file, source) in all_reports {
            report.eprint((input_file, source)).unwrap();
        }
        std::process::exit(1);
    }
}
