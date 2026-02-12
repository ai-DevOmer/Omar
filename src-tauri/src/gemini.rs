use crate::agent::AgentMode;
use crate::api::{ApiError, ApiResult, ContentBlock, ImageSource, Message, StreamEvent, ToolResultContent};
use crate::storage::Usage;
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

const GEMINI_API_URL: &str = "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions";

pub struct GeminiClient {
    client: Client,
    api_key: String,
    model: String,
}

impl GeminiClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
        }
    }

    pub async fn send_message_streaming(
        &self,
        messages: Vec<Message>,
        event_tx: mpsc::UnboundedSender<StreamEvent>,
        mode: AgentMode,
        _voice_mode: bool,
    ) -> Result<ApiResult, ApiError> {
        // تحويل الرسائل إلى تنسيق OpenAI/Gemini
        let mut openai_messages = Vec::new();
        
        // إضافة System Prompt
        let system_prompt = match mode {
            AgentMode::Computer => crate::api::SYSTEM_PROMPT,
            AgentMode::Browser => crate::api::BROWSER_SYSTEM_PROMPT,
        };
        
        openai_messages.push(serde_json::json!({
            "role": "system",
            "content": system_prompt
        }));

        for msg in messages {
            let mut content_parts = Vec::new();
            for block in msg.content {
                match block {
                    ContentBlock::Text { text } => {
                        content_parts.push(serde_json::json!({
                            "type": "text",
                            "text": text
                        }));
                    }
                    ContentBlock::Image { source } => {
                        content_parts.push(serde_json::json!({
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:{};base64,{}", source.media_type, source.data)
                            }
                        }));
                    }
                    _ => {} // تخطي أنواع الكتل الأخرى للتبسيط في النسخة الأولية
                }
            }
            openai_messages.push(serde_json::json!({
                "role": msg.role,
                "content": content_parts
            }));
        }

        let request_body = serde_json::json!({
            "model": "gemini-2.0-flash", // استخدام الموديل المتاح حالياً كبديل لـ 2.5 إذا لم يتوفر
            "messages": openai_messages,
            "stream": true,
            "max_tokens": 4096
        });

        let response = self.client
            .post(GEMINI_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(ApiError::Api(format!("Gemini API error: {}", error_text)));
        }

        let mut stream = response.bytes_stream();
        let mut content_blocks = Vec::new();
        let mut full_text = String::new();

        while let Some(item) = stream.next().await {
            let chunk = item?;
            let text = String::from_utf8_lossy(&chunk);
            
            for line in text.lines() {
                if line.starts_with("data: ") {
                    let data = &line[6..];
                    if data == "[DONE]" { break; }
                    
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(content) = json["choices"][0]["delta"]["content"].as_str() {
                            full_text.push_str(content);
                            let _ = event_tx.send(StreamEvent::TextDelta {
                                text: content.to_string(),
                            });
                        }
                    }
                }
            }
        }

        content_blocks.push(ContentBlock::Text { text: full_text });

        Ok(ApiResult {
            content: content_blocks,
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            },
        })
    }
}
