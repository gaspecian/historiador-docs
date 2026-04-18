"use client";

// Backwards-compatible wrapper around the TanStack Query hooks. Keeps
// the same `{ pages, isLoading, error, refresh, search }` shape that
// existing consumers rely on while routing through the shared cache
// so mutations elsewhere (publish, update, restore) invalidate this
// view automatically.

import { useCallback, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  queryKeys,
  usePagesQuery,
  useSearchPagesQuery,
} from "@/lib/queries";

export function usePages(collectionId: string | null) {
  const qc = useQueryClient();
  const [searchQuery, setSearchQuery] = useState("");

  const listQuery = usePagesQuery(collectionId);
  const searchQueryResult = useSearchPagesQuery(searchQuery, searchQuery.trim().length > 0);

  const active = searchQuery.trim().length > 0 ? searchQueryResult : listQuery;

  const refresh = useCallback(async () => {
    await qc.invalidateQueries({ queryKey: queryKeys.pages.all });
  }, [qc]);

  const search = useCallback((query: string) => {
    setSearchQuery(query);
  }, []);

  return {
    pages: active.data ?? [],
    isLoading: active.isLoading,
    error: active.error ? (active.error as Error).message : null,
    refresh,
    search,
  };
}
