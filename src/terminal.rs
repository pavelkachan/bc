//! Terminal raw mode handling for OSC 52 clipboard queries.
//! Unix-only - Windows does not support this feature.

#[cfg(unix)]
use anyhow::{Context, Result};
#[cfg(unix)]
use rustix::termios::{
    self, LocalModes, OptionalActions, SetArg, TerminalMode, Termios,
};
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::time::Duration;

/// RAII guard that restores terminal mode on drop.
/// Ensures terminal is restored even if panic occurs during raw mode operations.
#[cfg(unix)]
pub struct TerminalGuard {
    original_termios: Termios,
    fd: std::os::fd::OwnedFd,
}

#[cfg(unix)]
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        // Restore original terminal mode
        let _ = termios::tcsetattr(&self.fd, OptionalActions::Flush, &self.original_termios);
    }
}

/// Set terminal to raw mode and return a guard that restores it on drop.
/// The guard restores the original mode when dropped, even if a panic occurs.
#[cfg(unix)]
pub fn set_raw_mode() -> Result<TerminalGuard> {
    let fd = std::io::stdin().as_raw_fd();
    let owned_fd = rustix::fd::BorrowedFd::borrow_raw(fd).try_clone_to_owned()?;

    let original_termios =
        termios::tcgetattr(&owned_fd).context("Failed to get terminal attributes")?;

    let mut raw = original_termios.clone();
    raw.local_modes &= !(LocalModes::ECHO | LocalModes::ICANON | LocalModes::ISIG);

    termios::tcsetattr(&owned_fd, OptionalActions::Drain, &raw)
        .context("Failed to set terminal to raw mode")?;

    Ok(TerminalGuard {
        original_termios,
        fd: owned_fd,
    })
}

/// Read from stdin with a timeout.
/// Returns an empty string if no data is available within the timeout.
#[cfg(unix)]
pub fn read_with_timeout(timeout_ms: u64) -> Result<String> {
    use rustix::poll::{poll, PollFd, PollFlags};
    use std::io::Read;

    let stdin_fd = std::io::stdin().as_raw_fd();
    let borrowed = rustix::fd::BorrowedFd::borrow_raw(stdin_fd);
    let mut poll_fd = PollFd::new(&borrowed, PollFlags::IN);

    let timeout = Duration::from_millis(timeout_ms);
    let nready = poll(&mut poll_fd, timeout).context("Failed to poll stdin")?;

    if nready == 0 {
        // Timeout - no data available
        return Ok(String::new());
    }

    // Data available - read it
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let stdin = std::io::stdin();
    let mut handle = stdin.lock();

    loop {
        match handle.read(&mut chunk) {
            Ok(0) => break, // EOF
            Ok(n) => {
                // Enforce 10MB limit (matches OSC 52 write limit)
                if buffer.len() + n > osc52::OSC52_MAX_SIZE {
                    anyhow::bail!("Response exceeds maximum size ({} bytes)", osc52::OSC52_MAX_SIZE);
                }
                buffer.extend_from_slice(&chunk[..n]);

                // Check if we have a complete OSC 52 response
                let response = String::from_utf8_lossy(&buffer);
                if response.contains('\x07') || response.contains("\x1b\\") {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No more data available
                break;
            }
            Err(e) => return Err(e).context("Failed to read from stdin"),
        }
    }

    String::from_utf8(buffer).context("Response is not valid UTF-8")
}

/// Check if stdin is a terminal (TTY).
#[cfg(unix)]
pub fn is_stdin_tty() -> bool {
    rustix::termios::is_terminal(rustix::fd::BorrowedFd::borrow_raw(std::io::stdin().as_raw_fd()))
}

/// Windows does not support OSC 52 queries.
#[cfg(not(unix))]
pub fn set_raw_mode() -> anyhow::Result<()> {
    Err(anyhow::anyhow!("OSC 52 query is not supported on Windows"))
}

#[cfg(not(unix))]
pub fn read_with_timeout(_timeout_ms: u64) -> anyhow::Result<String> {
    Err(anyhow::anyhow!("OSC 52 query is not supported on Windows"))
}

#[cfg(not(unix))]
pub fn is_stdin_tty() -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(unix)]
    fn test_is_stdin_tty_returns_bool() {
        // Just verify it returns a boolean without panicking
        let _ = super::is_stdin_tty();
    }

    #[test]
    #[cfg(unix)]
    fn test_read_with_zero_timeout() {
        // Zero timeout should return immediately (may return empty string)
        let result = super::read_with_timeout(0);
        // We don't assert the result since we don't know if there's data available
        let _ = result;
    }
}
