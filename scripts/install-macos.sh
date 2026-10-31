#!/bin/bash
# Source installer: curl -fsSL https://raw.githubusercontent.com/cookedkid/RoALT/main/scripts/install-macos.sh | bash
# Optional: ROALT_REF=<commit-or-tag> bash install-macos.sh
set -euo pipefail

fail() { printf 'RoALT: %s\n' "$*" >&2; exit 1; }
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
