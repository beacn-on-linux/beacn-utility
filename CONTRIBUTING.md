# Contributing

Thanks for helping improve BEACN Utility for Linux.

## What helps most right now

The highest-value contributions are:

- distro validation and reproducible bug reports
- safer device-write workflows
- diagnostics and logging improvements
- packaging and release automation
- BEACN Mix / Mix Create testing and feature work
- UI polish that improves clarity without adding complexity

## Ground rules

- Keep changes practical
- Prefer reliability over cleverness
- Do not assume hardware behavior without testing
- Avoid risky writes unless you understand the device path involved
- Document user-visible behavior changes in the changelog

## Before opening a PR

Please try to include:

- what changed
- why it changed
- how you tested it
- any screenshots for UI changes
- any distro or device limitations you noticed

## Bug reports

A strong bug report includes:

- distro and version
- desktop environment
- kernel version
- PipeWire / PipeWeaver versions if relevant
- device model
- exact steps to reproduce
- logs or screenshots

## Development

Basic local workflow:

```bash
git clone https://github.com/ttmullins/beacn-utility.git
cd beacn-utility
cargo check
cargo run
```

Release build:

```bash
cargo build --release
```

## Code style

- Keep modules focused
- Prefer readable names over short names
- Keep comments useful and specific
- Remove dead code when it is clearly abandoned
- If something is intentionally left for future work, leave a short note explaining why

## Safety-sensitive changes

Please call these out clearly in PRs:

- device storage writes
- firmware-like behavior
- initialization sequencing
- autostart / background service behavior
- mixer-routing logic that can affect user audio flow

## Pull request checklist

- [ ] Build succeeds locally
- [ ] I tested the change
- [ ] I updated docs if behavior changed
- [ ] I added or updated changelog notes if needed
- [ ] I called out safety-sensitive behavior if applicable
