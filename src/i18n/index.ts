// UI translations: one flat JSON catalog per language in ./locales, English (en.json) the
// reference. `t` reads the current language from module state; Root sets it from
// Settings.language on every render, so a change re-renders the whole tree in the new language.
//
// Catalog strings use `{name}` placeholders. A plural key has one entry per CLDR category of its
// language (`key_one`, `key_few`, `key_many`, `key_other`); `t(key, { count })` picks one with
// Intl.PluralRules. `scripts/check-i18n.mjs` fails the build when a catalog misses a key.
import { Fragment, createElement, type ReactNode } from "react";
import bs from "./locales/bs.json";
import de from "./locales/de.json";
import en from "./locales/en.json";
import id from "./locales/id.json";
import pl from "./locales/pl.json";
import ru from "./locales/ru.json";
import sv from "./locales/sv.json";
import tr from "./locales/tr.json";

/** Language codes with their own name, in picker order. */
export const LANGUAGES = [
  ["en", "English"],
  ["bs", "Bosanski"],
  ["de", "Deutsch"],
  ["id", "Bahasa Indonesia"],
  ["pl", "Polski"],
  ["ru", "Русский"],
  ["sv", "Svenska"],
  ["tr", "Türkçe"],
] as const;
export type Lang = (typeof LANGUAGES)[number][0];

type Catalog = Record<string, string>;
const catalogs: Record<Lang, Catalog> = { en, bs, de, id, pl, ru, sv, tr };

type PluralSuffix = "_one" | "_few" | "_many" | "_other";
type Base<K> = K extends `${infer B}${PluralSuffix}` ? B : K;
export type Key = Base<keyof typeof en>;
type Params = Record<string, string | number>;

let lang: Lang = "en";
let rules = new Intl.PluralRules("en");
let numbers = new Intl.NumberFormat("en");

const isLang = (l: string): l is Lang => l in catalogs;

/** A Settings.language value ("" follows Windows) to a supported language; else English. */
export function resolveLang(pref: string | undefined): Lang {
  if (pref && isLang(pref)) return pref;
  for (const tag of navigator.languages ?? [navigator.language]) {
    const base = tag.toLowerCase().split("-")[0];
    if (isLang(base)) return base;
  }
  return "en";
}

/** Switches the UI language; cheap when it is already current. */
export function setLang(pref: string | undefined) {
  const next = resolveLang(pref);
  if (next === lang) return;
  lang = next;
  rules = new Intl.PluralRules(lang);
  numbers = new Intl.NumberFormat(lang);
  document.documentElement.lang = lang;
}

export const getLang = () => lang;

