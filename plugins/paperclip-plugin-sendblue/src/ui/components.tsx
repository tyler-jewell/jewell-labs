/** Small presentational pieces for the settings page. */

import { useEffect, useState, type ReactNode } from "react";
import type { ToolName } from "../tools.js";
import type { ApiTestDef } from "./api-test-defs.js";
import {
  buttonStyle,
  card,
  codeBlock,
  inputStyle,
  labelStyle,
  muted,
  primaryButton,
  row,
  stack,
} from "./styles.js";

export type TestResult = {
  ok: boolean;
  at: string;
  name: ToolName;
  error?: string;
  data?: unknown;
};

export function Section({
  title,
  children,
  action,
}: {
  title: string;
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <section style={card}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          gap: 8,
          marginBottom: 10,
        }}
      >
        <strong>{title}</strong>
        {action}
      </div>
      <div style={stack}>{children}</div>
    </section>
  );
}

export function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <label style={labelStyle}>
      <span style={{ fontWeight: 600 }}>{label}</span>
      {hint ? <span style={muted}>{hint}</span> : null}
      {children}
    </label>
  );
}

export function ApiTestRow({
  def,
  defaults,
  running,
  last,
  onRun,
}: {
  def: ApiTestDef;
  defaults: Record<string, string>;
  running: boolean;
  last: TestResult | null;
  onRun: (name: ToolName, fields: Record<string, string>) => void;
}) {
  const [fields, setFields] = useState<Record<string, string>>(() => {
    const init: Record<string, string> = {};
    for (const f of def.fields) {
      init[f.key] = defaults[f.key] ?? "";
    }
    return init;
  });

  useEffect(() => {
    setFields((current) => {
      const next = { ...current };
      for (const f of def.fields) {
        if (!next[f.key] && defaults[f.key]) next[f.key] = defaults[f.key] ?? "";
      }
      return next;
    });
  }, [defaults, def.fields]);

  return (
    <div style={{ ...card, padding: 12 }}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          gap: 8,
          alignItems: "flex-start",
          flexWrap: "wrap",
        }}
      >
        <div style={{ display: "grid", gap: 4, minWidth: 200, flex: 1 }}>
          <div style={row}>
            <strong style={{ fontSize: 13 }}>{def.label}</strong>
            <span style={{ fontSize: 11, opacity: 0.7 }}>
              {def.mutates ? "live write" : "read"}
            </span>
          </div>
          <div style={muted}>{def.description}</div>
          <code style={{ fontSize: 11 }}>{def.name}</code>
        </div>
        <button
          type="button"
          style={primaryButton}
          disabled={running}
          onClick={() => onRun(def.name, fields)}
        >
          {running ? "Running…" : "Run test"}
        </button>
      </div>
      {def.fields.length > 0 ? (
        <div
          style={{
            display: "grid",
            gap: 8,
            marginTop: 10,
            gridTemplateColumns: "repeat(auto-fit, minmax(160px, 1fr))",
          }}
        >
          {def.fields.map((f) => (
            <label key={f.key} style={labelStyle}>
              <span style={{ fontSize: 12 }}>
                {f.label}
                {f.required ? " *" : ""}
              </span>
              <input
                style={inputStyle}
                value={fields[f.key] ?? ""}
                placeholder={f.placeholder}
                onChange={(e) =>
                  setFields((c) => ({ ...c, [f.key]: e.target.value }))
                }
              />
            </label>
          ))}
        </div>
      ) : null}
      {last ? (
        <div style={{ marginTop: 10 }}>
          <div style={{ fontSize: 12, marginBottom: 4 }}>
            {last.ok ? "✓ OK" : "✗ Failed"} · {last.at}
            {last.error ? ` — ${last.error}` : ""}
          </div>
          {last.data !== undefined ? (
            <pre style={codeBlock}>{JSON.stringify(last.data, null, 2)}</pre>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

export { buttonStyle, muted, primaryButton, row, stack, inputStyle };
