//! OpenAI content parts at the wire boundary; text helpers stay text-only.

use super::{ChatMessage, ToolCall};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Parts(Vec<Part>),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Part {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Serialize, Deserialize)]
struct ImageUrl {
    url: String,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Message {
    role: String,
    content: Option<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

impl From<ChatMessage> for Message {
    fn from(message: ChatMessage) -> Self {
        let content = if message.images.is_empty() {
            message.content.map(Content::Text)
        } else {
            let mut parts = Vec::new();
            if let Some(text) = message.content.filter(|s| !s.is_empty()) {
                parts.push(Part::Text { text });
            }
            parts.extend(message.images.into_iter().map(|url| Part::ImageUrl {
                image_url: ImageUrl { url },
            }));
            Some(Content::Parts(parts))
        };
        Self {
            role: message.role,
            content,
            tool_calls: message.tool_calls,
            tool_call_id: message.tool_call_id,
            name: message.name,
        }
    }
}

impl From<Message> for ChatMessage {
    fn from(message: Message) -> Self {
        let mut images = Vec::new();
        let content = match message.content {
            None => None,
            Some(Content::Text(text)) => Some(text),
            Some(Content::Parts(parts)) => {
                let mut texts = Vec::new();
                for part in parts {
                    match part {
                        Part::Text { text } => texts.push(text),
                        Part::ImageUrl { image_url } => images.push(image_url.url),
                    }
                }
                Some(texts.join("\n\n"))
            }
        };
        Self {
            role: message.role,
            content,
            images,
            tool_calls: message.tool_calls,
            tool_call_id: message.tool_call_id,
            name: message.name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_parts_round_trip_without_changing_text_only_requests() {
        let mut message = ChatMessage::text("user", "Look here");
        assert_eq!(
            serde_json::to_value(&message).unwrap()["content"],
            "Look here"
        );
        message.images.push("data:image/png;base64,abc".into());
        let value = serde_json::to_value(&message).unwrap();
        assert_eq!(value["content"][1]["type"], "image_url");
        let restored: ChatMessage = serde_json::from_value(value).unwrap();
        assert_eq!(restored.content, message.content);
        assert_eq!(restored.images, message.images);
    }
}
