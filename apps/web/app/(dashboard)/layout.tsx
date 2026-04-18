"use client";

import { useEffect, useState } from "react";
import { useRouter, usePathname } from "next/navigation";
import Link from "next/link";
import { useAuth } from "@/lib/auth-context";
import { useCollections } from "@/lib/use-collections";
import { CollectionTree } from "@/components/collections/collection-tree";
import { CreateCollectionDialog } from "@/components/collections/create-collection-dialog";
import { UserMenu } from "@/components/layout/user-menu";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";

export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const router = useRouter();
  const pathname = usePathname();
  const { isAuthenticated, isAdmin, isLoading: authLoading } = useAuth();
  const collections = useCollections();
  const [showCreateCollection, setShowCreateCollection] = useState(false);

  useEffect(() => {
    if (!authLoading && !isAuthenticated) {
      router.replace("/login");
    }
  }, [authLoading, isAuthenticated, router]);

  if (authLoading) {
    return (
      <div className="flex min-h-screen items-center justify-center">
        <Spinner />
      </div>
    );
  }

  if (!isAuthenticated) return null;

  const handleCollectionSelect = (id: string | null) => {
    collections.setSelectedId(id);
    // Navigate to pages view if not already there
    if (!pathname.startsWith("/dashboard/pages") || pathname.includes("/dashboard/pages/")) {
      router.push("/dashboard/pages");
    }
  };

  return (
    <div className="flex h-screen overflow-hidden">
      {/* Sidebar */}
      <aside className="w-60 flex-shrink-0 border-r border-surface-border bg-surface-subtle flex flex-col text-[13px]">
        <div className="px-3.5 pt-3.5 pb-2">
          <Link
            href="/dashboard/pages"
            className="flex items-center gap-2 px-1.5 py-1 text-text-primary"
            style={{ fontFamily: "var(--font-display)", fontSize: 19, fontStyle: "italic" }}
          >
            <span className="text-primary-600">
              <svg width={20} height={20} viewBox="0 0 32 32" fill="none" stroke="currentColor" strokeWidth={2.2} strokeLinecap="round" strokeLinejoin="round">
                <path d="M16 7 V25" />
                <path d="M16 7 C 12 5, 8 5, 4.5 6 V 24 C 8 23, 12 23, 16 25" />
                <path d="M16 7 C 20 5, 24 5, 27.5 6 V 24 C 24 23, 20 23, 16 25" />
                <path d="M22 10 L 26 14" />
              </svg>
            </span>
            Historiador
          </Link>
        </div>

        {/* Navigation */}
        <nav className="px-2 space-y-1">
          <Link
            href="/dashboard/pages"
            className={`block px-3 py-1.5 text-[13px] rounded-md transition-colors ${pathname.startsWith("/dashboard/pages")
                ? "bg-primary-50 text-primary-700 font-medium"
                : "text-text-secondary hover:bg-surface-hover"
              }`}
          >
            Pages
          </Link>
          {isAdmin && (
            <Link
              href="/dashboard/admin"
              className={`block px-3 py-1.5 text-[13px] rounded-md transition-colors ${pathname === "/dashboard/admin"
                  ? "bg-primary-50 text-primary-700 font-medium"
                  : "text-text-secondary hover:bg-surface-hover"
                }`}
            >
              Admin
            </Link>
          )}
        </nav>

        {/* Collection tree */}
        <div className="flex-1 overflow-y-auto p-2">
          <div className="flex items-center justify-between px-2 py-1">
            <span className="text-xs font-medium text-text-tertiary uppercase tracking-wider">
              Collections
            </span>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setShowCreateCollection(!showCreateCollection)}
              className="text-xs"
            >
              + New
            </Button>
          </div>

          {showCreateCollection && (
            <CreateCollectionDialog
              onCreated={() => {
                setShowCreateCollection(false);
                collections.refresh();
              }}
              onCancel={() => setShowCreateCollection(false)}
            />
          )}

          <CollectionTree
            tree={collections.tree}
            selectedId={collections.selectedId}
            expandedIds={collections.expandedIds}
            isLoading={collections.isLoading}
            onSelect={handleCollectionSelect}
            onToggleExpand={collections.toggleExpanded}
          />
        </div>

        <div className="mt-auto border-t border-surface-border px-3 pt-2.5 pb-3 flex items-center gap-2 text-xs text-teal-700">
          <span className="relative inline-block h-[7px] w-[7px] rounded-full bg-teal-600">
            <span
              className="absolute rounded-full border-2 border-teal-600 opacity-35"
              style={{ inset: -3, animation: "pulse 1.6s infinite" }}
            />
          </span>
          MCP active
        </div>
      </aside>

      {/* Main content area */}
      <div className="flex-1 flex flex-col overflow-hidden bg-surface-page">
        {/* Top bar */}
        <header className="flex h-14 items-center justify-between bg-surface-canvas border-b border-surface-border px-6 gap-4">
          <div className="text-[13px] text-text-secondary flex-1">
            {collections.selectedId
              ? (
                <span>
                  <span className="text-text-disabled">/ </span>
                  <span className="font-medium text-text-primary">
                    {collections.collections.find((c) => c.id === collections.selectedId)?.name ?? "Collection"}
                  </span>
                </span>
              )
              : "All pages"}
          </div>
          <UserMenu />
        </header>

        {/* Page content */}
        <main className="flex-1 overflow-y-auto">
          {children}
        </main>
      </div>
    </div>
  );
}
