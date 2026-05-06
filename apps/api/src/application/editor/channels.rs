//! Parse `<chat>…</chat>` / `<canvas>…</canvas>` channel tags from the
//! LLM's response (Sprint 11, agent v1.md §Output channels).
//!
//! The agent's prompt mandates that every reply separates content
//! destined for the conversation pane from content destined for the
//! canvas. This module is the tiny, forgiving parser that splits
//! the raw response into those two buckets so the WS handler can
//! route each to its surface.
//!
//! Fallback policy: if the model forgets the tags entirely we
//! treat the whole response as `chat` content. That keeps a
//! poorly-behaving turn visible to the user instead of silently
//! dropping it. If only one tag is present, the other channel is
//! empty — callers should treat empty as "no change".

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChannelOutput {
    /// Text for the conversation pane. Trimmed.
    pub chat: String,
    /// Markdown for the canvas. Trimmed. Empty when the turn is
    /// pure conversation.
    pub canvas: String,
}

pub fn parse(raw: &str) -> ChannelOutput {
    let chat = extract_tag(raw, "chat");
    let canvas = extract_tag(raw, "canvas");

    match (chat, canvas) {
        (Some(c), Some(k)) => ChannelOutput {
            chat: c.trim().to_string(),
            canvas: k.trim().to_string(),
        },
        (Some(c), None) => ChannelOutput {
            chat: c.trim().to_string(),
            canvas: String::new(),
        },
        (None, Some(k)) => ChannelOutput {
            chat: String::new(),
            canvas: k.trim().to_string(),
        },
        // Fallback: untagged reply falls entirely into chat. Strip
        // orphan close tags (`</chat>`, `</canvas>`) before emitting
        // so a partial contract violation does not leak protocol
        // markers into the user-visible pane.
        (None, None) => ChannelOutput {
            chat: strip_orphan_tags(raw).trim().to_string(),
            canvas: String::new(),
        },
    }
}

fn strip_orphan_tags(raw: &str) -> String {
    // Whitespace-tolerant, case-insensitive removal of any standalone
    // opening or closing channel tag. `raw` tends to be short, so the
    // repeated replace is fine — and this only runs on the fallback
    // path (malformed reply) anyway.
    let mut out = raw.to_string();
    for pattern in [
        "</chat>",
        "<chat>",
        "</canvas>",
        "<canvas>",
        "</Chat>",
        "<Chat>",
        "</Canvas>",
        "<Canvas>",
        "</CHAT>",
        "<CHAT>",
        "</CANVAS>",
        "<CANVAS>",
    ] {
        out = out.replace(pattern, "");
    }
    out
}

fn extract_tag(raw: &str, tag: &str) -> Option<String> {
    // Case-insensitive, whitespace-tolerant opening tag match.
    let open = format!("<{tag}");
    let close = format!("</{tag}>");

    let lower = raw.to_ascii_lowercase();
    let open_pos = lower.find(&open)?;
    // Skip forward past the `>` that closes the opening tag — lets
    // the tag carry attributes without breaking the match.
    let rest_start = raw[open_pos..].find('>').map(|i| open_pos + i + 1)?;
    let close_pos = lower[rest_start..].find(&close)?;
    Some(raw[rest_start..rest_start + close_pos].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_tags_present() {
        let out = parse("<chat>Hi</chat>\n<canvas># Title\n\nbody</canvas>");
        assert_eq!(out.chat, "Hi");
        assert_eq!(out.canvas, "# Title\n\nbody");
    }

    #[test]
    fn only_chat_tag() {
        let out = parse("<chat>apenas perguntas</chat>");
        assert_eq!(out.chat, "apenas perguntas");
        assert!(out.canvas.is_empty());
    }

    #[test]
    fn only_canvas_tag() {
        let out = parse("<canvas># doc\n\nbody</canvas>");
        assert!(out.chat.is_empty());
        assert_eq!(out.canvas, "# doc\n\nbody");
    }

    #[test]
    fn untagged_response_falls_into_chat() {
        let out = parse("desculpa, respondendo direto");
        assert_eq!(out.chat, "desculpa, respondendo direto");
        assert!(out.canvas.is_empty());
    }

    #[test]
    fn trims_inner_whitespace() {
        let out = parse("<chat>\n   tight   \n</chat>");
        assert_eq!(out.chat, "tight");
    }

    #[test]
    fn case_insensitive_tag_matching() {
        let out = parse("<Chat>foo</Chat><CANVAS>bar</CANVAS>");
        assert_eq!(out.chat, "foo");
        assert_eq!(out.canvas, "bar");
    }

    #[test]
    fn tag_with_attributes_is_tolerated() {
        let out = parse(r#"<chat lang="pt-BR">oi</chat>"#);
        assert_eq!(out.chat, "oi");
    }

    #[test]
    fn canvas_can_contain_backticks_and_angle_brackets() {
        let md = "# heading\n\n```html\n<div>ok</div>\n```\n";
        let raw = format!("<chat>done</chat>\n<canvas>{md}</canvas>");
        let out = parse(&raw);
        assert_eq!(out.chat, "done");
        assert_eq!(out.canvas, md.trim());
    }

    #[test]
    fn unterminated_tag_is_treated_as_missing() {
        // No `</chat>` close — should fall back to chat-only with
        // orphan markers stripped.
        let out = parse("<chat>never closes");
        assert_eq!(out.chat, "never closes");
        assert!(out.canvas.is_empty());
    }

    #[test]
    fn orphan_close_tag_is_stripped_on_fallback() {
        let out = parse("</chat> Claro, vou ajudar.");
        assert_eq!(out.chat, "Claro, vou ajudar.");
        assert!(out.canvas.is_empty());
    }

    #[test]
    fn orphan_close_canvas_is_stripped_on_fallback() {
        let out = parse("</canvas>texto solto");
        assert_eq!(out.chat, "texto solto");
        assert!(out.canvas.is_empty());
    }
}
