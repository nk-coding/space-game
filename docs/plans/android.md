# Plan: the Android build and the phone UI

Status: **proposed plan**, written 13 Oct 2026 by a planning agent on `dev` `28aa0b5`. Nothing here is decided.
This is a **later phase**: Niklas decided on 13 Oct 2026 that Phase 7 is the first playable loop (GDR-0062), so
the lead gives this phase its number. The records it would produce are proposed in section 3 ("Records to
write"); the questions for Niklas are in section 8. Numbers marked *placeholder* are for feel, not a spec. Web
facts were checked on 13 Oct 2026 (links inline); facts about crates were read in the exact versions the
workspace locks (`Cargo.lock`), with docs.rs links. The dependency audit of the same day (the lead's
`reviews/deps.md`, section 3, "R" numbers below) is folded in.

## 1. Summary

The phase puts the game on Niklas's Android phone as the first end-user platform ([ADR-0006](../adr/0006-web-as-development-platform.md)):
an APK built in the VM without sudo and installed over adb, flight with the touch stick and lock of
[GDR-0009](../gdr/0009-stick-and-lock-controls.md) in the mockup's HUD, phone layouts of the drydock and the
behavior editor, the player storage on the phone, and the deferred phone measurements (spike questions 2 and 4–7:
phone hash, speed, look cost, temperature, flight feel). The first slice is small: the sim's own examples on the
phone (hashes and step times, no APK), then the existing sandbox as an APK with the existing touch widgets.
Today the game does not build for Android at all (2, "Current state"), so the slice starts with two manifest lines.

## 2. Already decided vs open

### Decided (records)

| What | Record |
|---|---|
| End users get Android (mid-range 2021+, Snapdragon 7-series class, 6 GB) and desktop; 60 fps with a 30 Hz sim; headroom rule on the flagship: 20 ships with debris in about 8 ms per frame, sim step under 4 ms, temperature after 10 minutes | [ADR-0004](../adr/0004-target-platforms-and-performance-first.md), [ADR-0011](../adr/0011-30hz-sim-60fps-interpolation.md) |
| The web build is for development only; phone measurements wait until Android work starts (this phase); everything stays Android-compatible | [ADR-0006](../adr/0006-web-as-development-platform.md) (supersedes [ADR-0005](../adr/0005-web-as-full-end-user-platform.md)) |
| All Rust, Bevy everywhere, no C++ anywhere, pinned engine versions | [ADR-0002](../adr/0002-all-rust-stack-with-bevy.md) |
| APKs reach the flagship phone over adb Wireless debugging; the VM network must reach the phone | [ADR-0007](../adr/0007-development-setup-vm-and-host-browser.md) |
| No hosted CI: checks are local scripts | [ADR-0008](../adr/0008-local-git-only.md) |
| The sim must give the same per-tick hashes on the phone as natively and in the browser | [ADR-0010](../adr/0010-cross-platform-determinism.md) |
| Content, icons, fonts and shaders are embedded in the binary (no asset files to ship) | [ADR-0017](../adr/0017-embedded-content-in-memory-loader.md), [ADR-0028](../adr/0028-icons-in-the-content-pack.md); fonts `include_bytes!` in `crates/game/src/flight_hud.rs`, shaders `embedded_asset!` |
| Renderer: one atlas mesh per piece, glow sprites, bloom as a toggle, sRGB compositing on an HDR camera (the HDR part is a WebGL2 workaround) | [ADR-0020](../adr/0020-style-2a-renderer.md), `crates/game/src/main.rs` `spawn_camera` |
| Raster textures are made at runtime at zoom × DPI | [ADR-0021](../adr/0021-no-fixed-raster-graphics.md) |
| Storage layer of named RON documents; on Android "the same files backend on the app's internal data path, later"; Android clipboard not built | [ADR-0031](../adr/0031-player-storage-and-design-sharing.md) |
| Landscape only on the phone; floating HUD (hull map top left, target panel top right, stick bottom left, fire and abilities bottom right); the drydock and the behavior editor exist on all platforms; HUD customisable later | [GDR-0008](../gdr/0008-ui-ship-mind-interface.md), mockups in `docs/concept/boards/ui-mockups/` |
| Touch: stick = "fly that way", push = speed, letting go brakes, drift toggle, tap a ship to lock it, the stick strafes under a lock | [GDR-0009](../gdr/0009-stick-and-lock-controls.md); PC keys changed by [GDR-0040](../gdr/0040-keyboard-flight-and-smart-lock-aim.md), touch unchanged |
| Fleet bar chips: a tap focuses, a tap on a dormant vessel wakes it | [GDR-0050](../gdr/0050-fleet-bar.md) (provisional) |

What already exists in the code (`docs/client.md` "Controls"): a fixed touch stick bottom left and a FIRE button
bottom right (`crates/game/src/controls.rs`, shown after the first touch or with `?touch=1`), tap to lock or
unlock, taps on fleet-bar chips, pinch zoom (`camera.rs`), and touch presses in the widget kit (`ui_kit.rs`).

### Current state of the Android build (audit, 13 Oct 2026)

- **It does not compile for `aarch64-linux-android`.** `android-activity` 0.6.1 (pulled by winit and
  bevy_android) stops with `compile_error!("Either \"game-activity\" or \"native-activity\" must be enabled")`,
  and the game's Bevy features enable neither. Fix: Bevy's `android-native-activity` (audit R2, which puts it in
  the main feature list; it only touches Android-target dependencies). `android-game-activity` would compile
  Google's C++ (3.1).
- **C in the Android tree**: blake3 (pulled by `bevy_asset`) compiles `c/blake3_neon.c` with the NDK's clang on
  little-endian aarch64 unless its `pure` or `no_neon` feature is on ([build.rs](https://docs.rs/crate/blake3/1.8.7/source/build.rs)).
  The audit flags blake3's assembly on x86 (R11: a `blake3` dependency with `pure` for non-wasm targets); the same
  switch keeps the Android build all Rust. The only other packages in `Cargo.lock` that build with `cc` are
  android-activity (C++ only with `game-activity`), wayland-backend (Linux) and tracing-oslog (Apple).
- **Correctly gated already**: arboard (manifest and `clipboard.rs`), bevy_egui's own arboard, web-sys / js-sys /
  wasm-bindgen (wasm only), x11 (absent from the Android tree); `sim`, `data` and `art` are pure Rust.
- **Missing for Android** (code, not dependencies): a library target with `#[bevy_main]` (the game is bin-only),
  a storage root (`storage::data_dir()` returns `None` on Android, so documents fall back to memory), the
  clipboard (a stub), and everything in 5.3.
- **wgpu is Vulkan-only on Android** (no GLES in the tree; Bevy 0.20 has no top-level `gles` feature).
- **Bevy runs single-threaded** (`multi_threaded` is off; audit R6 recommends it for native when native work
  starts).

### Open

- Toolchain, Android activity type, minimum and target Android version, renderer backend (section 3).
- How builds reach the phone if the VM can't reach it directly (ADR-0007 assumes it can).
- Where player storage lives on Android, and how exports and share codes leave the phone (clipboard, share sheet).
- Touch details GDR-0009 leaves open: fixed or floating stick, firing, target cycling, pause, Back key.
- Phone layouts of the flight HUD, the drydock and the behavior editor; touch gestures in both editors.
- Text input on the phone (names, search, notes, the behavior TEXT view), safe areas (notches, nav bar), the
  frame-rate cap on 90/120 Hz screens, what happens when the app is sent to the background.
- The phone measurements (spike questions 2, 4, 6, 7; question 5 compares them with the pending browser numbers).
- Which phone: the spike plan says "Niklas's flagship"; the model is not recorded anywhere.

## 3. Gaps: options and recommendations

### 3.1 Toolchain and activity type

Every Android Bevy app is a native library loaded by an Android `Activity`. Since Bevy 0.19 neither activity
type is a default feature; the app must pick one (bevy 0.20 `default_platform` has neither:
[bevy 0.20 Cargo.toml](https://docs.rs/crate/bevy/0.20.0/source/Cargo.toml)). Bevy 0.20 ships two Gradle examples,
`android_example` (GameActivity, minSdk 31, targetSdk 33, AGP 8.4.0, games-activity 4.4.0) and
`android_example_native` ([examples/mobile](https://github.com/bevyengine/bevy/tree/v0.20.0/examples/mobile)), and
documents cargo-ndk + Gradle ([examples README, Android](https://github.com/bevyengine/bevy/blob/v0.20.0/examples/README.md#android)).

| Option | For | Against |
|---|---|---|
| **A. NativeActivity, cargo-ndk, packaging by a short script** (aapt2, zipalign, apksigner from the SDK build-tools) | Pure Rust plus an XML manifest (with blake3's `pure` switch, section 2): android-activity's NativeActivity backend compiles no C/C++ and needs no Java ([android-activity README](https://github.com/rust-mobile/android-activity)); no Gradle (saves ~1–1.5 GB of caches, *estimate*); the soft keyboard sends key events the game already reads (see 3.7); a known pattern (Flutter's [native_activity_apk.py](https://flutter.googlesource.com/mirrors/engine/+/HEAD/testing/android/native_activity/native_activity_apk.py) does exactly aapt2 → zipalign → apksigner) | Bevy calls NativeActivity "legacy, should be avoided for most new Android games" ([cargo_features.md](https://github.com/bevyengine/bevy/blob/v0.20.0/docs/cargo_features.md)); our packaging script is ours to maintain; no AAB for a store (bundletool or Gradle later); true immersive mode (hiding the nav bar) needs a UI-thread Java call |
| B. GameActivity, cargo-ndk, Gradle (Bevy's default example) | The path Bevy and Google recommend ([GameActivity](https://developer.android.com/games/agdk/game-activity)); GameTextInput for real IME later; Gradle makes AABs | **Compiles C++**: android-activity's build script builds Google's `GameActivity.cpp`, `gametextinput.cpp` and links `c++_static` ([build.rs](https://docs.rs/crate/android-activity/0.6.1/source/build.rs)), against "No C++ anywhere"; needs a Java `MainActivity`, AndroidX AppCompat and Gradle; winit 0.30.13 drops GameActivity's soft-keyboard text (3.7); Bevy says GameActivity needs API 31+ |
| C. NativeActivity with Gradle (Bevy's `android_example_native`) | Bevy's own project layout; easy switch to GameActivity or AABs | Gradle's disk, RAM (daemon) and build time for no gain now |
| D. xbuild or cargo-apk | One command | xbuild is marked "unmaintained" ([repo](https://github.com/rust-mobile/xbuild)); cargo-apk is deprecated (Bevy README); the cargo-apk2 fork is a single-maintainer project ([docs.rs](https://docs.rs/crate/cargo-apk2/1.3.4)) |

**Recommendation: A** (the audit reached the same choice of activity, R2). It keeps the hard rules (no C++, no
Java; with blake3's `pure` switch, no C either), is the smallest download, and performance is the same (both
activities just hand a window and input to winit). cargo-ndk (4.x, [repo](https://github.com/bbqsrc/cargo-ndk))
only sets up the NDK linker; option C (Gradle) stays the fallback if the script gets in the way. Switching to B
later means a feature flag, a Java class and Gradle, nothing in the game code.

### 3.2 Minimum and target Android version

Google's distribution data (1 Dec 2025, [9to5Google](https://9to5google.com/2026/01/30/android-16-is-on-7-5-of-devices-in-latest-distribution-numbers-update/)):
Android 12+ 69.3 %, 10+ 90.8 %, 8+ 98.4 % of active devices.

| minSdk | For | Against |
|---|---|---|
| **31 (Android 12)** | ADR-0004's 2021+ mid-range phones mostly got Android 12 or newer; the NDK thermal API (for the temperature measurement) starts at 31 ([ADPF thermal](https://developer.android.com/games/optimize/adpf/thermal)); keeps the GameActivity door open (Bevy: API 31+) | Leaves out 2021 phones never updated past Android 11 |
| 29 (Android 10) | Google's suggested Vulkan floor (Android 10, Vulkan 1.1: [Vulkan support](https://developer.android.com/games/develop/vulkan/native-engine-support)) | Older drivers to test; thermal API only through Java |
| 26 (Android 8) | Bevy's docs example (`-P 26`); almost every device | Vulkan 1.0 drivers, a GLES fallback becomes necessary |

**Recommendation: minSdk 31, targetSdk 36** (Google Play requires target 36 for new apps and updates since
31 Aug 2026: [target SDK](https://developer.android.com/google/play/requirements/target-sdk); sideloading doesn't
need it, but it costs nothing now). Target 35+ means edge-to-edge and drawing under the display cutout by default
([display cutout](https://developer.android.com/develop/ui/views/layout/display-cutout)), so the HUD needs safe
margins (4.1). **arm64-v8a only**: the flagship and every 2021+ mid-range phone are 64-bit.

### 3.3 Renderer backend

Bevy 0.20 compiles wgpu with `vulkan` (plus dx12/metal) and no `gles` on native; on Android `Backends::all()`
therefore means Vulkan only ([bevy_render 0.20 Cargo.toml](https://docs.rs/crate/bevy_render/0.20.0/source/Cargo.toml),
[settings.rs](https://docs.rs/crate/bevy_render/0.20.0/source/src/settings.rs)). Our `webgl2` feature changes nothing
on Android (its cfgs need `target_arch = "wasm32"`).

| Option | For | Against |
|---|---|---|
| **Vulkan only** | Bevy's default; Android 10+ 64-bit devices should ship Vulkan 1.1 ([AOSP](https://source.android.com/docs/core/graphics/implement-vulkan)); Google calls it the primary API, with less CPU in the driver; one path | A broken driver means no game on that phone |
| Vulkan with a GLES fallback (`wgpu` feature `gles` added in our crate) | Survives bad Vulkan drivers, which Google warns about on older devices | Bigger binary, a second path to test; we would need a rule to pick it |
| GLES only | Closest to the WebGL2 dev build | More CPU per draw call, no reason to choose it |

**Recommendation: Vulkan only.** Only if a driver bug shows up: add wgpu's `gles` feature in our crate and a
launch parameter that picks GL. Keep **one shader and render path** for web and phone (WebGL2's limits are
a subset); the phone differs only in toggles we measure: HDR on/off (the HDR main texture exists only for WebGL2's
sRGB compositing, `main.rs`), MSAA (Bevy's default 4×; Bevy's own mobile example turns it off on Android), and the
frame-rate cap (3.7).

### 3.4 Touch control scheme (within GDR-0009)

GDR-0009 settles stick + lock. Open details and proposals (layout in 4.2):

- **Stick: floating** (recommended): the base appears where the left thumb lands in a large bottom-left zone and
  stays until release; at rest a faint ring shows the mockup's position. The alternative is the mockup's fixed
  ring (what `controls.rs` does now): predictable, but thumbs drift off it in a fight.
- **Firing: hold the FIRE button** (recommended, as in the mockup and the concept board): the lock already aims
  (GDR-0040's smart aim), so the right thumb only decides when to fire, which also manages heat (GDR-0022).
  Alternative: auto-fire at the locked target when guns bear; less thumb work, but it takes the heat decision
  away. A setting can add it later.
- **Lock**: tap a ship (finger slack 24 dp, *placeholder*); tap empty space unlocks (as now). **Next target**: a
  tap on the target panel (top right, near the right thumb) cycles to the next nearest, like Tab. The concept
  board names target cycling as stick + lock's weak spot, so this is in the first touch track.
- **Back gesture/button** = Esc: closes a layer, leaves a screen, or opens the pause menu in flight.
- **Pause menu** (Back, or a small ring at the top): RESUME, DRYDOCK (T), DRIFT on/off (X; a player setting per
  GDR-0009), WAVES (C), RESTART (Shift+R), HUD (H), TUNING (U, dev builds).
- **Vessel switching**: taps on fleet-bar chips or friendly ships (exists). **Zoom**: pinch with two fingers that
  are not on the stick or FIRE (exists).

### 3.5 Player storage on Android

`storage.rs`'s `FileBackend` already writes `<root>/<namespace>/<name>.ron` atomically; only the root is missing
(`data_dir()` returns `None` on Android). android-activity gives both roots without JNI
(`AndroidApp::internal_data_path()`, `external_data_path()`).

| Option | For | Against |
|---|---|---|
| **App-internal files dir** (`/data/data/<app>/files`) | Private, no permission; Android's Auto Backup includes it (25 MB per app; a design is 2–250 kB) ([Auto Backup](https://developer.android.com/identity/data/autobackup), [app-specific storage](https://developer.android.com/training/data-storage/app-specific)) | Removed on uninstall (unless restored from backup); agents read it only from debuggable builds (`adb shell run-as`) |
| App-specific external dir (`Android/data/<app>/files`) | Reachable over USB file transfer and `adb pull`; also backed up | Also removed on uninstall; partly hidden from file managers on newer Android |
| A folder the player picks (Storage Access Framework) | Survives uninstall, easy to copy | System picker on first start, document URIs instead of paths, much JNI |

**Recommendation: internal files dir** with Auto Backup on (the default). Exports leave the phone through the
**share sheet** (the `.ron` text or the share code, `Intent.ACTION_SEND`) and the **clipboard**
(`ClipboardManager`), both called from Rust through JNI (the `jni` crate is already in the tree through
android-activity). Measurement output (hash logs, LOADTEST lines) goes to the external files dir so `adb pull`
gets it from any build (to confirm on his phone; logcat is the fallback). `bevy_clipboard` 0.20 is no help: on
Android it is an in-process buffer ([lib.rs](https://docs.rs/crate/bevy_clipboard/0.20.0/source/src/lib.rs)).

### 3.6 How builds reach the phone

The VM is `10.77.9.2` on a network the host reaches; whether the phone reaches it is unknown. Wireless debugging
needs Android 11+ and the same network; discovery is mDNS, which doesn't cross routers
([adb](https://developer.android.com/tools/adb)).

| Option | For | Against |
|---|---|---|
| **adb in the VM over Wireless debugging (ADR-0007); if the VM can't reach the phone, adb on the host as a relay** (`adb -a nodaemon server` on the host, `adb -H <host>` in the VM: [adb(1)](https://man.archlinux.org/man/adb.1)) | Agents install, start with parameters, read logcat, take screenshots (`adb exec-out screencap -p`) and pull measurements without Niklas | One pairing; the relay needs platform-tools (a few MB) on the host and port 5037 open to the VM only |
| Host adb only (USB cable or Wi-Fi), VM scripts talk to it | Works on any VM network; a cable keeps the phone charged (but warmer) | Niklas starts the adb server after each host reboot |
| No adb: the phone downloads the APK from the VM's web server | Nothing to install | Only if the phone reaches the VM; no logs, no measurements, every install by hand |

**Recommendation: the first row.** Track A1 checks the route first (`adb pair` / `adb connect` from the VM).

### 3.7 Smaller gaps (plan defaults unless Niklas objects)

- **Entry point**: the `game` crate becomes lib + bin (`lib.rs` holds today's `main()` as `pub fn run()`;
  `main.rs` calls it; an Android-only `#[bevy_main]` calls it too). The `.so` is built with
  `cargo ndk … rustc --lib --crate-type cdylib`, so `Cargo.toml` needs no `crate-type` change and host and wasm
  builds stay as they are (fallback, as in Bevy's example and the audit: `crate-type = ["rlib", "cdylib"]`).
- **Window**: `WindowMode::BorderlessFullscreen`, not resizable; manifest `screenOrientation="sensorLandscape"`,
  `resizeableActivity="false"` and `configChanges` covering orientation, size, density, keyboard and uiMode, so
  the activity is never recreated: Bevy hangs on activity recreation today (draft fix [#26015](https://github.com/bevyengine/bevy/pull/26015)).
- **Background**: Bevy 0.20 drops the surface on suspend and recreates it on resume
  ([bevy_winit state.rs](https://docs.rs/crate/bevy_winit/0.20.0/source/src/state.rs); the resume panic of
  [#25682](https://github.com/bevyengine/bevy/issues/25682) was fixed by [#25684](https://github.com/bevyengine/bevy/pull/25684),
  which is in 0.20.0: [view/window/mod.rs](https://docs.rs/crate/bevy_render/0.20.0/source/src/view/window/mod.rs)),
  but can't run in the background ([#9057](https://github.com/bevyengine/bevy/issues/9057)). So on
  `AppLifecycle::WillSuspend` the game pauses the sim and writes pending autosaves. Today they are flushed only on
  `AppExit` (`drydock/blueprints.rs` `save_on_exit`), which a phone rarely sends: Android usually kills a
  backgrounded app without warning.
- **Frame rate**: 90/120 Hz phones run Fifo at the panel rate unless told otherwise; ask for 60 with
  `ANativeWindow_setFrameRate` after each surface creation ([frame rate](https://developer.android.com/media/optimize/performance/frame-rate);
  `ndk` 0.9 `NativeWindow::set_frame_rate`, API 30). The 120 Hz rate is useful for measuring (5.5).
- **Text input**: `Window::ime_enabled` → winit `set_ime_allowed` → `show_soft_input` on Android. With
  NativeActivity the soft keyboard sends key events and winit 0.30.13 fills `KeyEvent::text`, so `ui_kit`'s text
  fields should work for ASCII (android-activity's README: "basic ascii input"; to verify on the phone); accented
  letters can get lost ([winit #4067](https://github.com/rust-windowing/winit/issues/4067)).
  With GameActivity, winit 0.30.13 drops the keyboard's `TextEvent` entirely and leaves insets as a TODO
  ([android/mod.rs](https://docs.rs/crate/winit/0.30.13/source/src/platform_impl/android/mod.rs)). Names are ASCII
  slugs anyway; the behavior **TEXT view is read-only on the phone** in this phase (scroll, copy), editing is the
  graph's job.
- **Safe area**: a `SafeArea` resource in dp; *placeholder* margins (32 dp on the long edges, 16 dp top and
  bottom) until a JNI read of `WindowInsets` (cutout + system bars) replaces them. The status bar is hidden by a
  fullscreen theme or `AndroidApp::set_window_flags` (to verify under Android 15's edge-to-edge rules); the
  gesture handle stays visible (hiding it needs a UI-thread call).
- **Launch parameters**: today `GAME_PARAMS` (env) natively and the URL on the web (`params.rs`). On Android:
  an intent extra, `adb shell am start -n <app>/android.app.NativeActivity --es params 'load=20&seed=7'`, read
  through JNI.
- **Logs**: `bevy_log` writes to logcat on Android; `sim_bridge::console_out` uses `println!`, which goes nowhere
  on Android, so machine-readable lines go to logcat and to a file in the external files dir.
- **App id and icon**: id `dev.spacegame.game` (*placeholder*, choose before any store); launcher icon as an
  adaptive icon made of vector drawables (XML paths from an SVG in `art/`), so no raster file is committed (ADR-0021).
- **Threads**: the game builds Bevy without `multi_threaded`, so update and render share the main thread. The
  audit recommends turning it on for native (R6: parallel executor, pipelined rendering; risk: hidden system-order
  ambiguities, one frame more latency). Measure both on the phone and keep the faster.
- **16 KB pages**: Android 15+ devices may use 16 KB pages; NDK r28+ aligns by default, uncompressed `.so` files
  need `zipalign -P 16` ([page sizes](https://developer.android.com/guide/practices/page-sizes)). The build script
  checks both, and that no build script compiled C or C++ for the Android target (5.4).

### Records to write once decided

- ADR (proposed): *Android build: NativeActivity, cargo-ndk, scripted APK packaging, arm64 only, minSdk 31,
  targetSdk 36, Vulkan only.*
- ADR (proposed): *Android platform glue is Rust: JNI from Rust for clipboard, share sheet, intent parameters and
  insets; no Java, Kotlin or C++.*
- ADR (proposed, amends ADR-0031's Android line): *Android storage root is the app-internal files dir; exports by
  share sheet and clipboard; measurement output in the external files dir.*
- ADR (proposed): *Compact layout class by window size (height under 500 dp, placeholder), used on any platform.*
- GDR (proposed, refines GDR-0009): *Touch flight v1: floating stick, hold-to-fire, tap the target panel for the
  next target, Back opens the pause menu.*
- GDR (proposed): *Phone layouts of the drydock and the behavior editor* (4.3, 4.4).

## 4. Design proposal: touch controls and phone layouts

### 4.1 Common rules

- **Units**: on Android winit's scale factor is `densityDpi / 160` ([android/mod.rs](https://docs.rs/crate/winit/0.30.13/source/src/platform_impl/android/mod.rs)),
  so Bevy's logical pixels are Android dp. The mockups are 844 × 390 CSS px (`ui-mockups/source/*.dc.html`), which
  is a phone's landscape size in dp (current flagships are about 830–915 × 384–412 dp). All sizes below are dp.
- **Touch targets**: at least 48 × 48 dp ([Android accessibility](https://developer.android.com/guide/topics/ui/accessibility/apps#large-controls)).
  Where the mockup draws smaller pills (about 28–31 dp high), the hit area grows invisibly (`ui_kit` hit slop).
- **Compact layout class**: chosen by window size, not by platform: height under 500 dp (*placeholder*) uses the
  phone layouts, laid out for 844 × 390 and scaled by min(w / 844, h / 390) clamped to 0.9–1.3 (*placeholder*);
  larger windows keep today's 1280 × 720 layouts. Today both screens fit 1280 × 720 and only scale up
  (`drydock/view.rs` `ui_scale`, `behavior_ui/mod.rs` `fit_window`, clamp 1–2), so on a phone they would not fit.
  Because the class follows the window, the phone layouts can be built and screenshot-checked in the web build at
  844 × 390 (headless, with `--force-device-scale-factor 3`, `docs/client.md` "Headless HiDPI").
- **Safe area**: every screen's edge widgets sit inside `SafeArea` (3.7).
- **No hover on touch**: everything a hover shows on desktop (ghosts, highlights) shows while the finger is down.
- **Lists scroll by dragging** (today only the wheel and the scrollbars' thumbs scroll; since 14 Oct 2026 the kit
  owns the wheel for every scrolling area in `ui_kit/scrollbar.rs`, so a touch drag belongs there too: at once under
  the finger, a fling could reuse its `Glide`).

### 4.2 Flight HUD and touch flight

From `phone-flight-hud.png` (positions from `Flight.dc.html`) and GDR-0008:

| Where | What | Size (*placeholder* where not from the mockup) |
|---|---|---|
| Top left | HULL % and hull map (exists); AMMO / FUEL / REPAIR when those exist | as now |
| Left, middle | speed and TOP (exists; already ends at mid-height to clear the stick) | as now |
| Bottom left | **floating stick zone**: left 45 % of the width, below the speed block; the base appears under the thumb | ring 124 dp (mockup: left 26, bottom 24), knob 44 dp, dead zone 6 % of the radius, full speed at the rim |
| Bottom right | **FIRE**, hold to fire; its ring shows the heat of the firing guns | 76 dp (mockup: right 32, bottom 32), hit area 96 dp |
| Around FIRE | slots for overdrive, repair drones, scan, autopilot (mockup); hidden until the features exist | 44 dp each |
| Top right | target panel (exists); a tap cycles to the next target; ENGINES / WEAPONS priority chips later | panel hit area at least 48 dp high |
| Top centre | scene / wave line, fleet bar (exists, chips 68 × 50) and a **pause ring** left of the title | ring 36 dp, hit 48 dp |
| Bottom centre | warnings (exists); the keyboard help line is hidden on touch | – |
| Mockup's pencil | "Edit HUD layout" (GDR-0008 customisable HUD): later | 30 dp |

Behaviour: the touch widgets are on from the start on Android (today they wait for the first touch,
`controls.rs` `spawn_touch_ui`). A touch that starts on a widget or a panel never reaches the world
(`UiPointer::blocked` in `ui_kit.rs` checks only the mouse cursor today). Tap slack for locking grows with the
finger (24 dp instead of the mouse's 20 px + 2 m).

### 4.3 Drydock (phone)

From `phone-drydock.png` (2× PNG of 844 × 390):

- **Top**: ring menu button (36 dp, hit 48), breadcrumb and name (a tap renames: soft keyboard); ARMOR pills and
  OVERLAY pills on the right. The mockup shows three of each; we have five overlays (HEAT, MASS, BONUS, SHIELDS,
  ARCS), so the overlay row scrolls sideways or folds into an OVERLAYS pill with a popover.
- **Left (about 150 dp)**: the part palette as one scrolling list with category headers (HULL, POWER, CARGO, …)
  instead of the desktop's category pills; search behind a lens button.
- **Centre**: the grid. One finger = the current tool (paint strokes, the fill rectangle, erase strokes, select by
  tap; `tools::Gesture` is already device-free). Two fingers = pan and pinch zoom; a stroke that turns into a
  two-finger gesture within 120 ms (*placeholder*) is cancelled, not committed. Big parts are dragged from the
  palette onto the grid ("HOLD 2×2 · DROP TO PLACE" in the mockup) with the ghost shown 48 dp above the finger
  (*placeholder*) so the thumb doesn't hide it. Cells at "fit" are about 12 dp on a phone (24-cell frame in about
  300 dp), too small for a finger: the drydock opens zoomed to about 28 dp per cell (*placeholder*) and a
  double tap toggles fit.
- **Tools**: the mockup shows no tool row; proposal: a narrow vertical strip at the grid's left edge (PAINT, FILL,
  ERASE, SELECT) and a small floating bar beside a selected part (ROTATE ↺ ↻, FLIP, DELETE).
- **Right (about 150 dp)**: the stats column as in the mockup (POWER, PEAK HEAT, THRUST/MASS, YAW, FIREPOWER,
  CARGO, MIND, DEVICE LOAD). A selected part's inspector replaces it (with a back chevron); there is no room for
  both.
- **Bottom**: undo, redo, MIRROR, BEVEL, the status line, TEST-FLY (primary). BEHAVIOR, BLUEPRINTS, EXPORT and
  SAVE move behind the ring menu button (autosave covers most saving).
- **BLUEPRINTS and EXPORT**: full-screen sheets; EXPORT gets SHARE (share sheet) and COPY CODE; IMPORT gets
  PASTE (clipboard) instead of the file chooser.

### 4.4 Behavior editor (phone)

From `phone-behavior-editor.png`, which the desktop screen already follows:

- **Top**: ring menu button, breadcrumb and name, RULE SLOTS pill, GRAPH / TEXT, TEST IN RANGE (primary) with the
  WATCH / FLY ALONGSIDE choice in a small popover.
- **Left (about 600 dp)**: the graph. One-finger drag on empty space scrolls; the mockup's + / − rings (32 dp, hit
  48) and pinch zoom it. A tap selects a chip or a rule. **Reordering**: long-press a row (350 ms, *placeholder*)
  then drag, because a plain drag scrolls (today's 8 px drag in `behavior_ui/reorder.rs` would fight scrolling).
- **Right (about 200 dp)**: inspector (kicker, name, values with sliders and pills; slider thumbs get 48 dp hit
  areas) and ADD NODE below, as in the mockup.
- **Bottom**: the mockup has no bottom bar; DRYDOCK, BEHAVIORS and SAVE go behind the ring menu button, undo and
  redo sit beside + / −, the status line shows over the graph's bottom edge.
- **TEXT**: read-only on the phone in this phase (3.7).

### 4.5 Not in this phase

The home ring menu, the galaxy map and HUD editing (their mockups exist; the features don't), gamepads, a
left-handed mirror of the HUD, tablets (they get the desktop layout by size), audio.

## 5. Technical plan

### 5.1 Build setup in the VM (no sudo)

Checked on 13 Oct 2026: 17 GB free on `/home`, JDK 21 at `/usr/bin/java` (apksigner and keytool need a JDK),
`unzip`, `curl` present; no Android SDK, no `adb`, no Android Rust target.

| Piece | How | Disk |
|---|---|---|
| Rust target | `rustup target add aarch64-linux-android` | ~0.2 GB (*estimate*) |
| cargo-ndk | `cargo binstall cargo-ndk` (4.x) | small |
| SDK command-line tools | `commandlinetools-linux-…_latest.zip` (181.8 MB, sha-256 on [the download page](https://developer.android.com/studio#command-line-tools-only)) into `~/android-sdk/cmdline-tools/latest` ([sdkmanager](https://developer.android.com/tools/sdkmanager); Google now also offers an `android sdk` CLI) | ~0.2 GB |
| SDK packages | `sdkmanager "platform-tools" "build-tools;36.0.0" "platforms;android-36" "ndk;30.0.16248370"`, licenses accepted once (Niklas agrees to Google's licenses) | ~0.4 GB without the NDK (*estimate*) |
| NDK r30 (LTS) | 739 MB zip ([NDK downloads](https://developer.android.com/ndk/downloads)), unpacked about 2–2.5 GB (*estimate*, measure with `du`) | ~2.5 GB |
| Debug keystore | `keytool` into `~/.android/debug.keystore` | tiny |
| Android target dir | `target/android` (one fixed dir; worktree agents use `target/<name>-android` and delete it) | 2–4 GB (*estimate*) |

Total about 5–7 GB (*estimate*), the NDK and the target dir the bulk; Gradle (option B or C) would add about
1–1.5 GB. The setup script refuses to start with less than 8 GB free and deletes the zips after unpacking.

### 5.2 Cargo and cfg gates

- `crates/game/Cargo.toml`:
  - Bevy feature `android-native-activity` in the main list (audit R2; if the lead applies R1/R2 earlier, this is
    already done). It changes nothing on wasm or desktop.
  - `blake3` with `pure` for non-wasm targets (audit R11), so the aarch64 build compiles no C.
  - `[target.'cfg(target_os = "android")'.dependencies]`: `jni` (pinned; 0.21.1 and 0.22.4 are already in
    `Cargo.lock` through android-activity and its deps) and `ndk` (`=0.9.0`, in the lock too; features
    `nativewindow`, `api-level-30`) for the frame-rate hint.
  - `arboard` is already excluded on Android; bevy_egui excludes its clipboard there
    ([Cargo.toml](https://docs.rs/crate/bevy_egui/0.43.1/source/Cargo.toml)), so egui on the phone has no paste
    of its own. Audit R10 (drop `manage_clipboard`, feed pastes from the game's `Clipboard` into egui) fits
    Android too: one clipboard path, with the JNI backend underneath on the phone.
  - `web-sys` and `js-sys` are wasm-only. `Cargo.lock` changes go to the lead.
- No `.cargo/config.toml`: the build script passes `-C link-arg=-Wl,-z,max-page-size=16384` in `RUSTFLAGS` for
  the Android target only, to be explicit about 16 KB pages.
- `sim` and `data` need no change: Rapier's `enhanced-determinism` turns on parry's
  `glamx/scalar-math` and `glamx/libm`, so no NEON code in the physics ([parry2d Cargo.toml](https://docs.rs/crate/parry2d/0.31.1/source/Cargo.toml));
  Rust's float operations are exact IEEE 754 apart from NaN bits ([RFC 3514](https://rust-lang.github.io/rfcs/3514-float-semantics.html)),
  so no silent FMA; Rhai has no platform-specific dependency besides wasm's `web-time`
  ([Cargo.toml](https://docs.rs/crate/rhai/1.26.1/source/Cargo.toml)); `aarch64-linux-android` is a tier-2 target
  with std ([rustc](https://doc.rust-lang.org/rustc/platform-support/android.html)). The audit confirms the sim's
  crates get the same features inside the game as alone; its notes: libm's `arch` is on through num-traits
  (harmless, exactly rounded operations only), and nalgebra's `matrixmultiply` picks FMA kernels at runtime but
  only for multibody joints, which the sim doesn't use. The hash check still has to prove it on aarch64 (5.5).

### 5.3 Code that must change

| File | Change |
|---|---|
| `crates/game/Cargo.toml` | `android-native-activity` (R2), blake3 `pure` (R11), Android-only `jni` and `ndk` (5.2) |
| `crates/game/src/main.rs` → new `lib.rs` | App setup moves to `pub fn run()`; Android `#[bevy_main]`; window mode fullscreen on Android; `Hdr` and MSAA become measurable switches (`hdr=0`, `msaa=0`), Hdr stays on for WebGL2 |
| `crates/game/src/params.rs` | Android: parameters from the intent extra `params` (JNI) |
| `crates/game/src/storage.rs` | `data_dir()` on Android: `ANDROID_APP.internal_data_path()`; the rest of `FileBackend` stays |
| `crates/game/src/clipboard.rs` | Android `platform` module: `ClipboardManager` through JNI (copy and a PASTE button; Android 10+ reads the clipboard only while the app has focus); with R10, egui's pastes come from here too |
| `crates/game/src/drydock/share.rs` | `write_export` on Android: share sheet; file import becomes paste |
| `crates/game/src/drydock/blueprints.rs`, `behavior_ui/saving.rs` | Flush autosaves on `AppLifecycle::WillSuspend`, not only on `AppExit` |
| new `crates/game/src/platform/android.rs` (*name placeholder*) | Lifecycle (pause and resume the sim's clock), `SafeArea` (placeholder margins, then JNI insets), 60 Hz frame-rate hint after each resume, thermal headroom (`AThermal_*` from `libandroid` through `extern "C"` declarations in Rust, API 31), Back key (`NamedKey::BrowserBack` in winit 0.30.13) mapped to the game's Esc |
| `crates/game/src/sim_bridge.rs` | `console_out` on Android: logcat plus the measurement file |
| `crates/game/src/controls.rs` | Floating stick, FIRE in dp from the safe area, tap slack, next target from the target panel, touch widgets on from the start on Android |
| `crates/game/src/flight_hud.rs`, `hud.rs` | Phone HUD positions inside the safe area, pause ring and menu, help line off on touch; F3 overlay gains thermal headroom and the atlas tier |
| `crates/game/src/ui_kit.rs` | `UiPointer::blocked` also for touches; hit slop to 48 dp; drag-to-scroll; soft keyboard on focus (`Window::ime_enabled`) |
| `crates/game/src/drydock/{view,tools,panels,inspector}.rs` | Compact layout; touch gestures (one finger tool, two fingers pan and zoom, drag from the palette, cancel on a second finger); default zoom for touch |
| `crates/game/src/behavior_ui/{mod,screen,reorder,controls,text_view,list}.rs` | Compact layout; drag scroll; long-press reorder; slider hit areas; TEXT read-only on the phone |
| `crates/game/src/atlas.rs` | Nothing at first; log tier and need on the phone. At a phone's 2.6–3.5 scale factor tier 512 (84 MB, plus a second copy during the switch) comes quickly; cap it if memory or rebuild time hurts (5.5) |
| new `crates/game/android/` | `AndroidManifest.xml` (NativeActivity, `hasCode="false"`, `lib_name` = game, sensorLandscape, configChanges, `isGame`, Vulkan 1.1 feature, profileable), `res/` (theme with the cutout mode, adaptive vector icon) |

### 5.4 Scripts (local only, ADR-0008; every wait bounded with `timeout`)

| Script | What it does |
|---|---|
| `scripts/android-setup.sh` | One-time: checks free disk, downloads the command-line tools (sha-256), installs the pinned SDK packages into `$ANDROID_HOME` (default `~/android-sdk`), adds the Rust target, installs cargo-ndk, makes the debug keystore; prints what it used |
| `scripts/android-env.sh` | Sourced by the others: `ANDROID_HOME`, NDK path, `adb` (or `adb -H <host>` for the relay) |
| `scripts/android-build.sh [--release] [--out FILE]` | `cargo ndk -t arm64-v8a -P 31 rustc -p game --lib --crate-type cdylib` into `target/android`; `aapt2 link` (manifest, res, `android.jar` of API 36, min 31 / target 36), the `.so` stored uncompressed, `zipalign -P 16 -f 4`, `apksigner`; checks ELF alignment (`llvm-objdump -p`, `align 2**14`) and `zipalign -c -P 16`, and fails if a build script left C or C++ objects for the Android target; prints the size |
| `scripts/android-run.sh [--params 'load=20'] [--seconds N] [--shot FILE]` | `adb install -r`, `am start` with the params extra, logcat of the app for N seconds, optional `screencap`; exit 3 on a panic or `FATAL` in the log |
| `scripts/android-sim.sh [--scenario S] [--seed N] [--ticks N]` | Builds the sim examples `determinism`, `load_test` and `sim_perf` as aarch64 executables, `adb push` to `/data/local/tmp`, runs them, compares hashes with the native stream through `scripts/determinism-check.sh --compare` |
| `scripts/android-measure.sh` | The measurement matrix of 5.5: runs each case, pulls the LOADTEST and hash files from the external files dir, snapshots `dumpsys battery` and `dumpsys thermalservice` before and after, prints markdown tables into `target/android-measure/<date>/` |

### 5.5 Phone measurements (spike questions 2, 4–7)

All on the release APK (`profileable`, not `debuggable`; the same build for every run), phone at room temperature, unplugged (adb over Wi-Fi), brightness fixed, nothing else running.
Each case three times; median and p95 like the VM rules (CLAUDE.md).

| # | Question | How |
|---|---|---|
| 2 | Same hashes on the phone? | (a) `android-sim.sh`: `determinism --scenario core\|flight\|load\|behavior --seed 42\|7 --ticks 1800` on the phone against x86_64, the same table as the spike summary; (b) the APK with `hashlog=1&load=20&hashticks=600` (the browser's step C), file pulled and compared. No APK is needed for (a), so it comes first |
| 4 | Fast enough? | Sim alone: `load_test` on the phone (1, 5, 20 ships, 1800 ticks) against the VM's 0.011 ms median for 20. Whole game: `load=1\|5\|20`, the existing 60 s LOADTEST line. To see "half the frame budget" through vsync: run once **uncapped at the panel's 120 Hz**: holding 120 fps at 20 ships means at most 8.3 ms per frame, which is the ADR-0004 rule; at 60 Hz the frame interval only says "fits". GPU detail, if needed, from a Perfetto trace viewed in the host browser |
| 4 | Temperature after 10 minutes | `load=20` at the real 60 Hz cap for 10 minutes: thermal headroom every 10 s (NDK API, at most once per 10 s as Google advises) and frame p95 per minute in the log; battery temperature and `dumpsys thermalservice` before and after |
| 5 | Browser vs phone | Put the phone column next to the browser numbers once Niklas has sent them (`results/spike-summary.md` steps A–B) |
| 6 | Look cost | The browser's step B cases on the phone (`debug=1`, `glow=0`, `bloom=1`, `glow=0&bloom=1`), plus the phone switches `msaa=0`, `hdr=0`, `fps=60\|120` and later `multi_threaded`; each uncapped at 120 Hz for headroom |
| 7 | Flight feel | Niklas flies the sandbox for a few waves; checklist like the browser's step D plus touch items: stick (floating), braking on release, lock by tap, next target by the panel, FIRE reach, pinch zoom, HUD readable at arm's length. Answers become the GDR updates of 3.4 |

Results go to `docs/results/phone-*.md` (with screenshots from `screencap`, which are allowed in `docs/`) and into
`results/spike-summary.md` (lead). The physics benchmark's phone column (question 1) is covered by the game's own
load test, as the spike summary already says for the browser.

## 6. Tracks

Track ids use the prefix A (Android); the lead renumbers them with the phase (Phase 7 is the first playable
loop, GDR-0062). Size: S = part of a session, M = one agent session, L = several sessions or waves.

| Track | Goal | Depends on | Size | Parallel with |
|---|---|---|---|---|
| **A1 Toolchain and phone link** | `android-setup.sh`, `android-env.sh`; SDK, NDK, target, cargo-ndk, keystore in `~`; adb reaches the phone (directly or by the host relay) | Niklas's answers (phone, toolchain, install path) | S | A7, A8 (web side) |
| **A2 Sim on the phone** | `android-sim.sh`: phone hashes for all scenarios and seeds (Q2) and the sim-only load test (Q4, sim half), no APK; `sim` has no Bevy, so this works before the game compiles for Android | A1 | S | A3 |
| **A3 First APK: "it runs on the phone"** | `android-native-activity` and blake3 `pure` (audit R2, R11, unless the lead applied them earlier), lib entry with `#[bevy_main]`, manifest and vector icon, `android-build.sh`, `android-run.sh`; the sandbox flies with today's touch widgets, survives Home and back, stays landscape | A1 | M | A2, A7, A8 |
| A4 Platform layer | Storage root, lifecycle (pause, autosave on suspend), Back key, intent params, logs to logcat and file, safe area, 60 Hz hint, clipboard and share sheet by JNI, thermal headroom | A3 | M | A5, A7, A8 |
| A5 Touch flight | Floating stick, FIRE, tap slack, next target, pause menu, phone HUD in the safe area; first in the web build at 844 × 390 with `?touch=1`, then on the phone | A3 (phone checks), A4 (safe area, Back) | M | A6 (after its first runs), A7, A8 |
| A6 Phone measurements | `android-measure.sh`; Q2 (APK), Q4, Q6, Q7 with Niklas; `docs/results/phone-*.md` | A2, A3, A4; A5 for Q7 | M | A7, A8 |
| A7 Drydock on the phone | Compact layout class, `ui_kit` touch fixes (blocked, hit slop, drag scroll, soft keyboard), the phone drydock of 4.3, sheets for BLUEPRINTS and EXPORT | layout work: none (web build); device checks: A3, A4 | L | A5, A6, A8 after the `ui_kit` part |
| A8 Behavior editor on the phone | The phone layout of 4.4, drag scroll and zoom, long-press reorder, TEXT read-only | A7's layout class and `ui_kit` fixes | M | A5, A6 |
| A9 Follow-ups from the numbers | Only if A6 asks for them: MSAA or HDR off on Android, atlas tier cap, `multi_threaded`, thermal step-down, GLES fallback | A6 | S–M | – |

The minimal "it runs on the phone" slice is **A1 → A2 and A3**: hashes and sim times from the phone within one
session, then the sandbox on the phone. A7 and A8 can start at once in the web build, since the compact layout
follows the window size.

## 7. Risks and what to try first

| Risk | Try first |
|---|---|
| The VM can't reach the phone (ADR-0007 assumes it can) | A1 starts with `adb pair` / `adb connect` from the VM; else the host relay (3.6) |
| The game has never compiled for Android: today `android-activity` stops at a `compile_error!` (no activity feature), and other surprises may hide behind it | A3's first step is only the two manifest changes (R2, R11) and a `cargo ndk … rustc` of the plain game; fix compile errors before anything else |
| Bevy 0.20 with NativeActivity has rough edges (Bevy calls it legacy) | A3 with the plain sandbox before any phone UI work; if it fails in Bevy's code, option C/B (Gradle; B needs Niklas's word on the C++ glue) |
| C sneaks into the build through a dependency (blake3's NEON C today; any new crate with a `cc` build script) | blake3 `pure` (R11); `android-build.sh` fails when a build script left C or C++ objects (`*.o`, `*.a`) under the Android target's `build/` dirs |
| Activity recreation hangs Bevy ([#26015](https://github.com/bevyengine/bevy/pull/26015)); background is not supported ([#9057](https://github.com/bevyengine/bevy/issues/9057)) | Landscape lock and full `configChanges`; test Home, power button, notification shade, the recent-apps screen, and process death (does the design come back from storage?) |
| Our custom WESL shaders (atlas with LOD bias, shield bubbles with `fwidth`, dashed outlines) have never run on a mobile Vulkan driver | First APK run: `screencap` of the showcase and the drydock next to the web build's screenshots |
| Rendering, not the sim, is the phone's limit: HDR main texture, MSAA 4×, many blended glow layers, 120 Hz panels | 60 Hz hint from the start; measure `msaa=0`, `hdr=0` and glow modes in A6 before changing any default |
| Atlas tier 512 at phone DPI: 84 MB per atlas and about 0.45 s of rasterising on the VM (more on a phone) | Log tier and rebuild times on the phone; cap at 256 if needed (ADR-0021 allows choosing the resolution the display needs) |
| aarch64 hashes differ | A2 first, before any UI work; the hash tools name the first divergent tick |
| Hand-made APK packaging (alignment, signing, uncompressed libs) | Checks inside `android-build.sh`; cargo-apk2 or Gradle as fallback |
| Soft keyboard: only ASCII through key events; accents can drop | Names and search are ASCII anyway; TEXT view read-only on the phone; GameActivity's IME only if Niklas wants rich text later |
| Touches on panels leak into the world (`UiPointer::blocked` is mouse-only) | Fix in `ui_kit.rs` early in A5/A7 |
| Thermal throttling in the 10-minute run | Measure headroom; a step-down (lower frame rate or glow) is A9's job, only with numbers |
| Disk: 17 GB free, the Android setup takes 5–7 GB (*estimate*) | One shared `target/android`; delete zips; `df -h /home` in the setup script |
| Single-threaded Bevy on slower phone cores | Measure with and without `multi_threaded` in A6 (audit R6 recommends it for native); the default follows the numbers in A9 |
| `enhanced-determinism` makes all of glam scalar, Bevy's math in the client included (audit: unavoidable while Bevy and Rapier share glam) | Nothing to do; it is part of the phone's measured cost |

## 8. Questions for Niklas

Asked interactively later, one at a time; each stands on its own.

1. **Which phone do we build and measure for first?**
   - Your current flagship; please add its model and Android version (Recommended) — quickest start; the GPU family (Adreno, Mali, Xclipse, PowerVR) tells us which driver bugs to expect.
   - Your flagship plus an older or mid-range phone you have — real mid-range numbers for ADR-0004 instead of the headroom rule, but twice the installs.
   - A spare phone first, the flagship later — no risk to your daily phone, but slower GPU and unknown Vulkan support.

2. **How should the Android build be made?**
   - NativeActivity, cargo-ndk and a short packaging script, no Gradle (Recommended) — all Rust, no C++, no Java, smallest download; Bevy calls NativeActivity legacy and we maintain the script.
   - GameActivity with Gradle, like Bevy's default example — the path Bevy and Google recommend, better keyboard later; compiles Google's C++ glue, needs Java and about 1–1.5 GB more.
   - NativeActivity with Gradle (Bevy's second example) — Bevy's own layout and an easy switch later; Gradle's disk and build time for no gain now.

3. **Which Android versions should the game run on?**
   - Android 12 and newer (API 31) (Recommended) — about 69 % of devices, covers updated 2021 mid-range phones, gives the thermal API for the temperature test.
   - Android 10 and newer (API 29) — about 91 % of devices; older Vulkan drivers to test.
   - Android 8 and newer (API 26) — about 98 %; needs a GLES fallback for weak Vulkan drivers.

4. **Which graphics backend on Android?**
   - Vulkan only (Recommended) — Bevy's default, one path to test; a phone with a broken Vulkan driver can't run the game.
   - Vulkan with a GLES fallback built in — survives bad drivers; bigger build and a second path to test.
   - GLES only — closest to the WebGL2 dev build; more CPU per frame for no gain on modern phones.

5. **How should builds get from the VM to your phone?**
   - adb from the VM over Wireless debugging (ADR-0007), with adb on your host as a relay if the VM can't reach the phone (Recommended) — agents install, start, read logs and measure on their own; one pairing, maybe a small host setup.
   - Only adb on your host (USB cable or Wi-Fi), the VM's scripts talk to it — works on any VM network; you start the adb server after host reboots.
   - No adb: you download the APK in the phone's browser — nothing to set up; no logs or measurements by agents, every install by hand.

6. **Where should your designs and behaviors live on the phone?**
   - The app's private storage, backed up by Android's Auto Backup; export by share sheet and clipboard (Recommended) — nothing to set up, private; gone on uninstall unless restored from the backup.
   - The app's folder in Android/data — visible over USB and easy for adb to copy; also gone on uninstall, hidden by some file managers.
   - A folder you pick once (for example Documents/space-game) — survives uninstall, easy to copy; a system picker on first start and much more Android code.

7. **Should the touch stick be floating or fixed?**
   - Floating: it appears where your left thumb lands in the bottom-left area (Recommended) — forgiving in a fight, common in mobile action games; its position on screen moves.
   - Fixed at the mockup's place, bottom left — always in the same spot; thumbs slip off its ring.
   - Both, as a setting (floating by default) — suits everyone; one more setting to build and test.

8. **How should firing work on touch?**
   - Hold the FIRE button; the lock aims the guns (Recommended) — as in the mockup, you decide when to fire and manage heat; the right thumb is busy.
   - Fire automatically at the locked target when the guns bear and it is in range — less thumb work; firing and heat are no longer your call.
   - Hold FIRE by default, auto-fire as a setting — both styles; one more setting.
