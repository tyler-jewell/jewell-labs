/** Editable config form sections (credentials, allowlist, inbound, notify). */

import type { SyntheticEvent } from "react";
import {
  SUGGESTED_SECRET_KEYS,
  type SettingsFormState,
} from "./form-model.js";
import { Field, Section } from "./components.js";
import {
  buttonStyle,
  inputStyle,
  muted,
  primaryButton,
  row,
  stack,
} from "./styles.js";

type Props = {
  form: SettingsFormState;
  setField: <K extends keyof SettingsFormState>(
    key: K,
    value: SettingsFormState[K],
  ) => void;
  companyId: string | null;
  webhookUrlHint: string;
  configError: string | null;
  warnings: string[];
  saving: boolean;
  savedMessage: string | null;
  onSubmit: (event: SyntheticEvent<HTMLFormElement>) => void;
  onReload: () => void;
};

export function SettingsForm({
  form,
  setField,
  companyId,
  webhookUrlHint,
  configError,
  warnings,
  saving,
  savedMessage,
  onSubmit,
  onReload,
}: Props) {
  return (
    <form
      onSubmit={(e) => {
        onSubmit(e);
      }}
      style={stack}
    >
      <Section title="Credentials (company vault secrets)">
        <p style={muted}>
          Provision these secrets on each company that should use SendBlue
          (same key names → multi-company). Worker resolves them automatically.
          Optional ref overrides below if your host accepts secret refs in
          config (otherwise leave blank — soft settings still save). Never paste
          raw API keys here.
        </p>
        <ul style={{ ...muted, margin: "0 0 12px", paddingLeft: 18 }}>
          <li>
            <code>{SUGGESTED_SECRET_KEYS.apiKey}</code>
          </li>
          <li>
            <code>{SUGGESTED_SECRET_KEYS.apiSecret}</code>
          </li>
          <li>
            <code>{SUGGESTED_SECRET_KEYS.webhookSecret}</code>
          </li>
        </ul>
        <Field
          label="API key vault ref (optional override)"
          hint={`Default vault key: ${SUGGESTED_SECRET_KEYS.apiKey}`}
        >
          <input
            style={inputStyle}
            value={form.apiKeyRef}
            placeholder={SUGGESTED_SECRET_KEYS.apiKey}
            onChange={(e) => setField("apiKeyRef", e.target.value)}
          />
        </Field>
        <Field
          label="API secret vault ref (optional override)"
          hint={`Default vault key: ${SUGGESTED_SECRET_KEYS.apiSecret}`}
        >
          <input
            style={inputStyle}
            value={form.apiSecretRef}
            placeholder={SUGGESTED_SECRET_KEYS.apiSecret}
            onChange={(e) => setField("apiSecretRef", e.target.value)}
          />
        </Field>
        <Field
          label="Webhook secret vault ref (optional override)"
          hint={`Default: ${SUGGESTED_SECRET_KEYS.webhookSecret}`}
        >
          <input
            style={inputStyle}
            value={form.webhookSecretRef}
            placeholder={SUGGESTED_SECRET_KEYS.webhookSecret}
            onChange={(e) => setField("webhookSecretRef", e.target.value)}
          />
        </Field>
      </Section>

      <Section title="Lines & allowlist">
        <Field
          label="From number (E.164)"
          hint="Default SendBlue line used for outbound tools and notifies"
        >
          <input
            style={inputStyle}
            value={form.fromNumber}
            placeholder="+1645…"
            onChange={(e) => setField("fromNumber", e.target.value)}
          />
        </Field>
        <Field
          label="Notify number (E.164)"
          hint="Board event SMS recipient — must also appear on the allowlist"
        >
          <input
            style={inputStyle}
            value={form.notifyNumber}
            placeholder="+1…"
            onChange={(e) => setField("notifyNumber", e.target.value)}
          />
        </Field>
        <Field
          label="Allowlist"
          hint="Comma or newline separated E.164"
        >
          <textarea
            style={{ ...inputStyle, minHeight: 96, fontFamily: "inherit" }}
            value={form.allowlistText}
            placeholder={"+15551112222\n+15553334444"}
            onChange={(e) => setField("allowlistText", e.target.value)}
          />
        </Field>
        <label style={{ ...row, fontSize: 13 }}>
          <input
            type="checkbox"
            checked={form.emptyMeansDeny}
            onChange={(e) => setField("emptyMeansDeny", e.target.checked)}
          />
          Empty allowlist denies all SMS (recommended)
        </label>
      </Section>

      <Section title="Inbound messages">
        <Field
          label="Inbound mode"
          hint="What to do when an allowlisted sender texts the line"
        >
          <select
            style={inputStyle}
            value={form.inboundMode}
            onChange={(e) =>
              setField(
                "inboundMode",
                e.target.value as SettingsFormState["inboundMode"],
              )
            }
          >
            <option value="create_issue">
              create_issue — open a board issue + wake assignee (live board routing)
            </option>
            <option value="log_only">log_only — activity log only</option>
            <option value="ignore">ignore — ack webhook, no board action</option>
          </select>
        </Field>
        <p style={muted}>
          Company / project / assignee are <strong>not</strong> plugin settings —
          the worker resolves them live from the board (active company + engineer
          or CEO agent). Healthcheck reports whether routing works.
          {companyId ? (
            <>
              {" "}
              Active UI company: <code>{companyId}</code>.
            </>
          ) : null}
        </p>
        <div style={muted}>
          Point SendBlue <code>receive</code> webhooks at:
          <br />
          <code style={{ fontSize: 12 }}>{webhookUrlHint}</code>
        </div>
      </Section>

      <Section title="Notify on board events">
        <p style={muted}>
          When enabled, the plugin texts <code>notifyNumber</code> (allowlist
          gated).
        </p>
        {(
          [
            [
              "notifyOnIssueDone",
              "Issue marked done (off by default — agent SMS reply is enough)",
            ],
            ["notifyOnIssueCreated", "Issue created"],
            ["notifyOnApprovalCreated", "Approval requested"],
            ["notifyOnAgentError", "Agent run failed"],
          ] as const
        ).map(([key, label]) => (
          <label key={key} style={{ ...row, fontSize: 13 }}>
            <input
              type="checkbox"
              checked={form[key]}
              onChange={(e) => setField(key, e.target.checked)}
            />
            {label}
          </label>
        ))}
      </Section>

      {configError ? (
        <div style={{ color: "var(--destructive, #c00)", fontSize: 13 }}>
          {configError}
        </div>
      ) : null}
      {warnings.length > 0 ? (
        <div style={{ ...muted, color: "var(--destructive, #b45309)" }}>
          Worker warnings: {warnings.join("; ")}
        </div>
      ) : null}

      <div style={row}>
        <button type="submit" style={primaryButton} disabled={saving}>
          {saving ? "Saving…" : "Save settings"}
        </button>
        <button type="button" style={buttonStyle} onClick={onReload}>
          Reload
        </button>
        {savedMessage ? (
          <span style={{ fontSize: 12, opacity: 0.75 }}>{savedMessage}</span>
        ) : null}
      </div>
    </form>
  );
}
