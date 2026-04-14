// Re-export the openapi-typescript output so consumers can
// `import { paths, components } from '@historiador/types'`.
export * from "../generated/index.js";

// Client-only types (no Rust DTO).
export { type JwtPayload, type TreeNode } from "./manual.js";

// Ergonomic aliases so consumers can write
//   import type { PageResponse } from '@historiador/types'
// instead of  components["schemas"]["PageResponse"].
import type { components } from "../generated/index.js";

export type Collection = components["schemas"]["Collection"];
export type CollectionResponse = components["schemas"]["CollectionResponse"];
export type CreatePageRequest = components["schemas"]["CreatePageRequest"];
export type DraftRequest = components["schemas"]["DraftRequest"];
export type HealthResponse = components["schemas"]["HealthResponse"];
export type InviteRequest = components["schemas"]["InviteRequest"];
export type InviteResponse = components["schemas"]["InviteResponse"];
export type IterateRequest = components["schemas"]["IterateRequest"];
export type LlmProvider = components["schemas"]["LlmProvider"];
export type LoginRequest = components["schemas"]["LoginRequest"];
export type PageResponse = components["schemas"]["PageResponse"];
export type PageStatus = components["schemas"]["PageStatus"];
export type PageVersionResponse = components["schemas"]["PageVersionResponse"];
export type PageVersionsResponse = components["schemas"]["PageVersionsResponse"];
export type ProbeRequest = components["schemas"]["ProbeRequest"];
export type ProbeResponse = components["schemas"]["ProbeResponse"];
export type PublishResponse = components["schemas"]["PublishResponse"];
export type RegenerateTokenResponse = components["schemas"]["RegenerateTokenResponse"];
export type Role = components["schemas"]["Role"];
export type SetupRequest = components["schemas"]["SetupRequest"];
export type SetupResponse = components["schemas"]["SetupResponse"];
export type TokenResponse = components["schemas"]["TokenResponse"];
export type UpdatePageRequest = components["schemas"]["UpdatePageRequest"];
export type UserResponse = components["schemas"]["UserResponse"];
export type WorkspaceResponse = components["schemas"]["WorkspaceResponse"];

// Sprint 7: version history types
export type VersionHistoryListResponse = components["schemas"]["VersionHistoryListResponse"];
export type VersionHistorySummary = components["schemas"]["VersionHistorySummary"];
export type VersionHistoryDetailResponse = components["schemas"]["VersionHistoryDetailResponse"];

// Sprint 7: MCP analytics types
export type McpAnalyticsResponse = components["schemas"]["McpAnalyticsResponse"];
export type DayCountDto = components["schemas"]["DayCountDto"];
export type QueryFrequencyDto = components["schemas"]["QueryFrequencyDto"];
export type ZeroResultSummaryDto = components["schemas"]["ZeroResultSummaryDto"];
export type ZeroResultQueryDto = components["schemas"]["ZeroResultQueryDto"];

// Backwards-compatible alias: frontend uses "PageVersion" but the
// Rust DTO is named PageVersionResponse.
export type PageVersion = components["schemas"]["PageVersionResponse"];
