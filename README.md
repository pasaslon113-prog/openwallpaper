# OpenWallpaper

**OpenWallpaper** is an independent, clean-room desktop wallpaper application for Windows 10/11. It is written in Rust + Tauri 2 with a TypeScript UI, uses WebView2 for HTML wallpaper surfaces and dynamically loads libmpv for local media playback.

This repository does not contain Wallpaper Engine source code, proprietary assets, private Steam files, DRM bypasses, or a downloader for content that the current Steam account cannot access.

## Current MVP

- Local library scanner for MP4, WebM, MKV, MOV, AVI, GIF, PNG, JPEG, WebP and BMP.
- HTML wallpaper folders (`index.html`) and user-supplied HTTP(S) web wallpapers.
- First-frame previews for images and GIFs; video previews play only while the pointer is over a library card.
- Folder-based slideshows: a directory with at least two supported still images is also exposed as a slideshow.
- A clean-room `scene.json` layered-scene format (specification below).
- Search, type filters, favorites, collections and folder import.
- Automatic discovery of Steam library folders and the installed Wallpaper Engine Workshop content directory (Steam App ID `431960`). The scanner indexes compatible files in place; it does not change or repackage the source files.
- Steam Workshop search through the public Steam Community browse page, item details through the official `ISteamRemoteStorage/GetPublishedFileDetails` endpoint, and subscription hand-off through the Steam client URI. Subscription and downloading are performed by Steam itself.
- One wallpaper surface per monitor, an individual wallpaper assignment per monitor and a per-monitor FPS setting.
- Win32 WorkerW desktop placement, a tray menu, close-to-tray behaviour, Windows startup registration, saved-wallpaper restoration, manual pause/resume, game-process pause, lock-screen process detection and optional battery pause. During Windows sleep, the operating system suspends the app and playback; normal monitoring resumes after wake.
- JSON settings, Rust unit tests, Windows CI workflow and NSIS/WiX bundle configuration.

## Architecture

```text
Tauri management UI (WebView2)
          │ Tauri commands / events
          ▼
      Rust core
     ├─ Library index + previews
     ├─ JSON settings + monitor assignments
     ├─ Steam Community search / official details API
     ├─ Windows startup + system state watcher
     └─ Wallpaper lifecycle manager
          │
          ├─ Windows WorkerW host ── one surface per monitor
          │       ├─ libmpv (wid=HWND): video / GIF / image / slideshow
          │       └─ WebView2: HTML / external web / scene composition
          │
          └─ Windows system tray
```

The WorkerW method uses undocumented Windows Explorer behaviour. OpenWallpaper requires a separate `WorkerW` desktop host and reports an error when Explorer does not provide one; it does not treat `Progman` as a successful fallback. Windows shell updates may require compatibility adjustments.

## Requirements

- Windows 10 or Windows 11 (64-bit).
- Rust stable with the MSVC target (`x86_64-pc-windows-msvc`).
- Node.js 20+ and npm.
- Visual Studio Build Tools with the **Desktop development with C++** workload and Windows 10/11 SDK.
- Microsoft Edge WebView2 Runtime.
- A Windows build of **libmpv** providing `mpv-2.dll` and its required runtime DLLs.

The libmpv runtime is deliberately not vendored in this repository. Obtain it from an official mpv/libmpv distribution, respecting the distribution's licence, and place `mpv-2.dll` plus its sibling runtime dependencies next to `OpenWallpaper.exe`. Alternatively set the `OPENWALLPAPER_MPV` environment variable to the full path of `mpv-2.dll`.

HTML wallpapers can run without libmpv. Video, GIF, still-image and slideshow playback require the libmpv runtime.

## Development

From the repository root in PowerShell:

```powershell
npm install
npm run tauri dev
```

Run Rust tests:

```powershell
cd src-tauri
cargo test
cd ..
```

Build the frontend:

```powershell
npm run build
```

Build Windows installers:

```powershell
npm run tauri build -- --bundles nsis,msi
```

The installers are generated under `src-tauri/target/release/bundle/`.

