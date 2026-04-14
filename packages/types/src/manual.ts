// Client-only types that have no Rust DTO equivalent.
// These are not auto-generated from the OpenAPI spec.

import type { components } from "../generated/index.js";

/** Decoded JWT payload (client-side only — never sent to the API). */
export interface JwtPayload {
  sub: string; // user_id
  wsid: string; // workspace_id
  role: components["schemas"]["Role"];
  exp: number;
  iat: number;
}

/** Recursive tree node built client-side from flat Collection records. */
export type TreeNode = components["schemas"]["Collection"] & {
  children: TreeNode[];
};
