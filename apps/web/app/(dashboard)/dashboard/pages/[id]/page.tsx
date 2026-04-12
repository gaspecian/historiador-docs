"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import { apiFetch } from "@/lib/api";
import { EditorPanel } from "@/components/editor/editor-panel";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import type { PageResponse, WorkspaceResponse } from "@/lib/types";

export default function PageDetailPage() {
  const params = useParams();
  const router = useRouter();
  const pageId = params.id as string;

  const [page, setPage] = useState<PageResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeLanguage, setActiveLanguage] = useState<string | null>(null);
  const [workspaceLanguages, setWorkspaceLanguages] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    Promise.all([
      apiFetch<PageResponse>(`/pages/${pageId}`),
      apiFetch<WorkspaceResponse>("/admin/workspace").catch(() => null),
    ]).then(([pageData, ws]) => {
      setPage(pageData);
      if (ws) setWorkspaceLanguages(ws.languages);
      if (pageData.versions.length > 0) {
        setActiveLanguage(pageData.versions[0].language);
      }
      setLoading(false);
    }).catch(() => setLoading(false));
  }, [pageId]);

  if (loading) {
    return (
      <div className="flex justify-center py-8">
        <Spinner />
      </div>
    );
  }

  if (!page) {
    return <div className="text-center py-8 text-zinc-500">Page not found</div>;
  }

  const activeVersion = page.versions.find((v) => v.language === activeLanguage);

  const handleSave = async (markdown: string) => {
    setSaving(true);
    try {
      await apiFetch(`/pages/${pageId}`, {
        method: "PATCH",
        body: JSON.stringify({
          title: activeVersion?.title,
          content_markdown: markdown,
          language: activeLanguage,
        }),
      });
      // Refresh page data
      const updated = await apiFetch<PageResponse>(`/pages/${pageId}`);
      setPage(updated);
    } catch {
      // Alpha error handling
    } finally {
      setSaving(false);
    }
  };

  const handleToggleStatus = async () => {
    const endpoint = page.status === "draft" ? "publish" : "draft";
    await apiFetch(`/pages/${pageId}/${endpoint}`, { method: "POST" });
    const updated = await apiFetch<PageResponse>(`/pages/${pageId}`);
    setPage(updated);
  };

  return (
    <div className="max-w-4xl space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <Button variant="ghost" size="sm" onClick={() => router.push("/dashboard/pages")}>
            &larr; Back
          </Button>
          <h1 className="text-lg font-semibold">
            {activeVersion?.title || page.slug}
          </h1>
          <Badge variant={page.status === "published" ? "success" : "warning"}>
            {page.status}
          </Badge>
        </div>
        <div className="flex gap-2">
          <Button
            variant={page.status === "draft" ? "primary" : "secondary"}
            size="sm"
            onClick={handleToggleStatus}
          >
            {page.status === "draft" ? "Publish" : "Unpublish"}
          </Button>
        </div>
      </div>

      {/* Language tabs */}
      {page.versions.length > 1 && (
        <div className="flex gap-1 border-b border-zinc-200 dark:border-zinc-700">
          {page.versions.map((v) => (
            <button
              key={v.language}
              onClick={() => setActiveLanguage(v.language)}
              className={`px-3 py-1.5 text-sm border-b-2 transition-colors ${
                v.language === activeLanguage
                  ? "border-blue-600 text-blue-600"
                  : "border-transparent text-zinc-500 hover:text-zinc-700"
              }`}
            >
              {v.language}
            </button>
          ))}
        </div>
      )}

      {/* Content preview */}
      {activeVersion && (
        <div className="border border-zinc-200 dark:border-zinc-700 rounded p-4">
          <pre className="whitespace-pre-wrap break-words font-mono text-sm">
            {activeVersion.content_markdown}
          </pre>
        </div>
      )}

      {/* Editor */}
      <div className="border border-zinc-200 dark:border-zinc-700 rounded p-4">
        <h2 className="text-sm font-medium mb-3">AI Editor</h2>
        <EditorPanel
          initialContent={activeVersion?.content_markdown}
          language={activeLanguage || undefined}
          onSave={handleSave}
        />
        {saving && <p className="text-xs text-zinc-500 mt-2">Saving...</p>}
      </div>
    </div>
  );
}
