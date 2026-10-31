#!/bin/bash
# Installs the pinned Apple Silicon preview; no compiler or package manager needed.
set -euo pipefail

fail() { printf 'RoALT: %s\n' "$*" >&2; exit 1; }
cleanup() {
  if [[ -n "${install_stage:-}" ]]; then
    if [[ -d "$install_stage/previous.app" && ! -e "$HOME/Applications/RoALT.app" ]]; then
      mv "$install_stage/previous.app" "$HOME/Applications/RoALT.app" || return
    fi
    /bin/rm -rf -- "$install_stage"
  fi
  [[ -z "${work_dir:-}" ]] || /bin/rm -rf -- "$work_dir"
}
main() {
  [[ "$(uname -s)" == Darwin ]] || fail 'This installer requires macOS.'
  [[ "$(uname -m)" == arm64 ]] || fail 'This preview requires Apple Silicon. Run Terminal natively, not through Rosetta.'
  if pgrep -x roalt >/dev/null; then fail 'Quit RoALT before installing an update.'; fi
  local tag archive base expected actual bundle destination identifier architecture
  tag=macos-preview-0.8.0-4
  archive=RoALT-macOS-Apple-Silicon.zip
  base="https://github.com/cookedkid/RoALT/releases/download/$tag"
  destination="$HOME/Applications/RoALT.app"
  [[ ! -L "$destination" ]] || fail 'The install destination is a symlink; replace it manually.'
  work_dir="$(mktemp -d "${TMPDIR:-/tmp}/roalt-download.XXXXXX")"
  install_stage=''
  trap cleanup EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  printf 'Downloading RoALT %s…\n' "$tag"
  curl --fail --location --silent --show-error --proto '=https' --proto-redir '=https' --retry 2 --output "$work_dir/$archive" "$base/$archive" || fail 'The pinned Mac release could not be downloaded. Check https://github.com/cookedkid/RoALT/releases and try again.'
  curl --fail --location --silent --show-error --proto '=https' --proto-redir '=https' --retry 2 --output "$work_dir/SHA256SUMS.txt" "$base/SHA256SUMS.txt" || fail 'Could not download the release checksum; installation stopped.'
  # Require one exact filename entry. Never execute or trust paths from a checksum file.
  expected="$(awk -v name="$archive" '$2 == name { print $1; count++ } END { if (count != 1) exit 1 }' "$work_dir/SHA256SUMS.txt")" || fail 'The release checksum entry is missing or duplicated.'
  [[ "$expected" =~ ^[0-9a-fA-F]{64}$ ]] || fail 'The release checksum is not a SHA-256 value.'
  actual="$(shasum -a 256 "$work_dir/$archive")"
  actual="${actual%% *}"
  [[ "$(printf '%s' "$expected" | tr '[:upper:]' '[:lower:]')" == "$actual" ]] || fail 'The download checksum did not match; installation stopped.'
  mkdir "$work_dir/extracted"
  ditto -x -k "$work_dir/$archive" "$work_dir/extracted"
  bundle="$work_dir/extracted/RoALT.app"
  [[ -d "$bundle" && ! -L "$bundle" && -x "$bundle/Contents/MacOS/roalt" && ! -L "$bundle/Contents/MacOS/roalt" ]] || fail 'The download does not contain the expected app.'
  identifier="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$bundle/Contents/Info.plist")"
  [[ "$identifier" == com.inspectsoftware.roalt ]] || fail 'The app bundle identifier did not match.'
  [[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$bundle/Contents/Info.plist")" == 0.8.0 ]] || fail 'The app version did not match this installer.'
  architecture="$(file -b "$bundle/Contents/MacOS/roalt")"
  case "$architecture" in
    *'Mach-O 64-bit executable arm64'*) ;;
    *) fail 'The app executable is not the expected Apple Silicon binary.' ;;
  esac
  codesign --verify --deep --strict "$bundle" || fail 'The app signature could not be verified.'
  identifier="$(codesign --display --verbose=2 "$bundle" 2>&1 | sed -n 's/^Identifier=//p')"
  [[ "$identifier" == com.inspectsoftware.roalt ]] || fail 'The signed app identifier did not match.'
  mkdir -p "$HOME/Applications"
  install_stage="$(mktemp -d "$HOME/Applications/.roalt-install.XXXXXX")"
  ditto "$bundle" "$install_stage/RoALT.app"
  codesign --verify --deep --strict "$install_stage/RoALT.app"
  if [[ -e "$destination" ]]; then mv "$destination" "$install_stage/previous.app"; fi
  if ! mv "$install_stage/RoALT.app" "$destination"; then
    [[ ! -e "$install_stage/previous.app" ]] || mv "$install_stage/previous.app" "$destination"
    fail 'Could not install the new app; the previous app was restored.'
  fi
  printf 'Installed %s\nOpen it from Finder when ready. This preview is ad-hoc signed, not notarized; macOS permissions and Keychain approval may be needed.\n' "$destination"
}
main "$@"
