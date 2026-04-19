//! Outline builder (Sprint 11, phase A9 / ADR-015).
//!
//! The outline lives as structured events on the
//! `editor_conversations` transcript — no separate table. Three
//! event types flow through the conversation log:
//!
//! - `outline_proposed` — AI offers a section list
//! - `outline_revised` — AI updates a previous proposal
//! - `outline_approved` — user accepts the latest proposal
//!
//! The "latest approved outline" for a page + language is a pure
//! read: scan the transcript and pick the most recent `outline_approved`.
//! If A15's materialised-view optimisation becomes necessary later
//! it can project from the same source without changing this API.

use serde::{Deserialize, Serialize};

use super::context::OutlineEntry;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutlineSection {
    pub heading: String,
    /// H-level for the section heading (1..=6). Defaults to 2 when
    /// the model omits it.
    #[serde(default = "default_level")]
    pub level: u8,
    /// Optional bullet summary the UI can show under the heading.
    #[serde(default)]
    pub bullets: Vec<String>,
}

fn default_level() -> u8 {
    2
}

/// Event type discriminator used in the transcript JSONB array.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OutlineEventType {
    OutlineProposed,
    OutlineRevised,
    OutlineApproved,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutlineEvent {
    pub event_type: OutlineEventType,
    pub sections: Vec<OutlineSection>,
}

/// Scan the transcript (raw JSONB array from editor_conversations.
/// messages) and return the most recent approved outline, if any.
pub fn latest_approved_outline(messages: &serde_json::Value) -> Option<Vec<OutlineSection>> {
    let array = messages.as_array()?;
    for entry in array.iter().rev() {
        let Ok(event) = serde_json::from_value::<OutlineEvent>(entry.clone()) else {
            continue;
        };
        if event.event_type == OutlineEventType::OutlineApproved {
            return Some(event.sections);
        }
    }
    None
}

/// Convert outline sections into the `OutlineEntry` shape the context
/// assembler consumes. Passing the approved outline through this
/// function keeps the prompt in sync with what the user approved.
pub fn to_context_entries(sections: &[OutlineSection]) -> Vec<OutlineEntry> {
    sections
        .iter()
        .map(|s| OutlineEntry {
            heading: s.heading.clone(),
            level: s.level,
            block_id: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(event: OutlineEventType, sections: Vec<OutlineSection>) -> serde_json::Value {
        serde_json::to_value(OutlineEvent {
            event_type: event,
            sections,
        })
        .unwrap()
    }

    #[test]
    fn empty_transcript_returns_none() {
        let msgs = json!([]);
        assert!(latest_approved_outline(&msgs).is_none());
    }

    #[test]
    fn transcript_with_only_proposals_returns_none() {
        let msgs = json!([entry(
            OutlineEventType::OutlineProposed,
            vec![OutlineSection {
                heading: "One".into(),
                level: 2,
                bullets: vec![],
            }],
        )]);
        assert!(latest_approved_outline(&msgs).is_none());
    }

    #[test]
    fn approved_outline_is_returned() {
        let msgs = json!([
            entry(
                OutlineEventType::OutlineProposed,
                vec![OutlineSection {
                    heading: "v1".into(),
                    level: 2,
                    bullets: vec![],
                }]
            ),
            entry(
                OutlineEventType::OutlineApproved,
                vec![OutlineSection {
                    heading: "final".into(),
                    level: 2,
                    bullets: vec!["intro".into()],
                }]
            ),
        ]);
        let outline = latest_approved_outline(&msgs).unwrap();
        assert_eq!(outline.len(), 1);
        assert_eq!(outline[0].heading, "final");
    }

    #[test]
    fn later_approval_supersedes_earlier_one() {
        let msgs = json!([
            entry(
                OutlineEventType::OutlineApproved,
                vec![OutlineSection {
                    heading: "first approved".into(),
                    level: 2,
                    bullets: vec![],
                }]
            ),
            entry(
                OutlineEventType::OutlineApproved,
                vec![OutlineSection {
                    heading: "second approved".into(),
                    level: 2,
                    bullets: vec![],
                }]
            ),
        ]);
        let outline = latest_approved_outline(&msgs).unwrap();
        assert_eq!(outline[0].heading, "second approved");
    }

    #[test]
    fn unrelated_entries_are_ignored() {
        let msgs = json!([
            { "role": "user", "content": "hello" },
            entry(
                OutlineEventType::OutlineApproved,
                vec![OutlineSection {
                    heading: "ok".into(),
                    level: 2,
                    bullets: vec![],
                }]
            ),
            { "role": "assistant", "content": "sure" },
        ]);
        let outline = latest_approved_outline(&msgs).unwrap();
        assert_eq!(outline[0].heading, "ok");
    }

    #[test]
    fn missing_level_defaults_to_two() {
        let raw = json!({
            "event_type": "outline_approved",
            "sections": [{ "heading": "no-level" }]
        });
        let msgs = json!([raw]);
        let outline = latest_approved_outline(&msgs).unwrap();
        assert_eq!(outline[0].level, 2);
    }

    #[test]
    fn to_context_entries_preserves_heading_and_level() {
        let sections = vec![
            OutlineSection {
                heading: "Intro".into(),
                level: 1,
                bullets: vec![],
            },
            OutlineSection {
                heading: "Details".into(),
                level: 2,
                bullets: vec![],
            },
        ];
        let entries = to_context_entries(&sections);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].heading, "Intro");
        assert_eq!(entries[0].level, 1);
        assert_eq!(entries[1].level, 2);
    }
}
