"use client";

import { useCallback, useEffect, useState } from "react";
import { apiFetch } from "./api";
import type { PageResponse } from "./types";

export function usePages(collectionId: string | null) {
  const [pages, setPages] = useState<PageResponse[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchPages = useCallback(async () => {
    try {
      setIsLoading(true);
      const query = collectionId ? `?collection_id=${collectionId}` : "";
      const data = await apiFetch<PageResponse[]>(`/pages${query}`);
      setPages(data);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load pages");
    } finally {
      setIsLoading(false);
    }
  }, [collectionId]);

  useEffect(() => {
    fetchPages();
  }, [fetchPages]);

  const search = useCallback(async (query: string) => {
    if (!query.trim()) {
      fetchPages();
      return;
    }
    try {
      setIsLoading(true);
      const data = await apiFetch<PageResponse[]>(
        `/pages/search?q=${encodeURIComponent(query)}`,
      );
      setPages(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Search failed");
    } finally {
      setIsLoading(false);
    }
  }, [fetchPages]);

  return {
    pages,
    isLoading,
    error,
    refresh: fetchPages,
    search,
  };
}
