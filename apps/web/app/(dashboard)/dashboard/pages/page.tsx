"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { apiFetch } from "@/lib/api";
import { usePages } from "@/lib/use-pages";
import { PageList } from "@/components/pages/page-list";
import { SearchBar } from "@/components/pages/search-bar";
import { EditorPanel } from "@/components/editor/editor-panel";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { WorkspaceResponse } from "@historiador/types";

export default function PagesPage() {
  const router = useRouter();
  // Read selectedCollectionId from URL search params or use null
  const [selectedCollectionId] = useState<string | null>(null);
  const { pages, isLoading, refresh, search } = usePages(selectedCollectionId);
  const [showCreate, setShowCreate] = useState(false);
  const [createTitle, setCreateTitle] = useState("");
  const [workspaceLanguages, setWorkspaceLanguages] = useState<string[]>([]);
  const [primaryLanguage, setPrimaryLanguage] = useState("en");

  useEffect(() => {
    apiFetch<WorkspaceResponse>("/admin/workspace")
      .then((ws) => {
        setWorkspaceLanguages(ws.languages);
        setPrimaryLanguage(ws.primary_language);
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

  const handleSaveNewPage = async (markdown: string) => {
    try {
      const res = await apiFetch<{ id: string }>("/pages", {
        method: "POST",
        body: JSON.stringify({
          collection_id: selectedCollectionId,
          title: createTitle || "Untitled",
          content_markdown: markdown,
          language: primaryLanguage,
        }),
      });
      setShowCreate(false);
      setCreateTitle("");
      refresh();
      router.push(`/dashboard/pages/${res.id}`);
    } catch {
      // Error handling in alpha
    }
  };

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
          <Button onClick={() => setShowCreate(!showCreate)}>
            {showCreate ? "Cancel" : "+ Nova página"}
          </Button>
        </div>
      </div>

      {showCreate && (
        <div className="mb-6 rounded-lg border border-surface-border bg-surface-canvas p-5 space-y-3 shadow-sm">
          <h2 className="text-sm font-medium">Create new page</h2>
          <Input
            label="Page title"
            value={createTitle}
            onChange={(e) => setCreateTitle(e.target.value)}
            placeholder="Enter a title..."
          />
          <EditorPanel
            language={primaryLanguage}
            onSave={handleSaveNewPage}
          />
        </div>
      )}

      <PageList
        pages={pages}
        isLoading={isLoading}
        workspaceLanguages={workspaceLanguages}
        onRefresh={refresh}
      />
    </div>
  );
}
