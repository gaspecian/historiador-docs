//! Inline comments on canvas blocks (Sprint 11, phase B1 / ADR-016).
//!
//! Comments are first-class conversation inputs: they live as events
//! on `editor_conversations` (`comment_posted`, `comment_resolved`),
//! not in a side table. The AI reads them as structured inputs on
//! its next turn — the text plus the rendered text of the anchored
//! blocks — and may respond in conversation mode or by emitting
//! block_op proposals tagged with the source comment_id.
//!
//! Orphan-anchor handling: if the anchored block is deleted the
//! comment is marked `orphaned`. The author re-anchors by posting a
//! new comment and resolving the original.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Comment {
    pub comment_id: Uuid,
    pub author_id: Uuid,
    /// Block IDs the comment anchors to. Multiple IDs = the author
    /// selected across blocks. Empty = the comment scopes to the
    /// whole page.
    pub block_ids: Vec<String>,
    pub text: String,
    pub resolved: bool,
}

/// The two event types this phase ships into the transcript.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommentEventType {
    CommentPosted,
    CommentResolved,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommentEvent {
    pub event_type: CommentEventType,
    pub comment_id: Uuid,
    /// Full payload for posts; only `comment_id` is authoritative
    /// for resolves. Optional fields let the resolve event be
    /// narrow without a separate struct.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub author_id: Option<Uuid>,
    #[serde(default)]
    pub block_ids: Vec<String>,
    #[serde(default)]
    pub text: String,
}

/// ADR-016 §226 caps open comments in the system prompt to avoid
/// runaway context on a very busy page. Consumers slice the list.
pub const MAX_OPEN_COMMENTS_PER_TURN: usize = 10;

/// Scan a transcript and return the currently-open comments. A
/// comment is open if its `comment_posted` event has no following
/// `comment_resolved` for the same `comment_id`.
pub fn open_comments(messages: &serde_json::Value) -> Vec<Comment> {
    let Some(array) = messages.as_array() else {
        return Vec::new();
    };

    let mut open: std::collections::BTreeMap<Uuid, Comment> = std::collections::BTreeMap::new();
    let mut order: Vec<Uuid> = Vec::new();

    for entry in array {
        let Ok(event) = serde_json::from_value::<CommentEvent>(entry.clone()) else {
            continue;
        };
        match event.event_type {
            CommentEventType::CommentPosted => {
                if open.contains_key(&event.comment_id) {
                    continue;
                }
                order.push(event.comment_id);
                open.insert(
                    event.comment_id,
                    Comment {
                        comment_id: event.comment_id,
                        author_id: event.author_id.unwrap_or_else(Uuid::nil),
                        block_ids: event.block_ids,
                        text: event.text,
                        resolved: false,
                    },
                );
            }
            CommentEventType::CommentResolved => {
                open.remove(&event.comment_id);
                order.retain(|id| *id != event.comment_id);
            }
        }
    }

    order
        .into_iter()
        .filter_map(|id| open.remove(&id))
        .collect()
}

/// Slice `open_comments` down to the per-turn cap for inclusion in
/// the agent's system prompt.
pub fn open_comments_for_prompt(messages: &serde_json::Value) -> Vec<Comment> {
    let mut list = open_comments(messages);
    if list.len() > MAX_OPEN_COMMENTS_PER_TURN {
        list.truncate(MAX_OPEN_COMMENTS_PER_TURN);
    }
    list
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn uid() -> Uuid {
        Uuid::now_v7()
    }

    fn posted(id: Uuid, block_ids: Vec<&str>, text: &str) -> serde_json::Value {
        serde_json::to_value(CommentEvent {
            event_type: CommentEventType::CommentPosted,
            comment_id: id,
            author_id: Some(uid()),
            block_ids: block_ids.into_iter().map(String::from).collect(),
            text: text.into(),
        })
        .unwrap()
    }

    fn resolved(id: Uuid) -> serde_json::Value {
        serde_json::to_value(CommentEvent {
            event_type: CommentEventType::CommentResolved,
            comment_id: id,
            author_id: None,
            block_ids: vec![],
            text: String::new(),
        })
        .unwrap()
    }

    #[test]
    fn empty_transcript_has_no_comments() {
        assert!(open_comments(&json!([])).is_empty());
    }

    #[test]
    fn posted_comment_shows_as_open() {
        let id = uid();
        let msgs = json!([posted(id, vec!["b1"], "needs a rewrite")]);
        let open = open_comments(&msgs);
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].text, "needs a rewrite");
    }

    #[test]
    fn resolved_comment_is_filtered_out() {
        let id = uid();
        let msgs = json!([posted(id, vec!["b1"], "fix"), resolved(id)]);
        assert!(open_comments(&msgs).is_empty());
    }

    #[test]
    fn per_turn_cap_truncates() {
        let mut msgs: Vec<serde_json::Value> = Vec::new();
        for i in 0..(MAX_OPEN_COMMENTS_PER_TURN + 3) {
            msgs.push(posted(uid(), vec!["b1"], &format!("c{i}")));
        }
        let slice = open_comments_for_prompt(&json!(msgs));
        assert_eq!(slice.len(), MAX_OPEN_COMMENTS_PER_TURN);
    }

    #[test]
    fn duplicate_posts_of_same_id_are_ignored() {
        let id = uid();
        let msgs = json!([
            posted(id, vec!["b1"], "first"),
            posted(id, vec!["b1"], "duplicate — ignored"),
        ]);
        let open = open_comments(&msgs);
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].text, "first");
    }

    #[test]
    fn empty_block_ids_scope_to_whole_page() {
        let id = uid();
        let msgs = json!([posted(id, vec![], "global note")]);
        let open = open_comments(&msgs);
        assert_eq!(open.len(), 1);
        assert!(open[0].block_ids.is_empty());
    }
}
