mod clipboard;
mod osc52;

use anyhow::{Context, Result};
use clap::Parser;
use is_terminal::IsTerminal;
use std::io::{self, Read};
use std::process::ExitCode;

use clipboard::{
    clear_local, clear_remote, copy_local, copy_remote, is_remote_session, paste_clipboard,
};

/// Exit codes for different scenarios
#[repr(i32)]
enum BcExitCode {
    Success = 0,
    GeneralError = 1,
    EmptyInput = 2,
    ClipboardUnavailable = 3,
    InvalidInput = 4,
}

impl From<BcExitCode> for ExitCode {
    fn from(code: BcExitCode) -> Self {
        ExitCode::from(code as u8)
    }
}

/// Boring Clipboard - A simple cross-platform clipboard tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(after_help = "\
Examples:
  echo \"Hello\" | bc           # Copy \"Hello\\n\"
  echo \"Hello\" | bc -t        # Copy \"Hello\" (trim newline)
  cat file.txt | bc           # Copy file content
  bc -p                       # Paste clipboard content
  bc -c                       # Clear clipboard")]
struct Args {
    /// Trim trailing newline from input
    #[arg(short, long)]
    trim: bool,

    /// Force local clipboard usage (disable remote detection)
    #[arg(short, long)]
    local: bool,

    /// Read from clipboard and print to stdout (instead of writing)
    #[arg(short = 'p', long)]
    paste: bool,

    /// Clear the clipboard
    #[arg(short = 'c', long)]
    clear: bool,

    /// Force copy even if binary data is detected
    #[arg(short, long)]
    force: bool,

    /// Show preview of copied content
    #[arg(short = 'P', long)]
    preview: bool,

    /// Attempt OSC 52 clipboard query for remote paste (experimental, limited terminal support)
    #[arg(long)]
    force_paste: bool,
}

const PREVIEW_LENGTH: usize = 50;
/// Allowed control characters in text input
const ALLOWED_CONTROL_CHARS: [char; 4] = ['\n', '\r', '\t', '\x0c'];

fn main() -> ExitCode {
    let args = Args::parse();

    if args.paste && args.clear {
        eprintln!("Error: --paste and --clear are mutually exclusive");
        return BcExitCode::GeneralError.into();
    }

    if args.paste {
        return handle_paste(&args);
    }

    if args.clear {
        return handle_clear(&args);
    }

    handle_copy(&args)
}

/// Handle paste operation
fn handle_paste(args: &Args) -> ExitCode {
    match paste_clipboard(args) {
        Ok(text) if text.is_empty() => {
            eprintln!("Clipboard is empty");
            BcExitCode::ClipboardUnavailable.into()
        }
        Ok(text) => {
            println!("{}", text);
            BcExitCode::Success.into()
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            BcExitCode::ClipboardUnavailable.into()
        }
    }
}

/// Handle clear operation
fn handle_clear(args: &Args) -> ExitCode {
    let result = if !args.local && is_remote_session() {
        clear_remote().map(|_| {
            eprintln!("Clipboard cleared (via OSC 52)");
        })
    } else {
        clear_local().or_else(|e| {
            if args.local {
                return Err(e);
            }
            clear_remote()?;
            eprintln!("Clipboard cleared (via OSC 52)");
            Ok(())
        })
    };

    match result {
        Ok(_) => BcExitCode::Success.into(),
        Err(e) => {
            eprintln!("Error: {}", e);
            BcExitCode::GeneralError.into()
        }
    }
}

/// Handle copy operation
fn handle_copy(args: &Args) -> ExitCode {
    match copy_to_clipboard(args) {
        Ok(BcExitCode::Success) => BcExitCode::Success.into(),
        Ok(code) => code.into(),
        Err(e) => {
            eprintln!("Error: {}", e);
            BcExitCode::GeneralError.into()
        }
    }
}

