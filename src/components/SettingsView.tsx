import { useState } from "react";
import { api } from "../api";
import type { Settings } from "../types";

export function SettingsView({
  settings,
  usbRoot,
  onChange,
}: {
  settings: Settings;
  usbRoot: string;
  onChange: (s: Settings) => void;
}) {
  const [local, setLocal] = useState(settings);
  const [status, setStatus] = useState<string | null>(null);

  async function save() {
    const saved = await api.updateSettings(local);
    onChange(saved);
    setStatus("Settings saved");
  }

  return (
    <div className="h-full overflow-y-auto p-8">
      <div className="max-w-2xl mx-auto space-y-6">
        <header>
          <h1 className="text-2xl font-semibold">Settings</h1>
          <p className="text-[var(--text-muted)] mt-1 text-sm">USB root: {usbRoot}</p>
        </header>

        <Field label="Model name">
          <input
            className="w-full rounded-lg bg-[var(--bg-elevated)] border border-[var(--border)] px-3 py-2"
            value={local.modelName}
            onChange={(e) => setLocal({ ...local, modelName: e.target.value })}
          />
        </Field>

        <Field label="Ollama port (isolated; default 11435)">
          <input
            type="number"
            className="w-full rounded-lg bg-[var(--bg-elevated)] border border-[var(--border)] px-3 py-2"
            value={local.ollamaPort}
            onChange={(e) => setLocal({ ...local, ollamaPort: Number(e.target.value) || 11435 })}
          />
        </Field>

        <label className="flex items-start gap-3 rounded-xl border border-[var(--border)] p-4 cursor-pointer">
          <input
            type="checkbox"
            className="mt-1"
            checked={local.portableMode}
            onChange={(e) => setLocal({ ...local, portableMode: e.target.checked })}
          />
          <div>
            <div className="font-medium">Portable Mode (default)</div>
            <div className="text-sm text-[var(--text-muted)]">
              Copy model + runtime to the host drive so the USB can be removed while running.
            </div>
          </div>
        </label>

        <label className="flex items-start gap-3 rounded-xl border border-[var(--border)] p-4 cursor-pointer">
          <input
            type="checkbox"
            className="mt-1"
            checked={local.internetEnabled}
            onChange={(e) => setLocal({ ...local, internetEnabled: e.target.checked })}
          />
          <div>
            <div className="font-medium">Enable internet tools</div>
            <div className="text-sm text-[var(--text-muted)]">
              Allows controlled web_search / fetch_url. Core chat works offline either way.
              Inference always stays local.
            </div>
          </div>
        </label>

        <div className="rounded-xl border border-[var(--warning)]/30 bg-[#2a2418] p-4 text-sm">
          <div className="font-medium text-[var(--warning)] mb-1">Safety</div>
          PortableLLM will not modify PATH, registry, services, drivers, or a global Ollama
          install. If those changes are useful, the assistant may suggest them for you to do
          manually.
        </div>

        <button className="px-5 py-2.5 rounded-xl bg-[var(--accent)]" onClick={() => void save()}>
          Save settings
        </button>
        {status ? <div className="text-sm text-[var(--success)]">{status}</div> : null}
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block space-y-2">
      <div className="text-sm text-[var(--text-muted)]">{label}</div>
      {children}
    </label>
  );
}
