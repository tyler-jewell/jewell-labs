import { useState } from "react";

/**
 * Settings surface for SendBlue.
 * Host mounts this as settingsPage exportName: SendBlueSettingsPage.
 * Full secret_ref pickers are host-dependent; document vault keys in README.
 */
export function SendBlueSettingsPage() {
  const [allowlist, setAllowlist] = useState("");
  const [notifyNumber, setNotifyNumber] = useState("");
  const [fromNumber, setFromNumber] = useState("");
  const [defaultCompanyId, setDefaultCompanyId] = useState("");
  const [inboundMode, setInboundMode] = useState("create_issue");
  const [notifyDone, setNotifyDone] = useState(true);
  const [notifyCreated, setNotifyCreated] = useState(false);
  const [notifyApproval, setNotifyApproval] = useState(true);
  const [notifyAgentError, setNotifyAgentError] = useState(true);

  return (
    <div
      style={{
        padding: 16,
        maxWidth: 640,
        fontFamily: "system-ui, sans-serif",
      }}
    >
      <h2 style={{ marginTop: 0 }}>SendBlue</h2>
      <p style={{ color: "#666", fontSize: 14 }}>
        Bind company secrets{" "}
        <code>sendblue-api-key</code>, <code>sendblue-api-secret</code>,{" "}
        <code>sendblue-webhook-secret</code> in the Paperclip vault, then set
        config fields <code>apiKeyRef</code> / <code>apiSecretRef</code> /{" "}
        <code>webhookSecretRef</code> to those vault keys. Allowlist every phone
        that may send or receive.
      </p>

      <label style={{ display: "block", marginBottom: 12 }}>
        From number (E.164 SendBlue line)
        <input
          value={fromNumber}
          onChange={(e) => setFromNumber(e.target.value)}
          placeholder="+1645…"
          style={{ display: "block", width: "100%", marginTop: 4 }}
        />
      </label>

      <label style={{ display: "block", marginBottom: 12 }}>
        Notify number (E.164, must be on allowlist)
        <input
          value={notifyNumber}
          onChange={(e) => setNotifyNumber(e.target.value)}
          placeholder="+1…"
          style={{ display: "block", width: "100%", marginTop: 4 }}
        />
      </label>

      <label style={{ display: "block", marginBottom: 12 }}>
        Allowlist (comma or newline separated E.164)
        <textarea
          value={allowlist}
          onChange={(e) => setAllowlist(e.target.value)}
          rows={4}
          style={{ display: "block", width: "100%", marginTop: 4 }}
        />
      </label>

      <label style={{ display: "block", marginBottom: 12 }}>
        Default company ID (inbound issue creation)
        <input
          value={defaultCompanyId}
          onChange={(e) => setDefaultCompanyId(e.target.value)}
          placeholder="uuid"
          style={{ display: "block", width: "100%", marginTop: 4 }}
        />
      </label>

      <label style={{ display: "block", marginBottom: 12 }}>
        Inbound mode
        <select
          value={inboundMode}
          onChange={(e) => setInboundMode(e.target.value)}
          style={{ display: "block", width: "100%", marginTop: 4 }}
        >
          <option value="create_issue">create_issue</option>
          <option value="log_only">log_only</option>
          <option value="ignore">ignore</option>
        </select>
      </label>

      <fieldset
        style={{ border: "1px solid #ddd", padding: 12, marginBottom: 12 }}
      >
        <legend>Notify on events</legend>
        <label style={{ display: "block" }}>
          <input
            type="checkbox"
            checked={notifyDone}
            onChange={(e) => setNotifyDone(e.target.checked)}
          />{" "}
          Issue done
        </label>
        <label style={{ display: "block" }}>
          <input
            type="checkbox"
            checked={notifyCreated}
            onChange={(e) => setNotifyCreated(e.target.checked)}
          />{" "}
          Issue created
        </label>
        <label style={{ display: "block" }}>
          <input
            type="checkbox"
            checked={notifyApproval}
            onChange={(e) => setNotifyApproval(e.target.checked)}
          />{" "}
          Approval requested
        </label>
        <label style={{ display: "block" }}>
          <input
            type="checkbox"
            checked={notifyAgentError}
            onChange={(e) => setNotifyAgentError(e.target.checked)}
          />{" "}
          Agent run failed
        </label>
      </fieldset>

      <p style={{ fontSize: 13, color: "#444" }}>
        After install, point SendBlue{" "}
        <code>receive</code> webhook at:
        <br />
        <code>
          https://&lt;host&gt;/api/plugins/jewell-labs.sendblue/webhooks/inbound
        </code>
      </p>
    </div>
  );
}

export default SendBlueSettingsPage;
