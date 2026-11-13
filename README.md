# OpenWallpaper

**OpenWallpaper** is an independent, clean-room desktop wallpaper application for Windows 10/11. It is written in Rust + Tauri 2 with a TypeScript UI, uses WebView2 for HTML wallpaper surfaces and dynamically loads libmpv for local media playback.

This repository does not contain Wallpaper Engine source code, proprietary assets, private Steam files, DRM bypasses, or a downloader for content that the current Steam account cannot access.

## Current MVP

- Local library scanner for common FFmpeg/libmpv video containers (MP4/M4V, WebM, MKV, MOV, AVI, MPEG, TS/M2TS/MTS, VOB, WMV/ASF, FLV, OGV, 3GP, MXF, RMVB and others), plus GIF, PNG, JPEG, WebP and BMP. Actual codec support depends on the media encoding and hardware.
- HTML wallpaper folders (`index.html`) and user-supplied HTTP(S) web wallpapers.
- Three original, animated HTML wallpapers are installed into the local library on first launch. They work without Steam, Wallpaper Engine, libmpv or an internet connection.
- Pause/resume controls both native media playback and CSS animation in the bundled wallpapers.
- First-frame previews for images and GIFs; video previews play only while the pointer is over a library card.
- Folder-based slideshows: a directory with at least two supported still images is also exposed as a slideshow.
- A clean-room `scene.json` layered-scene format (specification below).
- Search, type filters, favorites, collections and folder import.
- Automatic discovery of Steam library folders and the installed Wallpaper Engine Workshop content directory (Steam App ID `431960`). The scanner indexes compatible files in place; it does not change or repackage the source files.
- Steam Workshop search through the public Steam Community browse page, item details through the official `ISteamRemoteStorage/GetPublishedFileDetails` endpoint, and subscription hand-off through the Steam client URI. Subscription and downloading are performed by Steam itself.
- One wallpaper surface per monitor, an individual wallpaper assignment per monitor and a per-monitor FPS setting.
- Win32 WorkerW desktop placement, a tray menu, close-to-tray behaviour, Windows startup registration, saved-wallpaper restoration, manual pause/resume, game-process pause, lock-screen process detection and optional battery pause. During Windows sleep, the operating system suspends the app and playback; normal monitoring resumes after wake.
- JSON settings with backup/recovery for malformed files, Rust unit tests, Windows CI workflow and NSIS/MSI bundle configuration.

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

The WorkerW method uses undocumented Windows Explorer behaviour. OpenWallpaper requires a separate `WorkerW` desktop host and reports an error when Explorer does not provide one; it does not treat `Progman` as a successful fallback. The app monitors only its active desktop surfaces, and rebuilds them after Explorer restarts or display-topology changes. Windows shell updates may require compatibility adjustments.

## Requirements

- Windows 10 or Windows 11 (64-bit).
- Rust stable with the MSVC target (`x86_64-pc-windows-msvc`).
- Node.js 20+ and npm.
- Visual Studio Build Tools with the **Desktop development with C++** workload and Windows 10/11 SDK.
- Microsoft Edge WebView2 Runtime (embedded as an offline installer in Windows packages, so no separate download or setup is needed).
- The Windows installer bundles the pinned x64 libmpv runtime and its dependent DLLs. No separate mpv installation or manual DLL copying is required.

The Windows build downloads the pinned official mpv v0.42.0 x64 LGPL libmpv archive during packaging and includes its runtime DLLs and licence notices under application resources. The Windows installer also embeds Microsoft's WebView2 offline installer. OpenWallpaper loads libmpv automatically and installs WebView2 if it is missing. Users do not need to install mpv or WebView2 manually, set environment variables, or copy DLLs beside the executable.

HTML wallpapers use WebView2. Video, GIF, still-image and slideshow playback use the bundled libmpv runtime.

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

