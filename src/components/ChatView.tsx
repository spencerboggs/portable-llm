import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import type { ChatStreamEvent, Conversation, RuntimeInfo } from "../types";
import { MessageBubble } from "./ChatMessage";

export function ChatView({
  runtime,
  conversation,
  onConversationChange,
  onSaveScript,
}: {
  runtime: RuntimeInfo;
  conversation: Conversation | null;
  onConversationChange: (c: Conversation | null | ((prev: Conversation | null) => Conversation | null)) => void;
  onSaveScript: (filename: string, content: string) => void;
}) {
  const [input, setInput] = useState("");
  const [streaming, setStreaming] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const bottomRef = useRef<HTMLDivElement>(null);
  const convRef = useRef(conversation);
  convRef.current = conversation;

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [conversation?.messages, streaming]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<ChatStreamEvent>("chat-stream", (event) => {
      const payload = event.payload;
      onConversationChange((prev) => {
        if (!prev || prev.id !== payload.conversationId) return prev;
        const messages = prev.messages.map((m) =>
          m.id === payload.messageId ? { ...m, content: m.content + payload.delta } : m,
        );
        return { ...prev, messages };
      });
      if (payload.done) {
        setStreaming(false);
        if (payload.error) setError(payload.error);
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, [onConversationChange]);

  async function send(regenerate = false) {
    if (runtime.status !== "running") {
      setError("Load the model from the Dashboard first.");
      return;
    }
    if (!regenerate && !input.trim()) return;
    setError(null);
    setStreaming(true);

    const message = regenerate ? "" : input.trim();
    if (!regenerate) setInput("");

    let conv = convRef.current;
    if (!conv) {
      conv = await api.createConversation();
      onConversationChange(conv);
    }

    try {
      const result = await api.sendChat({
        conversationId: conv.id,
        message,
        regenerate,
      });
      onConversationChange(result.conversation);
    } catch (e) {
      setError(String(e));
    } finally {
      setStreaming(false);
    }
  }

  return (
    <div className="h-full flex flex-col">
      <div className="flex-1 overflow-y-auto px-6 py-6 space-y-4">
        {!conversation || conversation.messages.length === 0 ? (
          <div className="h-full min-h-[320px] flex items-center justify-center">
            <div className="text-center max-w-md text-[var(--text-muted)]">
              <div className="text-xl text-white mb-2">Local chat</div>
              <p>
                Ask about code, debugging, or explanations. Profile and knowledge files are
                included in context. The assistant will not change PATH, registry, or other
                system settings on this computer.
              </p>
            </div>
          </div>
        ) : (
          conversation.messages.map((m) => (
            <MessageBubble key={m.id} message={m} onSaveScript={onSaveScript} />
          ))
        )}
        <div ref={bottomRef} />
      </div>

      {error ? (
        <div className="mx-6 mb-2 text-sm text-[var(--danger)] whitespace-pre-wrap">{error}</div>
      ) : null}

      <div className="border-t border-[var(--border)] p-4 bg-[var(--bg-sidebar)]/70">
        <div className="max-w-4xl mx-auto flex gap-3 items-end">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            rows={3}
            placeholder={
              runtime.status === "running"
                ? "Message PortableLLM..."
                : "Load the model on the Dashboard to start chatting..."
            }
            className="flex-1 resize-none rounded-xl bg-[var(--bg-elevated)] border border-[var(--border)] px-4 py-3 outline-none focus:border-[var(--accent)]"
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.shiftKey) {
                e.preventDefault();
                void send(false);
              }
            }}
            disabled={streaming || runtime.status !== "running"}
          />
          <div className="flex flex-col gap-2">
            {streaming ? (
              <button
                className="px-4 py-2 rounded-xl border border-[var(--border)]"
                onClick={() => void api.stopGeneration()}
              >
                Stop
              </button>
            ) : (
              <button
                className="px-4 py-2 rounded-xl bg-[var(--accent)] hover:bg-[var(--accent-dim)] disabled:opacity-40"
                disabled={!input.trim() || runtime.status !== "running"}
                onClick={() => void send(false)}
              >
                Send
              </button>
            )}
            <button
              className="px-4 py-2 rounded-xl border border-[var(--border)] text-sm disabled:opacity-40"
              disabled={streaming || !conversation?.messages.some((m) => m.role === "assistant")}
              onClick={() => void send(true)}
            >
              Regenerate
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
