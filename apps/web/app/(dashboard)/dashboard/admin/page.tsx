"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/lib/auth-context";
import { apiFetch } from "@/lib/api";
import { UserList } from "@/components/admin/user-list";
import { InviteUserForm } from "@/components/admin/invite-user-form";
import { McpSettings } from "@/components/admin/mcp-settings";
import { McpAnalytics } from "@/components/admin/mcp-analytics";
import { WorkspaceConfig } from "@/components/admin/workspace-config";
import { Spinner } from "@/components/ui/spinner";
import type { UserResponse, WorkspaceResponse } from "@historiador/types";

export default function AdminPage() {
  const router = useRouter();
  const { isAdmin, isLoading: authLoading } = useAuth();
  const [users, setUsers] = useState<UserResponse[]>([]);
  const [workspace, setWorkspace] = useState<WorkspaceResponse | null>(null);
  const [loading, setLoading] = useState(true);

  const fetchData = useCallback(async () => {
    try {
      const [usersData, wsData] = await Promise.all([
        apiFetch<UserResponse[]>("/admin/users"),
        apiFetch<WorkspaceResponse>("/admin/workspace"),
      ]);
      setUsers(usersData);
      setWorkspace(wsData);
    } catch {
      // Non-admin will be redirected
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!authLoading && !isAdmin) {
      router.replace("/dashboard/pages");
      return;
    }
    if (!authLoading && isAdmin) {
      fetchData();
    }
  }, [authLoading, isAdmin, router, fetchData]);

  if (authLoading || loading) {
    return (
      <div className="flex justify-center py-8">
        <Spinner />
      </div>
    );
  }

  if (!workspace) {
    return <div className="text-center py-8 text-zinc-500">Unable to load admin data.</div>;
  }

  return (
    <div className="max-w-4xl space-y-8">
      <h1 className="text-lg font-semibold">Admin Panel</h1>

      {/* User Management */}
      <section className="space-y-4">
        <h2 className="text-md font-medium border-b border-zinc-200 dark:border-zinc-700 pb-2">
          Users
        </h2>
        <InviteUserForm onInvited={fetchData} />
        <UserList users={users} onRefresh={fetchData} />
      </section>

      {/* MCP Settings */}
      <section className="space-y-4">
        <h2 className="text-md font-medium border-b border-zinc-200 dark:border-zinc-700 pb-2">
          MCP Server
        </h2>
        <McpSettings workspace={workspace} />
      </section>

      {/* Workspace Config */}
      <section className="space-y-4">
        <h2 className="text-md font-medium border-b border-zinc-200 dark:border-zinc-700 pb-2">
          Workspace Configuration
        </h2>
        <WorkspaceConfig workspace={workspace} />
      </section>

      {/* MCP Analytics */}
      <section className="space-y-4">
        <h2 className="text-md font-medium border-b border-zinc-200 dark:border-zinc-700 pb-2">
          MCP Analytics
        </h2>
        <McpAnalytics />
      </section>
    </div>
  );
}
