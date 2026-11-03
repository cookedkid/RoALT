#!/bin/bash
# Installs the latest published Apple Silicon preview; no compiler or package manager needed.
set -euo pipefail

fail() { printf 'RoALT: %s\n' "$*" >&2; exit 1; }
cleanup() {
  if [[ -n "${install_stage:-}" ]]; then
    if [[ -d "$install_stage/previous.app" && ! -e "$destination" ]]; then
      mv "$install_stage/previous.app" "$destination" || return
    fi
    /bin/rm -rf -- "$install_stage"
  fi
  [[ -z "${work_dir:-}" ]] || /bin/rm -rf -- "$work_dir"
}
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
  [[ "$(uname -m)" == arm64 ]] || fail 'This preview requires Apple Silicon. Run Terminal natively, not through Rosetta.'
  local tag archive base expected actual bundle identifier architecture
  local version='' api_digest='' parent_pid='' parent_dir
  # Keep destination available to the EXIT trap after main returns.
  tag=""
  destination="$HOME/Applications/RoALT.app"
  if [[ "${1:-}" == --in-app ]]; then
    [[ $# == 6 ]] || fail 'Invalid update parameters.'
    tag="$2"; version="$3"; api_digest="$4"; destination="$5"; parent_pid="$6"
    [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ && "$tag" == "macos-preview-${version}-${tag##*-}" ]] || fail 'Invalid Mac release.'
    [[ "${tag##*-}" =~ ^[0-9]+$ && "$api_digest" =~ ^[0-9a-f]{64}$ && "$parent_pid" =~ ^[1-9][0-9]*$ ]] || fail 'Invalid update metadata.'
    [[ "$destination" == "$HOME/Applications/RoALT.app" || "$destination" == /Applications/RoALT.app ]] || fail 'Move RoALT to Applications before updating.'
    [[ -d "$destination" ]] || fail 'Installed app is missing.'
  else
    if pgrep -x roalt >/dev/null; then fail 'Quit RoALT before installing an update.'; fi
    if [[ ! -e "$destination" && -e /Applications/RoALT.app ]]; then
      destination=/Applications/RoALT.app
    fi
  fi
  parent_dir="${destination%/*}"
  archive=RoALT-macOS-Apple-Silicon.zip
  [[ ! -L "$destination" ]] || fail 'The install destination is a symlink; replace it manually.'
  work_dir="$(mktemp -d "${TMPDIR:-/tmp}/roalt-download.XXXXXX")"
  install_stage=''
  trap cleanup EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  if [[ -z "$tag" ]]; then
    curl -fsSL --max-time 30 --retry 2 'https://api.github.com/repos/cookedkid/RoALT/releases?per_page=100' -o "$work_dir/releases.json" || fail 'Could not look up the latest Mac release.'
    local index=0 candidate draft
    while candidate=$(/usr/bin/plutil -extract "$index.tag_name" raw "$work_dir/releases.json" 2>/dev/null); do
      draft=$(/usr/bin/plutil -extract "$index.draft" raw "$work_dir/releases.json")
      if [[ "$draft" == false && "$candidate" =~ ^macos-preview-([0-9]+\.[0-9]+\.[0-9]+)-[0-9]+$ ]]; then
        version="${BASH_REMATCH[1]}"; tag="$candidate"; break
      fi
      index=$((index + 1))
    done
    [[ -n "$tag" ]] || fail 'No published Mac preview found.'
  fi
  base="https://github.com/cookedkid/RoALT/releases/download/$tag"
  if [[ -e "$destination" ]]; then
    [[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$destination/Contents/Info.plist")" == com.inspectsoftware.roalt ]] || fail 'Existing app has a different identifier.'
    local installed_version
    installed_version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$destination/Contents/Info.plist")
    [[ "$installed_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail 'Unknown installed app version.'
    /usr/bin/awk -v old="$installed_version" -v new="$version" 'BEGIN { split(old,a,"[.]");split(new,b,"[.]");for(i=1;i<=3;i++){if(a[i]+0>b[i]+0)exit 1;if(a[i]+0<b[i]+0)exit 0} }' || fail 'A newer copy is already installed.'
  fi
  printf 'Downloading RoALT %s…\n' "$tag"
  curl --connect-timeout 15 --max-time 240 --max-filesize 200000000 --fail --location --silent --show-error --proto '=https' --proto-redir '=https' --retry 2 --output "$work_dir/$archive" "$base/$archive" || fail 'The Mac release could not be downloaded. Check https://github.com/cookedkid/RoALT/releases and try again.'
  curl --connect-timeout 15 --max-time 240 --max-filesize 200000000 --fail --location --silent --show-error --proto '=https' --proto-redir '=https' --retry 2 --output "$work_dir/SHA256SUMS.txt" "$base/SHA256SUMS.txt" || fail 'Could not download the release checksum; installation stopped.'
  # Require one exact filename entry. Never execute or trust paths from a checksum file.
  expected="$(awk -v name="$archive" '$2 == name { print $1; count++ } END { if (count != 1) exit 1 }' "$work_dir/SHA256SUMS.txt")" || fail 'The release checksum entry is missing or duplicated.'
  [[ "$expected" =~ ^[0-9a-fA-F]{64}$ ]] || fail 'The release checksum is not a SHA-256 value.'
  actual="$(shasum -a 256 "$work_dir/$archive")"
  actual="${actual%% *}"
  [[ "$(printf '%s' "$expected" | tr '[:upper:]' '[:lower:]')" == "$actual" ]] || fail 'The download checksum did not match; installation stopped.'
  [[ -z "$api_digest" || "$actual" == "$api_digest" ]] || fail 'Downloaded ZIP does not match the release digest from GitHub.'
  mkdir "$work_dir/extracted"
  ditto -x -k "$work_dir/$archive" "$work_dir/extracted"
  bundle="$work_dir/extracted/RoALT.app"
  [[ -d "$bundle" && ! -L "$bundle" && -x "$bundle/Contents/MacOS/roalt" && ! -L "$bundle/Contents/MacOS/roalt" ]] || fail 'The download does not contain the expected app.'
  identifier="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$bundle/Contents/Info.plist")"
  [[ "$identifier" == com.inspectsoftware.roalt ]] || fail 'The app bundle identifier did not match.'
  [[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$bundle/Contents/Info.plist")" == "$version" ]] || fail 'The app version did not match this installer.'
  architecture="$(file -b "$bundle/Contents/MacOS/roalt")"
  case "$architecture" in
    *'Mach-O 64-bit executable arm64'*) ;;
    *) fail 'The app executable is not the expected Apple Silicon binary.' ;;
  esac
  codesign --verify --deep --strict "$bundle" || fail 'The app signature could not be verified.'
  identifier="$(codesign --display --verbose=2 "$bundle" 2>&1 | sed -n 's/^Identifier=//p')"
  [[ "$identifier" == com.inspectsoftware.roalt ]] || fail 'The signed app identifier did not match.'
  mkdir -p "$parent_dir"
  install_stage="$(mktemp -d "$parent_dir/.roalt-install.XXXXXX")"
  ditto "$bundle" "$install_stage/RoALT.app"
  codesign --verify --deep --strict "$install_stage/RoALT.app"
  if [[ -n "$parent_pid" ]]; then
    # The app stays open during verification. EOF cancels the staged update.
    printf 'ROALT_UPDATE_READY\n'
    IFS= read -r decision && [[ "$decision" == install ]] || fail 'Update cancelled.'
    exec </dev/null >/dev/null 2>&1
    for ((i=0; i<300; i++)); do
      kill -0 "$parent_pid" 2>/dev/null || break
      sleep 0.1
    done
    ! kill -0 "$parent_pid" 2>/dev/null || fail 'RoALT did not quit.'
  fi
  if [[ -e "$destination" ]]; then mv "$destination" "$install_stage/previous.app"; fi
  if ! mv "$install_stage/RoALT.app" "$destination"; then
    [[ ! -e "$install_stage/previous.app" ]] || mv "$install_stage/previous.app" "$destination"
    fail 'Could not install the new app; the previous app was restored.'
  fi
  reset_changed_permissions "$install_stage/previous.app" "$destination"
  if [[ -n "$parent_pid" ]] && ! /usr/bin/open "$destination"; then
    mv "$destination" "$install_stage/failed.app"
    mv "$install_stage/previous.app" "$destination"
    /usr/bin/open "$destination" || true
    fail 'Could not open the update. Restored the previous version.'
  fi
  printf 'Installed %s\nOpen it from Finder when ready. This preview is ad-hoc signed, not notarized; macOS permissions and Keychain approval may be needed.\n' "$destination"
}
main "$@"
