"use client";

// Backwards-compatible wrapper around the TanStack Query hook. Keeps
// expansion state local (purely UI) while routing data through the
// shared query cache. Selection state lives in
// CollectionSelectionContext so it can be shared across the dashboard
// layout and the pages list view.

import { useCallback, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { queryKeys, useCollectionsQuery } from "@/lib/queries";

export function useCollections() {
  const qc = useQueryClient();
  const [expandedIds, setExpandedIds] = useState<Set<string>>(new Set());

  const query = useCollectionsQuery();

  const refresh = useCallback(async () => {
    await qc.invalidateQueries({ queryKey: queryKeys.collections.all });
  }, [qc]);

  const toggleExpanded = useCallback((id: string) => {
    setExpandedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  return {
    collections: query.data ?? [],
    tree: query.tree,
    expandedIds,
    toggleExpanded,
    isLoading: query.isLoading,
    error: query.error ? (query.error as Error).message : null,
    refresh,
  };
}
