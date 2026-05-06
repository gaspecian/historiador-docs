"use client";

import { useEffect, useRef } from "react";

export interface ChatMessage {
  seq: number;
  role: string;
  content: string;
}

export interface MessageListProps {
  messages: ChatMessage[];
}

export function MessageList({ messages }: MessageListProps) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [messages]);

  if (messages.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center p-6 text-center">
        <p className="t-body-sm text-[var(--color-text-tertiary)] max-w-[240px]">
          Comece uma conversa com o assistente — ele vai escrever a página
          com você, seção por seção.
        </p>
      </div>
    );
  }

  return (
    <div
      ref={scrollRef}
      className="flex-1 overflow-y-auto px-4 py-4 flex flex-col gap-3"
    >
      {messages.map((m, i) => (
        <Message key={`${m.seq}-${m.role}-${i}`} message={m} />
      ))}
    </div>
  );
}

function Message({ message }: { message: ChatMessage }) {
  const isUser = message.role === "user";
  const isError = message.role === "error";

  const surfaceClass = isUser
    ? "bg-[var(--color-primary-600)] text-[var(--color-text-inverse)]"
    : isError
      ? "bg-[var(--color-red-50)] text-[var(--color-red-700)]"
      : "bg-[var(--color-surface-canvas)] text-[var(--color-text-primary)] border border-[var(--color-surface-border)]";

  const alignClass = isUser ? "self-end max-w-[90%]" : "self-start max-w-[95%]";

  return (
    <div
      className={`${alignClass} ${surfaceClass} px-3 py-2 rounded-[var(--radius-md)] whitespace-pre-wrap break-words t-body-sm`}
    >
      {message.content}
    </div>
  );
}