1. Start OpenWallpaper. It adds three original animated wallpapers and applies Aurora Drift to the primary monitor on a clean first launch. No Steam account or Wallpaper Engine installation is required.
2. To switch backgrounds, select another wallpaper and choose **All monitors** or a single monitor in the details panel, then select **Set wallpaper**.
3. To add your own backgrounds, select **Import folder**. HTML wallpaper folders with an `index.html` entry point work without libmpv.
4. Use the tray icon to reopen the manager, pause playback or exit.

Video, GIF, image and slideshow playback are ready after installation; the libmpv runtime is included in the Windows installer.

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

The Workshop page is an optional public catalogue for Wallpaper Engine (Steam App ID `431960`). It is not required to use OpenWallpaper. Public item cards and previews can be browsed without owning Wallpaper Engine, but subscription and downloads are controlled by Steam and depend on the Steam account's entitlement to the paid app. OpenWallpaper does not bypass that check or download Workshop binaries itself. For a standalone setup (including users without a Wallpaper Engine licence), the three bundled OpenWallpaper originals work immediately; you can also import your own folders or add a compatible HTTP(S) web wallpaper.

- Workshop search uses the public Steam Community browse page for App ID `431960`.
- Item details can be requested from the official Steam Web API.
- The **Open Steam page** action opens the corresponding Workshop item. Steam performs any subscription, entitlement checks and content download.
- After Steam finishes downloading, use **Refresh library**. OpenWallpaper scans the local `steamapps/workshop/content/431960` directory and the installed `Wallpaper Engine/projects` directory where discovered.
- Only files already present on the local machine are indexed. OpenWallpaper does not call private download endpoints, scrape authentication cookies, bypass DRM, or redistribute Workshop files.

Wallpaper Engine compatibility is import-by-file, not a binary project converter. The MVP indexes the supported media and HTML files found in installed project folders. Proprietary package formats, unsupported effects, and native Wallpaper Engine scenes are not decoded. Users can build an independent `scene.json` scene from files they are entitled to use.

## Performance model

- Rust owns scanning, persistence, process checks and monitor lifecycle. Initial scanning runs on a worker thread; manual import and rescans use `spawn_blocking` behind one async scan lock, so filesystem traversal and thumbnail decoding do not block the UI thread. Adding a web wallpaper is serialized with scans so the scan snapshot cannot silently drop it.
- libmpv is embedded in the native wallpaper surface through its Win32 `wid` API and uses `hwdec=auto-safe`. The quality-reducing `fast` profile is not enabled.
- A separate surface is created only for each monitor that needs a wallpaper.
- Still-image and first-frame GIF thumbnails are cached as PNG files under the app-data directory. The cache key includes canonical path, file size and modification time; rescans do not decode/re-encode unchanged previews.
- Library scans run outside the shared state lock, so browsing and settings reads are not blocked while folders are indexed.
- Volume/mute changes update existing playback instances; they do not tear down and recreate all wallpaper windows. Surfaces are restarted only when FPS/slideshow timing settings change. When a surface must be replaced, the new one is prepared and persisted before the previous HWND/player is retired. On display reconfiguration, libmpv instances are destroyed before their HWND hosts.
- Video card previews are loaded only on hover.
- Startup scan completion, pause state and restore errors are queryable/queued, so UI listeners cannot miss the initial status if WebView2 loads after Rust emits its first events.
- Corrupt settings JSON is copied to `openwallpaper.corrupt-*.json` and reported before safe defaults are loaded; it is never silently overwritten without a backup.
- HTML/scene content uses WebView2. The FPS control applies to native media rendered by libmpv. WebView2 pages and HTML scene layers use the browser compositor cadence; the application does not claim to impose a compositor-wide cap on arbitrary third-party JavaScript.

## Known MVP limitations

