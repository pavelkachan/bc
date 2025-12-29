use anyhow::{Context, Result};
use arboard::Clipboard;
use base64::{engine::general_purpose, Engine as _};
use clap::Parser;
use is_terminal::IsTerminal;
use std::env;
use std::io::{self, Read, Write};
use std::process::ExitCode;

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
    #[arg(short = 'f', long)]
    force: bool,

    /// Show preview of copied content
    #[arg(short = 'P', long)]
    preview: bool,
}

const PREVIEW_LENGTH: usize = 50;
const OSC52_MAX_SIZE: usize = 10 * 1024 * 1024; // 10MB limit for OSC 52

fn main() -> ExitCode {
    let args = Args::parse();

    // Handle mutually exclusive operations
    if args.paste && args.clear {
        eprintln!("Error: --paste and --clear are mutually exclusive");
        return BcExitCode::GeneralError.into();
    }

    // Handle paste operation
    if args.paste {
        return match paste_clipboard() {
            Ok(code) => code.into(),
            Err(e) => {
                eprintln!("Error: {}", e);
                BcExitCode::ClipboardUnavailable.into()
            }
        };
    }

    // Handle clear operation
    if args.clear {
        return match clear_clipboard() {
            Ok(_) => BcExitCode::Success.into(),
            Err(e) => {
                eprintln!("Error: {}", e);
                BcExitCode::GeneralError.into()
            }
        };
    }

    // Handle write operation (default)
    match copy_to_clipboard(&args) {
        Ok(BcExitCode::Success) => BcExitCode::Success.into(),
        Ok(code) => code.into(),
        Err(e) => {
            eprintln!("Error: {}", e);
            BcExitCode::GeneralError.into()
        }
    }
}

fn copy_to_clipboard(args: &Args) -> Result<BcExitCode> {
    // Check if we're receiving piped input. If not, show usage and exit.
    let mut buffer = String::new();
    if !io::stdin().is_terminal() {
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
    } else {
        eprintln!("Usage: echo 'text' | bc");
        eprintln!("Try 'bc --help' for more information.");
        return Ok(BcExitCode::GeneralError);
    }

    // Input validation - check for binary data (before trim)
    if contains_binary_data(&buffer) && !args.force {
        eprintln!("Warning: Input contains binary/control characters. Use --force to proceed.");
        return Ok(BcExitCode::InvalidInput);
    }

    // Apply trim if requested
    if args.trim && buffer.ends_with('\n') {
        buffer.truncate(buffer.trim_end_matches('\n').len());
    }

    // Check for empty input (after trim, since trim might make it empty)
    if buffer.is_empty() {
        eprintln!("Error: Input is empty");
        return Ok(BcExitCode::EmptyInput);
    }

    // Copy to appropriate clipboard
    if !args.local && is_remote_session() {
        copy_remote(&buffer)?;
    } else {
        // Try local clipboard first. If it fails, and --local wasn't forced, fallback to OSC 52.
        if let Err(e) = copy_local(&buffer) {
            if args.local {
                return Err(e);
            }
            // Silent fallback to OSC 52
            copy_remote(&buffer)?;
        }
    }

    // Show preview if requested
    if args.preview {
        show_preview(&buffer);
    }

    Ok(BcExitCode::Success)
}

fn paste_clipboard() -> Result<BcExitCode> {
    let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
    let text = clipboard
        .get_text()
        .context("Failed to read from clipboard")?;

    if text.is_empty() {
        eprintln!("Clipboard is empty");
        return Ok(BcExitCode::ClipboardUnavailable);
    }

    println!("{}", text);
    Ok(BcExitCode::Success)
}

fn clear_clipboard() -> Result<()> {
    let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
    clipboard
        .set_text("")
        .context("Failed to clear clipboard")?;
    eprintln!("Clipboard cleared");
    Ok(())
}

fn contains_binary_data(text: &str) -> bool {
    // Check for null bytes or excessive control characters
    text.contains('\0')
        || text.chars().any(|c| {
            c.is_control() && c != '\n' && c != '\r' && c != '\t' && c != '\x0c' // form feed
        })
}

fn show_preview(content: &str) {
    let total = content.len();
    let total_chars = content.chars().count();

    if total == 0 {
        eprintln!("Copied: <empty> (0 bytes)");
        return;
    }

    // Create a preview, escaping control characters
    let preview_text: String = content
        .chars()
        .take(PREVIEW_LENGTH)
        .map(|c| match c {
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            '\t' => "\\t".to_string(),
            '\x0c' => "\\f".to_string(),
            c if c.is_control() => format!("\\x{:02x}", c as u32),
            c => c.to_string(),
        })
        .collect();

    let preview = if total_chars > PREVIEW_LENGTH {
        format!("{}...", preview_text)
    } else {
        preview_text
    };

    eprintln!(
        "Copied: \"{}\" ({} bytes, {} chars)",
        preview,
        total,
        total_chars
    );
}

