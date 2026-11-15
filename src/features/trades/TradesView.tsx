import "./trades.css";
import { useId, useState } from "react";
import { useQueries, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, asError, type AccountSummary } from "../../api";
import { browseAs } from "../../app/navigation";
import type { TradeKind } from "../../bindings/TradeKind";
import type { TradeOffer } from "../../bindings/TradeOffer";
import type { TradeRow } from "../../bindings/TradeRow";
import { qk, useAccounts } from "../../data";
import { fmtNum, t } from "../../i18n";
import { Disclosure, Skeleton } from "../../ui/Bits";
import { Button } from "../../ui/Button";
import { ConfirmDialog } from "../../ui/Dialog";
import { Select } from "../../ui/Field";
import { useStored } from "../../ui/stored";
import { Tabs } from "../../ui/Tabs";
import { friendly, toast } from "../../ui/Toast";
import { useInView } from "../../ui/useInView";
import { accountName, ago, Avatar, formatDate } from "../accounts/shared";
import { GameIcon, Problem } from "../browse/Native";

const PAGE = 50;
const KINDS = ["inbound", "outbound", "completed"] as const;

/** What one side is worth: its items (at their Rolimons value while that setting is on, else RAP) and its Robux. */
const worth = (o: TradeOffer) => (o.value ?? o.rap) + o.robux;
const sentAt = (r: TradeRow) => (r.created && Date.parse(r.created)) || 0;

type Act = { what: "accept" | "decline"; trade: TradeRow; account: AccountSummary; give: number; get: number };

/** Trades: every valid account's trades in one list. Inbound ones can be accepted or declined,
 * outbound ones taken back. It loads on open and on Refresh, never on a timer. */
