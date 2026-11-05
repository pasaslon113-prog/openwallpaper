# OpenWallpaper code review and test report

Date: 2026-11-05

## Changes in this pass

- Added disk-backed PNG thumbnail caching. Cache keys use canonical path, source file size and last-modified timestamp; cached thumbnails are served through Tauri's asset protocol instead of being base64-embedded in the JSON library index.
- Moved folder scanning outside the application data mutex and serialized concurrent scan/import requests with a dedicated scan lock.
- Avoided recreating wallpaper windows when only volume or mute changes. Existing mpv players receive audio updates, and scene video layers receive live audio updates.
- Normalized FPS, slideshow duration and game-process settings before saving/applying them.
- Removed libmpv's `fast` profile so performance settings do not silently trade away picture quality. Kept `hwdec=auto-safe`, added output frame-drop handling, and skipped the FPS filter for still images.
- Moved slideshow playlist/file preparation before mpv handle allocation and made mpv option failures explicit to prevent leaked native instances.
- Changed settings persistence to write and sync a temporary file before atomic replacement (including the Windows `MoveFileExW` path).
- Fixed the moved `app_data` path in startup setup and added the thumbnail directory to the Tauri asset allowlist.

## Automated checks performed in this environment

- TypeScript strict check: passed using temporary local declarations for the Tauri modules. This checks the project's TypeScript types and logic but is not a replacement for installing the real npm dependencies.
- TypeScript transpile/syntax check: passed for `src/main.ts` and `src/wallpaper/main.ts`.
- JSON parsing: passed for `package.json`, `tsconfig.json`, `tauri.conf.json`, and both Tauri capability files.
- Source audit: all `WallpaperItem` constructors and `scan_library` call sites were updated for the new thumbnail-path field and cache directory.

## Tests added or retained

There are 14 Rust unit tests in the repository, covering media file classification, scene path traversal protection, valid image-based slideshow discovery, preserving manually added web wallpapers during rescans, thumbnail cache reuse, stable IDs, wallpaper-kind serialization, settings normalization and overwrite persistence, audio-only settings not recreating wallpaper windows, playback timing changes, Steam Workshop ID validation/extraction, and monitor window label stability.

## Not executed here

- `cargo test` could not run because this environment has no `cargo` or `rustc` executable. Network access required to install the Rust toolchain is unavailable.
- `npm install` timed out while resolving dependencies. The full Vite/Tauri build therefore could not run here.
- No Windows runtime smoke test was possible. The WorkerW desktop embedding, libmpv hardware decoder selection and multi-monitor geometry still require validation on Windows 10/11.

The Windows GitHub Actions workflow is configured to run the Rust tests and frontend/installer build on a Windows runner. Do not treat this report as evidence that the Windows binary has already been built or runtime-tested.
