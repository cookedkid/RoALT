import { invoke } from "@tauri-apps/api/core";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Switch } from "../../ui/controls";
import { toastError } from "../../ui/Toast";

type MacOptionsValue = { keyCode: 48 | 49; focusFallback: boolean; disableAppNap: boolean; releaseUnusedMemory: boolean };
const queryKey = ["mac-options"];

/** Render only on Mac; these commands are intentionally absent from Windows. */
export function MacOptions() {
  const qc = useQueryClient();
  const query = useQuery({ queryKey, queryFn: () => invoke<MacOptionsValue>("mac_options_get") });
  const change = useMutation({
    mutationFn: (options: MacOptionsValue) => invoke<MacOptionsValue>("mac_options_set", { options }),
    onSuccess: (options) => qc.setQueryData(queryKey, options),
    onError: toastError,
  });
  const options = query.data;
  return <section className="panel p-5" aria-label="Mac background behavior">
    <h2 className="section-title">Mac background behavior</h2>
    {query.isError ? <p role="alert" className="text-dim mt-3">Could not load Mac settings. <button className="btn btn-sm" onClick={() => query.refetch()}>Try again</button></p> : !options ? <p className="text-dim mt-3">Loading…</p> :
      <fieldset disabled={change.isPending} className="flex flex-col gap-5 mt-4 disabled:opacity-60">
        <div className="flex items-center justify-between gap-4">
          <div><p>Release unused memory sooner (experimental)</p><p className="text-dim text-[13px]">Applies on the next launch for accounts without Full quality. May shorten memory spikes after Roblox frees data, but can slow allocations.</p></div>
          <Switch label="Release unused memory sooner" checked={options.releaseUnusedMemory} onChange={(releaseUnusedMemory) => change.mutate({ ...options, releaseUnusedMemory })} />
        </div>
        <div className="flex items-center justify-between gap-4">
          <div><p>Disable Roblox App Nap</p><p className="text-dim text-[13px]">Applies to all Roblox clients the next time Roblox launches. May increase power use. Turning this off restores the prior preference. Effectiveness needs testing on your Mac.</p></div>
          <Switch label="Disable Roblox App Nap" checked={options.disableAppNap} onChange={(disableAppNap) => change.mutate({ ...options, disableAppNap })} />
        </div>
      </fieldset>}
  </section>;
}
