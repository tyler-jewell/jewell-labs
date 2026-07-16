/** UI pure models — form shape + API console defs. */

export {
  type SettingsFormState,
  DEFAULT_SETTINGS_FORM,
  SUGGESTED_SECRET_KEYS,
  formFromConfigJson,
  configJsonFromForm,
  pluginConfigUrl,
  inboundWebhookPath,
} from "./form-model.js";
export {
  type ApiTestField,
  type ApiTestDef,
  API_TEST_DEFS,
  buildTestParams,
} from "./api-test-defs.js";
