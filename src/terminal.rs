//! Terminal raw mode handling for OSC 52 clipboard queries (Unix-only).

#[cfg(unix)]
use anyhow::{Context, Result};
#[cfg(unix)]
use is_terminal::IsTerminal;
#[cfg(unix)]
use rustix::termios::{self, LocalModes, OptionalActions, Termios};
#[cfg(unix)]
use crate::osc52;
#[cfg(unix)]
use std::os::fd::AsRawFd;

/// RAII guard that restores terminal mode on drop.
#[cfg(unix)]
pub struct TerminalGuard {
    original_termios: Termios,
    fd: std::os::fd::OwnedFd,
}

#[cfg(unix)]
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = termios::tcsetattr(&self.fd, OptionalActions::Flush, &self.original_termios);
    }
}

/// Set terminal to raw mode and return a guard that restores it on drop.
#[cfg(unix)]
pub fn set_raw_mode() -> Result<TerminalGuard> {
    let fd = std::io::stdin().as_raw_fd();
    let owned_fd = unsafe { rustix::fd::BorrowedFd::borrow_raw(fd) }.try_clone_to_owned()?;
    let original_termios = termios::tcgetattr(&owned_fd).context("Failed to get terminal attributes")?;

    let mut raw = original_termios.clone();
    raw.local_modes &= !(LocalModes::ECHO | LocalModes::ICANON | LocalModes::ISIG);
    termios::tcsetattr(&owned_fd, OptionalActions::Drain, &raw)
        .context("Failed to set terminal to raw mode")?;

    Ok(TerminalGuard { original_termios, fd: owned_fd })
}

/// Read from stdin with a timeout. Returns empty string if no data available.
#[cfg(unix)]
pub fn read_with_timeout(timeout_ms: u64) -> Result<String> {
    use rustix::event::{poll, PollFd, PollFlags};
    use std::io::Read;

    let stdin_fd = std::io::stdin().as_raw_fd();
    let borrowed = unsafe { rustix::fd::BorrowedFd::borrow_raw(stdin_fd) };
    let mut poll_fds = [PollFd::new(&borrowed, PollFlags::IN)];

    if poll(&mut poll_fds, timeout_ms as i32).context("Failed to poll stdin")? == 0 {
        return Ok(String::new());
    }

    let mut buffer = Vec::new();
    let mut handle = std::io::stdin().lock();

    loop {
        let mut chunk = [0u8; 4096];
        match handle.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                if buffer.len() + n > osc52::OSC52_MAX_SIZE {
                    anyhow::bail!("Response exceeds maximum size ({} bytes)", osc52::OSC52_MAX_SIZE);
                }
                buffer.extend_from_slice(&chunk[..n]);
                let response = String::from_utf8_lossy(&buffer);
                if response.contains('\x07') || response.contains("\x1b\\") {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) => return Err(e).context("Failed to read from stdin"),
        }
    }

    String::from_utf8(buffer).context("Response is not valid UTF-8")
}

/// Check if stdin is a terminal (TTY).
#[cfg(unix)]
pub fn is_stdin_tty() -> bool {
    std::io::stdin().is_terminal()
}

/// OSC 52 queries are not supported on Windows.
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
    fn test_is_stdin_tty() {
        let _ = super::is_stdin_tty();
    }

    #[test]
    #[cfg(unix)]
    fn test_read_with_timeout() {
        let _ = super::read_with_timeout(0);
    }
}
