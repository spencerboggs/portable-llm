import { useEffect, useState } from "react";
import { api } from "../api";
import type { KnowledgeFileInfo } from "../types";
import { formatBytes } from "../types";

export function KnowledgeView() {
  const [files, setFiles] = useState<KnowledgeFileInfo[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [content, setContent] = useState("");
  const [status, setStatus] = useState<string | null>(null);
  const [newName, setNewName] = useState("custom/notes.md");

  async function refresh() {
    const list = await api.listKnowledge();
    setFiles(list);
  }

  useEffect(() => {
    void refresh();
  }, []);

  async function openFile(path: string) {
    setSelected(path);
    const text = await api.readKnowledgeFile(path);
    setContent(text);
  }

  async function save() {
    if (!selected) return;
    await api.writeKnowledgeFile(selected, content);
    setStatus(`Saved ${selected}`);
    await refresh();
  }

  async function reloadIndex() {
    const count = await api.reloadKnowledge();
    setStatus(`Reloaded knowledge index (${count} chunks)`);
  }

  async function createFile() {
    const path = newName.trim().replace(/\\/g, "/");
    if (!path) return;
    await api.writeKnowledgeFile(path, `# ${path}\n\n`);
    await refresh();
    await openFile(path);
  }

  async function removeFile() {
    if (!selected) return;
    if (selected === "profile.md" || selected === "personality.md") {
      setStatus("profile.md and personality.md should be edited, not deleted.");
      return;
    }
    await api.deleteKnowledgeFile(selected);
    setSelected(null);
    setContent("");
    await refresh();
  }

  return (
    <div className="h-full grid grid-cols-[280px_1fr]">
      <div className="border-r border-[var(--border)] p-4 overflow-y-auto space-y-3">
        <div className="flex gap-2">
          <button
            className="text-sm px-3 py-1.5 rounded-lg border border-[var(--border)]"
            onClick={() => void reloadIndex()}
          >
            Reload
          </button>
          <button
            className="text-sm px-3 py-1.5 rounded-lg border border-[var(--border)]"
            onClick={() => void refresh()}
          >
            Refresh
          </button>
        </div>
        <div className="space-y-1">
          {files.map((f) => (
            <button
              key={f.relativePath}
              onClick={() => void openFile(f.relativePath)}
              className={`w-full text-left px-3 py-2 rounded-lg text-sm ${
                selected === f.relativePath
                  ? "bg-[#1f2a38]"
                  : "hover:bg-[#182230] text-[var(--text-muted)]"
              }`}
            >
              <div className="truncate">{f.relativePath}</div>
              <div className="text-xs opacity-70">{formatBytes(f.sizeBytes)}</div>
            </button>
          ))}
        </div>
        <div className="pt-3 border-t border-[var(--border)] space-y-2">
          <input
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            className="w-full rounded-lg bg-[var(--bg-elevated)] border border-[var(--border)] px-3 py-2 text-sm"
            placeholder="custom/file.md"
          />
          <button
            className="w-full text-sm px-3 py-2 rounded-lg bg-[var(--accent)]"
            onClick={() => void createFile()}
          >
            Add Knowledge
          </button>
        </div>
      </div>

      <div className="flex flex-col h-full">
        <div className="px-4 py-3 border-b border-[var(--border)] flex items-center justify-between gap-3">
          <div className="text-sm text-[var(--text-muted)]">
            {selected ?? "Select a knowledge file"}
            {selected === "personality.md" ? " · behavioral instructions" : ""}
            {selected === "profile.md" ? " · factual profile" : ""}
          </div>
          <div className="flex gap-2">
            <button
              disabled={!selected}
              className="px-3 py-1.5 rounded-lg border border-[var(--border)] disabled:opacity-40"
              onClick={() => void save()}
            >
              Save
            </button>
            <button
              disabled={!selected}
              className="px-3 py-1.5 rounded-lg border border-[var(--danger)]/50 text-[var(--danger)] disabled:opacity-40"
              onClick={() => void removeFile()}
            >
              Delete
            </button>
          </div>
        </div>
        {status ? <div className="px-4 py-2 text-sm text-[var(--success)]">{status}</div> : null}
        <textarea
          className="flex-1 m-4 rounded-xl bg-[var(--bg-elevated)] border border-[var(--border)] p-4 font-mono text-sm outline-none focus:border-[var(--accent)]"
          value={content}
          onChange={(e) => setContent(e.target.value)}
          disabled={!selected}
          placeholder="Markdown knowledge. Save personality.md, then start a new message for it to apply."
        />
      </div>
    </div>
  );
}
