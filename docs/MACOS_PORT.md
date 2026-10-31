# RoALT for macOS

Native Apple Silicon Tauri/WKWebView build retaining the real account manager, Roblox APIs, SQLCipher database, encrypted backups, master passwords and recovery codes.

## Implemented Mac features

- **Multiple Roblox processes:** launch the installed, unchanged RobloxPlayer executable directly with `-protocolString`. Before a multi-instance launch, unlink its named `/RobloxPlayerUniq` semaphore. Existing clients retain their handles and remain running. Launches are serialized; each returned PID and Unix creation time identifies the new client. No executable patches, injections, task ports or anti-cheat changes.
- **Shared sign-in protection:** when enabled, each new client runs under a macOS child sandbox denying reads/writes to the user's `Library/Cookies` and `Library/HTTPStorages`. The restrictions remain for that process's lifetime, even if RoALT exits. Existing Roblox processes are not changed. This isolates the known native cookie stores, not all Roblox data/preferences or logs. Apple's `sandbox-exec` is deprecated; if unavailable or rejected, protected launch fails instead of silently falling back.
- **Native process/system metrics:** public `libproc` and Mach statistics provide CPU usage, resident memory, physical footprint and disk I/O, plus system CPU/memory. System GPU utilization is read from IOAccelerator PerformanceStatistics when available; per-process GPU and VRAM counters remain unavailable. Physical footprint is the closest useful Mac memory measure; it is not Windows private bytes. CPU percentages are normalized across machine cores.
- **Background priority:** Darwin background scheduling lowers CPU, disk and network priority. It can be set per client or automatically for clients not in the foreground. This is not a hard percentage cap. It is cleared when disabled, when a live client leaves tracking, and at normal RoALT exit. A forced OS termination of RoALT can leave the client's background priority in place until that client exits or the setting is reapplied.
- **FPS cap and low graphics:** use Roblox's own `~/Library/Roblox/GlobalBasicSettings_13.xml`, preserving/restoring the user's original file around a launch. Mac performance presets contain only supported settings.
- **Cooperative CPU caps:** per-client and adaptive background percentage limits, measured as a percentage of the entire Mac CPU capacity. Separate governor processes use brief SIGSTOP/SIGCONT pauses; these may stutter and are not kernel quotas. An independent guardian checks microsecond process identity and resumes clients on parent/governor death or a stalled governor (300 ms lease). Already-suspended clients are rejected. Changing/disabling caps, removing a tracked client and closing RoALT release the governor. Memory-triggered CPU throttling uses the same mechanism; it does not cap memory allocation.
- **High-memory restart:** uses physical footprint and the existing grace/restart-limit logic; default off.
- **Window focus, minimize and restore:** target the tracked process identity. Focus uses Cocoa; minimize/restore need Accessibility permission in System Settings → Privacy & Security → Accessibility. Permission denial is reported without pretending the action succeeded.
- **Tiling and tiny windows:** Accessibility resizes/moves only tracked windows; normal-size restoration remembers original bounds. Minimized windows are skipped, and Roblox may enforce a minimum size. Layout uses the main screen’s usable area.
- **Live previews:** named-window-only ScreenCaptureKit screenshots on macOS 14+, with the legacy named-window capture API on older systems. Requires Screen Recording permission; denied access is shown on each preview. Minimized windows have no live preview. Rates default to 5 FPS (2/5/10 available) to limit capture overhead.
- **Anti-AFK (opt-in):** Focus mode briefly activates visible tracked clients and posts Space directly to that PID, then restores the prior app only if no manual input occurred. Requires Accessibility and Input Monitoring permissions. A listen-only event tap records activity timestamps, not key contents; physical/other input interrupts the round. Hidden/minimized clients are skipped. It cannot guarantee that every Roblox game accepts synthetic input; background mode posts the configured key directly to the verified PID without changing focus or window state, including minimized clients. This is experimental and does not confirm Roblox accepted the input. Both modes allow Tab (default) or Space. Focus remains the default mode until real-game verification. Optional focus fallback applies only to event-creation failures, never missing permission or recent input.
- **Accounts and vault:** isolated ephemeral Roblox login/session windows, cookie import, account metadata, validation, favorites, launch resolution, backups and recovery. The Keychain holds one random wrapping key; each encrypted database/vault record has its own nonce and authenticated purpose. Set a master password to reveal/copy cookies; Windows Hello is unavailable.

- **App Nap override (opt-in):** discovers the installed Roblox bundle ID, journals the prior `NSAppSleepDisabled` boolean, and writes `YES`. Turning it off restores the prior value, removing the key if it was originally absent. External changes are preserved. Applies on the next Roblox launch; actual App Nap effects need manual verification. Mac settings and the restoration journal stay in the default RoALT support directory even when the account database is moved.
- **System GPU display:** Home and Monitor show the whole-Mac utilization only when a valid driver statistic is available. Per-client GPU columns are hidden on Mac.

## Not implemented

Kernel-enforced CPU quotas, core affinity/spreading, arbitrary memory trimming or memory allocation caps, per-process GPU/VRAM counters, bandwidth shaping, scheduled shutdown, network classification and automatic Roblox/app installers/updates. Unsupported controls are hidden in the Mac performance/instance UI and rejected by the backend. Windows rendering flags are not written into the signed Mac app. Disabling public networks blocks launches because macOS does not expose Windows' network categories.

## Validation and remaining acceptance check

