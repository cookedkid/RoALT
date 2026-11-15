import "./outfits.css";
import { useEffect, useRef, useState, type RefObject } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api, asError, type AccountSummary } from "../../api";
import type { Outfit } from "../../bindings/Outfit";
import { qk, useAccounts } from "../../data";
import { fmtList, t } from "../../i18n";
import { Skeleton } from "../../ui/Bits";
import { Button, IconButton } from "../../ui/Button";
import { ConfirmDialog, Dialog } from "../../ui/Dialog";
import { Field, Select } from "../../ui/Field";
import { Icon } from "../../ui/Icon";
import { MenuItem, MenuSeparator, Popover, useAnchor } from "../../ui/Popover";
import { NameForm } from "../../ui/NameForm";
import { StatusDot } from "../../ui/Status";
import { useStored } from "../../ui/stored";
import { friendly, LoadFailed, toast, toastError } from "../../ui/Toast";
import { accountName, Avatar } from "../accounts/shared";
import { AccountPicker } from "../launch/AccountPicker";

/** Roblox's own limit on an outfit's name. */
const NAME_MAX = 25;
/** How often the list is asked again while Roblox still makes pictures. */
const PICTURE_TRIES = 5;

type Act = { kind: "save" } | { kind: "rename" | "delete" | "copy"; outfit: Outfit };

/** Outfits: one account's saved Roblox outfits. Wear one, copy it to other accounts, save the current
 * look, rename, delete. */
