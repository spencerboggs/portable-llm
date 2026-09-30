import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import { Sidebar } from "./components/Sidebar";
import { Dashboard } from "./components/Dashboard";
import { ChatView } from "./components/ChatView";
import { KnowledgeView } from "./components/KnowledgeView";
import { ScriptsView } from "./components/ScriptsView";
import { SettingsView } from "./components/SettingsView";
import { StatusDot } from "./components/ChatMessage";
import type {
  Conversation,
  DriveInfo,
  HardwareInfo,
  RuntimeInfo,
  Settings,
  ViewId,
} from "./types";

export default function App() {
  const [ready, setReady] = useState(false);
  const [bootError, setBootError] = useState<string | null>(null);
  const [view, setView] = useState<ViewId>("dashboard");
  const [usbRoot, setUsbRoot] = useState("");
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [hardware, setHardware] = useState<HardwareInfo | null>(null);
  const [runtime, setRuntime] = useState<RuntimeInfo | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [modelLabel, setModelLabel] = useState("qwen3:4b");
  const [selectedDrive, setSelectedDrive] = useState<string | null>(null);
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [activeConversation, setActiveConversation] = useState<Conversation | null>(null);
  const [pendingScript, setPendingScript] = useState<{ name: string; content: string } | null>(
    null,
  );
  const [generatingId, setGeneratingId] = useState<string | null>(null);

  const refreshConversations = useCallback(async () => {
    const list = await api.listConversations();
    setConversations(list);
  }, []);

  useEffect(() => {
    void (async () => {
      try {
        const boot = await api.getBootstrap();
        setUsbRoot(boot.usbRoot);
        setDrives(boot.drives);
        setHardware(boot.hardware);
        setRuntime(boot.runtime);
        setSettings(boot.settings);
        setModelLabel(boot.modelLabel);
        setSelectedDrive(boot.settings.preferredDrive);
        await refreshConversations();
        setRuntime({
          ...boot.runtime,
          status: "loading",
          progressMessage: "Looking for an existing PortableLLM folder",
          progressPercent: 15,
        });
        setReady(true);
        const poll = setInterval(() => {
          api.getRuntime().then(setRuntime).catch(() => undefined);
        }, 400);
        try {
          const resumed = await api.resumeExisting();
          setRuntime(resumed);
          if (resumed.status === "running" && resumed.targetDrive) {
            setSelectedDrive(resumed.targetDrive);
          }
        } finally {
          clearInterval(poll);
        }
      } catch (e) {
        setBootError(String(e));
      }
    })();
  }, [refreshConversations]);

  async function refreshDrives() {
    setDrives(await api.refreshDrives());
  }

  const handleConversationChange = useCallback(
    (
      next:
        | Conversation
        | null
        | ((prev: Conversation | null) => Conversation | null),
    ) => {
      setActiveConversation((prev) => {
        const value = typeof next === "function" ? next(prev) : next;
        if (value) {
          setConversations((list) => {
            const others = list.filter((c) => c.id !== value.id);
            return [value, ...others];
          });
        }
        return value;
      });
    },
    [],
  );

  if (bootError) {
    return (
      <div className="h-full flex items-center justify-center p-8">
        <div className="max-w-lg rounded-2xl border border-[var(--danger)]/40 bg-[#2a1518] p-6">
          <h1 className="text-xl mb-2">Failed to start</h1>
          <pre className="text-sm whitespace-pre-wrap">{bootError}</pre>
        </div>
      </div>
    );
  }

  if (!ready || !runtime || !settings || !hardware) {
    return (
      <div className="h-full flex items-center justify-center text-[var(--text-muted)]">
        Starting PortableLLM...
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      <header className="h-12 shrink-0 border-b border-[var(--border)] px-4 flex items-center justify-between bg-[var(--bg-sidebar)]/80 backdrop-blur">
        <div className="font-medium tracking-tight">PortableLLM</div>
        <div className="flex items-center gap-4 text-sm text-[var(--text-muted)]">
          <span>{settings.internetEnabled ? "Internet tools on" : "Offline tools"}</span>
          <span>{runtime.usingGpu ? "GPU preferred" : "CPU ready"}</span>
          <StatusDot status={runtime.status} />
        </div>
      </header>

      <div className="flex-1 min-h-0 flex">
        <Sidebar
          view={view}
          onChange={setView}
          conversations={conversations}
          activeConversationId={activeConversation?.id ?? null}
          generatingConversationId={generatingId}
          onNewChat={async () => {
            const c = await api.createConversation();
            setActiveConversation(c);
            await refreshConversations();
            setView("chat");
          }}
          onSelectConversation={async (id) => {
            const c = await api.getConversation(id);
            setActiveConversation(c);
          }}
          onDeleteConversation={async (id) => {
            await api.deleteConversation(id);
            if (activeConversation?.id === id) setActiveConversation(null);
            await refreshConversations();
          }}
        />

        <main className="flex-1 min-w-0 min-h-0">
          {view === "dashboard" ? (
            <Dashboard
              drives={drives}
              hardware={hardware}
              runtime={runtime}
              modelLabel={modelLabel}
              selectedDrive={selectedDrive}
              onSelectDrive={(letter) => {
                setSelectedDrive(letter);
                void api
                  .updateSettings({ ...settings, preferredDrive: letter })
                  .then(setSettings);
              }}
              onLoaded={(rt) => {
                setRuntime(rt);
                setView("chat");
              }}
              onRemoved={(rt) => setRuntime(rt)}
              onRefreshDrives={() => void refreshDrives()}
            />
          ) : null}
          {view === "chat" ? (
            <ChatView
              runtime={runtime}
              conversation={activeConversation}
              onConversationChange={handleConversationChange}
              onGeneratingChange={setGeneratingId}
              onSaveScript={(name, content) => {
                setPendingScript({ name, content });
                setView("scripts");
              }}
            />
          ) : null}
          {view === "knowledge" ? <KnowledgeView /> : null}
          {view === "scripts" ? (
            <ScriptsView
              pendingScript={pendingScript}
              onConsumedPending={() => setPendingScript(null)}
            />
          ) : null}
          {view === "settings" ? (
            <SettingsView settings={settings} usbRoot={usbRoot} onChange={setSettings} />
          ) : null}
        </main>
      </div>
    </div>
  );
}
