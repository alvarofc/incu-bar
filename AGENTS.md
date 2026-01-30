# Development Notes

We are using bun for the project
## Server Status
The Tauri development server is already running. Do NOT attempt to start it again with `bun run tauri dev` or similar commands.

## Testing
When testing changes, the app will hot-reload automatically. Just make the code changes and observe the behavior in the running app.

## Releases
Pushes to `main` trigger the release workflow in `.github/workflows/release-on-main.yml`.
It bumps the version based on commit messages (major on breaking, minor on feat, patch on other non-docs/chore/test changes),
and skips releasing when commits since the last tag are only `docs:`, `chore:`, or `test:`.
