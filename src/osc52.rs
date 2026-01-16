use anyhow::{Context, Result};
use is_terminal::IsTerminal;
use std::io::{self, Write};

/// OSC 52 escape sequence prefix: \x1b]52;c;
const OSC52_PREFIX: &str = "\x1b]52;c;";
/// OSC 52 escape sequence terminator: \x07
const OSC52_TERMINATOR: char = '\x07';
/// Maximum size for OSC 52 clipboard content (10MB)
pub const OSC52_MAX_SIZE: usize = 10 * 1024 * 1024;

/// Build OSC 52 escape sequence with pre-encoded base64 data.
/// Format: \x1b]52;c;{base64}\x07
pub fn build_sequence_raw(encoded: &str) -> String {
    format!("{}{}{}", OSC52_PREFIX, encoded, OSC52_TERMINATOR)
}

/// Write OSC 52 sequence to terminal.
/// Uses stdout if it's a TTY, otherwise falls back to stderr.
/// Disables auto-wrap during the sequence to prevent corruption in legacy terminals.
pub fn write_sequence(osc52: &str) -> Result<()> {
    let mut stream: Box<dyn Write> = if io::stdout().is_terminal() {
        Box::new(io::stdout())
    } else {
        Box::new(io::stderr())
    };

    // Disable auto-wrap, write OSC 52, then re-enable (\x1b[?7l ... \x1b[?7h)
    // Prevents newline insertion in legacy terminals (e.g., conhost.exe)
    write!(stream, "\x1b[?7l{}\x1b[?7h", osc52).context("Failed to write OSC 52 sequence")?;
    stream.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose;
    use base64::Engine as _;

    #[test]
    fn test_build_sequence_raw_empty() {
        assert_eq!(build_sequence_raw(""), "\x1b]52;c;\x07");
    }

    #[test]
    fn test_build_sequence_raw_content() {
        assert_eq!(build_sequence_raw("SGVsbG8="), "\x1b]52;c;SGVsbG8=\x07");
    }

    #[test]
    fn test_osc52_size_limit() {
        let small_text = "Hello World";
        let encoded = general_purpose::STANDARD.encode(small_text);
        assert!(encoded.len() <= OSC52_MAX_SIZE);

        // 8MB text exceeds 10MB when base64-encoded
        let large_text = "x".repeat(8 * 1024 * 1024);
        let encoded = general_purpose::STANDARD.encode(&large_text);
        assert!(encoded.len() > OSC52_MAX_SIZE);
    }
}
