import type { Entry } from "../settings/registry";

const clips = { view: "clips" } as const;

/** Clips' settings for the settings registry (search, jumps, rows), in screen order. */
export const CLIPS_ENTRIES: readonly Entry[] = [
  {
    id: "clipAccount", place: clips, label: "clips.record", hint: "clips.recordHint", field: "clipAccount", control: { kind: "custom" },
    aliases: ["clip", "replay", "shadowplay", "record", "video", "highlight"],
  },
  {
    id: "clipSeconds", place: clips, label: "clips.length", hint: "clips.lengthHint", field: "clipSeconds",
    control: {
      kind: "segmented",
      label: "clips.length",
      options: [15, 30, 60, 120].map((n) => ({ value: n, label: { key: "clips.seconds", params: { n } } })),
    },
    aliases: ["seconds", "duration"],
  },
  {
    id: "clipHotkey", place: clips, label: "clips.key", hint: "clips.keyHint", field: "clipHotkey",
    control: { kind: "text", placeholder: "settings.placeholderOff", size: "md" }, aliases: ["hotkey", "shortcut", "save clip"],
  },
];
