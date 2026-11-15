# macOS feature audit — Windows v1.2.301 baseline

Checked November 14, 2026. Upstream: [v1.2.301](https://github.com/inspectsoftware/RoALT/releases/tag/v1.2.301), commit `5923226`. Previous Mac source: `6aa2778` (0.9.0). This integration merges the complete upstream tag and restores the native Apple Silicon adapters; it does not claim Windows/Mac runtime parity.

## How to read this report

- **Shared:** upstream implementation/UI included and compiled on Mac. Network/account operations still need live acceptance testing.
- **Mac equivalent:** native implementation exists, with the differences stated below.
- **Partial:** some parts work; the excluded parts are explicit.
- **Unavailable:** no Mac implementation; hidden, rejected or explanatory status rather than fabricated success.
- Tests/builds prove only what they exercise. No real account purchases, trades, client restarts, sign-ins, webhook sends or permission changes were performed during this integration.

## User-facing features, including small controls

| Feature | Status and exact difference |
|---|---|
| Upstream visual redesign | Shared: navigation/island, panels, cards, dialogs, drawers, popovers, tooltips, status pills and responsive styles. Mac-specific explanatory text retained. |
| Themes | Shared: presets, custom colors, accents, corner radius, theme sharing/import/export, custom background and background presentation controls. |
| Motion and sound | Shared: animation preferences/reduced motion, UI sounds/volume, transitions and number animation. |
| Language | Shared upstream catalogs/language picker. Mac-specific overrides are English; not every native message is translated. |
| Settings | Shared: unified registry, search, goal shortcuts, sections, links to account/client settings. Unsupported Mac controls hidden; explicit public-network guard can be reversed. |
| Shortcuts | Shared keyboard table and global show/hide, tile, minimize and panic bindings. Binding conflicts and macOS key delivery need live testing. Windows Ctrl labels are retained; not a complete Command-key convention rewrite. |
| Tray/menu bar | Shared Tauri menu, show/hide, favourites and exit behavior. Native tray startup/hide/show needs Mac acceptance. Windows watchdog recovery is not included. |
| Splash | Partial: Mac shows the window through its existing splash adapter; Windows native startup splash is not ported one-for-one. |
| Accounts list | Shared: search/filter/sort, selection/select-all, main-account protection, compact/cards/tabs, reorder, aliases, colors, favourites, notes, detail view and avatars. |
| Groups | Shared: create/edit/delete, colors, ordering, memberships and group defaults/profile inheritance. |
| Account profiles | Shared: saved per-account options, Full quality, main account, FPS, flags, launch args, rejoin and Discord opt-out. CPU/background profile fields use native Mac planner; trim/affinity cannot apply. |
| Add/import accounts | Shared API/cookie/RAM imports and duplicate handling; Mac login uses isolated ephemeral WKWebView. Captcha/human challenges remain user-operated. |
| Username/password login filling | Mac equivalent: fills empty Roblox fields, never submits login automatically; isolated browser and allowlisted navigation. |
| Session browsing | Partial: separate ephemeral account window, cookie rotation handling; opens Roblox home. Requested embedded navigation path is not carried into that window. No persistent browser profile/captcha trust cache. |
| Embedded Browse | Partial: native Popular, playtime and Limiteds tabs available. Protected embedded Roblox page, embedded navigation/cards/account switching and purchase-blocking are unavailable. |
| Account checks | Shared: validity, balances, user/account details, headshots, presence, sign-in watch and refresh actions. |
| Vault | Mac equivalent: SQLCipher, encrypted secrets/backups, master password, auto-lock, recovery creation/save/copy/recover; Keychain replaces DPAPI. Windows Hello is not available; native authentication where supported. |
| Secret export | Shared reveal/copy/export and clipboard cleanup safeguards. Windows DPAPI data cannot simply be copied to Mac; use encrypted export. |
| Data tools | Shared data folder info/open/move, backups, logs open/clean, diagnostic export/redaction and debug view. Browser reset has no persistent Mac profiles to erase. |
| Launch targets | Shared public/private/reserved-server targets, URL parsing, target validation, favourites, reorder, per-favourite options, server selection and friend join candidates. |
| Batch launch | Shared stagger, progress, cancel, retry, selection and group options. Native Mac launch/PID identity retained. |
| Multi-instance | Mac equivalent: installed Roblox executable and singleton semaphore handling; Windows handle takeover/Job mechanism not used. |
| Shared-login protection | Mac equivalent: child sandbox blocks known cookie/HTTP stores. Not all Roblox shared data isolated; sandbox-exec is deprecated. |
| Client state | Shared ticket/run/join/in states, loading and ended state, logs, join/server history, current game, uptime, ended-list clear and retry/stop-relaunch. Actual current-client log markers need game testing. |
| Auto recovery | Shared disconnect/crash relaunch, breaker, backoff, close grace, retry controls; Mac crash explanations from .ips reports. Windows autopsy/minidumps/ETW not ported. |
| Timers | Shared periodic restart, close-after and memory-triggered restart/throttle logic. Restart is not memory trimming and may require Roblox verification. Scheduled Mac system shutdown unavailable. |
| Anti-AFK | Mac equivalent: foreground F13, 1–19 minute interval, opt-in/off by default, per-client opt-out, recent-human-input guard and prior focus/minimized state restoration. No guaranteed background PID input/20-minute idle prevention. |
| CPU limits | Mac equivalent: cooperative cap/governor, per-client/manual/profile limits, fair budget, adaptive background exemptions and full-quality safeguards. Not a kernel quota; can stutter. |
| Efficiency | Mac equivalent: Darwin background scheduling/IO priority. macOS decides core placement; no guarantee of efficiency-only cores. |
| Memory optimization | Partial: monitoring and threshold policies. No EmptyWorkingSet equivalent, periodic trim, allocation cap or guaranteed RAM target. Experimental allocator launch option only encourages release of already-freed memory. |
| Affinity/spreading | Unavailable: no explicit CPU-core masks or Windows automatic spreading. Imported unsupported profile fields do not suppress usable CPU/priority limits. |
| Bandwidth | Unavailable: no Windows QoS shaping. |
| Performance presets/advice | Shared decision engine with Mac-specific presets and recommendations: no trim/spread/mute/Windows render mode, 800×632 starting tiny size, bounded preview rates. No measured new FPS/RAM savings claim. |
| FastFlags/graphics | Shared editor, per-account/global scope, FPS and allowlisted low graphics options. Mac temporarily merges settings into installed player and restores them. Roblox may ignore unsupported/changed flags. Direct3D/Vulkan/OpenGL selector unavailable. |
| Full quality | Shared opt-out from automatic performance changes; explicit manual limits still apply. |
| Monitor | Mac equivalent: CPU, memory physical footprint/resident, disk IO and system statistics. GPU is whole-system only when driver exposes utilization. No per-client GPU, VRAM or Windows commit figure. |
| Previews | Mac equivalent: ScreenCaptureKit screenshot previews, visible-card scheduling, error message on missing permission, 2/5/10 FPS with aggregate 10 requests/s. Minimized clients retain a last frame, not live capture. |
| Window controls | Mac equivalent: show, minimize/restore, all-client operations, geometry restore and AX tiling. Requires Accessibility. |
| Tiny/Fit layouts | Partial: shared slot geometry adapted to logical Mac points; main display only, conservative title-frame approximation. Roblox minimum size enforced (observed 800×632); no forced Windows-size bypass. |
| Invisible/click-through windows/DWM | Unavailable: no Windows style removal, hidden live thumbnail overlay or click-through rendering. |
| Audio muting | Unavailable per client/on launch. UI sounds are separate and available. |
| Games and servers | Shared game metadata/stats, popular discovery, target-place resolution, server list/filter/join, history, last-server handling and blocked-subplace fix. |
| Social | Shared friends requests/accept/decline/decline-all, befriending selected accounts and player statistics/value. Mutations need real-account acceptance. |
| Avatar/outfits | Shared list, wear, save, rename and delete. |
| Trades/limiteds | Shared listing, details, accept/decline, valuations and thumbnails. No live purchase/trade performed. |
| Private servers | Shared list/detail, settings update, renewal/purchase and owned list. API permissions/balance and human challenges still apply. |
| Robux/playtime | Shared transaction/totals, playtime per-day/list and CSV exports. |
| Discord | Shared Rich Presence and webhook configuration/test, event toggles, ping, summaries, account mute and trade/status alerts. Requires Discord/API access. |
| News/online/extras | Shared fetching, announcements/extras and upstream online-count behavior; live-service ignored tests not run. |
| Clips | Partial: existing file library/list, playback with range serving, rename/reveal/folder/delete-to-Trash. Replay buffer, capture selection, capture hotkey, H264/AAC encoder and per-client audio capture unavailable. |
| Mods | Partial: ZIP validation, import/export, metadata/list, ordering, remove and per-pack enable metadata. Global apply disabled; no texture/sound/cursor/font replacement in signed Roblox app. |
| Default launcher | Unavailable: Windows registry/protocol claiming, browser launch interception/picker and bootstrapper integration. Pasted links inside RoALT work. |
| Roblox player installer | Unavailable automatic install/update; launches installed Mac Roblox. Windows bootstrapper selection has no Mac equivalent here. |
| RoALT updater | Mac equivalent: fork release feed, manual/periodic checks, ZIP digest/app identity verification, replacement/rollback/reopen. Different from signed Windows EXE updater. Requires published newer Mac release to test actual handoff. |
| Permissions/install cleanup | Mac-specific: Accessibility, Screen Recording, Input Monitoring, microphone metadata/requests; installer repairs changed ad-hoc grants and removes verified obsolete copies while preserving vault. User must approve grants. |
| App Nap | Mac-specific opt-in installed-bundle-domain override, next-launch effect and restoration. May increase power usage. |
| Watchdog | Unavailable Windows native stack snapshots, hang worker inspection and forced application restart. Shared UI recovery changes included, not an equivalent native watchdog. |
| Signing/platform | Apple Silicon arm64 ad-hoc preview only; no Developer ID/notarization, Intel/universal or Windows installer in Mac download. |

## Individual IPC command inventory

This lists every handler registered in the integrated app source. Shared means its code is included, not that a live API call was made. Native platform adapters are selected with cfg; Windows implementations remain separate.

| Command | Mac status |
|---|---|
| `mac_options_get` | Mac-specific: App Nap/allocator launch options. |
| `mac_options_set` | Mac-specific: App Nap/allocator launch options. |
| `vault_status` | Mac equivalent: Keychain/native authentication and shared vault rules. |
| `vault_unlock` | Mac equivalent: Keychain/native authentication and shared vault rules. |
| `vault_lock` | Mac equivalent: Keychain/native authentication and shared vault rules. |
| `vault_set_mode` | Mac equivalent: Keychain/native authentication and shared vault rules. |
| `vault_recover` | Mac equivalent: Keychain/native authentication and shared vault rules. |
| `vault_recovery_create` | Mac equivalent: Keychain/native authentication and shared vault rules. |
| `vault_recovery_save` | Mac equivalent: Keychain/native authentication and shared vault rules. |
| `vault_recovery_copy` | Mac equivalent: Keychain/native authentication and shared vault rules. |
| `app_activity` | Shared; compiled/tested where covered, live acceptance pending. |
| `splash_done` | Partial: Mac startup adapter, no Windows native splash. |
| `accounts_list` | Shared; compiled/tested where covered, live acceptance pending. |
| `account_open_session` | Partial: isolated ephemeral home-page browser, not embedded Browse. |
| `account_add_cookie` | Shared; compiled/tested where covered, live acceptance pending. |
| `accounts_import_cookies` | Shared; compiled/tested where covered, live acceptance pending. |
| `accounts_import_ram` | Shared; compiled/tested where covered, live acceptance pending. |
| `account_add_login` | Mac equivalent: isolated ephemeral WKWebView login. |
| `account_update` | Shared; compiled/tested where covered, live acceptance pending. |
| `accounts_reorder` | Shared; compiled/tested where covered, live acceptance pending. |
| `accounts_remove` | Shared; compiled/tested where covered, live acceptance pending. |
| `accounts_validate` | Shared; compiled/tested where covered, live acceptance pending. |
| `accounts_robux` | Shared; compiled/tested where covered, live acceptance pending. |
| `accounts_befriend` | Shared; compiled/tested where covered, live acceptance pending. |
| `account_reveal_cookie` | Mac equivalent: Keychain/native authentication and shared vault rules. |
| `account_copy_cookie` | Mac equivalent: Keychain/native authentication and shared vault rules. |
| `accounts_export_cookies` | Shared; compiled/tested where covered, live acceptance pending. |
| `cookie_export_delete` | Shared; compiled/tested where covered, live acceptance pending. |
| `cookie_export_reveal` | Shared; compiled/tested where covered, live acceptance pending. |
| `account_info` | Shared; compiled/tested where covered, live acceptance pending. |
| `accounts_headshots` | Shared; compiled/tested where covered, live acceptance pending. |
| `accounts_presence` | Shared; compiled/tested where covered, live acceptance pending. |
| `browse_open` | Unavailable protected embedded browser. |
| `browse_bounds` | Unavailable protected embedded browser. |
| `browse_card` | Unavailable protected embedded browser. |
| `browse_nav` | Unavailable protected embedded browser. |
| `browse_close` | Unavailable protected embedded browser; close closes isolated Mac sessions. |
| `playtime_list` | Shared; compiled/tested where covered, live acceptance pending. |
| `games_popular` | Shared; compiled/tested where covered, live acceptance pending. |
| `target_parse` | Shared; compiled/tested where covered, live acceptance pending. |
| `take_pending_link` | Unavailable external protocol handoff; parsing pasted links works. |
| `market_items` | Shared; compiled/tested where covered, live acceptance pending. |
| `asset_thumbnails` | Shared; compiled/tested where covered, live acceptance pending. |
| `clips_status` | Unavailable recorder status with explanation. |
| `clips_save` | Unavailable: emits explanatory error. |
| `clips_list` | Partial: existing-file library only; capture unavailable. |
| `clips_rename` | Partial: existing-file library only; capture unavailable. |
| `clips_delete` | Mac equivalent: moves existing file to ~/.Trash. |
| `clips_reveal` | Partial: existing-file library only; capture unavailable. |
| `clips_open_folder` | Partial: existing-file library only; capture unavailable. |
| `clips_folder` | Partial: existing-file library only; capture unavailable. |
| `mods_list` | Partial: shared ZIP library only; application unavailable. |
| `mods_import` | Partial: shared ZIP library only; application unavailable. |
| `mods_export` | Partial: shared ZIP library only; application unavailable. |
| `mods_export_name` | Partial: shared ZIP library only; application unavailable. |
| `mods_remove` | Partial: shared ZIP library only; application unavailable. |
| `mods_set_on` | Partial: pack metadata only; global mod application disabled. |
| `mods_reorder` | Partial: shared ZIP library only; application unavailable. |
| `user_lookup` | Shared; compiled/tested where covered, live acceptance pending. |
| `launch` | Shared flow with native installed-player launch adapter. |
| `favourites_list` | Shared; compiled/tested where covered, live acceptance pending. |
| `favourite_save` | Shared; compiled/tested where covered, live acceptance pending. |
| `favourite_remove` | Shared; compiled/tested where covered, live acceptance pending. |
| `favourites_reorder` | Shared; compiled/tested where covered, live acceptance pending. |
| `join_candidates` | Shared; compiled/tested where covered, live acceptance pending. |
| `instances_list` | Shared; compiled/tested where covered, live acceptance pending. |
| `instance_close` | Mac equivalent: native tracked-process control. |
| `instance_move` | Mac equivalent: Accessibility/process identity. |
| `account_login_cancel` | Shared; compiled/tested where covered, live acceptance pending. |
| `instances_close_all` | Mac equivalent: native tracked-process control. |
| `instances_clear_ended` | Shared; compiled/tested where covered, live acceptance pending. |
| `instance_set_limits` | Mac equivalent: CPU/priority; rejects affinity/trim. |
| `instance_trim` | Unavailable: returns unsupported; no memory trimming. |
| `instance_set_anti_afk` | Shared; compiled/tested where covered, live acceptance pending. |
| `instances_metrics` | Mac equivalent: CPU/footprint/IO; GPU system-only. |
| `system_usage` | Mac equivalent: CPU/footprint/IO; GPU system-only. |
| `instance_stop_relaunch` | Shared; compiled/tested where covered, live acceptance pending. |
| `instances_retry_relaunch` | Shared; compiled/tested where covered, live acceptance pending. |
| `instance_thumbnail` | Mac equivalent: ScreenCaptureKit; minimized/permission status. |
| `instance_thumbs` | Unavailable: Windows DWM overlays; screenshot previews instead. |
| `instance_show` | Mac equivalent: Accessibility/process identity. |
| `instances_show_all` | Mac equivalent: Accessibility/process identity. |
| `instance_micro` | Partial: AX resize limited by Roblox minimum. |
| `instances_tile` | Partial: primary-display AX layout. |
| `layout_preview` | Partial: native logical coordinates/primary-display geometry. |
| `layout_preview_stop` | Mac equivalent: closes preview windows. |
| `bandwidth_status` | Unavailable: no bandwidth shaping. |
| `bandwidth_apply` | Unavailable: rejected. |
| `multi_instance_status` | Mac equivalent: native tracked-process control. |
| `multi_instance_set` | Mac equivalent: native tracked-process control. |
| `multi_instance_take_over` | Different: Mac singleton mechanism, not Windows handle takeover. |
| `roblox_close_all` | Mac equivalent: native tracked-process control. |
| `panic_kill` | Mac equivalent: native tracked-process control. |
| `crashes_list` | Shared; compiled/tested where covered, live acceptance pending. |
| `diagnostics_text` | Shared; compiled/tested where covered, live acceptance pending. |
| `diagnostics_save` | Shared; compiled/tested where covered, live acceptance pending. |
| `server_history` | Shared; compiled/tested where covered, live acceptance pending. |
| `server_history_recent` | Shared; compiled/tested where covered, live acceptance pending. |
| `crash_open_log` | Shared; compiled/tested where covered, live acceptance pending. |
| `discord_get` | Shared; compiled/tested where covered, live acceptance pending. |
| `discord_set` | Shared; compiled/tested where covered, live acceptance pending. |
| `discord_test` | Shared; compiled/tested where covered, live acceptance pending. |
| `bulk_launch` | Shared flow with native installed-player launch adapter. |
| `bulk_cancel` | Shared; compiled/tested where covered, live acceptance pending. |
| `bulk_current` | Shared; compiled/tested where covered, live acceptance pending. |
| `servers_list` | Shared; compiled/tested where covered, live acceptance pending. |
| `game_info` | Shared; compiled/tested where covered, live acceptance pending. |
| `game_stats` | Shared; compiled/tested where covered, live acceptance pending. |
| `account_value` | Shared; compiled/tested where covered, live acceptance pending. |
| `player_stats` | Shared; compiled/tested where covered, live acceptance pending. |
| `player_value` | Shared; compiled/tested where covered, live acceptance pending. |
| `target_place` | Shared; compiled/tested where covered, live acceptance pending. |
| `roblox_client` | Shared flow with native installed-player launch adapter. |
| `roblox_install` | Unavailable: use installed Mac player. |
| `roblox_launchers` | Unavailable: no registered launcher list. |
| `roblox_launcher_set` | Unavailable: registration rejected. |
| `news` | Shared; compiled/tested where covered, live acceptance pending. |
| `extras` | Shared; compiled/tested where covered, live acceptance pending. |
| `online_ping` | Shared; compiled/tested where covered, live acceptance pending. |
| `self_update_status` | Mac equivalent: fork ZIP update mechanism. |
| `self_update_check` | Mac equivalent: fork ZIP update mechanism. |
| `self_update_install` | Mac equivalent: fork ZIP update mechanism. |
| `data_dir_info` | Shared; compiled/tested where covered, live acceptance pending. |
| `data_dir_open` | Shared; compiled/tested where covered, live acceptance pending. |
| `data_dir_move` | Shared; compiled/tested where covered, live acceptance pending. |
| `logs_open` | Shared; compiled/tested where covered, live acceptance pending. |
| `logs_clean` | Shared; compiled/tested where covered, live acceptance pending. |
| `browser_data_reset` | Partial: ephemeral sessions have no persistent profiles; refuses while browsing. |
| `debug_tail` | Shared; compiled/tested where covered, live acceptance pending. |
| `debug_log` | Shared; compiled/tested where covered, live acceptance pending. |
| `background_set` | Shared; compiled/tested where covered, live acceptance pending. |
| `background_clear` | Shared; compiled/tested where covered, live acceptance pending. |
| `background_get` | Shared; compiled/tested where covered, live acceptance pending. |
| `shutdown_schedule` | Unavailable: system shutdown rejected. |
| `shutdown_cancel` | Unavailable system shutdown; shared cancellation state. |
| `shutdown_status` | Unavailable system shutdown; shared status. |
| `private_servers_list` | Shared; compiled/tested where covered, live acceptance pending. |
| `private_server_get` | Shared; compiled/tested where covered, live acceptance pending. |
| `private_server_update` | Shared; compiled/tested where covered, live acceptance pending. |
| `private_server_renew` | Shared; compiled/tested where covered, live acceptance pending. |
| `private_server_buy` | Shared; compiled/tested where covered, live acceptance pending. |
| `outfits_list` | Shared; compiled/tested where covered, live acceptance pending. |
| `outfit_wear` | Shared; compiled/tested where covered, live acceptance pending. |
| `outfit_save` | Shared; compiled/tested where covered, live acceptance pending. |
| `outfit_rename` | Shared; compiled/tested where covered, live acceptance pending. |
| `outfit_delete` | Shared; compiled/tested where covered, live acceptance pending. |
| `trades_list` | Shared; compiled/tested where covered, live acceptance pending. |
| `trade_details` | Shared; compiled/tested where covered, live acceptance pending. |
| `trade_accept` | Shared; compiled/tested where covered, live acceptance pending. |
| `trade_decline` | Shared; compiled/tested where covered, live acceptance pending. |
| `friend_requests_list` | Shared; compiled/tested where covered, live acceptance pending. |
| `friend_request_answer` | Shared; compiled/tested where covered, live acceptance pending. |
| `friend_requests_decline_all` | Shared; compiled/tested where covered, live acceptance pending. |
| `robux_transactions` | Shared; compiled/tested where covered, live acceptance pending. |
| `robux_totals` | Shared; compiled/tested where covered, live acceptance pending. |
| `private_servers_owned` | Shared; compiled/tested where covered, live acceptance pending. |
| `playtime_days` | Shared; compiled/tested where covered, live acceptance pending. |
| `crashes_export` | Shared; compiled/tested where covered, live acceptance pending. |
| `playtime_export` | Shared; compiled/tested where covered, live acceptance pending. |
| `groups_list` | Shared; compiled/tested where covered, live acceptance pending. |
| `group_save` | Shared; compiled/tested where covered, live acceptance pending. |
| `group_remove` | Shared; compiled/tested where covered, live acceptance pending. |
| `backup_export` | Shared; compiled/tested where covered, live acceptance pending. |
| `backup_import` | Shared; compiled/tested where covered, live acceptance pending. |
| `performance_profile` | Partial: Mac-valid CPU/graphics/preview subset. |
| `performance_export` | Shared; compiled/tested where covered, live acceptance pending. |
| `performance_read` | Shared; compiled/tested where covered, live acceptance pending. |
| `performance_parse` | Shared; compiled/tested where covered, live acceptance pending. |
| `performance_benchmark` | Partial: Mac-valid recommendations; not proof of saved RAM. |
| `performance_specs` | Mac equivalent: CPU sysctl, unified-memory GPU label; no VRAM claim. |
| `settings_get` | Partial: sanitizes imported unsupported Windows fields. |
| `settings_set` | Partial: rejects unsupported features; shared settings otherwise. |
| `network_status` | Unavailable Windows classification; no category detection. |

## Individual settings and shortcut inventory

Stable setting IDs below come from the integrated registry and feature entry files. Labels are English catalog labels; localizations use the same shared code. A setting may only appear when its parent/condition is enabled. Shared rows retain upstream behavior unless a specific difference is listed.

| ID | Setting | Mac status |
|---|---|---|
| `multi` | Multi-instance | Mac equivalent; see native differences above. |
| `protectLogin` | Protect each client's sign-in | Mac equivalent; see native differences above. |
| `launchDelay` | Launch delay | Shared. |
| `launchArgs` | Launch arguments | Shared. |
| `protectMain` | Keep MAIN out of select-all | Shared. |
| `autoRejoin` | Auto-rejoin | Shared. |
| `autoRestart` | Auto-restart after a crash | Shared. |
| `breaker` | Give up after | Shared. |
| `restartEvery` | Restart clients every | Shared. |
| `closeAfter` | Close clients after | Shared. |
| `antiAfk` | Anti-AFK | Mac equivalent; see native differences above. |
| `antiAfkEvery` | Press a key every | Mac equivalent; see native differences above. |
| `panic` | Panic hotkey | Mac equivalent; see native differences above. |
| `keyToggle` | Show or hide RoALT | Shared. |
| `keyTile` | Tile windows | Mac equivalent; see native differences above. |
| `keyMinimize` | Minimize all | Mac equivalent; see native differences above. |
| `closeGrace` | Close grace | Shared. |
| `shutdown` | Shut down the PC | Unavailable / hidden on Mac. |
| `defaultLauncher` | Default Roblox launcher | Unavailable / hidden on Mac. |
| `askBrowserLinks` | Pick the account for browser Play links | Unavailable / hidden on Mac. |
| `client` | Roblox client | Partial; see feature table for exact limitation. |
| `autoUpdate` | Update Roblox automatically | Unavailable / hidden on Mac. |
| `theme` | Theme | Shared. |
| `themes` | Themes | Shared. |
| `look` | Your colours | Shared. |
| `lookShare` | Share your theme | Shared. |
| `accent` | Accent colour | Shared. |
| `corners` | Corners | Shared. |
| `background` | Background image | Shared. |
| `backgroundDim` | How much the theme covers the picture | Shared. |
| `backgroundMotion` | Animated picture moves | Shared. |
| `density` | Density | Shared. |
| `tabLayout` | Tab layout | Shared. |
| `language` | Language | Shared. |
| `sounds` | Sound effects | Shared. |
| `motion` | Motion | Shared. |
| `closeToTray` | Close to tray | Shared. |
| `skipSplash` | Skip splash screen | Partial; see feature table for exact limitation. |
| `encryption` | Cookie encryption | Partial; see feature table for exact limitation. |
| `masterPassword` | Master password | Shared. |
| `recoveryCode` | Recovery code | Shared. |
| `autoLock` | Auto-lock after idle | Shared. |
| `clipboard` | Clear clipboard after copying a cookie | Shared. |
| `signInWatch` | Watch sign-ins | Shared. |
| `lockPurchases` | Lock purchases | Unavailable / hidden on Mac. |
| `publicNetworks` | Allow public networks | No classification: disabling permission blocks launch; kept reversible. |
| `dataFolder` | Data folder | Shared. |
| `backup` | Backup file | Shared. |
| `cache` | Keep account info for | Shared. |
| `cleanLogs` | Clean Roblox logs | Shared. |
| `browserData` | Roblox browser data | Partial; see feature table for exact limitation. |
| `selfUpdate` | Check for RoALT updates | Shared. |
| `onlineCount` | Show how many people use RoALT | Shared. |
| `showSponsor` | Show the sponsor in News | Shared. |
| `rolimons` | Rolimons values | Shared. |
| `version` | Open releases page | Shared. |
| `support` | Join our Discord | Shared. |
| `acct.main` | MAIN account | Shared. |
| `acct.fullQuality` | Full quality | Shared. |
| `acct.bringBack` | Bring it back if it drops | Shared. |
| `acct.discordMuted` | Mute Discord alerts | Shared. |
| `acct.launchArgs` | Launch arguments | Shared. |
| `acct.fpsCap` | FPS cap | Shared. |
| `acct.customFlags` | Custom FFlags | Partial; see feature table for exact limitation. |
| `launch.windowLayout` | Window layout | Partial; see feature table for exact limitation. |
| `launch.fpsCap` | FPS cap for this launch | Shared. |
| `client.antiAfk` | Anti-AFK for this client | Shared. |
| `mods.on` | Use modpacks | Unavailable applying mods; disabled with explanation. |
| `presets` | Presets | Partial; see feature table for exact limitation. |
| `maxAlts` | Max alts | Partial; see feature table for exact limitation. |
| `balanced` | Balanced | Partial; see feature table for exact limitation. |
| `defaults` | Roblox defaults | Partial; see feature table for exact limitation. |
| `bench` | For this PC | Partial; see feature table for exact limitation. |
| `fpsCap` | FPS cap | Shared. |
| `lowGraphics` | Lowest graphics | Shared. |
| `lighterRendering` | Lighter rendering | Shared. |
| `msaa` | Anti-aliasing (MSAA) | Shared. |
| `texture` | Texture quality | Shared. |
| `renderMode` | Rendering mode | Unavailable / hidden on Mac. |
| `graySky` | Gray sky | Shared. |
| `noGrass` | No grass | Shared. |
| `lowestFrm` | Lowest render quality | Shared. |
| `pauseVoxelizer` | Pause lighting updates | Shared. |
| `customFlags` | Custom FFlags | Partial; see feature table for exact limitation. |
| `minimize` | Minimize on launch | Mac equivalent; see native differences above. |
| `tinyOnLaunch` | Start clients tiny | Partial; see feature table for exact limitation. |
| `mute` | Mute clients on launch | Unavailable / hidden on Mac. |
| `adaptive` | Adaptive limits | Mac equivalent; see native differences above. |
| `adaptiveBg` | In the background | Partial; see feature table for exact limitation. |
| `fairCap` | Fair CPU cap | Mac equivalent; see native differences above. |
| `spread` | Spread clients over CPU cores | Unavailable / hidden on Mac. |
| `trim` | Trim memory every | Unavailable / hidden on Mac. |
| `memThrottle` | Throttle on high memory | Mac equivalent; see native differences above. |
| `memThrottleCpu` | Throttle CPU limit | Shared. |
| `memRestart` | Restart on high memory | Shared. |
| `previewFps` | Preview frame rate | Mac equivalent; see native differences above. |
| `bandwidth` | Bandwidth limit | Unavailable / hidden on Mac. |
| `share` | Share your setup | Shared. |
| `presence` | Show on Discord | Shared. |
| `webhook` | Webhook | Shared. |
| `ping` | Ping | Shared. |
| `statusSummary` | Status summary | Shared. |
| `clipAccount` | Record | Unavailable / hidden on Mac. |
| `clipSeconds` | Clip length | Unavailable / hidden on Mac. |
| `clipHotkey` | Clip key | Unavailable / hidden on Mac. |
| `ev.launch` | Launches | Shared. |
| `ev.crash` | Crashes | Shared. |
| `ev.hang` | Stopped responding | Shared. |
| `ev.disconnect` | Disconnects | Shared. |
| `ev.signIn` | Sign-in problems | Shared. |
| `ev.restart` | Rejoins and restarts | Shared. |
| `ev.memRestart` | Memory restarts | Shared. |
| `ev.breaker` | Gave up | Shared. |
| `ev.batchDone` | Batch finished | Shared. |
| `ev.throttle` | Memory throttle | Shared. |
| `ev.update` | RoALT updates | Shared. |
| `ev.trade` | New trades | Shared. |
| `ev.pc` | PC nearly full | Shared. |
| `key.dashboard` | Go to Home | Shared. |
| `key.accounts` | Go to Accounts | Shared. |
| `key.launch` | Go to Play | Shared. |
| `key.servers` | Go to Servers | Shared. |
| `key.settings` | Go to Settings | Shared. |
| `key.monitor` | Go to Monitor | Shared. |
| `key.discord` | Go to Discord | Shared. |
| `key.debug` | Go to Debug | Shared. |
| `key.performance` | Go to Performance | Shared. |
| `key.add` | Add account | Shared. |
| `key.find` | Find | Shared. |
| `key.selectAll` | Select all shown | Shared. |
| `key.validate` | Check selected, or all | Shared. |
| `key.remove` | Remove selected | Shared. |
| `key.launchNow` | Press Play | Shared. |
| `key.refresh` | Refresh what's shown | Shared. |
| `key.lock` | Lock vault (master password) | Shared. |
| `key.close` | Close dialog, clear selection | Shared. |
| `key.help` | Show shortcuts | Shared. |

The Mac-only App Nap and allocator-release controls are outside the shared registry. The UI also retains upstream sorting, filtering, pagination, selection, copy actions, empty/error/loading states, confirmation dialogs and responsive layouts inside each shared feature. Their source is merged; this is not a claim of pixel-by-pixel or every-path manual verification.

## Validation and what still needs a real Mac/client

- Frontend production build: passed (TypeScript, i18n validation, Vite).
- Sounds/news/liquid/look checks: passed.
- Rust locked suite: **517 passed, 0 failed, 5 ignored**. Ignored tests include live-service/diagnostic probes; they are not counted as passes.
- Apple Silicon app bundle: built. Local toolchain reported an optional debug-symbol stripping warning (missing LLVM dylib); linking/bundling succeeded.
- Added unit coverage for per-account profile limits/manual overrides and stripping unsupported Windows limits while preserving Mac CPU/priority settings.
- Windows CI must validate Windows compilation/tests; a Mac build does not prove that platform.
- No authenticated account workflows or new performance benchmark were run. No automatic RAM-reduction guarantee.

Manual acceptance before calling this release fully verified:

1. Export an encrypted backup; test an upgrade on a copy. Verify account/group/default/profile migrations. Old binaries cannot open the new schema.
2. Grant this built app its own privacy permissions. Test show/minimize/restore/tiling and previews, including Retina scaling, fullscreen, Spaces and denied permissions.
3. Launch multiple authenticated clients; check progress reaches In game, teleport/last-server behavior, batch cancel, disconnect/crash rejoin and breaker handling.
4. Exercise manual caps, inherited profiles, Full quality, foreground/loading exemptions and efficiency priority; check recovery when RoALT/governor exits.
5. Test F13 anti-AFK in a real experience for more than 20 minutes. Confirm Roblox counts it, human input interrupts it, and prior focus/minimized state is restored. No background-input guarantee.
6. Test login/captcha, cookie rotation, lock while account browser is open, reveal/export and data-folder migration without risking the only copy of the vault.
7. Test Discord Presence/webhook events, global shortcuts, menu-bar restore, themes and sound on this build.
8. Test social/trade/private-server actions intentionally with your accounts; their APIs can reject/challenge requests independently of this port.
9. Test fork update check and a newer signed artifact's installation/rollback/reopen. Ad-hoc identity changes can require renewed privacy grants.
10. Confirm unsupported Browse/capture/mod-apply/audio/trim/affinity/default-launcher functions remain unavailable rather than silently doing something else.

This inventory covers the merged public feature surfaces and every registered command/settings row. It is not proof of behavioral equivalence for every upstream bug fix or operating-system edge case.
