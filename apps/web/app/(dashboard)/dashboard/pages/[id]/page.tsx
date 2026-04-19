"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter, useSearchParams } from "next/navigation";
import { marked } from "marked";
import { Pencil } from "lucide-react";
import * as adminService from "@/lib/services/admin";
import * as pagesService from "@/lib/services/pages";
import * as exportService from "@/lib/services/export";
import { useAuth } from "@/lib/auth-context";
import { LanguageTabs } from "@/components/pages/language-tabs";
import { PublishConfirmModal } from "@/components/pages/publish-confirm-modal";
import { VersionHistoryPanel } from "@/components/pages/version-history-panel";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dropdown } from "@/components/ui/dropdown";
import { Spinner } from "@/components/ui/spinner";
import type { PageResponse } from "@historiador/types";

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
  const [showPublishModal, setShowPublishModal] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const { canEdit } = useAuth();

  useEffect(() => {
    Promise.all([
      pagesService.get(pageId),
      adminService.getWorkspace().catch(() => null),
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
    return <div className="text-center py-8 text-text-tertiary">Page not found</div>;
  }

  const activeVersion = page.versions.find((v) => v.language === activeLanguage);
  const isMissingLanguage = activeLanguage && !activeVersion;
  const primaryVersion = page.versions.find((v) => v.language === primaryLanguage);

  const handleCreateBlank = async () => {
    await pagesService.update(pageId, {
      title: activeVersion?.title || primaryVersion?.title || "Untitled",
      content_markdown: "",
      language: activeLanguage ?? undefined,
    });
    setPage(await pagesService.get(pageId));
  };

  const handleCopyFromPrimary = async () => {
    if (!primaryVersion) return;
    await pagesService.update(pageId, {
      title: activeVersion?.title || primaryVersion.title || "Untitled",
      content_markdown: primaryVersion.content_markdown,
      language: activeLanguage ?? undefined,
    });
    setPage(await pagesService.get(pageId));
  };

  const goToEditor = () => {
    const params = new URLSearchParams({ page_id: pageId });
    if (activeLanguage) params.set("lang", activeLanguage);
    router.push(`/editor?${params.toString()}`);
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
    if (page.status === "draft") {
      await pagesService.publish(pageId);
    } else {
      await pagesService.draft(pageId);
    }
    const updated = await pagesService.get(pageId);
    setPage(updated);
  };

  return (
    <div className="px-10 py-7 max-w-4xl mx-auto space-y-4">
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
          {canEdit && activeVersion && (
            <Button size="sm" onClick={goToEditor} title="Abrir no editor">
              <span className="inline-flex items-center gap-1.5">
                <Pencil className="w-4 h-4" aria-hidden />
                Editar
              </span>
            </Button>
          )}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setShowHistory(true)}
            title="Version history"
          >
            History
          </Button>
          {canEdit && (
            <Button
              variant={page.status === "draft" ? "primary" : "secondary"}
              size="sm"
              onClick={handlePublishClick}
            >
              {page.status === "draft" ? "Publish" : "Unpublish"}
            </Button>
          )}
          <Dropdown
            trigger={<span aria-label="More actions">⋮</span>}
            items={[
              {
                label: "Download as Markdown",
                onClick: () => {
                  exportService
                    .pageMarkdown(pageId, activeLanguage ?? undefined)
                    .catch(() => {
                      /* alpha error handling */
                    });
                },
                disabled: page.status !== "published" || !activeLanguage,
              },
            ]}
          />
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
        <div className="border border-amber-200 rounded p-6 text-center space-y-3 bg-amber-50">
          <p className="text-sm text-amber-800">
            Nenhuma versão em <strong>{activeLanguage}</strong> existe ainda.
          </p>
          {canEdit && (
            <div className="flex justify-center gap-3">
              <Button variant="secondary" size="sm" onClick={handleCreateBlank}>
                Criar versão em branco
              </Button>
              {primaryVersion && activeLanguage !== primaryLanguage && (
                <Button variant="primary" size="sm" onClick={handleCopyFromPrimary}>
                  Copiar de {primaryLanguage}
                </Button>
              )}
            </div>
          )}
        </div>
      ) : activeVersion ? (
        <article
          className="md-prose border border-surface-border rounded-lg bg-surface-canvas p-8"
          dangerouslySetInnerHTML={{ __html: renderPageMarkdown(activeVersion.content_markdown) }}
        />
      ) : null}

      <PublishConfirmModal
        open={showPublishModal}
        missingLanguages={missingLanguages}
        onConfirm={doToggleStatus}
        onCancel={() => setShowPublishModal(false)}
      />

      <VersionHistoryPanel
        pageId={pageId}
        language={activeLanguage || primaryLanguage}
        open={showHistory}
        onClose={() => setShowHistory(false)}
        onRestore={async () => {
          const updated = await pagesService.get(pageId);
          setPage(updated);
        }}
      />
    </div>
  );
}

function renderPageMarkdown(md: string): string {
  const cleaned = md.replace(/<!--\s*block:[0-9a-fA-F-]+\s*-->/g, "");
  const html = marked.parse(cleaned, { async: false, gfm: true, breaks: false });
  return typeof html === "string" ? html : "";
}
