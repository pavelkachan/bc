# Boring Clipboard (bc)

<div align="center">
  <pre>
  ██████╗  ██████╗ 
  ██╔══██╗██╔════╝ 
  ██████╔╝██║      
  ██╔══██╗██║      
  ██████╔╝╚██████╗ 
  ╚═════╝  ╚═════╝ 
  </pre>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-1.70%2B-orange.svg" alt="Rust"></a>
  <a href="https://github.com/pavelkachan/bc"><img src="https://img.shields.io/badge/platform-Windows%20%7C%20MacOS%20%7C%20Linux-blue" alt="Platform"></a>
</div>

A simple, cross-platform command-line tool to copy piped text to the system clipboard. It supports Windows, MacOS, Linux, and **remote SSH sessions** via OSC 52.

<!-- TABLE OF CONTENTS -->
<details>
  <summary>Table of Contents</summary>
  <ol>
    <li>
      <a href="#about-the-project">About The Project</a>
      <ul>
        <li><a href="#built-with">Built With</a></li>
      </ul>
    </li>
    <li>
      <a href="#getting-started">Getting Started</a>
      <ul>
        <li><a href="#installation">Installation</a></li>
      </ul>
    </li>
    <li><a href="#usage">Usage</a></li>
    <li><a href="#remote-usage-ssh">Remote Usage</a></li>
    <li><a href="#troubleshooting">Troubleshooting</a></li>
    <li><a href="#contributing">Contributing</a></li>
    <li><a href="#license">License</a></li>
  </ol>
</details>

## About The Project

`bc` (Boring Clipboard) is designed to do one thing and do it well: take standard input and put it on your clipboard.

Why another clipboard tool?
*   **Cross-Platform**: Works consistently on Windows, Linux (X11 & Wayland), and MacOS.
*   **Remote Aware**: Automatically detects if you are in an SSH session and uses OSC 52 escape sequences to copy to your *local* machine's clipboard. No more forwarding X11 or manually selecting text with your mouse.
*   **Zero Config**: No flags, no configuration files. Just pipe and go.

### Built With