- WorkerW is an undocumented Explorer integration and must be tested on each supported Windows 10/11 build, including Windows 11 24H2+ and mixed-DPI monitor layouts.
- The Steam Community page markup can change or rate-limit requests. The Steam browser link remains available as a manual fallback.
- Some external websites prohibit iframe embedding. Those pages cannot be used as embedded web wallpapers unless their server allows framing.
- Lock-screen pause detection currently checks the foreground Windows lock/logon processes. Some Windows configurations may expose a different process, so this should be validated on the target device.
- The user-defined game process list is used for game pause detection. It does not attempt to classify every game automatically.
- Native Wallpaper Engine scene projects, proprietary package formats, interactive input routing and a GPU-wide WebView2 frame limiter are outside this first MVP.
- A successful CI build is not a substitute for hardware validation: Explorer restart, sleep/wake, actual WorkerW layering, mixed-DPI multi-monitor changes and libmpv playback still need testing on physical Windows 10/11 systems.

## Security and privacy

- Local libraries are indexed from folders explicitly added by the user or standard Steam paths discovered from the current Windows account. The default Tauri asset scope is limited to OpenWallpaper's app-data directory; imported roots are added dynamically after explicit selection.
- Local scene layer paths are restricted to the scene root.
- Web wallpapers accept only HTTP(S) addresses.
- Workshop downloading remains under Steam's control and current-account permissions.
- No telemetry or analytics service is included.

## Tests

The Rust suite currently contains 24 unit tests covering media-format classification, scene path traversal, slideshow discovery, web-wallpaper preservation, thumbnail-cache reuse, settings normalization and backward compatibility, corrupted-settings backup, persistence, playback restart decisions, WorkerW/window-label helpers, Workshop ID validation and monitor remapping. Windows CI has recorded a successful run with all 24 tests passing, plus frontend build, Rust check/format, NSIS/MSI bundle build and a startup smoke test. Later changes remain under CI validation; see the Actions tab for the latest commit.

## License

MIT. See [`LICENSE`](LICENSE). The bundled libmpv is the upstream Windows x64 LGPL build from mpv v0.42.0 (the upstream release identifies it as built without GPL-only mpv/FFmpeg components); its notices ship with the runtime. The DLL is dynamically loaded and can be replaced by a compatible build. WebView2 is Microsoft's separately licensed runtime. See the upstream [mpv v0.42.0 release](https://github.com/mpv-player/mpv/releases/tag/v0.42.0) and [mpv licensing](https://github.com/mpv-player/mpv/blob/master/Copyright).

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

The first public build should be treated as an experimental MVP. It is not code-signed. Windows may display SmartScreen warnings for unsigned builds. The media engine dynamically loads the libmpv runtime bundled inside the installer, together with its dependent DLLs and accompanying licence notices. No extra runtime installation is required.

### Local Windows build

Run PowerShell as a normal user from the extracted project directory:

```powershell
Set-ExecutionPolicy -Scope Process Bypass
.\installer\build.ps1
```

Required tools: Node.js LTS, Rust stable with the MSVC target, Visual Studio Build Tools with **Desktop development with C++**, a Windows 10/11 SDK, and network access to download the pinned libmpv build. The preparation script downloads a small 7-Zip command-line extractor automatically when needed. The Tauri Windows bundler uses WebView2; Windows 10/11 normally provides the WebView2 runtime, otherwise install Microsoft's Evergreen WebView2 Runtime.


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

Windows CI runs `cargo fmt --check`, `cargo check`, `cargo test`, the TypeScript/Vite production build, Tauri NSIS/MSI packaging, and a 10-second process-startup smoke test. The run for commit `68132a67bb9c4d4ed8e2087c35298c40f6c47e16` passed all 24 Rust tests and produced workflow artifacts. The newer changes in the current pull request must also pass CI before they are merged.


## Startup diagnostics

If the manager window does not appear, check the tray area first: closing the main window hides OpenWallpaper instead of exiting. On startup panic or Tauri runtime failure, the app writes details to `%TEMP%\openwallpaper-startup-error.log` (usually under your Windows user Temp folder). Library indexing runs after the manager window is shown, so a large Steam Workshop folder should not block initial window creation. Restore or indexing errors are retained until the UI reads them and are displayed as notifications.
