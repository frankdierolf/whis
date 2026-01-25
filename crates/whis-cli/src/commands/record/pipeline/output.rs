//! Output pipeline phase

use anyhow::Result;
use std::fs;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use whis_core::{OutputMethod, Settings, autotype_text, copy_to_clipboard};

use crate::args::OutputFormat;

use super::super::types::ProcessedResult;

/// Output mode configuration
pub enum OutputMode {
    /// Print to stdout
    Print,
    /// Copy to clipboard (or autotype to window, based on settings)
    Clipboard,
    /// Write to file
    File(PathBuf),
}

// Subtitle timing constants
const CHARS_PER_SECOND: f64 = 15.0;
const SUBTITLE_GAP_SECS: f64 = 0.5;

/// A text segment with calculated start/end times
struct TimedSegment<'a> {
    text: &'a str,
    start: f64,
    end: f64,
}

/// Split text into timed segments for subtitle generation
fn split_into_timed_segments(text: &str) -> Vec<TimedSegment<'_>> {
    let segments: Vec<&str> = text
        .split(['.', '!', '?'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut result = Vec::with_capacity(segments.len());
    let mut time_offset = 0.0f64;

    for segment in segments {
        let duration = (segment.len() as f64 / CHARS_PER_SECOND).max(1.0);
        result.push(TimedSegment {
            text: segment,
            start: time_offset,
            end: time_offset + duration,
        });
        time_offset += duration + SUBTITLE_GAP_SECS;
    }

    result
}

/// Decompose seconds into (hours, minutes, seconds, milliseconds)
fn decompose_time(seconds: f64) -> (u32, u32, u32, u32) {
    let hours = (seconds / 3600.0) as u32;
    let minutes = ((seconds % 3600.0) / 60.0) as u32;
    let secs = (seconds % 60.0) as u32;
    let millis = ((seconds % 1.0) * 1000.0) as u32;
    (hours, minutes, secs, millis)
}

/// Format time as SRT timestamp (HH:MM:SS,mmm)
fn format_srt_time(seconds: f64) -> String {
    let (h, m, s, ms) = decompose_time(seconds);
    format!("{h:02}:{m:02}:{s:02},{ms:03}")
}

/// Format time as VTT timestamp (HH:MM:SS.mmm)
fn format_vtt_time(seconds: f64) -> String {
    let (h, m, s, ms) = decompose_time(seconds);
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}

/// Format text as SRT subtitle
fn format_srt(text: &str) -> String {
    let segments = split_into_timed_segments(text);
    if segments.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    for (i, seg) in segments.iter().enumerate() {
        output.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            format_srt_time(seg.start),
            format_srt_time(seg.end),
            seg.text
        ));
    }
    output.trim_end().to_string()
}

/// Format text as WebVTT subtitle
fn format_vtt(text: &str) -> String {
    let segments = split_into_timed_segments(text);
    if segments.is_empty() {
        return "WEBVTT\n".to_string();
    }

    let mut output = String::from("WEBVTT\n\n");
    for seg in &segments {
        output.push_str(&format!(
            "{} --> {}\n{}\n\n",
            format_vtt_time(seg.start),
            format_vtt_time(seg.end),
            seg.text
        ));
    }
    output.trim_end().to_string()
}

/// Format text according to the specified output format
pub fn format_text(text: &str, format: OutputFormat) -> String {
    match format {
        OutputFormat::Txt => text.to_string(),
        OutputFormat::Srt => format_srt(text),
        OutputFormat::Vtt => format_vtt(text),
    }
}

/// Execute output phase
pub fn output(
    result: ProcessedResult,
    mode: OutputMode,
    format: OutputFormat,
    quiet: bool,
) -> Result<()> {
    let text = result.text.trim();
    let formatted = format_text(text, format);

    match mode {
        OutputMode::Print => {
            println!("{}", formatted);
        }
        OutputMode::File(path) => {
            fs::write(&path, &formatted)?;
            if !quiet && io::stdout().is_terminal() {
                println!("Saved to {}", path.display());
            }
        }
        OutputMode::Clipboard => {
            let settings = Settings::load();

            // Handle output based on configured method
            match settings.ui.output_method {
                OutputMethod::Clipboard => {
                    copy_to_clipboard(&formatted, settings.ui.clipboard_backend)?;
                }
                OutputMethod::Autotype => {
                    autotype_text(
                        &formatted,
                        settings.ui.autotype_backend,
                        settings.ui.autotype_delay_ms,
                    )?;
                }
                OutputMethod::Both => {
                    copy_to_clipboard(&formatted, settings.ui.clipboard_backend)?;
                    autotype_text(
                        &formatted,
                        settings.ui.autotype_backend,
                        settings.ui.autotype_delay_ms,
                    )?;
                }
            }

            if !quiet && io::stdout().is_terminal() {
                match settings.ui.output_method {
                    OutputMethod::Clipboard => eprintln!("Copied to clipboard!"),
                    OutputMethod::Autotype => eprintln!("Autotyped to active window!"),
                    OutputMethod::Both => {
                        eprintln!("Copied to clipboard and autotyped to active window!")
                    }
                }
            }
        }
    }

    Ok(())
}
