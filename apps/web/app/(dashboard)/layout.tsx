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
      <aside className="w-64 flex-shrink-0 border-r border-zinc-200 dark:border-zinc-700 bg-zinc-50 dark:bg-zinc-900 flex flex-col">
        <div className="p-4 border-b border-zinc-200 dark:border-zinc-700">
          <Link href="/dashboard/pages" className="text-lg font-bold">
            Historiador
          </Link>
        </div>

        {/* Navigation */}
        <nav className="p-2 space-y-1">
          <Link
            href="/dashboard/pages"
            className={`block px-3 py-1.5 text-sm rounded transition-colors ${
              pathname.startsWith("/dashboard/pages")
                ? "bg-zinc-200 dark:bg-zinc-800 font-medium"
                : "hover:bg-zinc-100 dark:hover:bg-zinc-800"
            }`}
          >
            Pages
          </Link>
          {isAdmin && (
            <Link
              href="/dashboard/admin"
              className={`block px-3 py-1.5 text-sm rounded transition-colors ${
                pathname === "/dashboard/admin"
                  ? "bg-zinc-200 dark:bg-zinc-800 font-medium"
                  : "hover:bg-zinc-100 dark:hover:bg-zinc-800"
              }`}
            >
              Admin
            </Link>
          )}
        </nav>

        {/* Collection tree */}
        <div className="flex-1 overflow-y-auto p-2">
          <div className="flex items-center justify-between px-2 py-1">
            <span className="text-xs font-medium text-zinc-500 uppercase tracking-wider">
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
      </aside>

      {/* Main content area */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {/* Top bar */}
        <header className="flex items-center justify-between px-6 py-3 border-b border-zinc-200 dark:border-zinc-700">
          <div className="text-sm text-zinc-500">
            {collections.selectedId
              ? collections.collections.find((c) => c.id === collections.selectedId)?.name ?? "Collection"
              : "All Pages"}
          </div>
          <UserMenu />
        </header>

        {/* Page content */}
        <main className="flex-1 overflow-y-auto p-6">
          {children}
        </main>
      </div>
    </div>
  );
}
