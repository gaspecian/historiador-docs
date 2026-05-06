"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as adminService from "@/lib/services/admin";
import type {
  LlmPatchBody,
  LlmPatchResult,
} from "@/lib/services/admin";
import type { InviteRequest } from "@historiador/types";
import { queryKeys } from "./keys";

export function useUsersQuery() {
  return useQuery({
    queryKey: queryKeys.admin.users(),
    queryFn: () => adminService.listUsers(),
  });
}

export function useWorkspaceQuery() {
  return useQuery({
    queryKey: queryKeys.admin.workspace(),
    queryFn: () => adminService.getWorkspace(),
  });
}

export function useMcpAnalyticsQuery(days: number) {
  return useQuery({
    queryKey: queryKeys.admin.mcpAnalytics(days),
    queryFn: () => adminService.mcpAnalytics(days),
  });
}

// ---- mutations ----

export function useInviteUserMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (body: InviteRequest) => adminService.invite(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.admin.users() });
    },
  });
}

export function useDeactivateUserMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (userId: string) => adminService.deactivateUser(userId),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.admin.users() });
    },
  });
}

export function useRegenerateTokenMutation() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => adminService.regenerateToken(),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.admin.workspace() });
    },
  });
}

export function useUpdateLlmConfigMutation() {
  const qc = useQueryClient();
  return useMutation<LlmPatchResult, Error, LlmPatchBody>({
    mutationFn: (body) => adminService.updateLlmConfig(body),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: queryKeys.admin.workspace() });
    },
  });
}

export function useReindexMutation() {
  return useMutation({
    mutationFn: () => adminService.reindex(),
  });
}
