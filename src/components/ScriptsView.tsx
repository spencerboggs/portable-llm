import { useEffect, useState } from "react";
import { api } from "../api";
import type { ScriptInfo } from "../types";
import { formatBytes } from "../types";

export function ScriptsView({
  pendingScript,
  onConsumedPending,
}: {
  pendingScript: { name: string; content: string } | null;
  onConsumedPending: () => void;
}) {
  const [scripts, setScripts] = useState<ScriptInfo[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [content, setContent] = useState("");
  const [name, setName] = useState("script.py");
  const [status, setStatus] = useState<string | null>(null);

  async function refresh() {
    setScripts(await api.listScripts());
  }

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    if (!pendingScript) return;
    setName(pendingScript.name);
    setContent(pendingScript.content);
    setSelected(null);
    onConsumedPending();
  }, [pendingScript, onConsumedPending]);

  async function openScript(scriptName: string) {
    setSelected(scriptName);
    setName(scriptName);
    setContent(await api.readScript(scriptName));
  }

  async function save() {
    const saved = await api.saveScript(name, content);
    setStatus(`Saved ${saved.name} (not executed)`);
    setSelected(saved.name);
    await refresh();
  }

  return (
    <div className="h-full grid grid-cols-[280px_1fr]">
      <div className="border-r border-[var(--border)] p-4 overflow-y-auto space-y-3">
        <p className="text-xs text-[var(--text-muted)]">
          Saved scripts are for you to review and run yourself. The app does not execute them.
        </p>
        <button
          className="w-full text-sm px-3 py-2 rounded-lg border border-[var(--border)]"
          onClick={() => void api.openScriptsFolder()}
        >
          Open containing folder
        </button>
        <div className="space-y-1">
          {scripts.map((s) => (
            <button
              key={s.name}
              onClick={() => void openScript(s.name)}
              className={`w-full text-left px-3 py-2 rounded-lg text-sm ${
                selected === s.name ? "bg-[#1f2a38]" : "hover:bg-[#182230] text-[var(--text-muted)]"
              }`}
            >
              <div className="truncate">{s.name}</div>
              <div className="text-xs opacity-70">{formatBytes(s.sizeBytes)}</div>
            </button>
          ))}
        </div>
      </div>
      <div className="flex flex-col h-full p-4 gap-3">
        <div className="flex gap-3 items-center">
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            className="flex-1 rounded-lg bg-[var(--bg-elevated)] border border-[var(--border)] px-3 py-2"
            placeholder="filename.py"
          />
          <button className="px-4 py-2 rounded-lg bg-[var(--accent)]" onClick={() => void save()}>
            Save
          </button>
          <button
            className="px-4 py-2 rounded-lg border border-[var(--border)]"
            onClick={async () => {
              await navigator.clipboard.writeText(content);
              setStatus("Copied to clipboard");
            }}
          >
            Copy
          </button>
        </div>
        {status ? <div className="text-sm text-[var(--success)]">{status}</div> : null}
        <textarea
          className="flex-1 rounded-xl bg-[var(--bg-elevated)] border border-[var(--border)] p-4 font-mono text-sm outline-none focus:border-[var(--accent)]"
          value={content}
          onChange={(e) => setContent(e.target.value)}
          placeholder="Paste or generate a script in Chat, then Save here."
        />
      </div>
    </div>
  );
}
