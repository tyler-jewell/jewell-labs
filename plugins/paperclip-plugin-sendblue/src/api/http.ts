/** Auth, URL helpers, and low-level request preparation. */

import {
  ENV_API_KEY,
  ENV_API_SECRET,
  ENV_FROM_NUMBER,
  HEADER_API_KEY,
  HEADER_API_SECRET,
  SENDBLUE_API_BASE,
} from "../constants.js";
import {
  type Credentials,
  type PreparedRequest,
  SendBlueClientError,
} from "./types.js";

export { HEADER_API_KEY, HEADER_API_SECRET, SENDBLUE_API_BASE };

function firstEnv(names: readonly string[]): string | undefined {
  for (const n of names) {
    const v = process.env[n];
    if (typeof v === "string" && v.trim()) return v.trim();
  }
  return undefined;
}

export function credentialsFromEnv(): Credentials {
  const apiKey = firstEnv(ENV_API_KEY);
  const apiSecret = firstEnv(ENV_API_SECRET);
  if (!apiKey || !apiSecret) {
    throw new SendBlueClientError(
      "missing Sendblue credentials: set SENDBLUE_API_KEY and SENDBLUE_API_SECRET (or SENDBLUE_API_API_KEY / SENDBLUE_API_API_SECRET)",
      "missing_credentials",
    );
  }
  return { apiKey, apiSecret };
}

export function fromNumberFromEnv(): string | undefined {
  return firstEnv(ENV_FROM_NUMBER);
}

export function authHeaders(creds: Credentials): Record<string, string> {
  return {
    [HEADER_API_KEY]: creds.apiKey,
    [HEADER_API_SECRET]: creds.apiSecret,
  };
}

function jsonHeaders(creds: Credentials): Record<string, string> {
  return {
    ...authHeaders(creds),
    "Content-Type": "application/json",
    Accept: "application/json",
  };
}

function getUrl(pathAndQuery: string): string {
  if (pathAndQuery.startsWith("http")) return pathAndQuery;
  return `${SENDBLUE_API_BASE}${pathAndQuery.startsWith("/") ? "" : "/"}${pathAndQuery}`;
}

export function prepareGet(
  creds: Credentials,
  pathAndQuery: string,
): PreparedRequest {
  return {
    method: "GET",
    url: getUrl(pathAndQuery),
    headers: authHeaders(creds),
  };
}

export function prepareJson(
  creds: Credentials,
  method: PreparedRequest["method"],
  path: string,
  body: unknown,
): PreparedRequest {
  return {
    method,
    url: getUrl(path),
    headers: jsonHeaders(creds),
    body: JSON.stringify(body),
  };
}

export async function executePrepared(
  prep: PreparedRequest,
  fetchImpl: typeof fetch = fetch,
): Promise<unknown> {
  const res = await fetchImpl(prep.url, {
    method: prep.method,
    headers: prep.headers,
    body: prep.body,
  });
  const text = await res.text();
  let json: unknown = undefined;
  if (text) {
    try {
      json = JSON.parse(text);
    } catch {
      json = text;
    }
  }
  if (!res.ok) {
    throw new SendBlueClientError(
      `SendBlue API HTTP ${res.status}`,
      "api",
      res.status,
      text.slice(0, 2000),
    );
  }
  return json;
}
