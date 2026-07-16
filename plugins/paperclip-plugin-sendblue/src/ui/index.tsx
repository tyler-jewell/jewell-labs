/**
 * Settings surface for SendBlue.
 * Host mounts this as settingsPage exportName: SendBlueSettingsPage.
 */

import { useCallback, useMemo, useState, type SyntheticEvent } from "react";
import {
  usePluginAction,
  usePluginData,
  usePluginToast,
  type PluginSettingsPageProps,
} from "@paperclipai/plugin-sdk/ui";
import { PLUGIN_ID, PLUGIN_VERSION } from "../constants.js";
import type { ToolName } from "../tools.js";
import { ApiConsole } from "./api-console.js";
import type { TestResult } from "./components.js";
import { inboundWebhookPath, type SettingsFormState } from "./form-model.js";
import { buildTestParams } from "./api-test-defs.js";
import { getErrorMessage } from "./host-fetch.js";
import { SettingsForm } from "./settings-form.js";
import { muted, row, stack } from "./styles.js";
import { useSettingsConfig } from "./use-settings-config.js";

type SettingsOverview = {
  pluginId: string;
  version: string;
  credentialsConfigured: boolean;
  hasFromNumber: boolean;
  allowlistCount: number;
  emptyMeansDeny: boolean;
  inboundMode: string;
  notifyNumber?: string;
  warnings: string[];
  config: Record<string, unknown>;
};

export function SendBlueSettingsPage({ context }: PluginSettingsPageProps) {
  const toast = usePluginToast();
  const companyId = context.companyId;
  const {
    form,
    setForm,
    loading,
    saving,
    error: configError,
    save,
    reload,
  } = useSettingsConfig();

  const overview = usePluginData<SettingsOverview>(
    "settings-overview",
    companyId ? { companyId } : {},
  );
  const runApiTest = usePluginAction("test_api");

  const [savedMessage, setSavedMessage] = useState<string | null>(null);
  const [runningApi, setRunningApi] = useState<ToolName | null>(null);
  const [results, setResults] = useState<Partial<Record<ToolName, TestResult>>>(
    {},
  );
  const [showWrites, setShowWrites] = useState(false);

  const setField = useCallback(
    <K extends keyof SettingsFormState>(key: K, value: SettingsFormState[K]) => {
      setForm((current) => ({ ...current, [key]: value }));
    },
    [setForm],
  );

  const testDefaults = useMemo(() => {
    const firstAllowlisted =
      form.allowlistText.split(/[\n,]+/)[0]?.trim() ?? "";
    return {
      number: form.notifyNumber !== "" ? form.notifyNumber : firstAllowlisted,
      from_number: form.fromNumber,
      content: "SendBlue console test",
    };
  }, [form.allowlistText, form.fromNumber, form.notifyNumber]);

  const webhookUrlHint = useMemo(() => {
    if (typeof window !== "undefined" && window.location.origin) {
      return `${window.location.origin}${inboundWebhookPath()}`;
    }
    return `https://<host>${inboundWebhookPath()}`;
  }, []);

  async function onSubmit(event: SyntheticEvent<HTMLFormElement>) {
    event.preventDefault();
    try {
      await save(form);
      setSavedMessage("Saved — worker will pick up config on next call");
      window.setTimeout(() => setSavedMessage(null), 2500);
      overview.refresh();
      toast({
        title: "SendBlue settings saved",
        body: "Config written. Run a test below to verify credentials take effect.",
        tone: "success",
      });
    } catch (err) {
      toast({
        title: "Save failed",
        body: getErrorMessage(err),
        tone: "error",
      });
    }
  }

  async function handleRunTest(name: ToolName, fields: Record<string, string>) {
    setRunningApi(name);
    const at = new Date().toISOString();
    try {
      const params = buildTestParams(name, fields);
      const data = await runApiTest({
        api: name,
        params,
        companyId: companyId ?? undefined,
      });
      setResults((c) => ({ ...c, [name]: { ok: true, at, name, data } }));
      toast({ title: `${name} OK`, tone: "success" });
    } catch (err) {
      const message = getErrorMessage(err);
      setResults((c) => ({
        ...c,
        [name]: { ok: false, at, name, error: message },
      }));
      toast({ title: `${name} failed`, body: message, tone: "error" });
    } finally {
      setRunningApi(null);
    }
  }

  if (loading) {
    return (
      <div style={{ ...muted, padding: 16 }}>Loading SendBlue settings…</div>
    );
  }

  return (
    <div
      style={{
        padding: 16,
        maxWidth: 820,
        fontFamily: "system-ui, sans-serif",
        ...stack,
      }}
    >
      <div style={stack}>
        <h2 style={{ margin: 0 }}>SendBlue</h2>
        <p style={muted}>
          Configure the iMessage/SMS connector. Vault key <em>names</em> and
          operational settings (allowlist, notify, inbound). After save, use{" "}
          <strong>Run test</strong> on the APIs below.
        </p>
        <div style={row}>
          <span style={{ fontSize: 12, opacity: 0.75 }}>
            {PLUGIN_ID}@{PLUGIN_VERSION}
          </span>
          {companyId ? (
            <span style={{ fontSize: 12, opacity: 0.75 }}>
              company {companyId.slice(0, 8)}…
            </span>
          ) : (
            <span style={{ fontSize: 12, color: "var(--destructive, #b45309)" }}>
              No company selected — instance-scoped save.
            </span>
          )}
          {overview.data ? (
            <span style={{ fontSize: 12, opacity: 0.75 }}>
              {overview.data.credentialsConfigured
                ? "credentials ready"
                : "credentials missing"}
              {" · "}
              allowlist {overview.data.allowlistCount}
              {" · "}
              inbound {overview.data.inboundMode}
            </span>
          ) : null}
        </div>
      </div>

      <SettingsForm
        form={form}
        setField={setField}
        companyId={companyId}
        webhookUrlHint={webhookUrlHint}
        configError={configError}
        warnings={overview.data?.warnings ?? []}
        saving={saving}
        savedMessage={savedMessage}
        onSubmit={onSubmit}
        onReload={() => {
          void reload().then(() => overview.refresh());
        }}
      />

      <ApiConsole
        showWrites={showWrites}
        onShowWrites={setShowWrites}
        testDefaults={testDefaults}
        runningApi={runningApi}
        results={results}
        onRun={(name, fields) => void handleRunTest(name, fields)}
      />
    </div>
  );
}

export default SendBlueSettingsPage;