fn copy_to_clipboard(args: &Args) -> Result<BcExitCode> {
    let mut buffer = read_input()?;

    if contains_binary_data(&buffer) && !args.force {
        eprintln!("Warning: Input contains binary/control characters. Use --force to proceed.");
        return Ok(BcExitCode::InvalidInput);
    }

    if args.trim && buffer.ends_with('\n') {
        buffer.truncate(buffer.trim_end_matches('\n').len());
    }

    if buffer.is_empty() {
        eprintln!("Error: Input is empty");
        return Ok(BcExitCode::EmptyInput);
    }

    if !args.local && is_remote_session() {
        copy_remote(&buffer)?;
    } else {
        copy_local(&buffer).or_else(|e| {
            if !args.local {
                copy_remote(&buffer)?;
                Ok(())
            } else {
                Err(e)
            }
        })?;
    }

    if args.preview {
        show_preview(&buffer);
    }

    Ok(BcExitCode::Success)
}

/// Read input from stdin, or show usage if not piped
fn read_input() -> Result<String> {
    if !io::stdin().is_terminal() {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
        Ok(buffer)
    } else {
        eprintln!("Usage: echo 'text' | bc");
        eprintln!("Try 'bc --help' for more information.");
        Err(anyhow::anyhow!("No input provided"))
    }
}

fn contains_binary_data(text: &str) -> bool {
    text.contains('\0')
        || text
            .chars()
            .any(|c| c.is_control() && !ALLOWED_CONTROL_CHARS.contains(&c))
}

fn show_preview(content: &str) {
    if content.is_empty() {
        eprintln!("Copied: <empty> (0 bytes)");
        return;
    }

    let total = content.len();
    let total_chars = content.chars().count();

    let preview = escape_control_chars(content.chars().take(PREVIEW_LENGTH));
    let preview = if total_chars > PREVIEW_LENGTH {
        format!("{}...", preview)
    } else {
        preview
    };

    eprintln!(
        "Copied: \"{}\" ({} bytes, {} chars)",
        preview, total, total_chars
    );
}

/// Escape control characters for display
fn escape_control_chars(chars: impl Iterator<Item = char>) -> String {
    chars
        .map(|c| match c {
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            '\t' => "\\t".to_string(),
            '\x0c' => "\\f".to_string(),
            c if c.is_control() => format!("\\x{:02x}", c as u32),
            c => c.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_logic() {
        let mut buffer = String::from("hello\n");
        if buffer.ends_with('\n') {
            buffer.truncate(buffer.trim_end_matches('\n').len());
        }
        assert_eq!(buffer, "hello");

        let mut buffer = String::from("hello\n\n");
        if buffer.ends_with('\n') {
            buffer.truncate(buffer.trim_end_matches('\n').len());
        }
        assert_eq!(buffer, "hello");

        let mut buffer = String::from("hello");
        if buffer.ends_with('\n') {
            buffer.truncate(buffer.trim_end_matches('\n').len());
        }
        assert_eq!(buffer, "hello");

        let mut buffer = String::new();
        if buffer.ends_with('\n') {
            buffer.truncate(buffer.trim_end_matches('\n').len());
        }
        assert_eq!(buffer, "");
    }

    #[test]
    fn test_binary_data_detection() {
        assert!(contains_binary_data("hello\0world"));
        assert!(contains_binary_data("hello\x01world"));
        assert!(!contains_binary_data("hello\nworld"));
        assert!(!contains_binary_data("hello\rworld"));
        assert!(!contains_binary_data("hello\tworld"));
        assert!(!contains_binary_data("hello\r\nworld"));
        assert!(!contains_binary_data("hello world"));
        assert!(!contains_binary_data("hello\x0cworld"));
    }

    #[test]
    fn test_exit_codes() {
        assert_eq!(BcExitCode::Success as i32, 0);
        assert_eq!(BcExitCode::GeneralError as i32, 1);
        assert_eq!(BcExitCode::EmptyInput as i32, 2);
        assert_eq!(BcExitCode::ClipboardUnavailable as i32, 3);
        assert_eq!(BcExitCode::InvalidInput as i32, 4);
    }

    #[test]
    fn test_preview_formatting() {
        assert!(escape_control_chars("hello\n".chars()).contains("\\n"));
        assert!(escape_control_chars("hello\r".chars()).contains("\\r"));
        assert!(escape_control_chars("hello\t".chars()).contains("\\t"));
        assert!(escape_control_chars("hello\x01".chars()).contains("\\x01"));
    }

    #[test]
    fn test_preview_length() {
        let content = "x".repeat(100);
        assert!(content.len() > PREVIEW_LENGTH);
    }
}
