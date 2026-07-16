/** Account webhook management request builders. */

import { PATHS } from "../constants.js";
import { prepareGet, prepareJson } from "./http.js";
import {
  type Credentials,
  type PreparedRequest,
  SendBlueClientError,
} from "./types.js";

export function prepareListAccountWebhooks(
  creds: Credentials,
): PreparedRequest {
  return prepareGet(creds, PATHS.accountWebhooks);
}

/**
 * Append webhooks. Official body:
 * `{ webhooks: [url | {url, secret}], type?: "receive", globalSecret?: string }`
 */
export function prepareCreateAccountWebhook(
  creds: Credentials,
  input: {
    url: string;
    type?: string;
    secret?: string;
    globalSecret?: string;
  },
): PreparedRequest {
  const url = input.url.trim();
  if (!url || !/^https:\/\//i.test(url)) {
    throw new SendBlueClientError(
      "webhook url must be absolute https",
      "validation",
    );
  }
  const secret = input.secret?.trim();
  const entry = secret ? { url, secret } : url;
  const body: Record<string, unknown> = {
    webhooks: [entry],
    type: input.type ?? "receive",
  };
  const globalSecret = input.globalSecret?.trim();
  if (globalSecret) body.globalSecret = globalSecret;
  return prepareJson(creds, "POST", PATHS.accountWebhooks, body);
}

/** Alias of prepareCreateAccountWebhook (historical name). */
export const prepareAddAccountWebhook = prepareCreateAccountWebhook;

export function prepareReplaceAccountWebhooks(
  creds: Credentials,
  webhooks: Record<string, unknown>,
): PreparedRequest {
  if (typeof webhooks !== "object") {
    throw new SendBlueClientError(
      "webhooks object is required for replace",
      "validation",
    );
  }
  return prepareJson(creds, "PUT", PATHS.accountWebhooks, { webhooks });
}

/** Alias for replace-with-partial map (historical name). */
export function prepareUpdateAccountWebhook(
  creds: Credentials,
  input: { receive?: string[]; secret?: string; globalSecret?: string },
): PreparedRequest {
  const webhooks: Record<string, unknown> = {};
  if (input.receive) webhooks.receive = input.receive;
  if (input.globalSecret ?? input.secret) {
    webhooks.globalSecret = input.globalSecret ?? input.secret;
  }
  return prepareReplaceAccountWebhooks(creds, webhooks);
}

export function prepareDeleteAccountWebhooks(
  creds: Credentials,
  input: { urls: string[]; type?: string },
): PreparedRequest {
  if (!Array.isArray(input.urls) || input.urls.length === 0) {
    throw new SendBlueClientError(
      "urls array is required for delete",
      "validation",
    );
  }
  for (const u of input.urls) {
    const url = u.trim();
    if (!url || !/^https:\/\//i.test(url)) {
      throw new SendBlueClientError(
        `delete webhook url must be absolute https: ${u}`,
        "validation",
      );
    }
  }
  return prepareJson(creds, "DELETE", PATHS.accountWebhooks, {
    webhooks: input.urls.map((u) => u.trim()),
    type: input.type ?? "receive",
  });
}

/** Alias for single-URL delete (historical name). */
export function prepareDeleteAccountWebhook(
  creds: Credentials,
  url: string,
): PreparedRequest {
  return prepareDeleteAccountWebhooks(creds, { urls: [url] });
}
