## Auto Mac Layout

Auto Mac Layout is a lightweight macOS tray app that automatically saves and restores Finder desktop icon positions based on your current display setup.

When monitors are plugged in, removed, mirrored, or rearranged, Finder can scramble icon positions. This tool fingerprints the active display topology and applies the matching saved layout.

## Features

- Tray app with no Dock icon.
- Per-display-layout profile storage.
- Debounced auto-apply when display configuration changes.
- Manual "Save Current Layout" from tray menu.
- Auto-save when desktop file set changes.
- Optional "Launch at Login".
- Atomic layout file writes to reduce corruption risk.

## Requirements

- macOS (AppleScript/Finder automation required)
- Rust toolchain (only for building from source)

## Install

### Option A: Build from source

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

### Option B: Add to Launch at Login

Enable "Launch at Login" from tray menu after first run.

## Permissions (Important)

The app controls Finder desktop item positions through AppleScript. On first use, macOS may ask for permissions.

Please allow:

- Automation access for controlling Finder.
- Accessibility permission if your macOS setup requires it.

If layout fetch/apply fails, check:

- `System Settings -> Privacy & Security -> Automation`
- `System Settings -> Privacy & Security -> Accessibility`

## Tray Menu

- `Save Current Layout`: Snapshot current desktop icon positions for current display fingerprint.
- `Open Config File`: Open layout storage JSON file.
- `Switch Apply Delay`: Tune debounce delay (`0ms`, `300ms`, `1000ms`, `2000ms`, `3000ms`).
- `Launch at Login`: Enable/disable startup launch.
- `Quit`: Exit app.

## Storage Location

Layouts are stored in:

```text
~/Library/Application Support/auto-mac-layout/layouts.json
```

The file maps each display fingerprint to a list of desktop icon coordinates.

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

## Troubleshooting

- No icon movement after display change:
	- Confirm Finder desktop icons are visible.
	- Re-run `Save Current Layout` once on that display setup.
	- Verify Automation/Accessibility permissions.

- Config file seems empty:
	- Make sure at least one desktop item exists when saving.

- App starts but tray icon is missing:
	- Check if image load failed and review terminal logs from startup.

## Security and Privacy

- The app only stores desktop icon names and coordinates locally.
- No network communication is implemented.

## Open Source Readiness Checklist

- [x] Core logic and tray behavior documented.
- [x] Build and usage instructions documented.
- [x] Debug-only runtime logs removed from source.
- [ ] Add a license file (recommended before publishing).
- [ ] Add CI workflow (recommended for pull requests).

## Contributing

Issues and pull requests are welcome.

Suggested workflow:

1. Fork repository and create a feature branch.
2. Run `cargo fmt --check` and `cargo check`.
3. Open a pull request with behavior description and reproduction steps.