export function TradesView() {
  const qc = useQueryClient();
  const panelId = useId();
  const valid = (useAccounts().data ?? []).filter((a) => a.status === "valid");
  const [kind, setKind] = useState<TradeKind>("inbound");
  // Kept: coming back to Trades (or RoALT) shows the account last picked ("": all).
  const [only, setOnly] = useStored("roalt.tradesAccount", (s) => s ?? "", (v) => v);
  const [shown, setShown] = useState(PAGE);
  const [act, setAct] = useState<Act | null>(null);
  const [busy, setBusy] = useState(false);
  // The last answer Roblox refused, shown in its row.
  const [refused, setRefused] = useState<{ key: string; text: string } | null>(null);

  // A filter on an account that is no longer valid shows all.
  const picked = valid.some((a) => String(a.id) === only) ? only : "";
  const accounts = valid.filter((a) => !picked || String(a.id) === picked);
  const lists = useQueries({
    queries: accounts.map((a) => ({ queryKey: qk.trades(a.id, kind), queryFn: () => api.tradesList(a.id, kind), staleTime: 5 * 60_000 })),
  });
  const rows = lists
    .flatMap((q, i) => (q.data ?? []).map((trade) => ({ trade, account: accounts[i]!, key: `${accounts[i]!.id}:${trade.id}` })))
    .sort((a, b) => sentAt(b.trade) - sentAt(a.trade));
  const failed = lists.flatMap((q, i) => (q.error ? [{ account: accounts[i]!, text: friendly(asError(q.error)) }] : []));
  const loading = lists.some((q) => q.isPending);

  const answer = () => {
    if (!act || busy) return;
    const { what, trade, account } = act;
    setBusy(true);
    setRefused(null);
    (what === "accept" ? api.tradeAccept : api.tradeDecline)(account.id, trade.id)
      .then(
        () => toast(t(what === "accept" ? "trades.accepted" : "trades.declined")),
        (e) => setRefused({ key: `${account.id}:${trade.id}`, text: friendly(asError(e)) }),
      )
      .finally(() => {
        setBusy(false);
        setAct(null);
        // After a failure too: Roblox may have taken the answer all the same.
        void qc.invalidateQueries({ queryKey: ["trades", account.id] });
      });
  };

  return (
    <main className="scroll flex-1 min-w-0 page">
      <div className="trades-page">
        <header className="trades-head">
          <div className="trades-bar">
            <h1 className="page-title">{t("app.nav.trades")}</h1>
            {valid.length > 0 && (
              <Button icon="refresh" busy={lists.some((q) => q.isFetching)} onClick={() => void qc.invalidateQueries({ queryKey: ["trades"] })}>
                {t("ui.refresh")}
              </Button>
            )}
          </div>
          <p className="page-sub">{t("trades.lead")}</p>
          {valid.length > 0 && (
            <div className="trades-bar">
              <Tabs
                label={t("app.nav.trades")}
                value={kind}
                onChange={(k) => {
                  setKind(k);
                  setShown(PAGE);
                }}
                items={KINDS.map((k) => ({ value: k, label: t(`trades.${k}`) }))}
                panelId={panelId}
              />
              {valid.length > 1 && (
                <Select
                  aria-label={t("monitor.account")}
                  className="trades-select"
                  value={picked}
                  onChange={(v) => {
                    setOnly(v);
                    setShown(PAGE);
                  }}
                  options={[{ value: "", label: t("trades.allAccounts") }, ...valid.map((a) => ({ value: String(a.id), label: accountName(a) }))]}
                />
              )}
            </div>
          )}
        </header>

        {valid.length === 0 ? (
          <p className="text-fg-2">{t("servers.noValidAccount")}</p>
        ) : (
          <div id={panelId} role="tabpanel" aria-label={t(`trades.${kind}`)} className="flex flex-col gap-3">
            {failed.length > 0 && (
              // Open once every account failed: the reasons are then all there is to show.
              <Disclosure key={String(failed.length === lists.length)} defaultOpen={failed.length === lists.length} summary={t("trades.failedAccounts", { n: failed.length })}>
                <ul className="flex flex-col gap-1">
                  {failed.map(({ account, text }) => (
                    <li key={account.id}>
                      <span className="font-medium">{accountName(account)}</span>
                      <span className="text-fg-2">: {text}</span>
                    </li>
                  ))}
                </ul>
              </Disclosure>
            )}
            {rows.length > 0 && (
              <ul className="flex flex-col gap-3">
                {rows.slice(0, shown).map(({ trade, account, key }) => (
                  <TradeCard key={key} trade={trade} account={account} kind={kind} error={refused?.key === key ? refused.text : null} onAct={setAct} />
                ))}
              </ul>
            )}
            {loading ? (
              <Skeleton width="100%" height={180} />
            ) : (
              rows.length === 0 &&
              failed.length < lists.length && (
                <div className="empty">
                  <p className="font-semibold text-fg">{t("trades.emptyTitle")}</p>
                  <p>{t("trades.emptyText")}</p>
                </div>
              )
            )}
            {rows.length > shown && (
              <Button size="md" variant="ghost" className="self-start" onClick={() => setShown((n) => n + PAGE)}>
                {t("market.more", { shown, total: rows.length })}
              </Button>
            )}
          </div>
        )}
      </div>

      <ConfirmDialog
        open={act !== null}
        onClose={() => setAct(null)}
        onConfirm={answer}
        busy={busy}
        safe={act?.what === "accept"}
        title={t(act?.what === "accept" ? "trades.acceptTitle" : "trades.declineTitle", { name: act?.trade.partnerName ?? "" })}
        text={
          act?.what === "accept"
            ? t("trades.acceptText", { account: accountName(act.account), give: fmtNum(act.give), get: fmtNum(act.get) })
            : t("trades.declineText")
        }
        confirmLabel={t(act?.what === "accept" ? "trades.accept" : "trades.decline")}
      />
    </main>
  );
}

