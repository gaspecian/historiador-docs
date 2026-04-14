// Central fetch wrapper for all API calls.
// - Injects Authorization header from localStorage
// - Intercepts 401: attempts one token refresh, retries the request
// - Intercepts 423: redirects to /setup (first-run)

import type { TokenResponse } from "@historiador/types";

let isRefreshing = false;
let refreshPromise: Promise<boolean> | null = null;

function getAccessToken(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem("access_token");
}

function getRefreshToken(): string | null {
  if (typeof window === "undefined") return null;
  return localStorage.getItem("refresh_token");
}

function setTokens(access: string, refresh: string) {
  localStorage.setItem("access_token", access);
  localStorage.setItem("refresh_token", refresh);
}

function clearTokens() {
  localStorage.removeItem("access_token");
  localStorage.removeItem("refresh_token");
}

async function attemptRefresh(): Promise<boolean> {
  const refreshToken = getRefreshToken();
  if (!refreshToken) return false;

  try {
    const res = await fetch("/api/auth/refresh", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ refresh_token: refreshToken }),
    });
    if (!res.ok) return false;
    const data: TokenResponse = await res.json();
    setTokens(data.access_token, data.refresh_token);
    return true;
  } catch {
    return false;
  }
}

/**
 * Typed fetch wrapper for the API. All frontend code should use this
 * instead of raw `fetch()`.
 */
export async function apiFetch<T = unknown>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const headers = new Headers(options.headers);

  const token = getAccessToken();
  if (token) {
    headers.set("Authorization", `Bearer ${token}`);
  }
  if (!headers.has("Content-Type") && options.body) {
    headers.set("Content-Type", "application/json");
  }

  const res = await fetch(`/api${path}`, { ...options, headers });

  // First-run redirect
  if (res.status === 423) {
    if (typeof window !== "undefined") {
      window.location.href = "/setup";
    }
    throw new Error("Setup required");
  }

  // Token expired — try refresh once
  if (res.status === 401 && token) {
    if (!isRefreshing) {
      isRefreshing = true;
      refreshPromise = attemptRefresh().finally(() => {
        isRefreshing = false;
        refreshPromise = null;
      });
    }

    const refreshed = await (refreshPromise ?? Promise.resolve(false));
    if (refreshed) {
      // Retry with new token
      const newToken = getAccessToken();
      headers.set("Authorization", `Bearer ${newToken}`);
      const retry = await fetch(`/api${path}`, { ...options, headers });
      if (!retry.ok) {
        throw new ApiError(retry.status, await retry.text());
      }
      return retry.json();
    }

    // Refresh failed — clear and redirect to login
    clearTokens();
    if (typeof window !== "undefined") {
      window.location.href = "/login";
    }
    throw new Error("Session expired");
  }

  if (!res.ok) {
    throw new ApiError(res.status, await res.text());
  }

  // 204 No Content
  if (res.status === 204) {
    return undefined as T;
  }

  return res.json();
}

export class ApiError extends Error {
  status: number;

  constructor(status: number, body: string) {
    let message: string;
    try {
      const parsed = JSON.parse(body);
      message = parsed.message || parsed.error || body;
    } catch {
      message = body;
    }
    super(message);
    this.status = status;
    this.name = "ApiError";
  }
}

/**
 * SSE event yielded by {@link apiStream}. `event` is the event type
 * from the server (e.g. "delta", "done", "error"); `data` is the parsed
 * JSON payload.
 */
export interface ApiStreamEvent<T = unknown> {
  event: string;
  data: T;
}

/**
 * Authenticated GET that streams a binary response to a file download
 * via an anchor click. Handles 401 refresh the same way `apiFetch`
 * does, so a stale access token does not break the download.
 */
