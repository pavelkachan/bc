# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Boring Clipboard (bc) is a minimal Rust CLI tool that copies piped text to the system clipboard. Its key differentiator is automatic detection of SSH sessions and support for remote clipboard operations via OSC 52 escape sequences.

**Architecture**: Single-binary application with ~400 lines of code in `src/main.rs`. The tool uses a graceful fallback mechanism: attempts local clipboard operations via `arboard`, and if unavailable or in a remote session, falls back to OSC 52 escape sequences.

## Development Commands

### Building
```bash
cargo build                    # Development build
cargo build --release          # Optimized release build
cargo install --path .         # Install locally
```

### Testing & Quality
```bash
cargo test                     # Run tests
cargo test -- --nocapture      # Run tests with stdout
cargo clippy                   # Lint checks
cargo fmt                      # Format code
```

### Release Process
Releases are automated via GitHub Actions. Tag a commit with `git tag v0.1.x` and push to trigger the workflow. The CI generates:
- Windows `.exe`
- macOS `.pkg` installer
- Linux `.deb` and `.rpm` packages

### Platform-Specific Dependencies
**Linux (X11)**: `xorg-dev libxcb-shape0-dev libxcb-xfixes0-dev`
**Linux (Wayland)**: Requires Wayland compositor with `wl-clipboard` protocols

## Core Implementation Details

### Exit Codes
The application uses explicit exit codes for different scenarios:

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (I/O, clipboard write failure) |
| 2 | Empty input (when writing) |
| 3 | Clipboard unavailable or empty (when reading) |
| 4 | Invalid input (binary data detected) |

Exit codes are defined in the `BcExitCode` enum in `src/main.rs:10-18`.

### CLI Flags
All CLI arguments are managed via `clap` derive macros in `src/main.rs`:

| Flag | Description |
|------|-------------|
| `-t, --trim` | Trim trailing newline from input |
| `-l, --local` | Force local clipboard (disable remote detection) |
| `-p, --paste` | Read from clipboard and print to stdout |
| `-c, --clear` | Clear the clipboard |
| `-f, --force` | Force copy even if binary data detected |
| `-P, --preview` | Show preview of copied content |

### Input Validation
The `contains_binary_data()` function detects potentially problematic content:
- Null bytes (`\0`)
- Control characters (except `\n`, `\r`, `\t`, `\x0c`)
- If detected, exits with code 4 unless `--force` flag is used

### SSH Session Detection
The `is_remote_session()` function detects remote environments by checking environment variables:
- `SSH_CLIENT`, `SSH_CONNECTION`, `SSH_TTY`
- `AWS_SSM_SESSION_ID`, `SSM_SESSION_ID` (AWS Systems Manager)

### OSC 52 Implementation
Remote clipboard copy uses ANSI escape sequences:
```
\x1b]52;c;<base64_encoded_content>\x07
```

Key implementation notes:
- Content is base64-encoded before embedding in the sequence
- **Size limit**: Content larger than 10MB (when base64-encoded) will fail with an error message suggesting alternatives
- For legacy terminals (Windows conhost.exe), auto-wrap is temporarily disabled to prevent sequence corruption
- Output goes to stdout; if that fails, attempts stderr as fallback
- The clipboard specifier is `c` for clipboard

### Clipboard Operations
The tool supports three modes of operation:

1. **Write (default)**: Copies piped input to clipboard
2. **Read (`--paste`)**: Prints clipboard contents to stdout
3. **Clear (`--clear`)**: Clears the clipboard

Each mode uses the same local/remote detection logic.

### Clipboard Fallback Logic
1. If `--paste` or `--clear` is used, only local clipboard is available
2. If `--local` flag is set, force local clipboard via `arboard`
3. Otherwise, detect if in remote session
4. Remote: use OSC 52 escape sequences (writes to terminal) with 10MB size limit
5. Local: use `arboard` library (platform-specific clipboard APIs)
6. Silent fallback on failure (no error messages to stdout)

### arboard Configuration
The `arboard` dependency is configured with `default-features = false` to minimize binary size and avoid unnecessary platform-specific code.

## Common Work

### Adding New CLI Flags
CLI arguments are managed via `clap` derive macros in `src/main.rs`. Add new options to the `Args` struct and handle them in the `main()` function.

### Supporting New Terminals
Terminal OSC 52 support varies. For issues:
1. Test manually: `printf "\033]52;c;$(printf "Hello" | base64)\a"`
2. Check if terminal/multiplexer (tmux/screen) needs passthrough configuration
3. Legacy terminals may need the auto-wrap disable/enable pattern used for conhost.exe

### Testing Clipboard Operations
Since clipboard operations are side-effects, manual testing is typical:
```bash
echo "test" | ./target/release/bc           # Write (local)
./target/release/bc -p                      # Read (paste)
./target/release/bc -c                      # Clear
echo "test" | ./target/release/bc -P        # Write with preview
echo -e "\x00binary" | ./target/release/bc  # Test binary detection
echo "test" | ssh host "bc"                 # Remote
```

### Modifying Exit Codes
Exit codes are defined in the `BcExitCode` enum. When adding new codes:
1. Update the enum in `src/main.rs:10-18`
2. Ensure values fit in `u8` (0-255)
3. Update documentation in this file and README.md

## CI/CD Pipeline

Three GitHub Actions workflows:
1. **CI** (`ci.yml`): Tests on Ubuntu, Windows, macOS, plus Rocky Linux 8 container
2. **Release** (`release.yml`): Automated builds and GitHub releases on version tags
3. Build dependencies (managed via Cargo): `cargo-deb`, `cargo-generate-rpm` (pinned to v0.16.0)