## First run

1. Start OpenWallpaper. It scans the default Steam library locations it can discover.
2. Select **Import folder** and choose any additional wallpaper directory.
3. Select an item in the library.
4. Choose **All monitors** or a single monitor in the details panel.
5. Select **Set wallpaper**.
6. Use the tray icon to reopen the manager, pause playback or exit.

For video/GIF/image/slideshow playback, make sure `mpv-2.dll` is accessible to the application. The app reports a direct error when libmpv cannot be loaded; it does not silently substitute a different player.

## Supported clean-room scene format

A scene is a directory containing `scene.json` and the assets referenced by its layers. Version 1 supports `image`, `video` and `web` layers. Web layers may reference a local asset or an HTTP(S) page. Coordinates and dimensions are percentages of the monitor surface.

```json
{
  "version": 1,
  "name": "Layered evening",
  "description": "An original multi-layer scene",
  "layers": [
    {
      "type": "image",
      "source": "background.webp",
      "x": 0,
      "y": 0,
      "width": 100,
      "height": 100,
      "opacity": 1,
      "fit": "cover"
    },
    {
      "type": "video",
      "source": "particles.webm",
      "x": 0,
      "y": 0,
      "width": 100,
      "height": 100,
      "opacity": 0.55,
      "fit": "cover",
      "muted": true
    }
  ]
}
```

All layer paths are resolved relative to the scene directory. The backend canonicalizes each path and rejects paths that attempt to escape that directory. Scene manifests are independent of Wallpaper Engine's scene format.

## Steam Workshop and Wallpaper Engine content

- Workshop search uses the public Steam Community browse page for App ID `431960`.
- Item details can be requested from the official Steam Web API.
- The **Subscribe in Steam** action opens the corresponding Workshop item in the installed Steam client. Steam performs subscription, account checks and content download.
- After Steam finishes downloading, use **Refresh library**. OpenWallpaper scans the local `steamapps/workshop/content/431960` directory and the installed `Wallpaper Engine/projects` directory where discovered.
- Only files already present on the local machine are indexed. OpenWallpaper does not call private download endpoints, scrape authentication cookies, bypass DRM, or redistribute Workshop files.

Wallpaper Engine compatibility is import-by-file, not a binary project converter. The MVP indexes the supported media and HTML files found in installed project folders. Proprietary package formats, unsupported effects, and native Wallpaper Engine scenes are not decoded. Users can build an independent `scene.json` scene from files they are entitled to use.

## Performance model

- Rust owns scanning, persistence, process checks and monitor lifecycle.
- libmpv is embedded in the native wallpaper surface through its Win32 `wid` API and uses `hwdec=auto-safe`. The quality-reducing `fast` profile is not enabled.
- A separate surface is created only for each monitor that needs a wallpaper.
- Still-image and first-frame GIF thumbnails are cached as PNG files under the app-data directory. The cache key includes canonical path, file size and modification time; rescans do not decode/re-encode unchanged previews.
- Library scans run outside the shared state lock, so browsing and settings reads are not blocked while folders are indexed.
- Volume/mute changes update existing playback instances; they do not tear down and recreate all wallpaper windows. Surfaces are restarted only when FPS/slideshow timing settings change.
- Video card previews are loaded only on hover.
- HTML/scene content uses WebView2. The FPS control applies to native media rendered by libmpv. WebView2 pages and HTML scene layers use the browser compositor cadence; the application does not claim to impose a compositor-wide cap on arbitrary third-party JavaScript.

## Known MVP limitations

- WorkerW is an undocumented Explorer integration and must be tested on each supported Windows 10/11 build, including Windows 11 24H2+ and mixed-DPI monitor layouts.
- The Steam Community page markup can change or rate-limit requests. The Steam browser link remains available as a manual fallback.
- Some external websites prohibit iframe embedding. Those pages cannot be used as embedded web wallpapers unless their server allows framing.
- Lock-screen pause detection currently checks the foreground Windows lock/logon processes. Some Windows configurations may expose a different process, so this should be validated on the target device.
- The user-defined game process list is used for game pause detection. It does not attempt to classify every game automatically.
- Native Wallpaper Engine scene projects, proprietary package formats, interactive input routing and a GPU-wide WebView2 frame limiter are outside this first MVP.
- A real Windows build and runtime smoke test are required before calling the app production-ready. This build environment does not provide the Rust/MSVC Windows toolchain.

