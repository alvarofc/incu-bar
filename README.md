# IncuBar

AI usage tracker for Claude, Codex, Cursor, and other assistants. Built with Tauri, React, and TypeScript.
Based on https://github.com/steipete/CodexBar.

## Features

- Track usage across multiple AI assistants (Claude, Codex, Cursor, and more).
- Cross-platform desktop app for macOS, Windows, and Linux.
- Lightweight native shell with a React + TypeScript UI.
- Prebuilt releases for quick install.

## Downloads

Prebuilt apps are available from GitHub Releases.
Supported platforms: macOS (arm64 + x64), Windows (x64), Linux (x64).

## Development

Requirements:
- Rust toolchain
- Bun

Commands:
- `bun install`
- `bun run tauri dev`

## Build

- `bun install`
- `bun run tauri build`

## Employer Reporting Stack

This repo includes a full reporting stack for employer analytics:

- Cloudflare gateway worker: `infra/cloudflare-gateway/`
- Tinybird datasource and pipes: `infra/tinybird/`
- Browser dashboard (static): `infra/dashboard/`

## Release

Create a GitHub Release with a `vX.Y.Z` tag. The release workflow builds installers for macOS (arm64 + x64), Windows (x64), and Linux (x64) and uploads them to the release.
