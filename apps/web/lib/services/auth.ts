// Auth service — thin wrappers around the /auth/* endpoints. Tokens
// live in localStorage; lib/api.ts handles 401 refresh on its own.

import { apiFetch } from "../api";
import type {
  LoginRequest,
  TokenResponse,
} from "@historiador/types";

export async function login(body: LoginRequest): Promise<TokenResponse> {
  return apiFetch<TokenResponse>("/auth/login", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export async function refresh(refreshToken: string): Promise<TokenResponse> {
  return apiFetch<TokenResponse>("/auth/refresh", {
    method: "POST",
    body: JSON.stringify({ refresh_token: refreshToken }),
  });
}

export async function logout(refreshToken: string): Promise<void> {
  return apiFetch<void>("/auth/logout", {
    method: "POST",
    body: JSON.stringify({ refresh_token: refreshToken }),
  });
}

export async function activate(inviteToken: string, password: string): Promise<void> {
  return apiFetch<void>("/auth/activate", {
    method: "POST",
    body: JSON.stringify({ invite_token: inviteToken, password }),
  });
}
