"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { apiFetch } from "@/lib/api";
import { usePages } from "@/lib/use-pages";
import { PageList } from "@/components/pages/page-list";
import { SearchBar } from "@/components/pages/search-bar";
import { Button } from "@/components/ui/button";
import type { WorkspaceResponse } from "@historiador/types";

export default function PagesPage() {
  const router = useRouter();
  const [selectedCollectionId] = useState<string | null>(null);
  const { pages, isLoading, refresh, search } = usePages(selectedCollectionId);
  const [workspaceLanguages, setWorkspaceLanguages] = useState<string[]>([]);

  useEffect(() => {
    apiFetch<WorkspaceResponse>("/admin/workspace")
      .then((ws) => {
        setWorkspaceLanguages(ws.languages);
      })
      .catch(() => {
        // Non-admin users may not have access; use defaults
      });
  }, []);

  const handleSearch = useCallback(
    (query: string) => {
      search(query);
    },
    [search],
  );

  return (
    <div className="px-10 py-7 max-w-[1100px] mx-auto">
      <div className="flex items-end justify-between mb-6">
        <div>
          <h1
            className="text-text-primary"
            style={{ fontFamily: "var(--font-display)", fontSize: 40, lineHeight: 1, margin: 0, fontWeight: 400 }}
          >
            Páginas.
          </h1>
          <div className="mt-1 text-sm text-text-secondary">
            {pages.length} {pages.length === 1 ? "página" : "páginas"}
          </div>
        </div>
        <div className="flex items-center gap-3">
          <SearchBar onSearch={handleSearch} />
          <Button onClick={() => router.push("/editor")}>
            + Nova página
          </Button>
        </div>
      </div>

      <PageList
        pages={pages}
        isLoading={isLoading}
        workspaceLanguages={workspaceLanguages}
        onRefresh={refresh}
      />
    </div>
  );
}
