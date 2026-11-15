// UI translations: one flat JSON catalog per language in ./locales, English (en.json) the
// reference and built in. Every other catalog is its own chunk, loaded when its language is
// chosen: main.tsx loads the last one before the first render, and Root (`useLang`) follows
// Settings.language, re-rendering the whole tree once the new catalog is in.
//
// Catalog strings use `{name}` placeholders. A plural key has one entry per CLDR category of its
// language (`key_one`, `key_few`, `key_many`, `key_other`); `t(key, { count })` picks one with
// Intl.PluralRules. `scripts/check-i18n.mjs` fails the build when a catalog misses a key.
import { Fragment, createElement, useEffect, useReducer, type ReactNode } from "react";
import { readStored, writeStored } from "../ui/stored";
import en from "./locales/en.json";

/** Language codes with their own name, in picker order. */
export const LANGUAGES = [
  ["en", "English"],
  ["ar", "العربية"],
  ["bs", "Bosanski"],
  ["de", "Deutsch"],
  ["es", "Español"],
  ["fil", "Filipino"],
  ["fr", "Français"],
  ["id", "Bahasa Indonesia"],
  ["ja", "日本語"],
  ["ko", "한국어"],
  ["pl", "Polski"],
  ["pt", "Português (Brasil)"],
  ["ru", "Русский"],
  ["sv", "Svenska"],
  ["th", "ไทย"],
  ["tr", "Türkçe"],
  ["vi", "Tiếng Việt"],
  ["zh", "简体中文"],
] as const;
export type Lang = (typeof LANGUAGES)[number][0];

type Catalog = Record<string, string>;
const loaders = import.meta.glob<Catalog>(["./locales/*.json", "!./locales/en.json"], { import: "default" });
const catalogs: Partial<Record<Lang, Catalog>> = { en };
/** The last language shown, so the next start loads it before the first render. */
const LANG_KEY = "roalt.lang";

type PluralSuffix = "_one" | "_few" | "_many" | "_other";
type Base<K> = K extends `${infer B}${PluralSuffix}` ? B : K;
export type Key = Base<keyof typeof en>;
type Params = Record<string, string | number>;

let lang: Lang = "en";
let rules = new Intl.PluralRules("en");
let numbers = new Intl.NumberFormat("en");

const isLang = (l: string): l is Lang => LANGUAGES.some(([code]) => code === l);

/** A Settings.language value ("" follows Windows) to a supported language; else English. */
export function resolveLang(pref: string | undefined): Lang {
  if (pref && isLang(pref)) return pref;
  for (const tag of navigator.languages ?? [navigator.language]) {
    const base = tag.toLowerCase().split("-")[0];
    if (isLang(base)) return base;
  }
  return "en";
}

/** Switches to a language whose catalog is loaded; cheap when it is already current. */
function apply(next: Lang) {
  if (next === lang) return;
  lang = next;
  rules = new Intl.PluralRules(lang);
  numbers = new Intl.NumberFormat(lang);
  document.documentElement.lang = lang;
  document.documentElement.dir = lang === "ar" ? "rtl" : "ltr";
  // Without storage, the next start shows English until settings load.
  writeStored(LANG_KEY, lang);
}

/** Loads `next`'s catalog; false when it couldn't be (the current language stays). */
async function load(next: Lang) {
  try {
    catalogs[next] ??= await loaders[`./locales/${next}.json`]();
    return true;
  } catch (e) {
    console.error(`language ${next} failed to load: ${e}`);
    return false;
  }
}

/** Before the first render: the language shown last, so the start doesn't flash English. */
export async function loadLastLang() {
  // No storage: the Windows language.
  const next = resolveLang(readStored(LANG_KEY) ?? undefined);
  if (await load(next)) apply(next);
}

/** Shows Settings.language (`undefined` while settings load: what is shown stays). At once when
 *  its catalog is loaded; else the current language stays until it is, then the tree re-renders. */
export function useLang(pref: string | undefined) {
  const [, loaded] = useReducer((n: number) => n + 1, 0);
  const next = pref === undefined ? null : resolveLang(pref);
  if (next && catalogs[next]) apply(next);
  useEffect(() => {
    if (next && !catalogs[next]) void load(next).then((ok) => ok && loaded());
  }, [next]);
}

