import "./clips.css";
import { useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, events } from "../../api";
import type { ClipItem } from "../../bindings/ClipItem";
import type { ClipsStatus } from "../../bindings/ClipsStatus";
import { qk, useAccounts } from "../../data";
import { fmtDate, fmtSize, t } from "../../i18n";
import { Disclosure, Skeleton } from "../../ui/Bits";
import { Button, IconButton } from "../../ui/Button";
import { ConfirmDialog, Dialog } from "../../ui/Dialog";
import { Select } from "../../ui/Field";
import { Icon } from "../../ui/Icon";
import { MenuItem, MenuSeparator, Popover, useAnchor } from "../../ui/Popover";
import { NameForm } from "../../ui/NameForm";
import { StatusPill } from "../../ui/Status";
import { LoadFailed, toast, toastError } from "../../ui/Toast";
import { useInView } from "../../ui/useInView";
import { accountName } from "../accounts/shared";
import { useRow } from "../settings/kit/rowError";
import { Renderers, SettingRow, type RowRenderers } from "../settings/kit/SettingRow";
import { SettingSection } from "../settings/kit/SettingSection";
import { CLIPS_ENTRIES } from "./entries";

/** A clip as the player reads it (the `clip` protocol: only RoALT's own window, only clips). */
const clipUrl = (name: string) => `http://clip.localhost/${encodeURIComponent(name)}`;
const titleOf = (name: string) => name.replace(/\.mp4$/, "");
/** The one rename or delete dialog open, and the clip (its file name) it is about. */
type Act = { kind: "rename" | "delete"; name: string };

/** Which account's client is recorded; Off frees the clip key. A refusal shows at the row. */
function AccountControl() {
  const { ctx, write, describedBy } = useRow();
  const accounts = useAccounts().data ?? [];
  return (
    <Select
      aria-label={t("clips.record")}
      aria-describedby={describedBy}
      value={ctx.s.clipAccount == null ? "0" : String(ctx.s.clipAccount)}
      onChange={(v) => void write({ clipAccount: Number(v) })}
      options={[{ value: "0", label: t("common.off") }, ...accounts.map((a) => ({ value: String(a.id), label: accountName(a) }))]}
    />
  );
}

function AccountRow() {
  return (
    <SettingRow id="clipAccount">
      <AccountControl />
    </SettingRow>
  );
}

const RENDERERS: RowRenderers = { clipAccount: AccountRow };

/** Clips: the last seconds of one picked account's game, saved with a key, like ShadowPlay. */
export function ClipsView() {
  const qc = useQueryClient();
  const status = useQuery({ queryKey: qk.clipsStatus, queryFn: api.clipsStatus, refetchInterval: 2000 });
  const list = useQuery({ queryKey: qk.clips, queryFn: api.clipsList, staleTime: 0 });
  const folder = useQuery({ queryKey: ["clipsFolder"], queryFn: api.clipsFolder, staleTime: Infinity });
  const accounts = useAccounts().data ?? [];
  const [playing, setPlaying] = useState<string | null>(null);
  const [act, setAct] = useState<Act | null>(null);
  const [deleting, setDeleting] = useState(false);
  const close = () => setAct(null);
  const refresh = () => void qc.invalidateQueries({ queryKey: qk.clips });
  const remove = () => {
    if (!act || deleting) return;
    setDeleting(true);
    api
      .clipsDelete(act.name)
      .then(() => toast(t("clips.deleted", { name: titleOf(act.name) })), toastError)
      .finally(() => {
        setDeleting(false);
        // Only this dialog: the user may have cancelled and opened another meanwhile.
        setAct((a) => (a === act ? null : a));
        refresh();
      });
  };

  useEffect(() => {
    const off = events.onClipsChanged(() => void qc.invalidateQueries({ queryKey: qk.clips }));
    return () => void off.then((f) => f());
  }, [qc]);

  const s = status.data;
  const who = accounts.find((a) => a.id === s?.accountId);
  const recording = s?.state === "recording";
  return (
    <main className="scroll flex-1 min-w-0 page">
      <Renderers value={RENDERERS}>
        <div className="clips-page">
          <header className="clips-head">
            <div className="clips-title">
              <h1 className="page-title">{t("app.nav.clips")}</h1>
              {recording && (
                <Button variant="primary" icon="download" onClick={() => void api.clipsSave().catch(toastError)}>
                  {t("clips.save")}
                </Button>
              )}
            </div>
            <div className="clips-state">{s && <State status={s} name={who ? accountName(who) : null} />}</div>
            <Disclosure className="clips-lead" summary={t("clips.lead")}>
              {t("clips.intro")}
            </Disclosure>
          </header>
          <SettingSection ids={CLIPS_ENTRIES.map((e) => e.id)} />

          <section className="clips-library" aria-label={t("clips.library")}>
            {list.isError ? (
              <LoadFailed error={list.error} busy={list.isFetching} onRetry={() => void list.refetch()} />
            ) : !list.data ? (
              <div className="clips-grid">
                <Skeleton height={180} />
                <Skeleton height={180} />
              </div>
            ) : list.data.length === 0 ? (
              <div className="clips-empty">
                <Icon name="film" size={28} />
                <p className="clips-empty-title">{t("clips.emptyTitle")}</p>
                <p>{t("clips.emptyText")}</p>
              </div>
            ) : (
              <div className="clips-grid">
                {list.data.map((c) => (
                  <ClipCard key={c.name} clip={c} onPlay={() => setPlaying(c.name)} onAct={(kind) => setAct({ kind, name: c.name })} />
                ))}
              </div>
            )}
            <div className="clips-folder">
              <span className="clips-path mono" title={folder.data}>{folder.data}</span>
              <Button size="sm" icon="external" onClick={() => void api.clipsOpenFolder().catch(toastError)}>
                {t("clips.openFolder")}
              </Button>
            </div>
          </section>
        </div>
      </Renderers>

      <Dialog open={playing !== null} onClose={() => setPlaying(null)} title={playing ? titleOf(playing) : ""} width={880}>
        <div className="dlg-body">{playing && <video className="clips-player" src={clipUrl(playing)} controls autoPlay />}</div>
      </Dialog>
      <Dialog open={act?.kind === "rename"} onClose={close} title={t("clips.rename")}>
        {act?.kind === "rename" && (
          <NameForm
            key={act.name}
            initial={titleOf(act.name)}
            onClose={close}
            submit={(to) =>
              api.clipsRename(act.name, to).then(() => {
                close();
                refresh();
              })
            }
          />
        )}
      </Dialog>
      <ConfirmDialog
        open={act?.kind === "delete"}
        onClose={close}
        onConfirm={remove}
        busy={deleting}
        title={t("clips.deleteTitle", { name: act ? titleOf(act.name) : "" })}
        text={t("clips.deleteText")}
        confirmLabel={t("clips.delete")}
      />
    </main>
  );
}

