"use client";

import { useCallback, useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { useAuth } from "@/lib/auth-context";
import * as adminService from "@/lib/services/admin";
import { UserList } from "@/components/admin/user-list";
import { InviteUserForm } from "@/components/admin/invite-user-form";
import { McpSettings } from "@/components/admin/mcp-settings";
import { McpAnalytics } from "@/components/admin/mcp-analytics";
import { WorkspaceConfig } from "@/components/admin/workspace-config";
import { LlmSettingsForm } from "@/components/admin/llm-settings-form";
import { ExportSection } from "@/components/admin/export-section";
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
        adminService.listUsers(),
        adminService.getWorkspace(),
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
    return <div className="text-center py-8 text-text-tertiary">Unable to load admin data.</div>;
  }

  return (
    <div className="px-10 py-7 max-w-4xl mx-auto space-y-8">
      <h1 className="text-lg font-semibold">Admin Panel</h1>

      {/* User Management */}
      <section className="space-y-4">
        <h2 className="text-md font-medium border-b border-surface-border pb-2">
          Users
        </h2>
        <InviteUserForm onInvited={fetchData} />
        <UserList users={users} onRefresh={fetchData} />
      </section>

      {/* MCP Settings */}
      <section className="space-y-4">
        <h2 className="text-md font-medium border-b border-surface-border pb-2">
          MCP Server
        </h2>
        <McpSettings workspace={workspace} />
      </section>

      {/* Workspace Config */}
      <section className="space-y-4">
        <h2 className="text-md font-medium border-b border-surface-border pb-2">
          Workspace Configuration
        </h2>
        <WorkspaceConfig workspace={workspace} />
      </section>

      {/* LLM Settings */}
      <section className="space-y-4">
        <h2 className="text-md font-medium border-b border-surface-border pb-2">
          LLM Settings
        </h2>
        <LlmSettingsForm workspace={workspace} onSaved={fetchData} />
      </section>

      {/* Export */}
      <section className="space-y-4">
        <h2 className="text-md font-medium border-b border-surface-border pb-2">
          Export
        </h2>
        <ExportSection />
      </section>

      {/* MCP Analytics */}
      <section className="space-y-4">
        <h2 className="text-md font-medium border-b border-surface-border pb-2">
          MCP Analytics
        </h2>
        <McpAnalytics />
      </section>
    </div>
  );
}
