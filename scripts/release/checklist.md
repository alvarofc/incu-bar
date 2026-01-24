# Release checklist (IncuBar)

Follow the CodexBar flow in `../IncuBar/docs/RELEASING.md` as the source of truth.

Quick checklist:
- [ ] Ensure working tree is clean and on the release branch.
- [ ] Update `CHANGELOG.md` top section with release notes.
- [ ] Stamp version across `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml` using `npm run release:stamp -- <version>`.
- [ ] Run `npm test` and `npm run lint`.
- [ ] Build app and CLI artifacts, sign/notarize if needed for macOS.
- [ ] Tag and publish release; verify updater feed and release assets.
