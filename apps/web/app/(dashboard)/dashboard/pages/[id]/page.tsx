"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter, useSearchParams } from "next/navigation";
import { apiFetch } from "@/lib/api";
import { EditorPanel } from "@/components/editor/editor-panel";
import { LanguageTabs } from "@/components/pages/language-tabs";
import { PublishConfirmModal } from "@/components/pages/publish-confirm-modal";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import type { PageResponse, WorkspaceResponse } from "@historiador/types";

export default function PageDetailPage() {
  const params = useParams();
  const router = useRouter();
  const searchParams = useSearchParams();
  const pageId = params.id as string;

  const [page, setPage] = useState<PageResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeLanguage, setActiveLanguage] = useState<string | null>(null);
  const [workspaceLanguages, setWorkspaceLanguages] = useState<string[]>([]);
  const [primaryLanguage, setPrimaryLanguage] = useState<string>("en");
  const [saving, setSaving] = useState(false);
  const [showPublishModal, setShowPublishModal] = useState(false);

  useEffect(() => {
    Promise.all([
      apiFetch<PageResponse>(`/pages/${pageId}`),
      apiFetch<WorkspaceResponse>("/admin/workspace").catch(() => null),
    ]).then(([pageData, ws]) => {
      setPage(pageData);
      if (ws) {
        setWorkspaceLanguages(ws.languages);
        setPrimaryLanguage(ws.primary_language);
      }
      // Use ?lang= query param if provided, otherwise default to first version's language
      const langParam = searchParams.get("lang");
      if (langParam && ws?.languages.includes(langParam)) {
        setActiveLanguage(langParam);
      } else if (pageData.versions.length > 0) {
        setActiveLanguage(pageData.versions[0].language);
      } else if (ws) {
        setActiveLanguage(ws.primary_language);
      }
      setLoading(false);
    }).catch(() => setLoading(false));
  }, [pageId, searchParams]);

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
  const isMissingLanguage = activeLanguage && !activeVersion;
  const primaryVersion = page.versions.find((v) => v.language === primaryLanguage);

  const handleSave = async (markdown: string) => {
    setSaving(true);
    try {
      await apiFetch(`/pages/${pageId}`, {
        method: "PATCH",
        body: JSON.stringify({
          title: activeVersion?.title || primaryVersion?.title || "Untitled",
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

  const missingLanguages = workspaceLanguages.filter(
    (lang) => !page.versions.some((v) => v.language === lang),
  );

  const handlePublishClick = () => {
    if (page.status === "published") {
      // Unpublish — no check needed
      doToggleStatus();
      return;
    }
    // Publishing — check completeness
    if (missingLanguages.length > 0) {
      setShowPublishModal(true);
    } else {
      doToggleStatus();
    }
  };

  const doToggleStatus = async () => {
    setShowPublishModal(false);
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
            {activeVersion?.title || primaryVersion?.title || page.slug}
          </h1>
          <Badge variant={page.status === "published" ? "success" : "warning"}>
            {page.status}
          </Badge>
        </div>
        <div className="flex gap-2">
          <Button
            variant={page.status === "draft" ? "primary" : "secondary"}
            size="sm"
            onClick={handlePublishClick}
          >
            {page.status === "draft" ? "Publish" : "Unpublish"}
          </Button>
        </div>
      </div>

      {/* Language tabs */}
      <LanguageTabs
        workspaceLanguages={workspaceLanguages}
        versions={page.versions}
        activeLanguage={activeLanguage}
        onSelect={setActiveLanguage}
      />

      {/* Content preview or missing-language prompt */}
      {isMissingLanguage ? (
        <div className="border border-amber-200 dark:border-amber-800 rounded p-6 text-center space-y-3 bg-amber-50 dark:bg-amber-900/20">
          <p className="text-sm text-amber-800 dark:text-amber-200">
            No <strong>{activeLanguage}</strong> version exists yet.
          </p>
          <div className="flex justify-center gap-3">
            <Button
              variant="secondary"
              size="sm"
              onClick={() => handleSave("")}
            >
              Create blank version
            </Button>
            {primaryVersion && activeLanguage !== primaryLanguage && (
              <Button
                variant="primary"
                size="sm"
                onClick={() => handleSave(primaryVersion.content_markdown)}
              >
                Copy from {primaryLanguage}
              </Button>
            )}
          </div>
        </div>
      ) : activeVersion ? (
        <div className="border border-zinc-200 dark:border-zinc-700 rounded p-4">
          <pre className="whitespace-pre-wrap break-words font-mono text-sm">
            {activeVersion.content_markdown}
          </pre>
        </div>
      ) : null}

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

      <PublishConfirmModal
        open={showPublishModal}
        missingLanguages={missingLanguages}
        onConfirm={doToggleStatus}
        onCancel={() => setShowPublishModal(false)}
      />
    </div>
  );
}