## Security and privacy

- Local libraries are indexed from folders explicitly added by the user or standard Steam paths discovered from the current Windows account.
- Local scene layer paths are restricted to the scene root.
- Web wallpapers accept only HTTP(S) addresses.
- Workshop downloading remains under Steam's control and current-account permissions.
- No telemetry or analytics service is included.

## Tests

Fourteen Rust unit tests cover file-type recognition, stable IDs, scene path traversal rejection, valid image-based slideshow discovery, preserving manually added web wallpapers during rescans, thumbnail-cache reuse, settings normalization and persistence, avoiding wallpaper restarts for audio-only changes, Workshop ID parsing/validation, and monitor-window label generation. The workflow in `.github/workflows/windows.yml` runs the Rust tests and builds the frontend/Windows installers on a Windows runner.

## License

MIT. See [`LICENSE`](LICENSE). Third-party runtime components, including libmpv and WebView2, retain their own licences and terms.

## Getting a ready Windows installer

The repository includes a Windows GitHub Actions workflow. After the source is pushed to a GitHub repository:

1. Open **Actions** and run **Windows build and release** manually, or push a change to `main`/`master`. A successful run uploads temporary NSIS/MSI installers as workflow artifacts (kept for 14 days).
2. To create a draft prerelease with downloadable installers, create and push a version tag, for example:

   ```powershell
   git tag v0.1.0
   git push origin v0.1.0
   ```

3. Open **GitHub → Releases**, review the draft `OpenWallpaper v0.1.0`, and publish it when ready.
4. Download the `.exe` (NSIS) or `.msi` installer from that release.

The first public build should be treated as an experimental MVP. It is not code-signed. Windows may display SmartScreen warnings for unsigned builds. The media engine dynamically loads `mpv-2.dll`; the libmpv runtime is intentionally not bundled and must be obtained from a trusted official distribution in accordance with its licence. Without it, HTML wallpapers can work, but video/GIF/image/slideshow playback will report that the runtime is unavailable.

### Local Windows build

Run PowerShell as a normal user from the extracted project directory:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\installer\build.ps1
```

Required tools: Node.js LTS, Rust stable with the MSVC target, Visual Studio Build Tools with **Desktop development with C++**, and a Windows 10/11 SDK. The Tauri Windows bundler uses WebView2; Windows 10/11 normally provides the WebView2 runtime, otherwise install Microsoft's Evergreen WebView2 Runtime.


## Performance and validation

The library stores generated thumbnails in the application data directory (`thumbnails/`). Each cache key includes the canonical source path, file size, and last-modified timestamp. Re-scans reuse existing thumbnails instead of decoding and encoding every image again. The UI loads these files lazily through Tauri's asset protocol; thumbnail bytes are not embedded into the persisted JSON file.

The library scan runs against a snapshot outside the shared application-state lock, so a slow folder scan does not block settings and library reads. Favorite flags are merged from the latest state when the scan finishes.

Changing volume or mute no longer destroys and recreates desktop surfaces. Only FPS or slideshow timing changes trigger a playback restart. The mpv `fast` profile is intentionally not enabled because it can reduce rendering quality. Hardware decoding remains `auto-safe`, allowing mpv to fall back when the driver/backend does not support a format.

Run the important regression tests on Windows:

```powershell
cd src-tauri
cargo test --target x86_64-pc-windows-msvc
cd ..
npm run build
```

Tests cover media-format classification, scene path traversal rejection, valid image-based slideshow discovery, on-disk thumbnail cache reuse, settings normalization, wallpaper-kind serialization, and settings persistence. These tests still need to be executed on a Windows build machine before publishing a release.

