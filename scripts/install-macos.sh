#!/bin/bash
# Source installer: curl -fsSL https://raw.githubusercontent.com/cookedkid/RoALT/main/scripts/install-macos.sh | bash
# Optional: ROALT_REF=<commit-or-tag> bash install-macos.sh
set -euo pipefail

fail() { printf 'RoALT: %s\n' "$*" >&2; exit 1; }
# Ad-hoc signatures change between builds. TCC can display an enabled old grant
# while refusing the replacement binary. Reset only RoALT, only after installation.
reset_changed_permissions() {
  local previous="$1" current="$2" old_hash new_hash service
  [[ -d "$previous" ]] || return 0
  old_hash="$(codesign --display --verbose=2 "$previous" 2>&1 | sed -n 's/^CDHash=//p')" || old_hash=''
  new_hash="$(codesign --display --verbose=2 "$current" 2>&1 | sed -n 's/^CDHash=//p')" || new_hash=''
  if [[ -n "$old_hash" && "$old_hash" == "$new_hash" ]]; then return 0; fi
  printf 'RoALT signing identity changed. Clearing its old Accessibility, Screen Recording and Input Monitoring grants.\n'
  for service in Accessibility ScreenCapture ListenEvent; do
    if ! tccutil reset "$service" com.inspectsoftware.roalt; then
      printf 'Could not reset %s. To repair it, quit RoALT and run: tccutil reset %s com.inspectsoftware.roalt\n' "$service" "$service" >&2
    fi
  done
  printf 'Open RoALT and approve its new permissions in System Settings, then quit and reopen it. Saved accounts and Keychain items were not changed.\n'
}

main() {
  [[ "$(uname -s)" == Darwin ]] || fail 'This installer requires macOS.'
  [[ "$(uname -m)" == arm64 ]] || fail 'This build requires Apple Silicon. Run Terminal natively, not through Rosetta.'
  xcode-select -p >/dev/null 2>&1 && xcrun --find clang >/dev/null 2>&1 || fail 'Install Xcode Command Line Tools with: xcode-select --install'
  command -v node >/dev/null && command -v npm >/dev/null || fail 'Install Node.js 22 or newer from https://nodejs.org (or: brew install node).'
  node -e 'process.exit(Number(process.versions.node.split(".")[0]) >= 22 ? 0 : 1)' || fail 'Node.js 22 or newer is required. Update Node.js before retrying.'
  command -v rustc >/dev/null && command -v cargo >/dev/null || fail 'Install Rust from https://rustup.rs (or: brew install rust). Rust 1.95 or newer is required.'
  local rust_version rust_major rust_minor ref destination bundle
  rust_version="$(rustc --version)"
  rust_version="${rust_version#rustc }"
  rust_major="${rust_version%%.*}"
  rust_minor="${rust_version#*.}"
  rust_minor="${rust_minor%%.*}"
  [[ "$rust_major" =~ ^[0-9]+$ && "$rust_minor" =~ ^[0-9]+$ ]] || fail 'Could not determine the Rust version.'
  (( rust_major > 1 || (rust_major == 1 && rust_minor >= 95) )) || fail 'Rust 1.95 or newer is required. Update Rust (rustup update stable, or brew upgrade rust).'
  command -v git >/dev/null || fail 'Git is required; install Xcode Command Line Tools.'
  if pgrep -x roalt >/dev/null; then fail 'Quit RoALT before installing an update.'; fi
  ref="${ROALT_REF:-main}"
  [[ "$ref" =~ ^[a-zA-Z0-9][a-zA-Z0-9._/-]*$ ]] || fail 'ROALT_REF must be a branch, tag or commit SHA.'
  destination="$HOME/Applications/RoALT.app"
  [[ ! -L "$destination" ]] || fail 'The install destination is a symlink; replace it manually.'
  work_dir="$(mktemp -d "${TMPDIR:-/tmp}/roalt-build.XXXXXX")"
  install_stage=''
  trap cleanup EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  git init -q "$work_dir/source"
  git -C "$work_dir/source" remote add origin https://github.com/cookedkid/RoALT.git
  git -C "$work_dir/source" fetch --depth 1 origin "$ref"
  git -C "$work_dir/source" checkout --detach -q FETCH_HEAD
  printf 'Building RoALT from commit %s. This may take several minutes.\n' "$(git -C "$work_dir/source" rev-parse HEAD)"
  (
    cd "$work_dir/source"
    npm ci </dev/null
    npm run tauri -- build --bundles app </dev/null
  )
  bundle="$work_dir/source/src-tauri/target/release/bundle/macos/RoALT.app"
  [[ -x "$bundle/Contents/MacOS/roalt" ]] || fail 'The build did not produce a RoALT app.'
  codesign --force --deep --sign - --identifier com.inspectsoftware.roalt "$bundle"
  codesign --verify --deep --strict "$bundle"
  mkdir -p "$HOME/Applications"
  install_stage="$(mktemp -d "$HOME/Applications/.roalt-install.XXXXXX")"
  ditto "$bundle" "$install_stage/RoALT.app"
  codesign --verify --deep --strict "$install_stage/RoALT.app"
  if [[ -e "$destination" ]]; then mv "$destination" "$install_stage/previous.app"; fi
  if ! mv "$install_stage/RoALT.app" "$destination"; then
    [[ ! -e "$install_stage/previous.app" ]] || mv "$install_stage/previous.app" "$destination"
    fail 'Could not install the new app; the previous app was restored.'
  fi
  reset_changed_permissions "$install_stage/previous.app" "$destination"
  printf 'Installed %s\nOpen it from Finder when ready. macOS permissions and Keychain approval may be needed.\n' "$destination"
}
cleanup() {
  # Only generated temporary directories are removed. Restore the prior app after interruption.
  if [[ -n "${install_stage:-}" ]]; then
    if [[ -d "$install_stage/previous.app" && ! -e "$HOME/Applications/RoALT.app" ]]; then
      mv "$install_stage/previous.app" "$HOME/Applications/RoALT.app" || return
    fi
    /bin/rm -rf -- "$install_stage"
  fi
  [[ -z "${work_dir:-}" ]] || /bin/rm -rf -- "$work_dir"
}
main "$@"
