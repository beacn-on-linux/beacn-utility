# BEACN Utility for Linux

Linux utility for configuring and using BEACN hardware.

This repository is maintained as the `ttmullins/beacn-utility` fork of the original `beacn-on-linux/beacn-utility` project. The goal of this fork is to keep the app practical, easy to build, and easier for Linux users to trust and contribute to.

## What this project is

BEACN Utility gives Linux users a desktop UI for BEACN device configuration, especially for DSP and device-management workflows that do not have first-party Linux support.

## Current status

### Working today
- BEACN Mic support
- BEACN Studio support
- Core DSP adjustment workflows
- Linux desktop UI and tray/background behavior
- Source builds with `cargo`

### In progress / limited
- Mix and Mix Create support is present but still evolving
- PipeWeaver-dependent mixer workflows need more validation across distros
- Packaging and release hygiene need to improve in this fork

### Not planned right now
- Profiles
- Audio visualizations

## Safety notice

**Use this software carefully.**

This application can modify on-device storage for supported BEACN hardware. The implementation was derived through reverse engineering and community testing rather than official vendor support.

That does **not** mean it is unsafe by default, but it does mean users should treat it like low-level hardware software:

- back up settings before major changes
- avoid interrupting writes
- test carefully after changing device values
- open an issue if anything looks wrong

This project is not affiliated with or endorsed by BEACN.

## Quick start

### Install
Right now, the most reliable way to use this fork is to build from source.

```bash
git clone https://github.com/ttmullins/beacn-utility.git
cd beacn-utility
cargo run --release
```

To build a reusable release binary instead:

```bash
cargo build --release
```

Your compiled binary will be at:

```bash
target/release/beacn-utility
```

## Linux requirements

### udev rules
If you are on `systemd` older than `257.7`:

1. Copy `50-beacn.rules` to `/etc/udev/rules.d/`
2. Reload rules:

```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```

### ALSA UCM profiles
If you are using a BEACN Mic or Studio and your `alsa-ucm-conf` is older than `1.2.15`, install the community ALSA UCM profiles:

- <https://github.com/beacn-on-linux/beacn-ucm-profiles>

After changing udev rules or ALSA UCM content, unplug and reconnect the device.

### PipeWeaver
Some mixer workflows depend on PipeWeaver:

- <https://github.com/pipeweaver/pipeweaver>

If PipeWeaver is not running, mixer-related pages may show a disconnected or limited state.

## Tested / expected environments

This fork should be treated as **community-supported** unless a distro is explicitly validated in a release note.

### Good candidates
- Fedora
- Bazzite
- Arch / EndeavourOS
- Ubuntu
- Debian-based desktops with current PipeWire stacks

### Expect extra setup
- older enterprise-style distros
- minimal desktop environments
- systems with outdated `systemd`, `alsa-ucm-conf`, or PipeWire components

## Known limitations

- Mix and Mix Create support is not yet as mature as Mic / Studio support
- release artifacts are not yet consistently published from this fork
- hardware/software combinations on Linux vary a lot, so distro-specific bugs are expected
- first-run troubleshooting guidance still needs improvement

## Troubleshooting

### The app opens but the device is missing
Check:
- USB connection
- udev rules
- whether the device appears in your system audio stack
- whether unplug/replug fixes enumeration

### Mixer page looks disconnected
Check:
- PipeWeaver is installed
- PipeWeaver is running
- the versions of the utility and PipeWeaver are compatible

### Something seems unsafe after a change
Stop using the app, capture logs, and open an issue with:
- distro and version
- desktop environment
- device model
- steps taken
- screenshots and logs if possible

## Project roadmap

See [ROADMAP.md](ROADMAP.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Changelog

See [CHANGELOG.md](CHANGELOG.md).

## Credits

- Original upstream project: `beacn-on-linux/beacn-utility`
- Community BEACN on Linux contributors
- PipeWeaver contributors
