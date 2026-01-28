use anyhow::{Context, Result};
use is_terminal::IsTerminal;
use std::io::{self, Write};

/// OSC 52 escape sequence prefix: \x1b]52;c;
const OSC52_PREFIX: &str = "\x1b]52;c;";
/// OSC 52 escape sequence terminator: \x07
const OSC52_TERMINATOR: char = '\x07';
/// String terminator (ST) alternative to BEL
const OSC52_ST: &str = "\x1b\\";
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

/// Build OSC 52 query sequence to request clipboard contents from terminal.
/// Format: \x1b]52;c;?\x07
pub fn build_query_sequence() -> String {
    format!("{}?{}", OSC52_PREFIX, OSC52_TERMINATOR)
}

/// Parse OSC 52 response to extract base64-encoded clipboard content.
/// Response format: \x1b]52;c;<base64_data>\x07
///
/// Handles both BEL (\x07) and ST (\x1b\\) terminators.
/// Finds the LAST occurrence of the prefix to handle junk before the response.
///
/// Returns the base64-encoded string (empty string if clipboard is empty).
pub fn parse_response(input: &str) -> Result<String> {
    // Find the LAST occurrence of the prefix (in case of junk before response)
    let start_idx = input.rfind(OSC52_PREFIX).ok_or_else(|| {
        anyhow::anyhow!("Invalid OSC 52 response: missing prefix '\\x1b]52;c;'")
    })?;

    // Find the end: either BEL or ST terminator
    let response_part = &input[start_idx + OSC52_PREFIX.len()..];
    let end_idx = response_part
        .find(OSC52_TERMINATOR)
        .or_else(|| response_part.find(OSC52_ST))
        .ok_or_else(|| {
            anyhow::anyhow!("Invalid OSC 52 response: missing terminator (BEL or ST)")
        })?;

    let base64_data = &response_part[..end_idx];

    // Empty data means empty clipboard (not an error)
    Ok(base64_data.to_string())
}

/// Query clipboard via OSC 52 and return base64-encoded content.
///
/// Returns an empty string if:
/// - Terminal doesn't respond within timeout
/// - Clipboard is empty
///
/// Returns an error if:
/// - Terminal operations fail
/// - Response is malformed
/// - Response exceeds size limit
#[allow(clippy::let_unit_value)]
pub fn query_clipboard(timeout_ms: u64) -> Result<String> {
    use crate::terminal;

    if !terminal::is_stdin_tty() {
        anyhow::bail!("OSC 52 query requires a terminal (stdin is not a TTY)");
    }

    let _guard = terminal::set_raw_mode().context("Failed to set terminal to raw mode")?;
    write_sequence(&build_query_sequence()).context("Failed to write OSC 52 query sequence")?;

    let response = terminal::read_with_timeout(timeout_ms).context("Failed to read OSC 52 response")?;

    if response.is_empty() {
        anyhow::bail!("Terminal doesn't support OSC 52 query (no response)");
    }

    parse_response(&response)
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

    #[test]
    fn test_build_query_sequence() {
        assert_eq!(build_query_sequence(), "\x1b]52;c;?\x07");
    }

    #[test]
    fn test_parse_valid_response_with_bel() {
        let response = "\x1b]52;c;SGVsbG8=\x07";
        let parsed = parse_response(response).unwrap();
        assert_eq!(parsed, "SGVsbG8=");
    }

    #[test]
    fn test_parse_valid_response_with_st() {
        let response = "\x1b]52;c;SGVsbG8=\x1b\\";
        let parsed = parse_response(response).unwrap();
        assert_eq!(parsed, "SGVsbG8=");
    }

    #[test]
    fn test_parse_empty_response() {
        let response = "\x1b]52;c;\x07";
        let parsed = parse_response(response).unwrap();
        assert_eq!(parsed, "");
    }

    #[test]
    fn test_parse_response_with_junk_before() {
        let response = "some junk\x1b]52;c;SGVsbG8=\x07";
        let parsed = parse_response(response).unwrap();
        assert_eq!(parsed, "SGVsbG8=");
    }

    #[test]
    fn test_parse_response_finds_last_prefix() {
        let response = "\x1b]52;c;old\x07junk\x1b]52;c;SGVsbG8=\x07";
        let parsed = parse_response(response).unwrap();
        assert_eq!(parsed, "SGVsbG8=");
    }

    #[test]
    fn test_parse_malformed_response_missing_prefix() {
        let response = "SGVsbG8=\x07";
        assert!(parse_response(response).is_err());
    }

    #[test]
    fn test_parse_malformed_response_missing_terminator() {
        let response = "\x1b]52;c;SGVsbG8=";
        assert!(parse_response(response).is_err());
    }
}