// Platform-specific security copy must describe the actual credential provider.
const macCopy: Partial<Record<Key, string>> = {
 "settings.antiAfkHint": "Opt-in: briefly focus visible tracked clients and send Space while you are idle. Requires Accessibility and Input Monitoring in System Settings → Privacy & Security; restart RoALT after granting permissions.",
 "settings.antiAfkFocusHint": "After 5 seconds without keyboard or mouse activity, briefly focus each due client and send Space. Manual input interrupts the round. Minimized clients are skipped. Game acceptance is not guaranteed.",
 "dashboard.previewFpsHint": "Window-only captures require Screen Recording permission. Lower rates save CPU. Minimized windows have no live preview.",
 "launch.limits.cpu": "Cooperative CPU cap (% of whole Mac)",
 "settings.multiHint": "Launch separate macOS Roblox clients directly. Existing clients keep running; no Roblox executable is modified.",
 "settings.protectLoginHint": "New Roblox clients cannot read or write shared native cookie stores. Each client uses its own launch ticket; existing clients are unchanged. This does not isolate all Roblox preferences or logs.",
 "launch.multi.takeOverText": "Enable separate Roblox launches on macOS. Existing clients are left running and unchanged.",
 "settings.efficiency": "Background priority",
 "settings.adaptiveBgEco": "Lower background CPU and I/O priority",
 "settings.adaptiveBgHint": "Apply cooperative CPU caps and background scheduling priority while a client is not in front.",
 "launch.limits.eco": "Background priority",
 "launch.limits.help": "CPU cap is a percentage of total Mac CPU capacity, enforced by brief process pauses. It may stutter and is not a kernel quota. A separate guardian resumes clients if RoALT or the governor exits or stalls. Background priority lowers CPU and I/O scheduling priority.",
 "settings.memRestartHint": "Restart a client whose physical memory footprint stays over this value for 30 seconds. 0 turns it off.",
 "monitor.privateMemory": "Physical footprint",
 "monitor.privateMemoryAll": "Physical footprint, all clients",
 "performance.intro": "Tune FPS, graphics, cooperative CPU caps and background priority. CPU caps use brief pauses and can stutter; percentages refer to the entire Mac, not one core.",
 "settings.minimizeHint": "Minimize after launch. Enable RoALT in System Settings → Privacy & Security → Accessibility first.",
 "home.welcomeLead": "Manage your Roblox accounts on Mac. Three steps to your first game.",
 "home.noLimit": "Launch multiple Roblox clients with multi-instance enabled.",
 "home.step1Text": "Sign in through Roblox’s own page. Your sign-in is encrypted and stays on this Mac.",
 "app.encrypted": "Encrypted on this Mac",
 "app.vaultUnlockedDpapi": "Vault · Keychain · unlocked",
 "settings.encryptionDpapi": "macOS Keychain",
 "settings.encryptionHintDpapi": "macOS Keychain protects the local vault key. Set a master password for an additional unlock prompt.",
 "settings.clipboardHintHello": "Set a master password to copy or reveal cookies on macOS.",
 "accounts.detail.cookieAskHello": "Set a master password in Settings before revealing or copying cookies.",
 "settings.removeMasterText": "Cookies will be protected by macOS Keychain and open without a master-password prompt.",
 "settings.removeMasterDone": "Master password removed. The vault now opens with macOS Keychain.",
 "settings.languageWindows": "System",
 "settings.languageHint": "System follows your macOS display language when available, otherwise English.",
 "settings.themeHint": "System follows the macOS appearance.",
 "settings.publicHint": "Network classification is unavailable on macOS. Turning this off blocks all launches.",
};

function lookup(key: Key, p?: Params): string {
  if (navigator.platform.startsWith("Mac") && macCopy[key]) return macCopy[key]!;
  const cat = catalogs[lang];
  const plural = typeof p?.count === "number" ? `${key}_${rules.select(p.count)}` : null;
  return (plural && (cat[plural] ?? cat[`${key}_other`])) ?? cat[key] ?? (en as Catalog)[key] ?? (en as Catalog)[`${key}_other`] ?? key;
}

/** `count` is shown as a localized number; other numbers as they are (ports, ids, years). */
const show = (name: string, v: string | number) => (name === "count" && typeof v === "number" ? numbers.format(v) : String(v));

/** The translated string for `key`, with `{name}` placeholders filled from `p`. */
export function t(key: Key, p?: Params): string {
  return lookup(key, p).replace(/\{(\w+)\}/g, (m, n: string) => (p && n in p ? show(n, p[n]) : m));
}

/** Like `t`, for strings with markup: `{name}` may be a React node, and `<b>text</b>` calls
 *  `tags.b("text")`. Tags don't nest. */
export function tx(key: Key, p: Record<string, ReactNode | string | number> = {}, tags: Record<string, (s: string) => ReactNode> = {}): ReactNode {
  const text = lookup(key, p as Params);
  const out: ReactNode[] = [];
  let last = 0;
  for (const m of text.matchAll(/<(\w+)>(.*?)<\/\1>|\{(\w+)\}/g)) {
    out.push(text.slice(last, m.index));
    if (m[1]) out.push(tags[m[1]] ? tags[m[1]](m[2]) : m[2]);
    else {
      const v = p[m[3]];
      out.push(v === undefined ? m[0] : typeof v === "number" ? show(m[3], v) : v);
    }
    last = m.index + m[0].length;
  }
  out.push(text.slice(last));
  return createElement(Fragment, null, ...out);
}

/** A number in the current language (grouping, decimals). */
export const fmtNum = (n: number, o?: Intl.NumberFormatOptions) => n.toLocaleString(lang, o);
/** A date, time or both in the current language. */
export const fmtDate = (d: Date | number, o?: Intl.DateTimeFormatOptions) => new Date(d).toLocaleString(lang, o);
/** A list like "a, b and c" in the current language. */
export const fmtList = (items: string[], type: Intl.ListFormatType = "conjunction") => new Intl.ListFormat(lang, { type }).format(items);
