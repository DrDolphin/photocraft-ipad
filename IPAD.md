# PhotoCraft native iPad development port

This is a separate, experimental fork of PhotoCraft **0.16.0**, pinned to upstream commit `b1794c6e0608ccfc887213dcf58398977c5cb75e`. The app is Rust, egui and wgpu using **Metal on iPadOS**. It does not use a browser or a webview. The original image engine and UI are reused.

The native ARM64 device IPA was linked with Apple's iOS 18.5 SDK and its package was verified on **2026-10-25**. The same app source also built and launched on an **iPad (10th generation) Simulator**, survived the startup check, and produced a visually inspected screenshot. **Physical-iPad installation, Apple Pencil, rotation and background/memory behavior remain untested.** This is an experimental preview, not an official ArtCraft release.

Download the unsigned device IPA from the [iPad preview release](https://github.com/DrDolphin/photocraft-ipad/releases/tag/ipad-v0.16.0-alpha.1), then follow [INSTALL-WINDOWS.txt](packaging/ipad/INSTALL-WINDOWS.txt). An unsigned IPA must be personally signed before installation. The package targets **iPadOS 17 or later**.

The tested app source is commit `d762959ce0a73585fe990df1bb994b8ddafc37e3`. Build evidence: [device](https://github.com/DrDolphin/photocraft-ipad/actions/runs/40241659720), [Simulator](https://github.com/DrDolphin/photocraft-ipad/actions/runs/40241266957), and [verification record](packaging/ipad/verification.json). Documentation updates after that commit do not change the app code.

## What the first port includes

- A separate `photocraft-ipad` executable, iPad-only app packaging, and an explicit Metal renderer.
- The existing editor, PSD and image import/export, including the optional HEIC decoder enabled by default.
- A native in-app file chooser for the app's sandboxed Documents folder, atomic saves, preferences and recovery snapshots.
- Files integration: copy images into **Files → On My iPad → PhotoCraft**; open them through **File → Open**. Saves and exports appear in that folder and can be copied or shared using Files.
- A manual cloud Mac workflow that can compile a Simulator app and capture a screenshot, or compile an **unsigned** device IPA.

This initial shell does not implement the iCloud/system document picker, Photos picker, Share/Open With, or system image clipboard. The normal editor's touch events are reused. Apple Pencil force/tilt, rotation, background suspension and memory limits require a real iPad test before treating the port as ready for daily use. Save work before changing apps.

## Checks available on Windows

```powershell
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
cargo check --locked -p photocraft-ipad --target aarch64-apple-ios
cargo clippy --locked -p photocraft-ipad --target aarch64-apple-ios -- -D warnings
cargo test --locked -p photocraft-ipad --bin photocraft-ipad
python packaging/ipad/test_build.py
```

The file-service tests cover a real PNG → PSD → reopen round trip, invalid images, atomic saves, and file-name/container boundaries. An additional ignored visual check drives the real File → Open menu and renders the native shell at three iPad screen sizes. Set `PHOTOCRAFT_IPAD_QA_DIR` to an output folder and run `cargo test -p photocraft-ipad render_native_file_chooser -- --ignored` to repeat it. These desktop renders are layout checks, not iPad hardware tests.

The native shell also builds on a desktop for development. To keep its test files separate, run `cargo run -p photocraft-ipad -- --documents <a-test-folder>`.

## Build on a Mac

Requires full Xcode, its iOS SDK and an installed iPad Simulator runtime, the Xcode license accepted, and Rust 1.99.1. Apple command-line tools alone are insufficient. The development profile uses two build workers and disables LTO to fit a standard hosted Mac.

```sh
rustup toolchain install 1.99.1 --profile minimal --component clippy --component rustfmt
rustup override set 1.99.1
python3 packaging/ipad/build.py --check-prerequisites
python3 packaging/ipad/build.py --simulator --smoke-test --output dist/ipad-simulator
python3 packaging/ipad/build.py --output dist/ipad-device
```

Outputs include the native `.app`, a Simulator ZIP or unsigned device IPA, and `build-report.json`. A successful Simulator smoke test also writes `ipad-simulator.png`. Inspect that screenshot; the smoke test checks process survival rather than every editing operation.

## A Windows-only workflow

The prepared **iPad Native** GitHub Actions workflow runs on a cloud Mac and starts only through **Run workflow**. It does not contain signing secrets and never asks for an Apple password. Simulator and device builds are separate manual runs.

The public fork is [DrDolphin/photocraft-ipad](https://github.com/DrDolphin/photocraft-ipad). GitHub's standard Mac runners are free for public repositories. In the fork's **Actions → iPad Native → Run workflow**, choose `device` for the unsigned IPA or `simulator` for the startup check. The workflow uses a standard `macos-15` runner and does not run automatically on pushes. Workflow artifacts expire after seven days; the preview release retains its download.

An unsigned IPA **cannot be installed directly**. It must first be signed for the device. A developer membership can support TestFlight/ad-hoc distribution. Personal sideloading tools for Windows, such as Sideloadly, can sign a personal build using an Apple account; enter account details only in the signing tool yourself. Free signing has renewal and app-count limits. No Apple credentials belong in this repository or chat.

## Upstream and licenses

PhotoCraft is by the ArtCraft team and contributors, under MIT OR Apache-2.0. The PhotoCraft kitsune icon is separately covered by the same licenses; its notice is included in the app bundle. This is an independent development port and is not an official ArtCraft release. Restricted ArtCraft wordmarks are not used in the iPad app bundle.

- [PhotoCraft 0.16.0](https://github.com/storytold/photocraft/releases/tag/v0.16.0)
- [Apple: run an app on a device](https://developer.apple.com/documentation/xcode/running-your-app-on-simulated-or-physical-devices)
- [GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [Sideloadly for Windows](https://sideloadly.io/)
