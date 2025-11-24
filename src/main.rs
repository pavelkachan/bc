use anyhow::{Context, Result};
use arboard::Clipboard;
use base64::{engine::general_purpose, Engine as _};
use clap::Parser;
use is_terminal::IsTerminal;
use std::env;
use std::io::{self, Read, Write};

/// Boring Clipboard - A simple cross-platform clipboard tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(after_help = "\
Examples:
  echo \"Hello\" | bc           # Copy \"Hello\\n\"
  echo \"Hello\" | bc -t        # Copy \"Hello\" (trim newline)
  cat file.txt | bc           # Copy file content")]
struct Args {
    /// Trim trailing newline from input
    #[arg(short, long)]
    trim: bool,

    /// Force local clipboard usage (disable remote detection)
    #[arg(short, long)]
    local: bool,
}

const LARGE_INPUT_THRESHOLD: usize = 5 * 1024 * 1024; // 5MB

fn main() -> Result<()> {
    let args = Args::parse();

    // Check if we're receiving piped input. If not, show usage and exit.
    let mut buffer = String::new();
    if !io::stdin().is_terminal() {
        io::stdin().read_to_string(&mut buffer).context("Failed to read from stdin")?;
    } else {
        eprintln!("Usage: echo 'text' | bc");
        eprintln!("Try 'bc --help' for more information.");
        return Ok(());
    }

    // Warn if input is unusually large (5MB+), as some clipboard managers might choke.
    if buffer.len() > LARGE_INPUT_THRESHOLD {
        eprintln!("Warning: Input size exceeds 5MB. This might cause issues with some clipboard managers.");
    }

    if args.trim && buffer.ends_with('\n') {
        buffer.truncate(buffer.trim_end_matches('\n').len());
    }

    // If --local is passed, skip remote detection.
    if !args.local && is_remote_session() {
        copy_remote(&buffer)?;
    } else {
        // Try local clipboard first. If it fails (e.g., no X11/Wayland in SSM session),
        // and --local wasn't forced, fallback to OSC 52.
        if let Err(e) = copy_local(&buffer) {
            if args.local {
                return Err(e);
            }
            // Silent fallback to OSC 52
            copy_remote(&buffer)?;
        }
    }

    Ok(())
}

fn is_remote_session() -> bool {
    env::var("SSH_CLIENT").is_ok() || env::var("SSH_TTY").is_ok() || env::var("SSH_CONNECTION").is_ok()
}

fn copy_local(text: &str) -> Result<()> {
    let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
    clipboard.set_text(text).context("Failed to copy text to local clipboard")?;
    Ok(())
}

fn copy_remote(text: &str) -> Result<()> {
    let osc52 = build_osc52_sequence(text);

    // We try to write to stdout first. If that's redirected (e.g. to a file),
    // the terminal won't see the escape sequence, so we fallback to stderr.
    let mut stream: Box<dyn Write> = if io::stdout().is_terminal() {
        Box::new(io::stdout())
    } else {
        Box::new(io::stderr())
    };

    write!(stream, "{}", osc52).context("Failed to write OSC 52 sequence")?;
    
    // Flush to ensure it's sent
    stream.flush()?;

    Ok(())
}

fn build_osc52_sequence(text: &str) -> String {
    // OSC 52 escape sequence: \x1b]52;c;{base64}\x07
    // 'c' stands for clipboard.
    let encoded = general_purpose::STANDARD.encode(text);
    format!("\x1b]52;c;{}\x07", encoded)
}

#[cfg(test)]
mod tests {

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
        assert_eq!(buffer, "hello"); // Trims all trailing newlines

        let mut buffer = String::from("hello");
        if buffer.ends_with('\n') {
            buffer.truncate(buffer.trim_end_matches('\n').len());
        }
        assert_eq!(buffer, "hello");
        
        // Empty string
        let mut buffer = String::new();
        if buffer.ends_with('\n') {
            buffer.truncate(buffer.trim_end_matches('\n').len());
        }
        assert_eq!(buffer, "");
    }

    #[test]
    fn test_osc52_generation() {
        use super::build_osc52_sequence;
        
        let text = "Hello World";
        let seq = build_osc52_sequence(text);
        // Base64 of "Hello World" is "SGVsbG8gV29ybGQ="
        assert_eq!(seq, "\x1b]52;c;SGVsbG8gV29ybGQ=\x07");

        let text = "";
        let seq = build_osc52_sequence(text);
        assert_eq!(seq, "\x1b]52;c;\x07");
    }
}