/** Off, waiting for the client, recording (with or without its sound), or why it can't. */
function State({ status, name }: { status: ClipsStatus; name: string | null }) {
  const who = name ?? t("clips.pickAccount");
  switch (status.state) {
    case "off":
      return <StatusPill tone="neutral" label={t("common.off")} />;
    case "waiting":
      return <StatusPill tone="info" label={t("clips.waiting", { name: who })} />;
    case "recording":
      return <StatusPill tone="ok" pulse label={t("clips.recording", { name: who })} detail={status.sound ? undefined : (status.note ?? undefined)} />;
    case "unavailable":
      return <StatusPill tone="attention" label={t("clips.unavailable")} detail={status.note ?? undefined} />;
  }
}

function ClipCard({ clip, onPlay, onAct }: { clip: ClipItem; onPlay: () => void; onAct: (kind: Act["kind"]) => void }) {
  const more = useAnchor();
  const menuId = `clip-${clip.name.replace(/[^a-zA-Z0-9]/g, "")}-${clip.modified}`;
  // Only the cards in view load their picture: each is a video decoder.
  const [card, seen] = useInView<HTMLDivElement>();
  const title = titleOf(clip.name);

  return (
    <div ref={card} className="clips-card">
      <button type="button" className="clips-thumb" onClick={onPlay} aria-label={t("clips.play", { name: title })}>
        {seen && <video src={`${clipUrl(clip.name)}#t=1`} preload="metadata" muted tabIndex={-1} />}
        <span className="clips-play" aria-hidden="true">
          <Icon name="play" size={18} />
        </span>
      </button>
      <div className="clips-meta">
        <div className="clips-text">
          <span className="clips-name" title={title}>{title}</span>
          <span className="clips-sub">{[fmtDate(new Date(clip.modified), { dateStyle: "medium", timeStyle: "short" }), fmtSize(clip.bytes)].join(" · ")}</span>
        </div>
        <IconButton icon="dots" label={t("home.more")} popoverTarget={menuId} style={more.style} />
        <Popover id={menuId} anchor={more.name} placement="below-end" role="menu" label={t("home.more")}>
          <MenuItem icon="pencil" onSelect={() => onAct("rename")}>{t("clips.rename")}</MenuItem>
          <MenuItem icon="external" onSelect={() => void api.clipsReveal(clip.name).catch(toastError)}>{t("clips.reveal")}</MenuItem>
          <MenuSeparator />
          <MenuItem icon="trash" danger onSelect={() => onAct("delete")}>{t("clips.delete")}</MenuItem>
        </Popover>
      </div>
    </div>
  );
}
