<div align="center">

# RoALT for macOS

### A native Apple Silicon port of the Roblox account manager

This fork ports [inspectsoftware/RoALT](https://github.com/inspectsoftware/RoALT) to macOS. It keeps the original React interface and replaces Windows system calls with macOS implementations. **The port is still in progress; it does not yet have every Windows feature.**

![RoALT interface](docs/images/home.png)

[Features](#features) · [Install](#install-on-macos) · [FAQ](#faq) · [Technical notes](docs/MACOS_PORT.md)

</div>

## Features

| Area | macOS status |
|---|---|
| Accounts and vault | Roblox account management, account checks, encrypted database, master password, recovery code and encrypted backups are ported. Keychain replaces Windows DPAPI; Touch ID is used where available. |
| Launch and multi-instance | Native Roblox launch, multiple clients, batches and shared sign-in protection are ported. Two test clients ran together, but simultaneous authenticated game joins still need user testing. |
| Performance | CPU and memory monitoring, background priority, FPS and low graphics settings, and cooperative CPU caps are ported. CPU caps can cause stutter and are not kernel quotas. |
| Windows and previews | Focus, minimize/restore, tiling, tiny windows and live previews are ported. Accessibility and Screen Recording permission are required. Preview rates are 2, 5 or 10 FPS. |
| Anti-AFK | Opt-in focus mode sends Space to a visible tracked client after checking for recent human input. It requires Accessibility and Input Monitoring. Kick prevention in an actual game has not been verified. |

### Still missing or different from Windows

- Background or minimized anti-AFK, kernel CPU quotas, core affinity and spreading, memory trimming, bandwidth shaping, and reliable GPU counters.
- Scheduled shutdown, Windows-style public-network classification, and automatic Roblox or RoALT installation and updates.
- A notarized Mac installer, Intel or universal build, and a downloadable Mac GitHub release.

Unsupported controls are hidden or rejected on macOS. See [the port notes](docs/MACOS_PORT.md) for implementation details and the remaining acceptance checks. The [original Windows repository](https://github.com/inspectsoftware/RoALT) documents Windows-specific features.

## Install on macOS

**Apple Silicon only.** There is not yet a Mac download on GitHub Releases. For now, build and install the app from this repository. You need [Xcode Command Line Tools](https://developer.apple.com/xcode/resources/), Node.js 22 or newer, and Rust 1.95 or newer. After installing those prerequisites, paste this into Terminal:

```sh
git clone https://github.com/cookedkid/RoALT.git
cd RoALT
npm ci
npm run tauri -- build --bundles app
mkdir -p "$HOME/Applications"
ditto src-tauri/target/release/bundle/macos/RoALT.app "$HOME/Applications/RoALT.app"
open "$HOME/Applications/RoALT.app"
```

The build may take several minutes. The app is ad-hoc signed and not notarized. macOS may ask you to approve opening it and to allow access to an existing RoALT Keychain item after a rebuild. The app requests missing Screen Recording, Input Monitoring, event-posting and Accessibility permissions on first launch. Approve them in macOS, then restart RoALT. Anti-AFK remains off until you enable it.

A downloadable ZIP should be the normal install method once a Mac release is published; a Terminal build is the current option, not a requirement of the app itself. The [local build and validation notes](docs/MACOS_PORT.md#build-and-install) have more detail.

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

There is no Mac GitHub release asset yet. Use the [Terminal build-and-install commands](#install-on-macos) above. When a release ZIP is published, it will be the simpler download option. The Windows `.exe` on the original project's release page is not a Mac installer.

</details>

<details>
<summary><b>Why does macOS ask for permissions?</b></summary>
<br>

Accessibility allows RoALT to arrange Roblox windows and focus them for opt-in anti-AFK. Screen Recording provides live window previews. Input Monitoring lets anti-AFK detect recent human activity; macOS also controls permission to post input. RoALT asks for missing access on first launch, but only you can grant it in **System Settings → Privacy & Security**. Restart RoALT after granting it. Features that need a declined permission will remain unavailable until you grant it.

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

No. The Mac implementation sends a Space press to a visible client when you have been away, but individual Roblox games may not count synthetic input. Keep auto-rejoin as a fallback. Background or minimized anti-AFK is not available on Mac.

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
