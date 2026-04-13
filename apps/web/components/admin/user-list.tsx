"use client";

import { useState } from "react";
import { apiFetch } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { UserResponse } from "@historiador/types";

interface Props {
  users: UserResponse[];
  onRefresh: () => void;
}

export function UserList({ users, onRefresh }: Props) {
  const [deactivating, setDeactivating] = useState<string | null>(null);

  const handleDeactivate = async (userId: string) => {
    if (!confirm("Deactivate this user?")) return;
    setDeactivating(userId);
    try {
      await apiFetch(`/admin/users/${userId}/deactivate`, { method: "PATCH" });
      onRefresh();
    } catch {
      // Alpha error handling
    } finally {
      setDeactivating(null);
    }
  };

  if (users.length === 0) {
    return <p className="text-sm text-zinc-500">No users found.</p>;
  }

  return (
    <div className="border border-zinc-200 dark:border-zinc-700 rounded overflow-hidden">
      <table className="w-full text-sm">
        <thead className="bg-zinc-50 dark:bg-zinc-800">
          <tr>
            <th className="text-left px-4 py-2 font-medium text-zinc-600 dark:text-zinc-400">Email</th>
            <th className="text-left px-4 py-2 font-medium text-zinc-600 dark:text-zinc-400">Role</th>
            <th className="text-left px-4 py-2 font-medium text-zinc-600 dark:text-zinc-400">Status</th>
            <th className="px-4 py-2" />
          </tr>
        </thead>
        <tbody className="divide-y divide-zinc-200 dark:divide-zinc-700">
          {users.map((user) => (
            <tr key={user.id} className="hover:bg-zinc-50 dark:hover:bg-zinc-800/50">
              <td className="px-4 py-2">{user.email}</td>
              <td className="px-4 py-2">
                <Badge variant={user.role === "admin" ? "warning" : user.role === "author" ? "success" : "neutral"}>
                  {user.role}
                </Badge>
              </td>
              <td className="px-4 py-2">
                {user.pending ? (
                  <Badge variant="warning">Pending</Badge>
                ) : user.active ? (
                  <Badge variant="success">Active</Badge>
                ) : (
                  <Badge variant="danger">Deactivated</Badge>
                )}
              </td>
              <td className="px-4 py-2 text-right">
                {user.active && !user.pending && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleDeactivate(user.id)}
                    disabled={deactivating === user.id}
                  >
                    {deactivating === user.id ? "..." : "Deactivate"}
                  </Button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