export function OutfitsView() {
  const qc = useQueryClient();
  const valid = (useAccounts().data ?? []).filter((a) => a.status === "valid");
  // Kept: coming back to Outfits (or RoALT) shows the account last picked.
  const [pickedId, setPickedId] = useStored<number | null>("roalt.outfitsAccount", (s) => Number(s) || null, String);
  const account = valid.find((a) => a.id === pickedId) ?? valid[0];
  const id = account?.id;
  const [act, setAct] = useState<Act | null>(null);
  const [wearing, setWearing] = useState<number | null>(null);
  const [deleting, setDeleting] = useState(false);
  // Bumped when the copy dialog closes: its loop stops before the next account.
  const copyRun = useRef(0);

  // Pictures Roblox is still making come on a later ask: a few more tries, then no more.
  const tries = useRef(0);
  const list = useQuery({
    queryKey: qk.outfits(id),
    enabled: id != null,
    staleTime: 0,
    queryFn: async () => {
      const o = await api.outfitsList(id!);
      tries.current = o.complete ? 0 : tries.current + 1;
      return o;
    },
    refetchInterval: (q) => (q.state.data?.complete === false && tries.current < PICTURE_TRIES ? 3000 : false),
  });
  const reload = () => {
    tries.current = 0;
    return qc.invalidateQueries({ queryKey: qk.outfits(id) });
  };
  // Roblox makes the new picture of the account a few seconds after its look changed.
  const later = useRef<ReturnType<typeof setTimeout>>(undefined);
  // Leaving the view stops a copy as closing its dialog does.
  useEffect(
    () => () => {
      clearTimeout(later.current);
      copyRun.current++;
    },
    [],
  );
  const lookChanged = () => {
    clearTimeout(later.current);
    later.current = setTimeout(() => void reload(), 4000);
  };

  const wear = (o: Outfit) => {
    if (id == null || wearing !== null) return;
    setWearing(o.id);
    api
      .outfitWear(id, o.id, id)
      .then((skipped) => {
        toast(skipped.length ? t("outfits.wornWithout", { name: o.name, items: fmtList(skipped) }) : t("outfits.nowWearing", { name: o.name }));
        lookChanged();
      }, toastError)
      .finally(() => setWearing(null));
  };
  const remove = (o: Outfit) => {
    if (id == null || deleting) return;
    setDeleting(true);
    api
      .outfitDelete(id, o.id)
      .then(() => toast(t("outfits.deleted", { name: o.name })), toastError)
      .finally(() => {
        setDeleting(false);
        // Only this dialog: the user may have cancelled and opened another meanwhile.
        setAct((a) => (a?.kind === "delete" && a.outfit === o ? null : a));
        void reload();
      });
  };
  const close = () => setAct(null);
  const named = act && act.kind !== "save" ? act.outfit : null;

  return (
    <main className="scroll flex-1 min-w-0 page">
      <div className="outfits-page">
        <header className="outfits-head">
          <div className="outfits-title">
            <h1 className="page-title">{t("app.nav.outfits")}</h1>
            {account && (
              <Button variant="primary" icon="plus" onClick={() => setAct({ kind: "save" })}>
                {t("outfits.saveLook")}
              </Button>
            )}
          </div>
          {account ? (
            <div className="outfits-who">
              <div className="outfits-current" aria-hidden="true">
                {list.data?.current && <img src={list.data.current} alt="" />}
              </div>
              <Field label={t("monitor.account")} hint={t("outfits.note")} className="flex-1 min-w-0">
                {({ id: fieldId, describedBy }) => (
                  <Select
                    id={fieldId}
                    aria-describedby={describedBy}
                    className="outfits-select"
                    value={String(account.id)}
                    onChange={(v) => {
                      tries.current = 0;
                      setPickedId(Number(v));
                    }}
                    options={valid.map((a) => ({ value: String(a.id), label: accountName(a) }))}
                  />
                )}
              </Field>
            </div>
          ) : (
            <p className="text-fg-2">{t("servers.noValidAccount")}</p>
          )}
        </header>

        {account && (
          <section aria-label={t("outfits.saved")}>
            {list.isError ? (
              <LoadFailed error={list.error} busy={list.isFetching} onRetry={() => void list.refetch()} />
            ) : !list.data ? (
              <div className="outfits-grid">
                <Skeleton height={230} />
                <Skeleton height={230} />
                <Skeleton height={230} />
              </div>
            ) : list.data.items.length === 0 ? (
              <div className="outfits-empty">
                <Icon name="shirt" size={28} />
                <p className="outfits-empty-title">{t("outfits.emptyTitle")}</p>
                <p>{t("outfits.emptyText")}</p>
              </div>
            ) : (
              <div className="outfits-grid">
                {list.data.items.map((o) => (
                  <Card key={o.id} outfit={o} busy={wearing === o.id} others={valid.length > 1} onWear={() => wear(o)} onAct={(kind) => setAct({ kind, outfit: o })} />
                ))}
              </div>
            )}
          </section>
        )}
      </div>

      <Dialog open={act?.kind === "save" || act?.kind === "rename"} onClose={close} title={act?.kind === "rename" ? t("clips.rename") : t("outfits.saveLook")}>
        {account && act && (act.kind === "save" || act.kind === "rename") && (
          <NameForm
            key={act.kind === "rename" ? act.outfit.id : "save"}
            initial={act.kind === "rename" ? act.outfit.name : ""}
            maxLength={NAME_MAX}
            onClose={close}
            submit={(name) =>
              (act.kind === "rename" ? api.outfitRename(account.id, act.outfit.id, name) : api.outfitSave(account.id, name)).then(() => {
                if (act.kind === "save") toast(t("outfits.savedAs", { name }));
                close();
                void reload();
              })
            }
          />
        )}
      </Dialog>
      <ConfirmDialog
        open={act?.kind === "delete"}
        onClose={close}
        onConfirm={() => named && remove(named)}
        busy={deleting}
        title={t("clips.deleteTitle", { name: named?.name ?? "" })}
        text={t("outfits.deleteText")}
        confirmLabel={t("outfits.delete")}
      />
      <Dialog
        open={act?.kind === "copy"}
        onClose={() => {
          copyRun.current++;
          close();
        }}
        title={t("outfits.copyTitle", { name: named?.name ?? "" })}
        width={480}
      >
        {account && act?.kind === "copy" && (
          <CopyForm
            key={act.outfit.id}
            from={account.id}
            outfit={act.outfit}
            others={valid.filter((a) => a.id !== account.id)}
            run={copyRun}
            onClose={() => {
              copyRun.current++;
              close();
            }}
          />
        )}
      </Dialog>
    </main>
  );
}

