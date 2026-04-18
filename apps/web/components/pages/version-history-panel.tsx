"use client";

import { useCallback, useEffect, useState } from "react";
import * as pagesService from "@/lib/services/pages";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import type {
 VersionHistoryListResponse,
 VersionHistoryDetailResponse,
 VersionHistorySummary,
} from "@historiador/types";

interface VersionHistoryPanelProps {
 pageId: string;
 language: string;
 open: boolean;
 onClose: () => void;
 onRestore: () => void;
}

export function VersionHistoryPanel({
 pageId,
 language,
 open,
 onClose,
 onRestore,
}: VersionHistoryPanelProps) {
 const [loading, setLoading] = useState(false);
 const [data, setData] = useState<VersionHistoryListResponse | null>(null);
 const [selectedId, setSelectedId] = useState<string | null>(null);
 const [detail, setDetail] = useState<VersionHistoryDetailResponse | null>(null);
 const [detailLoading, setDetailLoading] = useState(false);
 const [restoring, setRestoring] = useState(false);
 const [page, setPage] = useState(1);

 const fetchHistory = useCallback(async () => {
 setLoading(true);
 try {
 const result = await pagesService.history(pageId, {
 language,
 page,
 per_page: 20,
 });
 setData(result);
 } catch {
 // Silently fail for alpha
 } finally {
 setLoading(false);
 }
 }, [pageId, language, page]);

 useEffect(() => {
 if (open) {
 fetchHistory();
 setSelectedId(null);
 setDetail(null);
 }
 }, [open, fetchHistory]);

 const handleSelect = async (entry: VersionHistorySummary) => {
 setSelectedId(entry.id);
 setDetailLoading(true);
 try {
 const result = await pagesService.historyItem(pageId, entry.id);
 setDetail(result);
 } catch {
 setDetail(null);
 } finally {
 setDetailLoading(false);
 }
 };

 const handleRestore = async () => {
 if (!selectedId) return;
 setRestoring(true);
 try {
 await pagesService.restoreVersion(pageId, selectedId);
 onRestore();
 onClose();
 } catch {
 // Alpha error handling
 } finally {
 setRestoring(false);
 }
 };

 if (!open) return null;

 const totalPages = data ? Math.ceil(data.total / data.per_page) : 1;

 return (
 <div className="fixed inset-0 z-50 flex justify-end">
 {/* Backdrop */}
 <div className="absolute inset-0 bg-black/30" onClick={onClose} />

 {/* Panel */}
 <div className="relative flex w-full max-w-2xl bg-white shadow-xl">
 {/* Timeline list */}
 <div className="w-1/2 border-r border-surface-border overflow-y-auto">
 <div className="flex items-center justify-between p-3 border-b border-surface-border">
 <h3 className="text-sm font-semibold">Version History</h3>
 <Button variant="ghost" size="sm" onClick={onClose}>
 &times;
 </Button>
 </div>

 {loading ? (
 <div className="flex justify-center py-8">
 <Spinner />
 </div>
 ) : data && data.versions.length > 0 ? (
 <>
 <ul className="divide-y divide-zinc-100">
 {data.versions.map((v) => (
 <li
 key={v.id}
 className={`p-3 cursor-pointer hover:bg-surface-subtle transition-colors ${
 selectedId === v.id
 ? "bg-primary-50 border-l-2 border-primary-500"
 : ""
 }`}
 onClick={() => handleSelect(v)}
 >
 <div className="flex items-center gap-2 mb-1">
 <span className="text-xs font-mono text-text-tertiary">
 v{v.version_number}
 </span>
 {v.is_published && (
 <Badge variant="success">Published</Badge>
 )}
 </div>
 <p className="text-sm font-medium truncate">{v.title}</p>
 <p className="text-xs text-text-tertiary mt-1">
 {new Date(v.created_at).toLocaleString()}
 </p>
 </li>
 ))}
 </ul>

 {/* Pagination */}
 {totalPages > 1 && (
 <div className="flex items-center justify-between p-3 border-t border-surface-border">
 <Button
 variant="ghost"
 size="sm"
 disabled={page <= 1}
 onClick={() => setPage((p) => p - 1)}
 >
 Prev
 </Button>
 <span className="text-xs text-text-tertiary">
 {page} / {totalPages}
 </span>
 <Button
 variant="ghost"
 size="sm"
 disabled={page >= totalPages}
 onClick={() => setPage((p) => p + 1)}
 >
 Next
 </Button>
 </div>
 )}
 </>
 ) : (
 <p className="text-sm text-text-tertiary text-center py-8">
 No version history yet.
 </p>
 )}
 </div>

 {/* Preview pane */}
 <div className="w-1/2 overflow-y-auto">
 <div className="p-3 border-b border-surface-border">
 <h3 className="text-sm font-semibold">Preview</h3>
 </div>

 {detailLoading ? (
 <div className="flex justify-center py-8">
 <Spinner />
 </div>
 ) : detail ? (
 <div className="p-3 space-y-3">
 <div className="flex items-center gap-2">
 <span className="text-xs font-mono text-text-tertiary">
 v{detail.version_number}
 </span>
 {detail.is_published && (
 <Badge variant="success">Published</Badge>
 )}
 </div>
 <h4 className="text-sm font-medium">{detail.title}</h4>
 <p className="text-xs text-text-tertiary">
 {new Date(detail.created_at).toLocaleString()}
 </p>
 <pre className="whitespace-pre-wrap break-words font-mono text-xs bg-surface-subtle rounded p-3 max-h-96 overflow-y-auto">
 {detail.content_markdown}
 </pre>
 <Button
 variant="secondary"
 size="sm"
 onClick={handleRestore}
 disabled={restoring}
 >
 {restoring ? "Restoring..." : "Restore as draft"}
 </Button>
 </div>
 ) : (
 <p className="text-sm text-text-tertiary text-center py-8">
 Select a version to preview.
 </p>
 )}
 </div>
 </div>
 </div>
 );
}
