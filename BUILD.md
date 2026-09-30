# Build Instructions

This guide covers how to set up the development environment and build Echo from source across different platforms.

## Prerequisites

### All Platforms

- [Rust](https://rustup.rs/) (latest stable)
- [Bun](https://bun.sh/) package manager
- [Tauri Prerequisites](https://tauri.app/start/prerequisites/)

### Platform-Specific Requirements

#### macOS

- Xcode Command Line Tools
- Install with: `xcode-select --install`

#### Windows

- Microsoft C++ Build Tools
- Visual Studio 2019/2022 with C++ development tools
- Or Visual Studio Build Tools 2019/2022

#### Linux

- Build essentials
- ALSA development libraries
- Install with:

  ```bash
  # Ubuntu/Debian
  sudo apt update
  sudo apt install build-essential libasound2-dev pkg-config libssl-dev libvulkan-dev vulkan-tools glslc libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libgtk-layer-shell0 libgtk-layer-shell-dev patchelf cmake

  # Fedora/RHEL
  sudo dnf groupinstall "Development Tools"
  sudo dnf install alsa-lib-devel pkgconf openssl-devel vulkan-devel \
    gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel \
    gtk-layer-shell gtk-layer-shell-devel \
    cmake

  # Arch Linux
  sudo pacman -S base-devel alsa-lib pkgconf openssl vulkan-devel \
    gtk3 webkit2gtk-4.1 libappindicator-gtk3 librsvg gtk-layer-shell \
    cmake
  ```

## Setup Instructions

### 1. Clone the Repository

```bash
git clone <your-echo-repo>.git
cd echo
```

### 2. Install Dependencies

```bash
npm install
```

### 3. Start Dev Server

```bash
npm run tauri dev
```

### 4. Build for Production

```bash
npm run tauri build
```

This compiles a release binary and generates platform-specific bundles (deb, rpm, AppImage on Linux; dmg on macOS; msi on Windows).

## Windows: GPU Whisper (Vulkan)

The Windows build uses the `whisper-vulkan` backend so Whisper runs on the GPU
(any vendor — NVIDIA/AMD/Intel — auto-detected at runtime; pick the device in
Settings → Advanced → Hardware Acceleration). CI installs the Vulkan SDK
automatically. For a **local** build you need it too:

1. Install the [Vulkan SDK](https://vulkan.lunarg.com/) (provides `glslc`).

On a **stable** toolchain (VS 2022 + a 3.x CMake) `npm run tauri dev` just
works once the Vulkan SDK is installed.

On a **bleeding-edge** toolchain (VS preview + CMake 4.x), whisper.cpp's
`ggml-vulkan` shader-gen subproject fails under the MSVC/msbuild generator
(`vs_link_exe` manifest LNK1104, exceptions off). Build with LLVM + Ninja
instead — from a PowerShell with the VS dev environment imported:

```powershell
# install once: scoop install ninja llvm   (LLVM gives clang-cl + lld-link)
$env:CC = "clang-cl"; $env:CXX = "clang-cl"; $env:RC = "llvm-rc"
$env:CMAKE_GENERATOR = "Ninja"
$env:CMAKE_LINKER_TYPE = "LLD"            # lld-link dodges the MSVC manifest bug
$env:CMAKE_POLICY_VERSION_MINIMUM = "3.5" # accept whisper.cpp's old CMake policies
$env:CXXFLAGS = "/EHsc"; $env:CL = "/EHsc" # ggml omits /EHsc for non-MSVC compiler id
$env:VULKAN_SDK = "C:\VulkanSDK\<version>"
# clear VS instance/platform/toolset (Ninja rejects them):
Remove-Item env:CMAKE_GENERATOR_INSTANCE,env:CMAKE_GENERATOR_PLATFORM,env:CMAKE_GENERATOR_TOOLSET -EA SilentlyContinue
cargo tauri dev   # or: cargo run --manifest-path src-tauri/Cargo.toml
```

If you switch toolchains, `cargo clean -p whisper-rs-sys` first so CMake
reconfigures from scratch (a stale cache keeps the old generator/instance).

## Fast builds (daily workflow)

For daily development, a `fast` cargo profile is available which provides
release-grade optimization (thin LTO, 16 codegen units) with much faster
compile times than the full `release` profile (fat LTO).

### Quick start with script

The easiest way to build on Windows is using the provided PowerShell script
which handles the LLVM environment and Vulkan SDK detection:

```powershell
# Default: fast build
./scripts/build-fast.ps1

# Even faster (skips heavy Intel MKL dependency)
./scripts/build-fast.ps1 -NoDiarization

# Build and bundle
./scripts/build-fast.ps1 -Bundle

# Release-grade build
./scripts/build-fast.ps1 -Profile release
```

### Manual workflow

1.  **Cargo Profile**: Use `--profile fast` for daily builds.

    ```bash
    cargo build --manifest-path src-tauri/Cargo.toml --profile fast
    ```

    Note: `npm run tauri build` always uses the `release` profile — tauri-cli
    appends `--release` itself and the bundler expects `target/release`
    artifacts, so custom profiles apply to bare `cargo build` only.

2.  **sccache**: Speeds up repeated builds by caching dependency crates across
    clean builds.
    - Prerequisite: `RUSTC_WRAPPER=sccache` must be set as an environment variable.
    - Check status: `sccache --show-stats`.

3.  **Skipping Diarization**: To significantly speed up compilation, skip the
    heavy Intel MKL dependency by disabling default features:
    ```bash
    npm run tauri dev -- -- --no-default-features
    ```
    _Note: If built without default features, passing `--diarize` at runtime
    will error by design._

## Linux Install (from source)

The raw binary (`src-tauri/target/release/echo`) cannot run standalone — it needs Tauri resource files (tray icons, sounds, VAD model) to be co-located at the expected path.

**Install from the deb bundle** (works on any Linux distro):

```bash
cd /tmp
ar x /path/to/echo/src-tauri/target/release/bundle/deb/Echo_*_amd64.deb data.tar.gz
tar xzf data.tar.gz
sudo cp usr/bin/echo /usr/bin/
sudo cp -r usr/lib/Echo /usr/lib/
sudo cp -r usr/share/icons/hicolor/* /usr/share/icons/hicolor/
sudo cp usr/share/applications/Echo.desktop /usr/share/applications/
```

After subsequent rebuilds, only the binary needs re-copying:

```bash
sudo cp src-tauri/target/release/echo /usr/bin/
```

Resources only need re-copying if they change upstream (new icons, sounds, etc.).

## Troubleshooting

### AppImage build fails on Arch / rolling-release distros

`linuxdeploy` bundles its own `strip` binary which is too old to process system libraries built with newer toolchains on rolling-release distros (Arch, CachyOS, Manjaro, EndeavourOS).

The error from Tauri:

```
Bundling Echo_*_amd64.AppImage
failed to bundle project `failed to run linuxdeploy`
```

Tauri swallows the real linuxdeploy error. To see it, run linuxdeploy manually:

```bash
cd src-tauri/target/release/bundle/appimage
~/.cache/tauri/linuxdeploy-x86_64.AppImage --appimage-extract-and-run \
  --appdir Echo.AppDir --plugin gtk --output appimage
```

**Workaround:** The binary, deb, and rpm bundles all build fine — only the AppImage step fails. To skip it:

```bash
npm run tauri build -- --bundles deb
```

Then install using the deb extraction method above.

## CLI / Offline Transcription

Echo can transcribe an audio/video file headlessly from the command line:

```bash
echo --transcribe-file path/to/audio.mp3                          # transcript to stdout
echo --transcribe-file talk.mp4 -o talk.txt                       # transcript to a file
echo --transcribe-file ru-en.wav --model turbo --language auto    # RU/EN code-switch
echo --transcribe-file talk.mp4 -o talk.srt --format srt          # subtitles with timestamps
echo --transcribe-file meeting.mp3 --diarize --format inline      # label speakers
```

Flags:

- `--transcribe-file <FILE>` — input audio or video file.
- `-o, --output <FILE>` — save the transcript (otherwise printed to stdout).
- `--model <ID>` — model to use (default: the app's selected model). **Use a Whisper
  model (e.g. `turbo`) for Russian/English code-switching**; single-script engines
  (Parakeet/GigaAM) transliterate embedded English into Cyrillic.
- `--language <LANG>` — `auto` (default) or a code like `ru`/`en`. `auto` enables the
  bilingual RU/EN initial-prompt steering on Whisper models.
- `--format <FMT>` — output format: `plain` (default), `inline` (`[mm:ss]` prefixes),
  `srt`, `vtt`, `json`, or `karaoke`. When `--format` is omitted, it is inferred from the
  `-o` extension (`.srt`/`.vtt`/`.json`), else `plain` (`karaoke` is explicit-only — a
  `.vtt` extension stays plain WebVTT).
  - **`karaoke`** — WebVTT with per-word `<mm:ss.mmm>` highlight tags. Cues are grouped
    by speaker run (with `--diarize`) or by sentence punctuation otherwise.
  - **`json` + `--diarize`** — adds a Deepgram-shaped per-word array (`word`, `start`,
    `end`, 0-based `speaker`, and an overlap-based `speaker_confidence` proxy).
  - Word-level output is available on **Whisper** models only (Parakeet/GigaAM/… stay
    segment-level). Token timestamps are requested only when `--diarize` or a word-level
    format is used, so plain transcription pays no extra cost.
- `--diarize` — label each section with the speaker (`Speaker 1`, `Speaker 2`, …).
  Diarization models download from HuggingFace on first use. If it fails, the transcript
  is still emitted with timestamps (speakers omitted).
- `--speakers <N>` — **accepted but currently ignored**: the speakrs 0.4.2 engine
  auto-detects the speaker count and exposes no count hint.

**Requirement:** [`ffmpeg`](https://ffmpeg.org/) must be on `PATH` (used to decode and
normalize the input to 16 kHz mono). On Windows: `winget install Gyan.FFmpeg`.

The CLI reuses the same engine and bilingual prompt steering as the GUI, so RU/EN mixing
quality matches the app, and it runs in its own process — independent of any running GUI
instance. The same options are available from the GUI's **Transcribe file** tab.

> **transcribe-rs fork note:** `src-tauri/Cargo.toml` patches `transcribe-rs` to
> [`master5d/transcribe-rs@word-timestamps`](https://github.com/master5d/transcribe-rs/tree/word-timestamps)
> via `[patch.crates-io]`. That branch is the published 0.3.8 plus a surgical patch that
> adds a `TimestampGranularity` knob to the Whisper engine and surfaces whisper.cpp
> word/token timestamps (used for word-level output and per-word diarization). The patch
> is feature-compatible with the crates.io release; cargo fetches and builds it like any
> other dependency.

> **Diarization backend note:** speaker diarization pulls `speakrs` → `ndarray-linalg`
> with the **Intel MKL static** backend, which adds ~150 MB to the binary. The `staticlib`
> crate-type is intentionally disabled for desktop (`src-tauri/Cargo.toml`) because the
> large MKL static archive crashes rustc's `ar_archive_writer`; only `cdylib` + `rlib`
> (desktop) are built. Restore `staticlib` behind a mobile cfg if iOS/Android is added.

### Windows release build (LLVM toolchain)

The release build uses LLVM/clang-cl + Ninja (the default MSVC/msbuild generator fails to
build `ggml-vulkan`). Set up the environment, then `cargo build --release`:

```bat
:: Visual Studio C++ build tools environment
call "C:\Program Files\Microsoft Visual Studio\<edition>\Common7\Tools\VsDevCmd.bat" -arch=x64
:: LLVM + Ninja on PATH (e.g. via scoop: scoop install llvm ninja)
set "CC=clang-cl"
set "CXX=clang-cl"
set "CMAKE_GENERATOR=Ninja"
set "CMAKE_LINKER_TYPE=LLD"
set "CMAKE_POLICY_VERSION_MINIMUM=3.5"
set "CXXFLAGS=/EHsc"
set "VULKAN_SDK=C:\VulkanSDK\<version>"
:: clear VS instance specs that break the Ninja generator
set "VSINSTALLDIR="
set "CMAKE_GENERATOR_INSTANCE="
set "CMAKE_GENERATOR_PLATFORM="
set "CMAKE_GENERATOR_TOOLSET="
cargo build --release
```

## Meeting Copilot — "Consult council" (optional wellbeing sidecar)

The Meeting Copilot (capture, live transcript, Brief/SOAP analysis, session save) runs
entirely inside melori. The practitioner features — clients with consent, per-client
sessions and the psychology council — are served by the bundled local engine
(`engine/`), which the app starts itself on a loopback port with a per-run token.
Install it once with `./scripts/setup-engine.ps1` (or `./scripts/setup-engine.sh`).
The council uses any OpenAI-compatible endpoint set in Settings (a local server by
default); the app warns before session content leaves the machine.
