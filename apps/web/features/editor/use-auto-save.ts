"use client";

// Generic debounced auto-save hook for Sprint 10's editor persistence
// (ADR-009). Fires `save(value)` after `idleMs` of no further changes,
// with a guaranteed flush at most every `maxMs`. Exposes a status
// enum the banner component reads verbatim.
//
// This is transport-agnostic on purpose: the caller decides whether to
// persist draft markdown, a conversation transcript, or anything else.
// The editor page wires one instance per target so each has its own
// timer and status.

import { useCallback, useEffect, useRef, useState } from "react";

export type AutoSaveStatus =
  | { kind: "idle" }
  | { kind: "dirty" }
  | { kind: "saving" }
  | { kind: "saved"; at: Date }
  | { kind: "error"; message: string };

export interface UseAutoSaveOptions<T> {
  /** Handler that actually persists the value. May throw. */
  save: (value: T) => Promise<void>;
  /** Milliseconds of inactivity before firing (default 2000). */
  idleMs?: number;
  /** Hard cap: flush at most this many ms after the first unsaved change (default 30000). */
  maxMs?: number;
}

export interface UseAutoSaveResult<T> {
  status: AutoSaveStatus;
  /** Record a new value. Resets the idle timer. */
  schedule: (next: T) => void;
  /** Force an immediate flush of the pending value. No-op if clean. */
  flush: () => Promise<void>;
}

export function useAutoSave<T>(opts: UseAutoSaveOptions<T>): UseAutoSaveResult<T> {
  const { save, idleMs = 2000, maxMs = 30000 } = opts;

  const [status, setStatus] = useState<AutoSaveStatus>({ kind: "idle" });
  const pendingRef = useRef<{ value: T } | null>(null);
  const idleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const maxTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Always read the latest save function without retriggering scheduling.
  const saveRef = useRef(save);
  useEffect(() => {
    saveRef.current = save;
  }, [save]);

  const clearTimers = useCallback(() => {
    if (idleTimerRef.current) {
      clearTimeout(idleTimerRef.current);
      idleTimerRef.current = null;
    }
    if (maxTimerRef.current) {
      clearTimeout(maxTimerRef.current);
      maxTimerRef.current = null;
    }
  }, []);

  const runSave = useCallback(async () => {
    const pending = pendingRef.current;
    if (!pending) return;
    pendingRef.current = null;
    clearTimers();
    setStatus({ kind: "saving" });
    try {
      await saveRef.current(pending.value);
      setStatus({ kind: "saved", at: new Date() });
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setStatus({ kind: "error", message });
    }
  }, [clearTimers]);

  const schedule = useCallback(
    (next: T) => {
      const wasClean = pendingRef.current === null;
      pendingRef.current = { value: next };
      setStatus({ kind: "dirty" });

      if (idleTimerRef.current) clearTimeout(idleTimerRef.current);
      idleTimerRef.current = setTimeout(() => {
        void runSave();
      }, idleMs);

      if (wasClean && !maxTimerRef.current) {
        maxTimerRef.current = setTimeout(() => {
          void runSave();
        }, maxMs);
      }
    },
    [idleMs, maxMs, runSave],
  );

  const flush = useCallback(async () => {
    await runSave();
  }, [runSave]);

  // Flush on unmount so navigation doesn't drop a pending change.
  useEffect(
    () => () => {
      clearTimers();
      if (pendingRef.current) {
        void saveRef.current(pendingRef.current.value).catch(() => {
          /* unmount path — swallow; server will just miss this turn */
        });
      }
    },
    [clearTimers],
  );

  return { status, schedule, flush };
}