export const getLang = () => lang;
/** The layout runs right to left: screen X in pointer and arrow-key logic is mirrored. */
export const isRtl = () => lang === "ar";

// Platform-specific security copy must describe the actual credential provider.
const macCopy: Partial<Record<Key, string>> = {
 "settings.browserDataHint": "Mac account windows use temporary browser sessions. Close them to discard their browser data. Saved account cookies stay in the vault.",
 "settings.selfUpdateHint": "Check this fork’s Apple Silicon Mac releases at startup and every six hours. Installation verifies the download and may require renewed macOS permissions.",
 "performance.graphicsHint": "Applies when RoALT launches an alt. RoALT restores the original graphics file after startup.",
 "performance.hintLight": "Use the graphics overrides below for new clients. Full quality accounts keep their own graphics.",
 "settings.antiAfkEvery": "Press F13 every",
 "settings.antiAfkHint": "Opt-in: briefly bring each due client forward and press F13 while you are idle, then restore your previous focus. Requires Accessibility and Input Monitoring in System Settings → Privacy & Security; restart RoALT after granting permissions.",
 "performance.previewFpsHint": "Window-only captures require Screen Recording permission. Lower rates save CPU. Minimized windows have no live preview.",
 "launch.limits.cpu": "Cooperative CPU cap (% of whole Mac)",
 "settings.multiHint": "Launch separate macOS Roblox clients directly. Existing clients keep running; no Roblox executable is modified.",
 "settings.protectLoginHint": "New Roblox clients cannot read or write shared native cookie stores. Each client uses its own launch ticket; existing clients are unchanged. This does not isolate all Roblox preferences or logs.",
 "settings.efficiency": "Efficiency mode (background priority)",
 "settings.adaptiveBgEco": "Lower background CPU and I/O priority",
 "settings.adaptiveBgHint": "Apply cooperative CPU caps and background scheduling priority while a client is not in front.",
 "launch.limits.eco": "Efficiency mode (background priority)",
 "launch.limits.help": "A manual CPU cap overrides only the CPU cap; other automatic background limits still apply. Enable Full quality on the account to opt out of automatic limits. CPU cap is a percentage of total Mac CPU capacity, enforced by brief process pauses. It may stutter and is not a kernel quota. A separate guardian resumes clients if RoALT or the governor exits or stalls. Background priority lowers CPU and I/O scheduling priority.",
 "settings.memRestartHint": "Restart a client whose physical memory footprint stays over this value for 30 seconds. 0 turns it off.",
 "monitor.memory": "Physical footprint",
 "monitor.memoryAll": "Physical footprint, all clients",
 "performance.intro": "Tune FPS, graphics, cooperative CPU caps and background priority. CPU caps use brief pauses and can stutter; percentages refer to the entire Mac, not one core.",
 "settings.minimizeHint": "Minimize after launch. Enable RoALT in System Settings → Privacy & Security → Accessibility first.",
 "home.welcomeLead": "Manage your Roblox accounts on Mac. Three steps to your first game.",
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
  const cat = catalogs[lang] ?? (en as Catalog);
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

// Formatters by language + options: toLocaleString builds a new one per call when given options,
// and Num formats every animation frame. The key holds the language, so a switch never reuses one.
const formats = new Map<string, Intl.NumberFormat>();
/** A number in the current language (grouping, decimals). */
export const fmtNum = (n: number, o?: Intl.NumberFormatOptions) => {
  if (!o) return n.toLocaleString(lang);
  const key = lang + JSON.stringify(o);
  let f = formats.get(key);
  if (!f) formats.set(key, (f = new Intl.NumberFormat(lang, o)));
  return f.format(n);
};
/** A file's size in MB; a tiny one shows as 0.1 MB, not 0. */
export const fmtSize = (bytes: number) => t("mods.size", { mb: fmtNum(Math.max(0.1, bytes / 1_048_576), { maximumFractionDigits: bytes < 10_485_760 ? 1 : 0 }) });
/** A date, time or both in the current language. */
export const fmtDate = (d: Date | number, o?: Intl.DateTimeFormatOptions) => new Date(d).toLocaleString(lang, o);
/** A list like "a, b and c" in the current language. */
export const fmtList = (items: string[], type: Intl.ListFormatType = "conjunction") => new Intl.ListFormat(lang, { type }).format(items);