function Card({ outfit: o, busy, others, onWear, onAct }: { outfit: Outfit; busy: boolean; others: boolean; onWear: () => void; onAct: (kind: "rename" | "delete" | "copy") => void }) {
  const more = useAnchor();
  const menuId = `outfit-${o.id}`;
  return (
    <div className="outfits-card">
      <div className="outfits-thumb">{o.thumb ? <img src={o.thumb} alt="" loading="lazy" /> : <Icon name="shirt" size={28} />}</div>
      <span className="outfits-name" title={o.name}>{o.name}</span>
      <div className="outfits-actions">
        <Button size="md" block busy={busy} onClick={onWear}>{t("outfits.wear")}</Button>
        <IconButton icon="dots" label={t("home.more")} popoverTarget={menuId} style={more.style} />
        <Popover id={menuId} anchor={more.name} placement="below-end" role="menu" label={t("home.more")}>
          {others && <MenuItem icon="copy" onSelect={() => onAct("copy")}>{t("outfits.copy")}</MenuItem>}
          <MenuItem icon="pencil" onSelect={() => onAct("rename")}>{t("clips.rename")}</MenuItem>
          <MenuSeparator />
          <MenuItem icon="trash" danger onSelect={() => onAct("delete")}>{t("outfits.delete")}</MenuItem>
        </Popover>
      </div>
    </div>
  );
}

type Copied = { account: AccountSummary; skipped?: string[]; error?: string };

/** Pick accounts, then each wears the outfit, one after another. `run` changes when the dialog
 * closes, which stops the loop before the next account; Stop lets the account under way finish. */
function CopyForm({ from, outfit, others, run, onClose }: { from: number; outfit: Outfit; others: AccountSummary[]; run: RefObject<number>; onClose: () => void }) {
  const [picked, setPicked] = useState<number[]>([]);
  const [total, setTotal] = useState(0);
  const [done, setDone] = useState<Copied[]>([]);
  const [state, setState] = useState<"running" | "stopping" | "done">("done");
  const stop = useRef(false);
  const targets = others.filter((a) => picked.includes(a.id));

  const start = async () => {
    const mine = run.current;
    setTotal(targets.length);
    setState("running");
    for (const account of targets) {
      const r: Copied = await api.outfitWear(from, outfit.id, account.id).then(
        (skipped) => ({ account, skipped }),
        (e) => ({ account, error: friendly(asError(e)) }),
      );
      if (run.current !== mine) return;
      setDone((list) => [...list, r]);
      if (stop.current) break;
    }
    setState("done");
  };

  if (!total)
    return (
      <>
        <div className="dlg-body flex flex-col gap-3">
          <p className="text-fg-2">{t("outfits.copyLead")}</p>
          <AccountPicker variant="list" accounts={others} picked={picked} onPicked={setPicked} label={t("outfits.copyTo")} />
        </div>
        <div className="dlg-foot">
          <Button size="md" variant="primary" disabled={!targets.length} onClick={() => void start()}>
            {t("common.copy")}
          </Button>
        </div>
      </>
    );
  return (
    <>
      <div className="dlg-body flex flex-col gap-3">
        <p className="tabular-nums" role="status">{t("outfits.progress", { done: done.length, total })}</p>
        <ul className="well scroll max-h-[260px] p-1.5">
          {done.map(({ account, skipped, error }) => (
            <li key={account.id} className="outfits-result">
              <Avatar account={account} size={20} />
              <span className="outfits-result-text">
                <span className="truncate">{accountName(account)}</span>
                <span className="outfits-result-note">
                  {error ?? (skipped?.length ? t("outfits.skipped", { items: fmtList(skipped) }) : t("outfits.worn"))}
                </span>
              </span>
              {/* The note beside it already says it in words. */}
              <span aria-hidden="true">
                <StatusDot tone={error ? "err" : skipped?.length ? "attention" : "ok"} label={error ? t("outfits.failed") : t("outfits.worn")} />
              </span>
            </li>
          ))}
        </ul>
      </div>
      <div className="dlg-foot">
        {state === "done" ? (
          <Button size="md" onClick={onClose}>{t("common.done")}</Button>
        ) : (
          <Button size="md" busy={state === "stopping"} disabled={state === "stopping"} onClick={() => {
            stop.current = true;
            setState("stopping");
          }}>{t("accounts.add.stop")}</Button>
        )}
      </div>
    </>
  );
}
