import "../trades/trades.css";
import { useState } from "react";
import { useQueries, useQueryClient } from "@tanstack/react-query";
import { api, asError, type AccountSummary, type PrivateServer } from "../../api";
import { browseAs } from "../../app/navigation";
import type { OwnedPrivateServer } from "../../bindings/OwnedPrivateServer";
import { qk, useAccounts, useGame } from "../../data";
import { fmtNum, t } from "../../i18n";
import { Disclosure, Skeleton } from "../../ui/Bits";
import { Button } from "../../ui/Button";
import { friendly } from "../../ui/Toast";
import { accountName, Avatar, formatDate } from "../accounts/shared";
import { GameIcon } from "../browse/Native";
import { PrivateServerDialog } from "./PrivateServerDialog";

/** A server that runs out within this long, and won't renew, is marked. */
const SOON_MS = 7 * 86_400_000;

type Row = { server: OwnedPrivateServer; account: AccountSummary };
const dueAt = (s: OwnedPrivateServer) => (s.expires && Date.parse(s.expires)) || Infinity;
/** The ones that run out first: active and not renewing, then the renewing ones, then the inactive. */
const rank = (s: OwnedPrivateServer) => (!s.active ? 2 : s.willRenew ? 1 : 0);

/** Private servers: the ones every valid account owns, with when each runs out or renews. It loads
 * on open and on Refresh, never on a timer. */
export function OwnedServersView() {
  const qc = useQueryClient();
  const valid = (useAccounts().data ?? []).filter((a) => a.status === "valid");
  // With the server as the settings dialog takes it, made once: the dialog follows that object.
  const [managing, setManaging] = useState<(Row & { asServer: PrivateServer }) | null>(null);
  const lists = useQueries({
    queries: valid.map((a) => ({ queryKey: qk.ownedServers(a.id), queryFn: () => api.privateServersOwned(a.id), staleTime: 5 * 60_000 })),
  });
  const rows: Row[] = lists
    .flatMap((q, i) => (q.data ?? []).map((server) => ({ server, account: valid[i]! })))
    .sort((a, b) => rank(a.server) - rank(b.server) || dueAt(a.server) - dueAt(b.server));
  const failed = lists.flatMap((q, i) => (q.error ? [{ account: valid[i]!, text: friendly(asError(q.error)) }] : []));
  const loading = lists.some((q) => q.isPending);
  // What the settings dialog asks for; the counts it doesn't show are left at 0.
  const manage = (r: Row) =>
    setManaging({ ...r, asServer: { vipServerId: r.server.vipServerId, name: r.server.name, ownerId: r.account.userId, ownerName: r.account.username, playing: 0, maxPlayers: 0, jobId: null, accessCode: null } });

  return (
    <main className="scroll flex-1 min-w-0 page">
      <div className="trades-page">
        <header className="trades-head">
          <div className="trades-bar">
            <h1 className="page-title">{t("servers.privateServers")}</h1>
            {valid.length > 0 && (
              <Button icon="refresh" busy={lists.some((q) => q.isFetching)} onClick={() => void qc.invalidateQueries({ queryKey: ["ownedServers"] })}>
                {t("ui.refresh")}
              </Button>
            )}
          </div>
          <p className="page-sub">{t("privates.lead")}</p>
        </header>

        {valid.length === 0 ? (
          <p className="text-fg-2">{t("servers.noValidAccount")}</p>
        ) : (
          <div className="flex flex-col gap-3">
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
                {rows.map((r) => (
                  <ServerCard key={`${r.account.id}:${r.server.vipServerId}`} row={r} onManage={() => manage(r)} />
                ))}
              </ul>
            )}
            {loading ? (
              <Skeleton width="100%" height={120} />
            ) : (
              rows.length === 0 &&
              failed.length < lists.length && (
                <div className="empty">
                  <p className="font-semibold text-fg">{t("privates.emptyTitle")}</p>
                  <p>{t("privates.emptyText")}</p>
                </div>
              )
            )}
          </div>
        )}
      </div>

      <PrivateServerDialog
        server={managing?.asServer ?? null}
        owner={managing?.account ?? null}
        accounts={valid}
        onClose={() => setManaging(null)}
        onChanged={() => managing && void qc.invalidateQueries({ queryKey: qk.ownedServers(managing.account.id) })}
      />
    </main>
  );
}

function ServerCard({ row: { server: s, account }, onManage }: { row: Row; onManage: () => void }) {
  const game = useGame(s.placeId).data;
  const due = dueAt(s);
  const date = Number.isFinite(due) ? formatDate(due) : null;
  const soon = s.active && !s.willRenew && due - Date.now() < SOON_MS;
  return (
    <li className="panel trades-card">
      <div className="trades-card-head">
        <GameIcon url={game?.iconUrl} />
        <div className="flex-1 min-w-0 flex flex-col">
          <h2 className="font-medium truncate">{s.name || t("servers.privateServer")}</h2>
          <span className="meta truncate">{s.game || game?.name || (s.placeId !== null ? t("dashboard.place", { id: s.placeId }) : "")}</span>
        </div>
        <span className="trades-who">
          <Avatar account={account} size={20} />
          <span className="truncate">{accountName(account)}</span>
        </span>
      </div>
      <div className="trades-card-foot">
        <span className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[13px]">
          {!s.active ? (
            <span className="text-fg-2">{t("privates.inactive")}</span>
          ) : date === null ? null : s.willRenew ? (
            <span className="text-fg-2">{t("privates.renews", { date })}</span>
          ) : (
            <span>{t(due < Date.now() ? "privates.ranOut" : "privates.runsOut", { date })}</span>
          )}
          {soon && (
            <span className="tag" style={{ color: "var(--attention)" }}>
              {t("privates.soon")}
            </span>
          )}
          {s.price !== null && <span className="text-fg-3">{s.price > 0 ? t("servers.pricePerMonth", { price: fmtNum(s.price) }) : t("privates.free")}</span>}
        </span>
        <div className="trades-actions">
          {s.placeId !== null && (
            <Button size="md" variant="ghost" icon="globe" onClick={() => browseAs(account.id, `games/${s.placeId}`)}>
              {t("servers.openOnRoblox")}
            </Button>
          )}
          <Button size="md" aria-haspopup="dialog" onClick={onManage}>
            {t("privates.manage")}
          </Button>
        </div>
      </div>
    </li>
  );
}
