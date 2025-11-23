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
    <li><a href="#contact">Contact</a></li>
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

Go to the [Releases](https://github.com/pavelkachan/bc/releases) page and download the binary for your system.

```bash
# Example for Linux
wget https://github.com/pavelkachan/bc/releases/download/v0.1.0/bc
chmod +x bc
sudo mv bc /usr/local/bin/
```

#### Option 2: Install via Cargo

If you have Rust installed:

```bash
cargo install --path .
```

## Usage

Pipe any text into `bc` to copy it to your clipboard.

```bash
# Copy a string
echo "Hello World" | bc

# Copy without trailing newline
echo "Hello World" | bc --trim
# OR
echo "Hello World" | bc -t

# Force local copy (disable remote detection)
echo "Hello World" | bc --local
# OR
echo "Hello World" | bc -l

# Copy a file content
cat secret_key.pem | bc

# Copy command output
ls -la | bc
```

## Remote Usage (SSH)

When running `bc` inside an SSH session, it detects the remote environment and attempts to copy to your *local* clipboard using OSC 52.

**Requirements for Remote Copy:**
1.  **Terminal Support**: Your local terminal emulator must support OSC 52.
    *   *Supported*: Windows Terminal, iTerm2, Alacritty, Kitty, WezTerm, Rio.
    *   *Unsupported*: Standard Gnome Terminal (often requires plugins), older terminals.
2.  **Multiplexers**: If using `tmux` or `screen` on the remote server, you may need to configure them to pass through escape sequences.

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
