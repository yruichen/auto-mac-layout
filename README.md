<!-- LANGUAGE SELECTION -->
<div align="center">
  <strong>
    <a href="README.md">English</a> | <a href="docs/README_zh.md">中文</a>
  </strong>
</div>

<!-- PROJECT LOGO -->
<br />
<div align="center">
    <img src="assets/logo.png" alt="Logo" width="80" height="80">

  <h3 align="center">AUTO-MAC-LAYOUT</h3>

  <p align="center">
    Automatically save and restore Finder desktop icon positions based on your display setup
  </p>
</div>



<!-- TABLE OF CONTENTS -->
<details>
  <summary>Table of Contents</summary>
  <ol>
    <li>
      <a href="#about-the-project">About The Project</a>
      <ul>
        <li><a href="#features">Features</a></li>
      </ul>
    </li>
    <li>
      <a href="#getting-started">Getting Started</a>
      <ul>
        <li><a href="#requirements">Requirements</a></li>
        <li><a href="#installation">Installation</a></li>
        <li><a href="#permissions">Permissions</a></li>
      </ul>
    </li>
    <li><a href="#usage">Usage</a></li>
    <li><a href="#development">Development</a></li>
    <li><a href="#troubleshooting">Troubleshooting</a></li>
    <li><a href="#contributing">Contributing</a></li>
  </ol>
</details>



<!-- ABOUT THE PROJECT -->
## About The Project

Auto Mac Layout is a lightweight macOS tray app that automatically saves and restores Finder desktop icon positions based on your current display setup.

When monitors are plugged in, removed, mirrored, or rearranged, Finder can scramble icon positions. This tool fingerprints the active display topology and applies the matching saved layout.

### Features

- Tray app with no Dock icon
- Per-display-layout profile storage
- Debounced auto-apply when display configuration changes
- Manual "Save Current Layout" from tray menu
- Auto-save when desktop file set changes
- Optional "Launch at Login"
- Atomic layout file writes to reduce corruption risk

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- GETTING STARTED -->
## Getting Started

### Requirements

- macOS (AppleScript/Finder automation required)
- Rust toolchain (only for building from source)

### Installation

#### Option A: Build from source

```bash
cargo build --release
```

Binary output:

```text
target/release/auto-mac-layout
```

Run:

```bash
./target/release/auto-mac-layout
```

#### Option B: Add to Launch at Login

Enable "Launch at Login" from tray menu after first run.

### Permissions

The app controls Finder desktop item positions through AppleScript. On first use, macOS may ask for permissions.

Please allow:

- Automation access for controlling Finder
- Accessibility permission if your macOS setup requires it

If layout fetch/apply fails, check:

- `System Settings -> Privacy & Security -> Automation`
- `System Settings -> Privacy & Security -> Accessibility`

<p align="right">(<a href="#readme-top">back to top</a>)</p>



<!-- USAGE EXAMPLES -->
## Usage

### Tray Menu

- `Save Current Layout`: Snapshot current desktop icon positions for current display fingerprint
- `Open Config File`: Open layout storage JSON file
- `Switch Apply Delay`: Tune debounce delay (`0ms`, `300ms`, `1000ms`, `2000ms`, `3000ms`)
- `Launch at Login`: Enable/disable startup launch
- `Quit`: Exit app

### Storage Location

Layouts are stored in:

```text
~/Library/Application Support/auto-mac-layout/layouts.json
```

The file maps each display fingerprint to a list of desktop icon coordinates.

<p align="right">(<a href="#readme-top">back to top</a>)</p>



<!-- DEVELOPMENT -->
## Development

Format check:

```bash
cargo fmt --check
```

Compile check:

```bash
cargo check
```

Build release:

```bash
cargo build --release
```

<p align="right">(<a href="#readme-top">back to top</a>)</p>



<!-- TROUBLESHOOTING -->
## Troubleshooting

**No icon movement after display change:**
- Confirm Finder desktop icons are visible
- Re-run `Save Current Layout` once on that display setup
- Verify Automation/Accessibility permissions

**Config file seems empty:**
- Make sure at least one desktop item exists when saving

**App starts but tray icon is missing:**
- Check if image load failed and review terminal logs from startup

<p align="right">(<a href="#readme-top">back to top</a>)</p>



<!-- CONTRIBUTING -->
## Contributing

Issues and pull requests are welcome.

Suggested workflow:

1. Fork the repository and create a feature branch
2. Run `cargo fmt --check` and `cargo check`
3. Open a pull request with behavior description and reproduction steps

<p align="right">(<a href="#readme-top">back to top</a>)</p>




