/** Same-origin host REST helper for plugin settings. */

export function getErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "object" && error !== null && "message" in error) {
    const m = (error as { message?: unknown }).message;
    if (typeof m === "string") return m;
  }
  return String(error);
}

export async function hostFetchJson<T>(
  path: string,
  init?: RequestInit,
): Promise<T> {
  const headers = new Headers(init?.headers);
  if (!headers.has("content-type")) {
    headers.set("content-type", "application/json");
  }
  const response = await fetch(path, {
    ...init,
    credentials: "include",
    headers,
  });
  if (!response.ok) {
    const text = await response.text();
    try {
      const parsed = JSON.parse(text) as { error?: string; message?: string };
      const msg = parsed.error ?? parsed.message;
      if (typeof msg === "string" && msg.trim()) {
        throw new Error(msg);
      }
    } catch (err) {
      if (err instanceof Error && err.message !== text) throw err;
    }
    throw new Error(text || `Request failed: ${response.status}`);
  }
  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}
