# Voice Typing - Rust Rewrite

This is a complete rewrite of the Better Voice Typing application in Rust, targeting Windows only.

## Changes from Python Version

### Removed Features
- ❌ OpenAI STT provider
- ❌ Google STT provider
- ❌ Anthropic integration
- ❌ LiteLLM provider routing
- ❌ LLM-based text cleaning

### Kept Features
- ✅ Custom STT (parakeet.kaki.dev) - **ONLY** supported provider
- ✅ Audio recording (WAV format)
- ✅ Caps Lock keyboard trigger
- ✅ System tray icon with menu
- ✅ Microphone selection & favorites
- ✅ Silence detection & timeout
- ✅ Text insertion (clipboard/typing)
- ✅ UI feedback overlay (recording indicator)
- ✅ Settings management (JSON)
- ✅ Transcription history

## Architecture

### Modules

```
src/
├── main.rs              # Main application loop and event handling
├── settings.rs          # JSON-based settings management
├── audio.rs             # Audio recording using cpal + hound
├── stt.rs               # STT client for parakeet.kaki.dev
├── keyboard.rs          # Windows keyboard hooks (Caps Lock)
├── tray.rs              # System tray icon and menu
├── ui.rs                # UI feedback overlay (Windows native)
├── history.rs           # Transcription history (max 10 entries)
└── text_inserter.rs     # Clipboard and keyboard automation
```

### Dependencies

- **Audio**: `cpal` (cross-platform audio), `hound` (WAV file format)
- **HTTP**: `reqwest` (HTTP client for STT API)
- **Serialization**: `serde`, `serde_json`
- **Windows**: `windows` crate (native Windows APIs)
- **System Tray**: `tray-icon`
- **Clipboard**: `arboard`
- **Keyboard**: `enigo`
- **Threading**: `parking_lot`, `crossbeam-channel`
- **Logging**: `log`, `env_logger`

## Building

### Requirements

- Rust 1.85+ (nightly) - Required due to dependency constraints
- Windows 10/11
- Visual Studio Build Tools or Windows SDK

### Build Instructions

⚠️ **IMPORTANT**: This application is **Windows-only** and uses Windows-specific APIs. It will not compile on Linux/macOS.

1. **On Windows**, update Rust to nightly (temporary requirement):
   ```powershell
   rustup update nightly
   rustup default nightly
   ```

2. **Build the project**:
   ```powershell
   cargo build --release
   ```

3. **Run the application**:
   ```powershell
   .\run_voice_typing.bat
   ```

   Or directly:
   ```powershell
   .\target\release\voice-typing.exe
   ```

### Linux/WSL Note

If you try to build on Linux/WSL, you'll see compilation errors for `keyboard.rs` and `ui.rs`. This is **expected** because these modules use Windows-specific APIs (`windows` crate) that only compile on Windows targets. The application must be built and run on Windows.

### Known Issues

- **Dependency Issue**: The current version has a dependency (`moxcms`) that requires Rust edition 2024, which is only available in nightly builds. This will be resolved once:
  - The `tray-icon` crate updates to avoid this dependency, OR
  - Rust 1.85+ stable is released with edition 2024 support

## Settings

Settings are stored in `settings.json` in the same directory as the executable:

```json
{
  "silent_start_timeout": 4.0,
  "silence_threshold": 0.01,
  "stt_base_url": "https://parakeet.kaki.dev",
  "stt_model": "parakeet-tdt-0.6b-v3",
  "selected_microphone": null,
  "favorite_microphones": [],
  "ui_indicator_position": "top-right",
  "ui_indicator_size": "normal",
  "log_retention_days": 60
}
```

## Comparison with Python Version

### Advantages
- ✅ **No Python Runtime**: Single `.exe` file, no dependencies
- ✅ **Faster Startup**: Native binary, instant startup
- ✅ **Smaller Binary**: ~5-10MB vs ~50MB+ for PyInstaller
- ✅ **Lower Memory**: ~20-30MB vs ~60-80MB for Python
- ✅ **Better Performance**: Native code, more efficient

### Disadvantages
- ❌ **Harder to Modify**: Requires Rust knowledge vs Python
- ❌ **Longer Build Times**: Rust compilation is slower
- ❌ **Windows Only**: Removed cross-platform support

## Running from Batch File

The `run_voice_typing.bat` file has been updated to run the Rust binary:

```batch
@echo off
chcp 65001 >nul
title Voice Typing
cd "%~dp0"
start /b "" ".\target\release\voice-typing.exe"
echo 🚀 Better Voice Typing is starting up...
echo    Please wait a few moments for the system tray icon to appear...
timeout /t 5 /nobreak >nul
exit
```

## Future Improvements

- [ ] Add retry functionality for failed transcriptions
- [ ] Add configurable keyboard shortcut (not just Caps Lock)
- [ ] Add audio device hot-plugging detection
- [ ] Optimize UI overlay rendering
- [ ] Add installer/setup wizard
- [ ] Migrate to stable Rust once dependencies allow
