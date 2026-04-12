"use client";

import Link from "next/link";
import { Badge } from "@/components/ui/badge";
import { Spinner } from "@/components/ui/spinner";
import { DraftPublishToggle } from "./draft-publish-toggle";
import { LanguageBadges } from "./language-badges";
import type { PageResponse } from "@/lib/types";

interface Props {
  pages: PageResponse[];
  isLoading: boolean;
  workspaceLanguages: string[];
  onRefresh: () => void;
}

export function PageList({ pages, isLoading, workspaceLanguages, onRefresh }: Props) {
  if (isLoading) {
    return (
      <div className="flex justify-center py-8">
        <Spinner />
      </div>
    );
  }

  if (pages.length === 0) {
    return (
      <div className="text-center py-8 text-sm text-zinc-500">
        No pages yet. Create one to get started.
      </div>
    );
  }

  return (
    <div className="border border-zinc-200 dark:border-zinc-700 rounded overflow-hidden">
      <table className="w-full text-sm">
        <thead className="bg-zinc-50 dark:bg-zinc-800">
          <tr>
            <th className="text-left px-4 py-2 font-medium text-zinc-600 dark:text-zinc-400">
              Title
            </th>
            <th className="text-left px-4 py-2 font-medium text-zinc-600 dark:text-zinc-400">
              Status
            </th>
            <th className="text-left px-4 py-2 font-medium text-zinc-600 dark:text-zinc-400">
              Languages
            </th>
            <th className="text-left px-4 py-2 font-medium text-zinc-600 dark:text-zinc-400">
              Updated
            </th>
            <th className="px-4 py-2" />
          </tr>
        </thead>
        <tbody className="divide-y divide-zinc-200 dark:divide-zinc-700">
          {pages.map((page) => {
            const primaryVersion = page.versions[0];
            const title = primaryVersion?.title || page.slug;
            return (
              <tr key={page.id} className="hover:bg-zinc-50 dark:hover:bg-zinc-800/50">
                <td className="px-4 py-2">
                  <Link
                    href={`/dashboard/pages/${page.id}`}
                    className="text-blue-600 dark:text-blue-400 hover:underline"
                  >
                    {title}
                  </Link>
                </td>
                <td className="px-4 py-2">
                  <Badge variant={page.status === "published" ? "success" : "warning"}>
                    {page.status}
                  </Badge>
                </td>
                <td className="px-4 py-2">
                  <LanguageBadges
                    versions={page.versions}
                    workspaceLanguages={workspaceLanguages}
                  />
                </td>
                <td className="px-4 py-2 text-zinc-500">
                  {new Date(page.updated_at).toLocaleDateString()}
                </td>
                <td className="px-4 py-2">
                  <DraftPublishToggle
                    pageId={page.id}
                    status={page.status}
                    onToggled={onRefresh}
                  />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
