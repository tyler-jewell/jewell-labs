import type { PaperclipPluginManifestV1 } from "@paperclipai/plugin-sdk";
import {
  PLUGIN_DISPLAY_NAME,
  PLUGIN_ID,
  PLUGIN_VERSION,
  WEBHOOK_ENDPOINT_KEYS,
} from "./constants.js";
import { TOOL_NAMES, toolParameterSchemas } from "./tools.js";

/** Manifest may include webhooks on hosts that support webhooks.receive. */
const manifest = {
  id: PLUGIN_ID,
  apiVersion: 1 as const,
  version: PLUGIN_VERSION,
  displayName: PLUGIN_DISPLAY_NAME,
  description:
    "SendBlue iMessage/SMS connector: agent tools for send/list/contacts/webhooks, inbound receive webhooks, allowlisted notify-on-events. Issues remain the system of record.",
  author: "jewell-labs",
  categories: ["connector", "automation"] as ("connector" | "workspace" | "automation" | "ui")[],
  capabilities: [
    "companies.read",
    "projects.read",
    "issues.read",
    "issues.create",
    "issues.update",
    "issues.wakeup",
    "issue.comments.read",
    "issue.comments.create",
    "agents.read",
    "agents.invoke",
    "agent.tools.register",
    "events.subscribe",
    "plugin.state.read",
    "plugin.state.write",
    "http.outbound",
    "secrets.read-ref",
    "activity.log.write",
    "webhooks.receive",
    "instance.settings.register",
    "ui.page.register",
  ],
  entrypoints: {
    worker: "./dist/worker.js",
    ui: "./dist/ui",
  },
  webhooks: [
    {
      endpointKey: WEBHOOK_ENDPOINT_KEYS.inbound,
      displayName: "SendBlue inbound",
      description:
        "SendBlue receive + status_callback + call_log. POST JSON; set webhook secret and send x-webhook-secret or x-sendblue-secret header.",
    },
  ],
  tools: TOOL_NAMES.map((name) => ({
    name,
    displayName: name
      .split("_")
      .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
      .join(" "),
    description: `SendBlue ${name.replace(/_/g, " ")}`,
    parametersSchema: toolParameterSchemas[name],
  })),
  ui: {
    slots: [
      {
        type: "settingsPage" as const,
        id: "sendblue-settings",
        displayName: "SendBlue",
        exportName: "SendBlueSettingsPage",
      },
    ],
  },
} satisfies PaperclipPluginManifestV1;

export default manifest;
