/** Load / save instance plugin config for the settings form. */

import { useCallback, useEffect, useState } from "react";
import {
  configJsonFromForm,
  DEFAULT_SETTINGS_FORM,
  formFromConfigJson,
  pluginConfigUrl,
  type SettingsFormState,
} from "./form-model.js";
import { getErrorMessage, hostFetchJson } from "./host-fetch.js";

export function useSettingsConfig() {
  const [form, setForm] = useState<SettingsFormState>(DEFAULT_SETTINGS_FORM);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const result = await hostFetchJson<{
        configJson?: Record<string, unknown> | null;
      } | null>(pluginConfigUrl(null));
      setForm(formFromConfigJson(result?.configJson ?? {}));
      setError(null);
    } catch (err) {
      setError(getErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function save(next: SettingsFormState) {
    setSaving(true);
    try {
      const configJson = configJsonFromForm(next);
      // Instance-level only — companyId POST 422s on this host today.
      await hostFetchJson(pluginConfigUrl(null), {
        method: "POST",
        body: JSON.stringify({ configJson }),
      });
      setForm(next);
      setError(null);
      return configJson;
    } catch (err) {
      const message = getErrorMessage(err);
      setError(message);
      throw err;
    } finally {
      setSaving(false);
    }
  }

  return { form, setForm, loading, saving, error, save, reload: load };
}
