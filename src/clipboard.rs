use anyhow::{Context, Result};
use arboard::Clipboard;
use base64::Engine as _;
use is_terminal::IsTerminal;
use std::{env, io};

use crate::osc52;
use crate::Args;

/// Environment variables that indicate a remote session
const REMOTE_SESSION_VARS: &[&str] = &[
    "SSH_CLIENT",
    "SSH_TTY",
    "SSH_CONNECTION",
    "AWS_SSM_SESSION_ID",
    "SSM_SESSION_ID",
];

/// Error messages for remote paste operations
const REMOTE_PASTE_ERROR: &str = "\
Clipboard reading is not supported in remote sessions (SSH detected).

OSC 52 clipboard querying has limited terminal support and is disabled
by default in most terminals for security reasons.

Alternatives:
  - Use X11 forwarding: ssh -X host
  - Copy file to remote: scp file.txt host:/tmp/ && cat /tmp/file.txt
  - Force local clipboard with --local flag (if display available)
  - Try experimental OSC 52 query: bc -p --force-paste";

const REMOTE_PASTE_UNSUPPORTED: &str = "\
OSC 52 query requires:
  - A terminal (stdin must be a TTY, not piped input)
  - Terminal that supports clipboard reading (XTerm, kitty, tmux)
  - Proper terminal configuration

Most terminals (WezTerm, iTerm2, Alacritty, Ghostty) do NOT support
clipboard reading for security reasons.

Currently supported terminals:
  - XTerm (set 'XTerm*allowWindowOps: true' in ~/.Xresources)
  - kitty (enable 'clipboard_control read' in kitty.conf)
  - tmux 3.0+ (set 'set -s set-clipboard on' in tmux.conf)

Alternatives:
  - X11 forwarding: ssh -X host
  - File transfer: scp file.txt host:/tmp/ && cat /tmp/file.txt
  - Force local clipboard: bc -p --local";

/// Detect if running in a remote session (SSH, AWS SSM, etc.)
pub fn is_remote_session() -> bool {
    REMOTE_SESSION_VARS.iter().any(|var| env::var(var).is_ok())
}

/// Copy text to local clipboard via arboard
pub fn copy_local(text: &str) -> Result<()> {
    Clipboard::new()
        .context("Failed to initialize clipboard")?
        .set_text(text)
        .context("Failed to write to local clipboard")
}

/// Copy text to remote clipboard via OSC 52
pub fn copy_remote(text: &str) -> Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);

    if encoded.len() > osc52::OSC52_MAX_SIZE {
        anyhow::bail!(
            "Content too large for OSC 52 clipboard ({} bytes, max {} bytes). \
             Use --local flag or alternative transfer method.",
            encoded.len(),
            osc52::OSC52_MAX_SIZE
        );
    }

    osc52::write_sequence(&osc52::build_sequence_raw(&encoded))
}

/// Clear local clipboard
pub fn clear_local() -> Result<()> {
    Clipboard::new()
        .context("Failed to initialize clipboard")?
        .set_text("")
        .context("Failed to clear local clipboard")
}

/// Clear remote clipboard via OSC 52 (empty write)
pub fn clear_remote() -> Result<()> {
    osc52::write_sequence(&osc52::build_sequence_raw(""))
}

/// Clear clipboard with automatic fallback logic
/// Returns Ok(true) if OSC 52 was used, Ok(false) if local only
pub fn clear_clipboard(prefer_remote: bool, force_local: bool) -> Result<bool> {
    let remote_result = clear_remote().map(|_| true);

    if prefer_remote {
        if remote_result.is_ok() || force_local {
            return remote_result;
        }
        // Fallback to local if remote failed
        return clear_local().map(|_| false);
    }

    // Prefer local: try local first, fallback to remote
    clear_local()
        .map(|_| false)
        .or_else(|e| {
            if force_local {
                Err(e)
            } else {
                remote_result
            }
        })
}

/// Paste from clipboard (supports local and experimental OSC 52 query)
pub fn paste_clipboard(args: &Args) -> Result<String> {
    if !args.local && is_remote_session() {
        return handle_remote_paste(args);
    }

    Clipboard::new()
        .context("Failed to initialize clipboard")?
        .get_text()
        .context("Failed to read from clipboard")
}

/// Handle paste in remote sessions
fn handle_remote_paste(args: &Args) -> Result<String> {
    if !args.force_paste {
        return Err(anyhow::anyhow!(REMOTE_PASTE_ERROR));
    }

    eprintln!("Warning: --force-paste is experimental");
    eprintln!("OSC 52 clipboard querying requires terminal support (XTerm, kitty, tmux)");
    eprintln!("Most terminals (WezTerm, iTerm2, etc.) do not support clipboard reading");

    if !io::stdin().is_terminal() {
        return Err(anyhow::anyhow!(
            "OSC 52 query requires a terminal (stdin is not a TTY).\n\n{}",
            REMOTE_PASTE_UNSUPPORTED
        ));
    }

    if env::var("TMUX").is_ok() || env::var("STY").is_ok() {
        eprintln!("WARNING: Detected terminal multiplexer (tmux/screen).");
        eprintln!("OSC 52 query requires: set-clipboard on (tmux) or passthrough config.");
    }

    osc52::query_clipboard(2000)
        .and_then(|encoded| {
            if encoded.is_empty() {
                return Ok(String::new());
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&encoded)
                .context("Failed to decode base64 clipboard content")?;
            String::from_utf8(bytes).context("Clipboard content is not valid UTF-8")
        })
        .map_err(|e| anyhow::anyhow!("OSC 52 query failed: {}\n\n{}", e, REMOTE_PASTE_UNSUPPORTED))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_osc52_clear_sequence() {
        assert_eq!(osc52::build_sequence_raw(""), "\x1b]52;c;\x07");
    }
}
