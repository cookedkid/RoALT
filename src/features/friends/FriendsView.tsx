import "../trades/trades.css";
import { useState } from "react";
import { useQueries, useQueryClient } from "@tanstack/react-query";
import { api, asError, type AccountSummary } from "../../api";
import { browseAs } from "../../app/navigation";
import type { FriendRequest } from "../../bindings/FriendRequest";
import { qk, useAccounts } from "../../data";
import { t } from "../../i18n";
import { Disclosure, Skeleton } from "../../ui/Bits";
import { Button } from "../../ui/Button";
import { ConfirmDialog } from "../../ui/Dialog";
import { Select } from "../../ui/Field";
import { useStored } from "../../ui/stored";
import { friendly, toast } from "../../ui/Toast";
import { accountName, ago, Avatar } from "../accounts/shared";

/** Friend requests: the ones every valid account got, each to accept or decline, and all of one
 * account's to decline at once. There is no "accept all" on purpose. It loads on open and on
 * Refresh, never on a timer. */
export function FriendsView() {
  const qc = useQueryClient();
  const valid = (useAccounts().data ?? []).filter((a) => a.status === "valid");
  // Kept: coming back shows the account last picked ("": all).
  const [only, setOnly] = useStored("roalt.friendsAccount", (s) => s ?? "", (v) => v);
  const [declineAll, setDeclineAll] = useState<AccountSummary | null>(null);
  // The answer being sent: its row waits.
  const [busy, setBusy] = useState<string | null>(null);
  // The last answer Roblox refused, shown in its row.
  const [refused, setRefused] = useState<{ key: string; text: string } | null>(null);

  const picked = valid.some((a) => String(a.id) === only) ? only : "";
  const accounts = valid.filter((a) => !picked || String(a.id) === picked);
  const lists = useQueries({
    queries: accounts.map((a) => ({ queryKey: qk.friendRequests(a.id), queryFn: () => api.friendRequestsList(a.id), staleTime: 5 * 60_000 })),
  });
  const failed = lists.flatMap((q, i) => (q.error ? [{ account: accounts[i]!, text: friendly(asError(q.error)) }] : []));
  const loading = lists.some((q) => q.isPending);
  const withSome = accounts.flatMap((a, i) => (lists[i]?.data?.length ? [{ account: a, rows: lists[i]!.data! }] : []));

  // After a failure too: Roblox may have taken the answer all the same.
  const reread = (accountId: number) => void qc.invalidateQueries({ queryKey: qk.friendRequests(accountId) });
  const answer = (account: AccountSummary, r: FriendRequest, accept: boolean) => {
    if (busy) return;
    const key = `${account.id}:${r.userId}`;
    setBusy(key);
    setRefused(null);
    api
      .friendRequestAnswer(account.id, r.userId, accept)
      .then(
        () => toast(t(accept ? "friends.accepted" : "friends.declined")),
        (e) => setRefused({ key, text: friendly(asError(e)) }),
      )
      .finally(() => {
        setBusy(null);
        reread(account.id);
      });
  };
  const declineEvery = () => {
    const account = declineAll;
    if (!account || busy) return;
    setBusy(`${account.id}:all`);
    api
      .friendRequestsDeclineAll(account.id)
      .then(
        () => toast(t("friends.declinedAll")),
        (e) => toast(friendly(asError(e)), "error"),
      )
      .finally(() => {
        setBusy(null);
        setDeclineAll(null);
        reread(account.id);
      });
  };

  return (
    <main className="scroll flex-1 min-w-0 page">
      <div className="trades-page">
        <header className="trades-head">
          <div className="trades-bar">
            <h1 className="page-title">{t("app.nav.friends")}</h1>
            {valid.length > 0 && (
              <Button icon="refresh" busy={lists.some((q) => q.isFetching)} onClick={() => void qc.invalidateQueries({ queryKey: ["friendRequests"] })}>
                {t("ui.refresh")}
              </Button>
            )}
          </div>
          <p className="page-sub">{t("friends.lead")}</p>
          {valid.length > 1 && (
            <div className="trades-bar">
              <Select
                aria-label={t("monitor.account")}
                className="trades-select"
                value={picked}
                onChange={setOnly}
                options={[{ value: "", label: t("trades.allAccounts") }, ...valid.map((a) => ({ value: String(a.id), label: accountName(a) }))]}
              />
            </div>
          )}
        </header>

        {valid.length === 0 ? (
          <p className="text-fg-2">{t("servers.noValidAccount")}</p>
        ) : (
          <div className="flex flex-col gap-5">
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
            {withSome.map(({ account, rows }) => (
              <section key={account.id} aria-label={accountName(account)} className="flex flex-col gap-2">
                <div className="trades-bar">
                  <h2 className="trades-who">
                    <Avatar account={account} size={20} />
                    <span className="truncate font-medium">{accountName(account)}</span>
                    <span className="meta">{t("friends.count", { n: rows.length })}</span>
                  </h2>
                  <Button size="sm" variant="ghost" aria-haspopup="dialog" onClick={() => setDeclineAll(account)}>
                    {t("friends.declineAll")}
                  </Button>
                </div>
                <ul className="flex flex-col gap-2">
                  {rows.map((r) => {
                    const key = `${account.id}:${r.userId}`;
                    return (
                      <li key={r.userId} className="panel trades-card">
                        <div className="trades-card-head">
                          {r.avatarUrl ? <img src={r.avatarUrl} alt="" width={32} height={32} className="rounded-full flex-none" loading="lazy" /> : <span aria-hidden="true" className="w-8 h-8 rounded-full bg-surface-3 flex-none" />}
                          <div className="flex-1 min-w-0 flex flex-col">
                            <h3 className="font-medium truncate">{r.displayName || r.name || t("accounts.detail.userId", { id: r.userId })}</h3>
                            <span className="meta truncate">
                              {r.name && <span className="mono">@{r.name}</span>}
                              {r.name && r.mutual > 0 && " · "}
                              {r.mutual > 0 && t("friends.mutual", { n: r.mutual })}
                            </span>
                          </div>
                          {r.sentAt && <span className="meta">{t("trades.sent", { when: ago(Date.parse(r.sentAt)) })}</span>}
                        </div>
                        <div className="trades-card-foot">
                          <span />
                          <div className="trades-actions">
                            <Button size="md" variant="ghost" icon="globe" onClick={() => browseAs(account.id, `users/${r.userId}/profile`)}>{t("servers.openOnRoblox")}</Button>
                            <Button size="md" busy={busy === key} onClick={() => answer(account, r, false)}>{t("trades.decline")}</Button>
                            <Button size="md" variant="primary" busy={busy === key} onClick={() => answer(account, r, true)}>{t("trades.accept")}</Button>
                          </div>
                        </div>
                        {refused?.key === key && <p role="alert" className="alert">{refused.text}</p>}
                      </li>
                    );
                  })}
                </ul>
              </section>
            ))}
            {loading ? (
              <Skeleton width="100%" height={120} />
            ) : (
              withSome.length === 0 &&
              failed.length < lists.length && (
                <div className="empty">
                  <p className="font-semibold text-fg">{t("friends.emptyTitle")}</p>
                  <p>{t("friends.emptyText")}</p>
                </div>
              )
            )}
          </div>
        )}
      </div>

      <ConfirmDialog
        open={declineAll !== null}
        onClose={() => setDeclineAll(null)}
        onConfirm={declineEvery}
        busy={busy !== null}
        title={t("friends.declineAllTitle", { name: declineAll ? accountName(declineAll) : "" })}
        text={t("friends.declineAllText")}
        confirmLabel={t("friends.declineAll")}
      />
    </main>
  );
}
