/** Live SendBlue API console with Run test per tool. */

import type { ToolName } from "../tools.js";
import { API_TEST_DEFS } from "./api-test-defs.js";
import { ApiTestRow, Section, type TestResult } from "./components.js";
import { muted, row } from "./styles.js";

type Props = {
  showWrites: boolean;
  onShowWrites: (v: boolean) => void;
  testDefaults: Record<string, string>;
  runningApi: ToolName | null;
  results: Partial<Record<ToolName, TestResult>>;
  onRun: (name: ToolName, fields: Record<string, string>) => void;
};

export function ApiConsole({
  showWrites,
  onShowWrites,
  testDefaults,
  runningApi,
  results,
  onRun,
}: Props) {
  const readApis = API_TEST_DEFS.filter((d) => !d.mutates);
  const writeApis = API_TEST_DEFS.filter((d) => d.mutates);

  return (
    <Section
      title="API console"
      action={
        <label style={{ ...row, fontSize: 12 }}>
          <input
            type="checkbox"
            checked={showWrites}
            onChange={(e) => onShowWrites(e.target.checked)}
          />
          Show write APIs
        </label>
      }
    >
      <p style={muted}>
        Each <strong>Run test</strong> hits live SendBlue via the worker using{" "}
        <em>saved</em> config. Save first if you just changed settings. Write
        APIs send real traffic.
      </p>

      <strong style={{ fontSize: 13 }}>Read APIs</strong>
      {readApis.map((def) => (
        <ApiTestRow
          key={def.name}
          def={def}
          defaults={testDefaults}
          running={runningApi === def.name}
          last={results[def.name] ?? null}
          onRun={onRun}
        />
      ))}

      {showWrites ? (
        <>
          <strong style={{ fontSize: 13 }}>Write APIs (live side effects)</strong>
          {writeApis.map((def) => (
            <ApiTestRow
              key={def.name}
              def={def}
              defaults={testDefaults}
              running={runningApi === def.name}
              last={results[def.name] ?? null}
              onRun={onRun}
            />
          ))}
        </>
      ) : (
        <div style={muted}>
          {writeApis.length} write APIs hidden — enable “Show write APIs”.
        </div>
      )}
    </Section>
  );
}
