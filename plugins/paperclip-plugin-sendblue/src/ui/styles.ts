/** Shared CSSProperties for the settings page. */

import type { CSSProperties } from "react";

export const stack: CSSProperties = { display: "grid", gap: 12 };
export const card: CSSProperties = {
  border: "1px solid var(--border, #ddd)",
  borderRadius: 12,
  padding: 14,
  background: "var(--card, transparent)",
};
export const muted: CSSProperties = {
  fontSize: 13,
  color: "var(--muted-foreground, #666)",
  lineHeight: 1.45,
};
export const labelStyle: CSSProperties = {
  display: "grid",
  gap: 4,
  fontSize: 13,
};
export const inputStyle: CSSProperties = {
  display: "block",
  width: "100%",
  boxSizing: "border-box",
  border: "1px solid var(--border, #ccc)",
  borderRadius: 8,
  padding: "8px 10px",
  background: "transparent",
  color: "inherit",
  fontSize: 13,
};
export const buttonStyle: CSSProperties = {
  appearance: "none",
  border: "1px solid var(--border, #ccc)",
  borderRadius: 999,
  background: "transparent",
  color: "inherit",
  padding: "6px 12px",
  fontSize: 12,
  cursor: "pointer",
};
export const primaryButton: CSSProperties = {
  ...buttonStyle,
  background: "var(--foreground, #111)",
  color: "var(--background, #fff)",
  borderColor: "var(--foreground, #111)",
};
export const row: CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  alignItems: "center",
  gap: 8,
};
export const codeBlock: CSSProperties = {
  margin: 0,
  padding: 10,
  borderRadius: 8,
  border: "1px solid var(--border, #ddd)",
  background: "color-mix(in srgb, var(--muted, #888) 14%, transparent)",
  overflowX: "auto",
  fontSize: 11,
  lineHeight: 1.45,
  maxHeight: 220,
};