*   [Rust](https://www.rust-lang.org/)
*   [arboard](https://crates.io/crates/arboard) (Local clipboard)
*   [OSC 52](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h3-Operating-System-Commands) (Remote clipboard)

## Getting Started

### Installation

#### Option 1: Download Binary (Recommended)

Download the latest binary for your operating system from the [Releases](https://github.com/pavelkachan/bc/releases) page.

**Linux**
*   **Debian/Ubuntu**: Download `bc.deb` and run `sudo dpkg -i bc.deb`
*   **Fedora/RedHat**: Download `bc.rpm` and run `sudo rpm -i bc.rpm`
*   **Manual**:
    ```bash
    curl -L https://github.com/pavelkachan/bc/releases/latest/download/bc -o bc
    chmod +x bc
    sudo mv bc /usr/local/bin/
    ```

**MacOS**
*   **Installer (Recommended)**: Download `bc.pkg` from [Releases](https://github.com/pavelkachan/bc/releases) and run it.
*   **Manual**:
    ```bash
    curl -L https://github.com/pavelkachan/bc/releases/latest/download/bc-macos -o bc
    chmod +x bc
    mkdir -p ~/.local/bin
    mv bc ~/.local/bin/
    # Ensure ~/.local/bin is in your PATH
    ```
    *Note: If macOS blocks the binary, run `xattr -d com.apple.quarantine ~/.local/bin/bc` to allow execution.*

**Windows (PowerShell)**
```powershell
curl -L https://github.com/pavelkachan/bc/releases/latest/download/bc.exe -o bc.exe
# Move bc.exe to a folder in your PATH
```

#### Option 2: Build from Source

If you have Rust installed, you can build `bc` from source:

```bash
git clone https://github.com/pavelkachan/bc.git
cd bc
cargo install --path .
```

## Usage

Pipe any text into `bc` to copy it to your clipboard.

```bash
# Copy a string
echo "Hello World" | bc

# Copy without trailing newline
echo "Hello World" | bc -t

# Force local copy (disable remote detection)
echo "Hello World" | bc -l

# Copy with preview confirmation
echo "Hello World" | bc -P

# Copy a file content
cat secret_key.pem | bc

# Copy command output
ls -la | bc

# Read from clipboard (paste)
bc -p

# Read from clipboard (force local)
bc -p --local

# Clear clipboard
bc -c

# Experimental: Attempt remote paste via OSC 52 query
bc -p --force-paste
```

## Remote Usage (SSH)

When running `bc` inside an SSH session, it detects the remote environment and attempts to copy to your *local* clipboard using OSC 52.

**Supported Operations in SSH:**
- ✅ **Copy**: `echo "text" | bc` - Works automatically
- ✅ **Clear**: `bc -c` - Clears your local clipboard via OSC 52
- ❌ **Paste**: `bc -p` - Not supported (see alternatives below)

**Requirements for Remote Copy:**
1.  **Terminal Support**: Your local terminal emulator must support OSC 52.
    *   *Supported*: Windows Terminal, iTerm2, Alacritty, Kitty, WezTerm, Rio.
    *   *Unsupported*: Standard Gnome Terminal (often requires plugins), older terminals.
2.  **Multiplexers**: If using `tmux` or `screen` on the remote server, you may need to configure them to pass through escape sequences.

### Remote Paste Limitations

Reading from clipboard (`bc -p`) doesn't work over SSH because most terminals don't support OSC 52 clipboard querying for security reasons. When you attempt this, `bc` will provide helpful alternatives:

```bash
# In SSH session, this will show alternatives
bc -p

# Alternatives:
# 1. Use X11 forwarding: ssh -X host
# 2. Copy file to remote: scp file.txt host:/tmp/ && cat /tmp/file.txt
# 3. Force local clipboard: bc -p --local (if display available)
# 4. Try experimental OSC 52 query: bc -p --force-paste (limited terminal support)
```

### Experimental OSC 52 Query

The `--force-paste` flag attempts to read clipboard via OSC 52 query. This is **experimental** and only works with specific terminals:

**Supported Terminals:**
- **XTerm**: Set `XTerm*allowWindowOps: true` in `~/.Xresources`
- **kitty**: Enable `clipboard_control read` in `kitty.conf`
- **tmux**: Version 3.0+ with `set-clipboard enabled`

**Unsupported Terminals:**
- WezTerm, iTerm2, Alacritty, and most others (security feature)

```bash
# Experimental remote paste (may not work)
bc -p --force-paste
```

## Advanced Features

### Input Validation

`bc` detects binary data and control characters in input. If detected, it will warn and exit with code 4. Use `--force` to bypass:

```bash
# Force copy even with binary data
cat binary_file | bc --force
```

### Exit Codes

`bc` uses specific exit codes for scripting:

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Empty input |
| 3 | Clipboard unavailable |
| 4 | Invalid input (binary data) |

Example usage in scripts:

```bash
if echo "data" | bc; then
    echo "Copied successfully!"
else
    exit_code=$?
    echo "Copy failed with code: $exit_code"
fi
```

### Large File Support

`bc` supports content up to 10MB (when base64-encoded) when using OSC 52. Content exceeding this limit will fail with an error message. For larger files, use `--local` flag or alternative transfer methods (scp, rsync, etc.).

### Clipboard Preview

The `--preview` flag shows what was copied:

```bash
echo "Very long text..." | bc -P
# Output: Copied: "Very long text..." (12345 bytes, 12345 chars)
```

## Troubleshooting

*   **Linux (X11)**: Ensure `xorg-dev` or `libxcb` dependencies are installed.
    *   Ubuntu/Debian: `sudo apt-get install xorg-dev libxcb-shape0-dev libxcb-xfixes0-dev`
*   **Linux (Wayland)**: `bc` uses `wl-clipboard` protocols. Ensure you have a Wayland compositor running.
*   **Remote Copy Not Working**: Check if your terminal supports OSC 52. Try running `printf "\033]52;c;$(printf "Hello" | base64)\a"` manually to test.

## Contributing

Contributions are what make the open source community such an amazing place to learn, inspire, and create. Any contributions you make are **greatly appreciated**.

1.  Fork the Project
2.  Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3.  Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4.  Push to the Branch (`git push origin feature/AmazingFeature`)
5.  Open a Pull Request

## License

Distributed under the MIT License. See [LICENSE](LICENSE.md) for more information.