fn is_remote_session() -> bool {
    env::var("SSH_CLIENT").is_ok()
        || env::var("SSH_TTY").is_ok()
        || env::var("SSH_CONNECTION").is_ok()
        || env::var("AWS_SSM_SESSION_ID").is_ok()
        || env::var("SSM_SESSION_ID").is_ok()
}

fn copy_local(text: &str) -> Result<()> {
    let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
    clipboard.set_text(text).context("Failed to write to local clipboard")?;
    Ok(())
}

fn copy_remote(text: &str) -> Result<()> {
    let encoded = general_purpose::STANDARD.encode(text);

    // Check if content exceeds OSC 52 practical limit
    if encoded.len() > OSC52_MAX_SIZE {
        return Err(anyhow::anyhow!(
            "Content too large for OSC 52 clipboard ({} bytes when encoded, max {} bytes). \
             Use --local flag or alternative transfer method (scp, rsync, etc.)",
            encoded.len(),
            OSC52_MAX_SIZE
        ));
    }

    let osc52 = build_osc52_sequence_raw(&encoded);
    write_osc52_sequence(&osc52)?;
    Ok(())
}

fn write_osc52_sequence(osc52: &str) -> Result<()> {
    // We try to write to stdout first. If that's redirected (e.g. to a file),
    // the terminal won't see the escape sequence, so we fallback to stderr.
    let mut stream: Box<dyn Write> = if io::stdout().is_terminal() {
        Box::new(io::stdout())
    } else {
        Box::new(io::stderr())
    };

    // Disable auto-wrap (\x1b[?7l), write OSC 52, then re-enable auto-wrap (\x1b[?7h)
    // This prevents legacy consoles (like conhost.exe) from inserting newlines in the middle of the sequence.
    write!(stream, "\x1b[?7l{}\x1b[?7h", osc52).context("Failed to write OSC 52 sequence")?;

    // Flush to ensure it's sent
    stream.flush()?;

    Ok(())
}

#[allow(dead_code)]
fn build_osc52_sequence(text: &str) -> String {
    // OSC 52 escape sequence: \x1b]52;c;{base64}\x07
    // 'c' stands for clipboard.
    let encoded = general_purpose::STANDARD.encode(text);
    format!("\x1b]52;c;{}\x07", encoded)
}

fn build_osc52_sequence_raw(encoded: &str) -> String {
    // OSC 52 escape sequence with pre-encoded data
    format!("\x1b]52;c;{}\x07", encoded)
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
    fn test_osc52_generation() {
        let text = "Hello World";
        let seq = build_osc52_sequence(text);
        // Base64 of "Hello World" is "SGVsbG8gV29ybGQ="
        assert_eq!(seq, "\x1b]52;c;SGVsbG8gV29ybGQ=\x07");

        let text = "";
        let seq = build_osc52_sequence(text);
        assert_eq!(seq, "\x1b]52;c;\x07");
    }

    #[test]
    fn test_binary_data_detection() {
        // Null bytes should be detected
        assert!(contains_binary_data("hello\0world"));

        // Other control characters (except common ones) should be detected
        assert!(contains_binary_data("hello\x01world"));

        // Common whitespace should NOT be detected as binary
        assert!(!contains_binary_data("hello\nworld"));
        assert!(!contains_binary_data("hello\rworld"));
        assert!(!contains_binary_data("hello\tworld"));
        assert!(!contains_binary_data("hello\r\nworld"));

        // Clean text should not be detected as binary
        assert!(!contains_binary_data("hello world"));

        // Form feed should not be detected
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
    fn test_osc52_size_limit() {
        // Small data should be within limit
        let small_text = "Hello World";
        let encoded = general_purpose::STANDARD.encode(small_text);
        assert!(encoded.len() <= OSC52_MAX_SIZE);

        // Very large data should exceed limit
        // Base64 encoding increases size by ~33%, so 8MB of text will exceed 10MB when encoded
        let large_text = "x".repeat(8 * 1024 * 1024); // 8MB of text
        let encoded = general_purpose::STANDARD.encode(&large_text);
        assert!(encoded.len() > OSC52_MAX_SIZE);
    }

    #[test]
    fn test_preview_formatting() {
        // Empty content
        let content = "";
        let total = content.len();
        let total_chars = content.chars().count();
        assert_eq!(total, 0);
        assert_eq!(total_chars, 0);

        // Short content
        let content = "Hello World";
        let total = content.len();
        let total_chars = content.chars().count();
        assert_eq!(total, 11);
        assert_eq!(total_chars, 11);

        // Long content should be truncated in preview
        let content = "x".repeat(100);
        assert!(content.len() > PREVIEW_LENGTH);
    }
}
