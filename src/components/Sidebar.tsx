import clsx from "clsx";
import type { ViewId } from "../types";

const items: { id: ViewId; label: string }[] = [
  { id: "dashboard", label: "Dashboard" },
  { id: "chat", label: "Chat" },
  { id: "knowledge", label: "Knowledge" },
  { id: "scripts", label: "Scripts" },
  { id: "settings", label: "Settings" },
];

export function Sidebar({
  view,
  onChange,
  conversations,
  activeConversationId,
  generatingConversationId,
  onNewChat,
  onSelectConversation,
  onDeleteConversation,
}: {
  view: ViewId;
  onChange: (v: ViewId) => void;
  conversations: { id: string; title: string }[];
  activeConversationId: string | null;
  generatingConversationId: string | null;
  onNewChat: () => void;
  onSelectConversation: (id: string) => void;
  onDeleteConversation: (id: string) => void;
}) {
  return (
    <aside className="w-[260px] shrink-0 border-r border-[var(--border)] bg-[var(--bg-sidebar)]/90 backdrop-blur flex flex-col">
      <div className="px-4 py-5 border-b border-[var(--border)]">
        <div className="text-lg font-semibold tracking-tight">PortableLLM</div>
        <div className="text-xs text-[var(--text-muted)] mt-1">USB personal AI</div>
      </div>

      <nav className="p-3 flex flex-col gap-1">
        {items.map((item) => (
          <button
            key={item.id}
            onClick={() => onChange(item.id)}
            className={clsx(
              "text-left px-3 py-2 rounded-lg transition",
              view === item.id
                ? "bg-[#1f2a38] text-white"
                : "text-[var(--text-muted)] hover:bg-[#182230] hover:text-white",
            )}
          >
            {item.label}
          </button>
        ))}
      </nav>

      <div className="px-3 pt-2 pb-1 text-xs uppercase tracking-wide text-[var(--text-muted)]">
        Conversations
      </div>
      <div className="px-3 mb-2">
        <button
          onClick={onNewChat}
          className="w-full text-sm px-3 py-2 rounded-lg border border-[var(--border)] hover:border-[var(--accent)] hover:text-white text-[var(--text-muted)]"
        >
          New Chat
        </button>
      </div>
      <div className="flex-1 overflow-y-auto px-2 pb-4 space-y-1">
        {conversations.map((c) => (
          <div
            key={c.id}
            className={clsx(
              "group flex items-center gap-1 rounded-lg px-2 py-2 cursor-pointer",
              activeConversationId === c.id ? "bg-[#1f2a38]" : "hover:bg-[#182230]",
            )}
            onClick={() => {
              onSelectConversation(c.id);
              onChange("chat");
            }}
          >
            <div className="flex-1 min-w-0">
              <div className="truncate text-sm">{c.title}</div>
              {generatingConversationId === c.id ? (
                <div className="flex items-center gap-1.5 text-xs text-[var(--accent)]">
                  <span className="inline-block w-1.5 h-1.5 rounded-full bg-[var(--accent)] animate-pulse" />
                  Generating
                </div>
              ) : null}
            </div>
            <button
              className="opacity-0 group-hover:opacity-100 text-xs text-[var(--danger)] px-1"
              onClick={(e) => {
                e.stopPropagation();
                onDeleteConversation(c.id);
              }}
            >
              ×
            </button>
          </div>
        ))}
      </div>
    </aside>
  );
}
