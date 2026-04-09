# Changelog

All notable changes to this fork should be documented here.

## [Unreleased]

### Changed
- Updated fork metadata in `Cargo.toml` so the package points at `ttmullins/beacn-utility`
- Rewrote the README to better explain status, safety, setup, and contribution paths
- Added maintainer-facing project docs: roadmap, contributing guide, issue templates, and PR template

### Code health
- Removed one unused helper from shared PipeWeaver state
- Marked the currently-unused `DeviceMessage` event path as intentionally reserved to avoid noisy warnings until it is wired back into active flows
