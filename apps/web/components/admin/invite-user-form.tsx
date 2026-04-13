"use client";

import { useState } from "react";
import { apiFetch } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { CopyButton } from "@/components/ui/copy-button";
import type { InviteResponse, Role } from "@historiador/types";

interface Props {
  onInvited: () => void;
}

const ROLE_OPTIONS = [
  { value: "author", label: "Author" },
  { value: "viewer", label: "Viewer" },
  { value: "admin", label: "Admin" },
];

export function InviteUserForm({ onInvited }: Props) {
  const [email, setEmail] = useState("");
  const [role, setRole] = useState<Role>("author");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [result, setResult] = useState<InviteResponse | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setResult(null);
    setLoading(true);

    try {
      const data = await apiFetch<InviteResponse>("/admin/users/invite", {
        method: "POST",
        body: JSON.stringify({ email, role }),
      });
      setResult(data);
      setEmail("");
      onInvited();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Invite failed");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-3">
      <form onSubmit={handleSubmit} className="flex items-end gap-2">
        <div className="flex-1">
          <Input
            label="Email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder="user@example.com"
            required
          />
        </div>
        <div className="w-32">
          <Select
            label="Role"
            options={ROLE_OPTIONS}
            value={role}
            onChange={(e) => setRole(e.target.value as Role)}
          />
        </div>
        <Button type="submit" disabled={loading}>
          {loading ? "Inviting..." : "Invite"}
        </Button>
      </form>

      {error && <p className="text-sm text-red-600">{error}</p>}

      {result && (
        <div className="rounded border border-green-200 dark:border-green-800 bg-green-50 dark:bg-green-900/30 p-3 space-y-2">
          <p className="text-sm text-green-800 dark:text-green-200">
            Invite sent! Share this activation link:
          </p>
          <div className="flex items-center gap-2">
            <code className="flex-1 text-xs bg-white dark:bg-zinc-800 p-2 rounded border break-all">
              {result.activation_url}
            </code>
            <CopyButton text={result.activation_url} />
          </div>
          <p className="text-xs text-zinc-500">
            Expires: {new Date(result.expires_at).toLocaleString()}
          </p>
        </div>
      )}
    </div>
  );
}