export async function apiDownload(path: string, filename?: string): Promise<void> {
  const headers = new Headers();
  const token = getAccessToken();
  if (token) headers.set("Authorization", `Bearer ${token}`);

  let res = await fetch(`/api${path}`, { method: "GET", headers });
  if (res.status === 401 && token) {
    if (!isRefreshing) {
      isRefreshing = true;
      refreshPromise = attemptRefresh().finally(() => {
        isRefreshing = false;
        refreshPromise = null;
      });
    }
    const refreshed = await (refreshPromise ?? Promise.resolve(false));
    if (!refreshed) {
      clearTokens();
      if (typeof window !== "undefined") window.location.href = "/login";
      throw new Error("Session expired");
    }
    const retry = new Headers();
    const newToken = getAccessToken();
    if (newToken) retry.set("Authorization", `Bearer ${newToken}`);
    res = await fetch(`/api${path}`, { method: "GET", headers: retry });
  }

  if (!res.ok) {
    throw new ApiError(res.status, await res.text());
  }

  const blob = await res.blob();
  const chosen = filename ?? filenameFromResponse(res) ?? "download";
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = chosen;
  document.body.appendChild(link);
  link.click();
  link.remove();
  URL.revokeObjectURL(url);
}

function filenameFromResponse(res: Response): string | null {
  const disposition = res.headers.get("content-disposition");
  if (!disposition) return null;
  const match = /filename="?([^";]+)"?/i.exec(disposition);
  return match ? match[1] : null;
}

async function sseStartRequest(path: string, options: RequestInit): Promise<Response> {
  const headers = new Headers(options.headers);
  const token = getAccessToken();
  if (token) headers.set("Authorization", `Bearer ${token}`);
  if (!headers.has("Content-Type") && options.body) {
    headers.set("Content-Type", "application/json");
  }
  headers.set("Accept", "text/event-stream");
  return fetch(`/api${path}`, { ...options, headers });
}

/**
 * SSE-aware streaming request. Yields one {@link ApiStreamEvent} per
 * server-sent event. Terminates when the server closes the stream.
 *
 * Uses `fetch` + `ReadableStream` (not `EventSource`) so the
 * Authorization header travels with the request and the 401-refresh
 * path stays consistent with {@link apiFetch}.
 */
export async function* apiStream<T = unknown>(
  path: string,
  options: RequestInit = {},
): AsyncGenerator<ApiStreamEvent<T>, void, void> {
  let res = await sseStartRequest(path, options);

  if (res.status === 423) {
    if (typeof window !== "undefined") window.location.href = "/setup";
    throw new Error("Setup required");
  }

  if (res.status === 401 && getAccessToken()) {
    if (!isRefreshing) {
      isRefreshing = true;
      refreshPromise = attemptRefresh().finally(() => {
        isRefreshing = false;
        refreshPromise = null;
      });
    }
    const refreshed = await (refreshPromise ?? Promise.resolve(false));
    if (!refreshed) {
      clearTokens();
      if (typeof window !== "undefined") window.location.href = "/login";
      throw new Error("Session expired");
    }
    res = await sseStartRequest(path, options);
  }

  if (!res.ok) {
    throw new ApiError(res.status, await res.text());
  }
  if (!res.body) {
    throw new Error("Streaming response had no body");
  }

  const reader = res.body.getReader();
  const decoder = new TextDecoder();
  let buf = "";

  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    buf += decoder.decode(value, { stream: true });

    // Events are delimited by a blank line.
    let sep: number;
    while ((sep = buf.indexOf("\n\n")) !== -1) {
      const frame = buf.slice(0, sep);
      buf = buf.slice(sep + 2);
      const parsed = parseSseFrame<T>(frame);
      if (parsed) yield parsed;
    }
  }
  const tail = buf.trim();
  if (tail) {
    const parsed = parseSseFrame<T>(tail);
    if (parsed) yield parsed;
  }
}

function parseSseFrame<T>(frame: string): ApiStreamEvent<T> | null {
  let eventName = "message";
  const dataLines: string[] = [];
  for (const raw of frame.split("\n")) {
    if (!raw || raw.startsWith(":")) continue;
    const colon = raw.indexOf(":");
    const field = colon === -1 ? raw : raw.slice(0, colon);
    const value = colon === -1 ? "" : raw.slice(colon + 1).replace(/^ /, "");
    if (field === "event") eventName = value;
    else if (field === "data") dataLines.push(value);
  }
  if (dataLines.length === 0) return null;
  const raw = dataLines.join("\n");
  try {
    return { event: eventName, data: JSON.parse(raw) as T };
  } catch {
    return { event: eventName, data: raw as unknown as T };
  }
}
