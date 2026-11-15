import { useState, type FormEvent } from "react";
import { asError } from "../api";
import { t } from "../i18n";
import { Button } from "./Button";
import { Field, Input } from "./Field";
import { friendly } from "./Toast";

/** A dialog's body for one name (new, or a rename). `submit` closes the dialog itself; its
 * refusal shows at the field. Sends once: a second Enter while it is under way does nothing. */
export function NameForm({ initial, maxLength, submit, onClose }: { initial: string; maxLength?: number; submit: (name: string) => Promise<unknown>; onClose: () => void }) {
  const [value, setValue] = useState(initial);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const send = (e: FormEvent) => {
    e.preventDefault();
    if (busy || !value.trim()) return;
    setBusy(true);
    submit(value.trim())
      .catch((err) => setError(friendly(asError(err))))
      .finally(() => setBusy(false));
  };
  return (
    <form onSubmit={send}>
      <div className="dlg-body">
        <Field label={t("clips.name")} error={error}>
          {({ id, describedBy, invalid }) => (
            <Input id={id} value={value} maxLength={maxLength} invalid={invalid} aria-describedby={describedBy} autoFocus onChange={(e) => {
              setValue(e.target.value);
              setError(null);
            }} />
          )}
        </Field>
      </div>
      <div className="dlg-foot">
        <Button size="md" onClick={onClose}>{t("common.cancel")}</Button>
        <Button size="md" type="submit" variant="primary" busy={busy} disabled={!value.trim()}>{t("common.save")}</Button>
      </div>
    </form>
  );
}
