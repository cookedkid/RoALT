<div align="center">

# RoALT for macOS — 0.8.4

### A native Apple Silicon port of the Roblox account manager

This fork ports [inspectsoftware/RoALT](https://github.com/inspectsoftware/RoALT) to macOS. It keeps the original React interface and replaces Windows system calls with macOS implementations. **The port is still in progress; it does not yet have every Windows feature.**

![RoALT interface](docs/images/home.png)

[Features](#features) · [Install](#install-on-macos) · [FAQ](#faq) · [Technical notes](docs/MACOS_PORT.md)

</div>

## beta 0.8.0 Mac update

Includes upstream account tabs, sortable/resizable preview cards, compact view, expanded languages, settings layout and launch/profile fixes. Optimization comes first: a shared cooperative CPU budget scales with client count; foreground, loading, unresponsive and Full quality clients are exempt from automatic limits. Explicit per-client limits remain in effect. Efficiency-only mode disables the shared cap and avoids intentional pauses. Mac previews stay at 2/5/10 FPS with at most 10 requests per second across visible cards.

Home CPU and memory now total tracked Roblox clients. Mac memory is physical footprint; GPU remains clearly labeled as whole-Mac utilization. Windows memory trimming, hard CPU quotas and invisible/click-through live windows are not ported. Mac uses ordinary minimize/restore, and minimized clients have no live preview. No claim is made that Windows' 295 MB example is achievable on Mac.

## Features

| Area | macOS status |
|---|---|
| Accounts and vault | Roblox account management, account checks, encrypted database, master password, recovery code and encrypted backups are ported. Keychain replaces Windows DPAPI; Touch ID is used where available. |
| Launch and multi-instance | Native Roblox launch, multiple clients, batches and shared sign-in protection are ported. Two test clients ran together, but simultaneous authenticated game joins still need user testing. |
| Performance | CPU and memory monitoring, background priority, FPS and low graphics settings, and cooperative CPU caps are ported. CPU caps can cause stutter and are not kernel quotas. |
| Windows and previews | Focus, minimize/restore, tiling, tiny windows and live previews are ported. Accessibility and Screen Recording permission are required. Preview rates are 2, 5 or 10 FPS. |
| Anti-AFK | While you are idle, RoALT brings a tracked game forward, clicks its center, and minimizes it a second later. |

### Still missing or different from Windows

- Kernel CPU quotas, core affinity/spreading, memory trimming, bandwidth shaping and per-client GPU/VRAM counters remain unsupported.
- Scheduled shutdown, Windows-style public-network classification, and automatic installation and updating of Roblox itself.
- Developer ID signing/notarization and Intel/universal builds. The Mac ZIP is an ad-hoc signed preview.

Unsupported controls are hidden or rejected on macOS. See [the port notes](docs/MACOS_PORT.md) for implementation details and the remaining acceptance checks. The [original Windows repository](https://github.com/inspectsoftware/RoALT) documents Windows-specific features.

## Install on macOS

**Apple Silicon only.** Download `RoALT-macOS-Apple-Silicon.zip` from the [newest Mac preview release](https://github.com/cookedkid/RoALT/releases), unzip it, and drag `RoALT.app` into Applications.

Or install the same preview with one Terminal command:

```sh
curl -fsSL https://raw.githubusercontent.com/cookedkid/RoALT/main/scripts/install-macos-release.sh | bash
```

This updates your existing copy in `/Applications` or `~/Applications`. A fresh install goes into `~/Applications`. If both folders have a copy, keep one before running the installer. **No Rust, Node, Xcode or Homebrew is required.** The installer checks the SHA-256, app identity, version and architecture before replacing an existing app, and restores the old copy if replacement fails. When an update changes the app signature, it clears only RoALT’s old Accessibility, Screen Recording and Input Monitoring grants after successful installation. Approve those permissions again when you open the new app; saved accounts and Keychain items are preserved. Reinstalling the identical signed build leaves grants alone. It does not launch RoALT. You can read [the installer](scripts/install-macos-release.sh) first.

The preview is ad-hoc signed and not notarized. macOS may require approval to open it or access an existing RoALT Keychain item. Approve requested Accessibility, Screen Recording and Input Monitoring access yourself, then restart RoALT. Anti-AFK remains off until enabled.

Starting with 0.8.1, open **Settings → Check now** for Mac releases. Starting with 0.8.2, fresh installs check GitHub at startup and every six hours by default. Existing installs keep their saved update preference. Click **Update** to verify and install a newer version, then reopen the app automatically. RoALT replaces the running copy in its current Applications folder. Older builds need the Terminal installer or ZIP once to gain this feature.

<details>
<summary>Build from source instead</summary>

Requires Xcode Command Line Tools, Node.js 22+ and Rust 1.95+:

```sh
curl -fsSL https://raw.githubusercontent.com/cookedkid/RoALT/main/scripts/install-macos.sh | bash
```

The source installer builds the current `main` branch, verifies the app and installs into `~/Applications`. It checks prerequisites and does not silently install developer tools. See [build details](docs/MACOS_PORT.md#build-and-install).

</details>

### New Mac controls

Performance → Mac background behavior offers an App Nap override for Roblox. App Nap changes affect all Roblox clients on their next launch and may increase power use. Disabling the override restores the previous preference. Efficiency-only adaptive limits lower CPU/I/O priority without deliberate pauses; core placement remains controlled by macOS. System GPU utilization appears in Home and Monitor only when the driver reports it, and is never attributed to individual clients.

## Quick start

1. Open RoALT and finish any macOS permission prompts you want to grant. Restart after granting them.
2. Add an account through Roblox's sign-in page or import an encrypted `.roalt` backup.
3. Select an account in **Play**, paste a game link and launch it.
4. Enable **Multi-instance** in **Settings → Launching** before launching several clients. RoALT launches the installed Roblox player directly on Mac.
5. Use **Home** for tiling and previews. Enable anti-AFK only if you want RoALT to send input to a running client.

## FAQ

<details>
<summary><b>How do I download this Mac version?</b></summary>
<br>

Download the Mac preview ZIP or use the [one-command binary installer](#install-on-macos). Both install the same Apple Silicon app. The Terminal binary installer needs no developer tools.

</details>

<details>
<summary><b>Why does macOS ask for permissions?</b></summary>
<br>

Accessibility allows RoALT to arrange Roblox windows and focus them for opt-in anti-AFK. Screen Recording provides live window previews. Input Monitoring lets anti-AFK detect recent human activity; macOS also controls permission to post input. RoALT asks for missing access on first launch, but only you can grant it in **System Settings → Privacy & Security**. Restart RoALT after granting it. If Accessibility is already on but window controls fail after an update, quit RoALT, remove its old Accessibility entry, add the current installed RoALT.app, enable it and reopen RoALT. Ad-hoc preview builds have different code identities. Features that need a declined permission will remain unavailable until you grant it.

</details>

<details>
<summary><b>Will it run on an Intel Mac?</b></summary>
<br>

This build targets Apple Silicon. Intel and universal builds have not been tested or published.

</details>

<details>
<summary><b>Can I run several Roblox clients? What if one is already open?</b></summary>
<br>

Enable **Multi-instance** in RoALT and launch clients from RoALT. The Mac port uses the installed Roblox player and a macOS-specific singleton mechanism; it does not use the Windows **Close Roblox** or **Take over** handle workflow. Two test clients ran alongside an existing client, but simultaneous authenticated game joins still need acceptance testing.

</details>

<details>
<summary><b>Does anti-AFK guarantee I will not be kicked?</b></summary>
<br>

Idle prevention depends on whether the game accepts the click. Test it in the experience you run.

</details>

<details>
<summary><b>Can I use Bloxstrap?</b></summary>
<br>

Bloxstrap is a Windows bootstrapper. The Mac port launches the installed macOS Roblox player directly.

</details>

<details>
<summary><b>What if an account says its cookie expired?</b></summary>
<br>

Roblox ended the sign-in or requires a human check. Open the account on roblox.com from RoALT and complete the check, or remove and add the account again.

</details>

<details>
<summary><b>What if I forget my master password?</b></summary>
<br>

If you created a recovery code, use **Forgot password?** on the unlock screen. A recovery code works once and is tied to the same Mac Keychain item; keep it somewhere separate from this Mac. For moving to another computer, use a password-protected `.roalt` backup. Without the password, a working recovery code or a backup, the encrypted vault cannot be recovered. Do not delete RoALT's data or Keychain item while trying to recover it.

</details>

<details>
<summary><b>Where is my data? Can I move from Windows?</b></summary>
<br>

Mac data is in `~/Library/Application Support/RoALT`; **Settings → Data** can open or move it. Import a password-protected `.roalt` backup to move accounts from Windows. A Windows DPAPI database cannot be copied directly to Mac.

</details>

<details>
<summary><b>Is RoALT safe to use with my account?</b></summary>
<br>

RoALT uses Roblox's own sign-in and the installed, unchanged Roblox player. Roblox does not officially support running several clients at once, and its behavior can change. Use RoALT only with accounts you own. Never share a `.ROBLOSECURITY` cookie, a `.roalt` backup or its password.

</details>

## For developers

RoALT uses Tauri 2, a Rust backend, React and TypeScript. The Mac backend uses Keychain, Cocoa/Accessibility, ScreenCaptureKit and macOS process APIs. Read [the Mac port notes](docs/MACOS_PORT.md) for behavior and test results. To check source changes:

```sh
npm ci
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

The app is independent and is not affiliated with or endorsed by Roblox Corporation. Bug reports and contributions can go to [this fork's issues](https://github.com/cookedkid/RoALT/issues). Windows development and its installer remain in the [upstream repository](https://github.com/inspectsoftware/RoALT).
