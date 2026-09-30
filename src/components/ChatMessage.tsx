import { useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeHighlight from "rehype-highlight";
import clsx from "clsx";
import type { ChatMessage } from "../types";

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <button
      className="text-xs px-2 py-1 rounded border border-[var(--border)] text-[var(--text-muted)] hover:text-white"
      onClick={async () => {
        await navigator.clipboard.writeText(text);
        setCopied(true);
        setTimeout(() => setCopied(false), 1200);
      }}
    >
      {copied ? "Copied" : "Copy"}
    </button>
  );
}

export function MessageBubble({
  message,
  generating,
  onSaveScript,
}: {
  message: ChatMessage;
  generating?: boolean;
  onSaveScript?: (filename: string, content: string) => void;
}) {
  const isUser = message.role === "user";

  return (
    <div className={clsx("flex w-full", isUser ? "justify-end" : "justify-start")}>
      <div
        className={clsx(
          "max-w-[860px] w-full rounded-2xl px-4 py-3 border",
          isUser
            ? "bg-[var(--user-bubble)] border-[#2b4d7a]"
            : "bg-[var(--assistant-bubble)] border-[var(--border)]",
        )}
      >
        <div className="flex items-center justify-between mb-2 gap-3">
          <span className="text-xs uppercase tracking-wide text-[var(--text-muted)]">
            {isUser ? "You" : "PortableLLM"}
          </span>
          {!isUser && message.content ? <CopyButton text={message.content} /> : null}
        </div>
        <div className="prose prose-invert max-w-none text-[0.95rem] leading-relaxed">
          {generating ? (
            <div className="flex items-center gap-2 text-sm text-[var(--text-muted)]">
              <span className="inline-block w-2 h-2 rounded-full bg-[var(--accent)] animate-pulse" />
              Generating response
            </div>
          ) : (
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            rehypePlugins={[rehypeHighlight]}
            components={{
              pre({ children }) {
                return <pre>{children}</pre>;
              },
              code({ className, children, ...props }) {
                const match = /language-(\w+)/.exec(className || "");
                const codeText = String(children).replace(/\n$/, "");
                const isBlock = Boolean(match) || codeText.includes("\n");
                if (!isBlock) {
                  return (
                    <code className={className} {...props}>
                      {children}
                    </code>
                  );
                }
                const lang = match?.[1] ?? "txt";
                const filename = `script.${lang === "javascript" ? "js" : lang === "typescript" ? "ts" : lang === "csharp" ? "cs" : lang === "python" ? "py" : lang}`;
                return (
                  <div className="relative group">
                    <div className="absolute right-2 top-2 flex gap-2 opacity-90">
                      <CopyButton text={codeText} />
                      {onSaveScript ? (
                        <button
                          className="text-xs px-2 py-1 rounded border border-[var(--border)] text-[var(--text-muted)] hover:text-white bg-[#0b1016]/90"
                          onClick={() => onSaveScript(filename, codeText)}
                        >
                          Save
                        </button>
                      ) : null}
                    </div>
                    <code className={className} {...props}>
                      {children}
                    </code>
                  </div>
                );
              },
            }}
          >
            {message.content || (isUser ? "" : "...")}
          </ReactMarkdown>
          )}
        </div>
      </div>
    </div>
  );
}

export function StatusDot({ status }: { status: string }) {
  const color = useMemo(() => {
    switch (status) {
      case "running":
        return "bg-[var(--success)] shadow-[0_0_10px_#3ecf8e88]";
      case "loading":
      case "removing":
        return "bg-[var(--warning)] animate-pulse";
      case "error":
        return "bg-[var(--danger)]";
      default:
        return "bg-[#64748b]";
    }
  }, [status]);

  const label = useMemo(() => {
    switch (status) {
      case "running":
        return "Running";
      case "loading":
        return "Loading";
      case "removing":
        return "Removing";
      case "error":
        return "Error";
      default:
        return "Not Loaded";
    }
  }, [status]);

  return (
    <div className="flex items-center gap-2 text-sm text-[var(--text-muted)]">
      <span className={clsx("inline-block w-2.5 h-2.5 rounded-full", color)} />
      {label}
    </div>
  );
}