/** One trade. Its items load once the card comes near the viewport: a call to Roblox each. */
function TradeCard({ trade, account, kind, error, onAct }: { trade: TradeRow; account: AccountSummary; kind: TradeKind; error: string | null; onAct: (a: Act) => void }) {
  const [card, seen] = useInView<HTMLLIElement>();
  const detail = useQuery({ queryKey: qk.trade(account.id, trade.id), queryFn: () => api.tradeDetails(account.id, trade.id), enabled: seen });
  const d = detail.data;
  const ids = d ? [...new Set([...d.give.items, ...d.get.items].map((i) => i.assetId))] : [];
  const thumbs = useQuery({ queryKey: ["assetThumbs", ids], queryFn: () => api.assetThumbnails(ids), enabled: ids.length > 0 });
  const pic = new Map(thumbs.data ?? []);
  const net = d ? worth(d.get) - worth(d.give) : 0;
  const done = kind === "completed";
  const ask = (what: Act["what"]) => onAct({ what, trade, account, give: d ? worth(d.give) : 0, get: d ? worth(d.get) : 0 });

  return (
    <li ref={card} className="panel trades-card">
      <div className="trades-card-head">
        <span className="trades-who">
          <Avatar account={account} size={20} />
          <span className="truncate">{accountName(account)}</span>
        </span>
        <h2 className="font-medium truncate flex-1 min-w-0">{t("trades.with", { name: trade.partnerName })}</h2>
        <span className="meta">
          {trade.created && t("trades.sent", { when: ago(Date.parse(trade.created)) })}
          {!done && trade.created && trade.expires && " · "}
          {!done && trade.expires && t("trades.expires", { date: formatDate(Date.parse(trade.expires)) })}
        </span>
      </div>

      {detail.error ? (
        <Problem error={detail.error} onRetry={() => void detail.refetch()} />
      ) : !d ? (
        <Skeleton width="100%" height={120} />
      ) : (
        <div className="trades-sides">
          <Side title={t(done ? "trades.gave" : "trades.give")} offer={d.give} pic={pic} />
          <Side title={t(done ? "trades.got" : "trades.get")} offer={d.get} pic={pic} />
        </div>
      )}

      <div className="trades-card-foot">
        {d && (
          <span className="trades-verdict" data-tone={net > 0 ? "gain" : net < 0 ? "loss" : undefined}>
            {net > 0 ? t("trades.gain", { n: fmtNum(net) }) : net < 0 ? t("trades.loss", { n: fmtNum(-net) }) : t("trades.even")}
          </span>
        )}
        <div className="trades-actions">
          <Button size="md" variant="ghost" icon="globe" onClick={() => browseAs(account.id, "trades")}>{t("servers.openOnRoblox")}</Button>
          {!done && <Button size="md" onClick={() => ask("decline")}>{t("trades.decline")}</Button>}
          {/* Accept waits for the items: its question repeats what both sides are worth. */}
          {kind === "inbound" && <Button size="md" variant="primary" disabled={!d} onClick={() => ask("accept")}>{t("trades.accept")}</Button>}
        </div>
      </div>
      {error && <p role="alert" className="alert">{error}</p>}
    </li>
  );
}

function Side({ title, offer, pic }: { title: string; offer: TradeOffer; pic: Map<number, string> }) {
  return (
    <section className="well trades-side">
      <h3 className="meta">{title}</h3>
      <ul className="flex flex-col gap-2">
        {offer.items.map((it, i) => (
          // By place: one trade can hold two copies of the same item.
          <li key={i} className="trades-item">
            <GameIcon url={pic.get(it.assetId)} />
            <div className="min-w-0 flex flex-col">
              <span className="truncate">
                {it.name}
                {it.serial !== null && <span className="text-fg-3"> #{it.serial}</span>}
              </span>
              <span className="flex flex-wrap items-center gap-x-3 text-[13px] text-fg-2">
                <span>{t("value.rap")} <span className="mono">{fmtNum(it.rap)}</span></span>
                {it.rolimons?.value != null && <span>{t("value.value")} <span className="mono">{fmtNum(it.rolimons.value)}</span></span>}
                {it.rolimons?.projected && (
                  <span className="tag" style={{ color: "var(--attention)" }} title={t("value.projectedHint")}>
                    {t("value.projected")}
                  </span>
                )}
              </span>
            </div>
          </li>
        ))}
        {offer.robux > 0 && (
          <li className="trades-item">
            {t("accounts.detail.robux")} <span className="mono">{fmtNum(offer.robux)}</span>
          </li>
        )}
      </ul>
      {offer.items.length > 0 && (
        <p className="trades-total">
          <span>{t("value.rap")} <span className="mono">{fmtNum(offer.rap)}</span></span>
          {offer.value !== null && <span>{t("value.value")} <span className="mono">{fmtNum(offer.value)}</span></span>}
        </p>
      )}
    </section>
  );
}