Two owned, protected Roblox processes were started together and stayed alive beside an existing user-owned Roblox process. Only the two test children were terminated/reaped; the original process remained running. No account credentials or real launch tickets were used. This verifies real process coexistence and direct client launch, **not two authenticated accounts joining games**; that final check requires signing in and launching with your accounts.

Automated checks cover real native metrics (including Mach-timebase conversion checked against POSIX CPU time), background scheduling priority decrease/restoration on an owned process, restoration when tracking ends, shutdown latching, sandbox read/write denial, vault recovery/Keychain/SQLCipher persistence, and API fixtures. The live public Roblox API test and native login-window/cancellation smoke also pass. CPU safety tests use owned busy processes and verify reduced CPU consumption, parent pipe closure, governor termination/stall recovery, refusal of already-stopped processes and stale identities. An owned Cocoa fixture verified real AX resizing, exact original-geometry restoration and ScreenCaptureKit pixels (320×207, 57,856 expected green marker pixels). With existing test-host permissions, anti-AFK detected recent manual activity and correctly aborted without delivering a key. Positive Space delivery and Roblox game acceptance remain unverified because the user was active; packaged RoALT requires its own OS grants. No authenticated game was used. No test typed into an existing user app. Per-process GPU counters are left unavailable: the public SDK process-rusage records do not expose reliable per-process GPU utilization or VRAM accounting.

## Build and install

Apple Silicon, configured macOS 11 minimum. Requires Xcode Command Line Tools, Rust 1.95+ and Node 22+ to build:

```sh
npm ci
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri -- build --bundles app
```

Bundle: `src-tauri/target/release/bundle/macos/RoALT.app`. Delivered app is ad-hoc signed; it is not Developer ID signed/notarized. Replacing an earlier build changes its signing identity: macOS may ask you to authorize RoALT to access the existing Keychain item. Approve that native prompt yourself; do not delete the vault or Keychain key. Intel/universal builds and older macOS versions are untested.

Data: `~/Library/Application Support/RoALT` (current-user permissions). Roblox logs: `~/Library/Logs/Roblox`. Migrate Windows accounts using a password-protected `.roalt` backup; Windows DPAPI databases cannot be copied directly to Mac.

On its first launch, the macOS app requests Screen Recording, Input Monitoring and event-posting access when missing, then asks macOS to show the Accessibility approval page. macOS controls these prompts; RoALT cannot approve them. Finish any grants in System Settings → Privacy & Security and restart RoALT. The initial request is recorded in the RoALT data folder so it does not recur on every launch. If you decline a permission, grant it later in System Settings.

## Implementation references

- [Toolblox background services](https://toolblox.dev/docs/app/background-services/) and [account launch documentation](https://toolblox.dev/docs/app/accounts/) describe the Mac semaphore/direct-protocol launch mechanism; the installed Roblox binary also contains `/RobloxPlayerUniq` and `-protocolString`.
- [Apple XNU resource control implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_resource.c) defines Darwin background scheduling. Verification uses actual `libproc` thread priority, avoiding the target-query problem in `getpriority(PRIO_DARWIN_PROCESS, pid)`.
- Native structures/constants are compiled against Apple's installed SDK headers: `libproc.h`, `sys/resource.h`, `sys/proc_info.h`, `mach/vm_statistics.h`, Cocoa and ApplicationServices.

- [Apple process-policy implementation](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/process_policy.c): macOS limits CPU-policy changes to the calling process. Owned-child probes confirmed external changes fail with EPERM and self-set limits do not survive exec; RoALT therefore uses a supervised cooperative governor, not this private SPI.
- [Apple ScreenCaptureKit](https://developer.apple.com/documentation/screencapturekit/capturing-screen-content-in-macos): capture filters select only the named client window. Installed SDK headers define SCScreenshotManager, Accessibility, CGEventPostToPid and listen-only event taps.

Grant permissions yourself in **System Settings → Privacy & Security**, then restart RoALT. This build never presses protected permission dialogs or requests administrator access. Keep anti-AFK off unless you intend the brief focus changes and Space inputs.

## 0.7.1 verification

Frontend production build and the Mac Rust test suite pass (291 passed, 2 ignored). New tests cover key validation, fallback decisions, App Nap preference restoration and journal serialization. Installer shell syntax and mocked prerequisite failures were checked. Windows has not been built on this Mac.

Manual acceptance remains required: test background and minimized Roblox clients beyond 20 minutes; verify Tab/Space are counted by the game; confirm no focus changes or input during human activity; confirm permission denial; test App Nap before/after relaunch in Activity Monitor; compare efficiency-only scheduling with cooperative caps; check GPU display on a real Apple Silicon Mac. No successful event post is reported as proof of idle prevention.

## 0.7.2 join tracking and Accessibility fixes

Mac clients that omit the browser tracker ID now bind to their own open player log through libproc, checking the PID creation time and user identity before and after enumeration. Ambiguous logs are rejected. This lets real join/replicator events advance the existing progress bar without treating a running process as proof of a successful join. Windows log binding is unchanged.

Accessibility is checked and requested again at startup if the current build is untrusted, even when an older build wrote the first-run permission marker. Native prompts are dispatched on the main thread. Window-control errors explain how to repair a stale grant: quit RoALT, remove the old entry in System Settings → Privacy & Security → Accessibility, add the current installed RoALT.app, enable it, then reopen RoALT. The app cannot grant itself permission; ad-hoc signatures change between builds.
