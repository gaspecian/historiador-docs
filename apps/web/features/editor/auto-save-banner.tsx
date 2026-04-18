"use client";

// Lightweight status banner for the editor's auto-save. Reads a
// status value emitted by `useAutoSave` and renders the human-readable
// line the sprint DoD asks for ("Saved <timestamp>" / "Unsaved
// changes…"). Styling follows the project's Tailwind conventions;
// callers can override via `className` if they need a different
// container.

import type { AutoSaveStatus } from "./use-auto-save";

export interface AutoSaveBannerProps {
  status: AutoSaveStatus;
  /** Short label used to disambiguate when more than one auto-save target is visible. */
  label?: string;
  className?: string;
}

export function AutoSaveBanner({ status, label, className }: AutoSaveBannerProps) {
  const prefix = label ? `${label}: ` : "";
  const { text, tone } = describe(status);
  return (
    <span
      role="status"
      aria-live="polite"
      className={
        className ??
        `inline-flex items-center gap-1.5 text-xs tabular-nums ${toneClass(tone)}`
      }
    >
      <span
        aria-hidden
        className={`h-1.5 w-1.5 rounded-full ${dotClass(tone)}`}
      />
      {prefix}
      {text}
    </span>
  );
}

type Tone = "idle" | "dirty" | "saving" | "saved" | "error";

function describe(status: AutoSaveStatus): { text: string; tone: Tone } {
  switch (status.kind) {
    case "idle":
      return { text: "Ready", tone: "idle" };
    case "dirty":
      return { text: "Unsaved changes\u2026", tone: "dirty" };
    case "saving":
      return { text: "Saving\u2026", tone: "saving" };
    case "saved":
      return { text: `Saved ${formatTime(status.at)}`, tone: "saved" };
    case "error":
      return { text: `Save failed: ${status.message}`, tone: "error" };
  }
}

function formatTime(d: Date): string {
  const hh = d.getHours().toString().padStart(2, "0");
  const mm = d.getMinutes().toString().padStart(2, "0");
  const ss = d.getSeconds().toString().padStart(2, "0");
  return `${hh}:${mm}:${ss}`;
}

function toneClass(tone: Tone): string {
  switch (tone) {
    case "error":
      return "text-red-600";
    case "dirty":
    case "saving":
      return "text-amber-600";
    case "saved":
      return "text-emerald-600";
    default:
      return "text-zinc-500";
  }
}

function dotClass(tone: Tone): string {
  switch (tone) {
    case "error":
      return "bg-red-500";
    case "dirty":
    case "saving":
      return "bg-amber-500 animate-pulse";
    case "saved":
      return "bg-emerald-500";
    default:
      return "bg-zinc-400";
  }
}
