"use client";

/**
 * Open-comment list for the editor-v2 canvas (Sprint 11, phase B1 /
 * ADR-016).
 *
 * `comment_posted` events add a comment; `comment_resolved` events
 * remove it. Consumers (the WS hook + the CommentPanel) share this
 * hook so every surface sees the same state.
 */

import { useCallback, useState } from "react";

export interface Comment {
  commentId: string;
  authorId: string;
  blockIds: string[];
  text: string;
}

export interface CommentStore {
  comments: Comment[];
  add: (comment: Comment) => void;
  resolve: (commentId: string) => void;
  clear: () => void;
}

export function useCommentStore(): CommentStore {
  const [comments, setComments] = useState<Comment[]>([]);

  const add = useCallback((c: Comment) => {
    setComments((prev) => {
      if (prev.some((x) => x.commentId === c.commentId)) return prev;
      return [...prev, c];
    });
  }, []);

  const resolve = useCallback((commentId: string) => {
    setComments((prev) => prev.filter((c) => c.commentId !== commentId));
  }, []);

  const clear = useCallback(() => setComments([]), []);

  return { comments, add, resolve, clear };
}
