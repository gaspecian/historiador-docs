// Export service — binary downloads for zip / single-page exports.

import { apiDownload } from "../api";

export async function workspaceZip(filename?: string): Promise<void> {
  return apiDownload("/export", filename);
}

export async function pageMarkdown(
  pageId: string,
  language?: string,
  filename?: string,
): Promise<void> {
  const qs = language ? `?language=${encodeURIComponent(language)}` : "";
  return apiDownload(`/pages/${encodeURIComponent(pageId)}/export${qs}`, filename);
}
