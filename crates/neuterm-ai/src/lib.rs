//! Ollama client, command help, and chat helpers.
//!
//! See `specs/ai-agents.md`.

use std::time::Duration;

use futures_util::StreamExt;
use neuterm_config::Config;
use serde::{Deserialize, Serialize};
use tracing::debug;

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    #[error("AI features are disabled in config (set ai.enabled: true)")]
    Disabled,
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Ollama error: {0}")]
    Ollama(String),
    #[error("{0}")]
    Other(String),
}

#[derive(Clone)]
pub struct OllamaClient {
    http: reqwest::Client,
    base_url: String,
    model: String,
    enabled: bool,
}

impl OllamaClient {
    pub fn from_config(config: &Config) -> Result<Self, AiError> {
        let timeout = Duration::from_millis(config.ai.ollama.timeout_ms.max(1));
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()?;
        Ok(Self {
            http,
            base_url: config.ollama_base_url(),
            model: config.ai.ollama.model.clone(),
            enabled: config.ai.enabled,
        })
    }

    pub fn reload(&mut self, config: &Config) -> Result<(), AiError> {
        *self = Self::from_config(config)?;
        Ok(())
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn ping(&self) -> Result<Vec<String>, AiError> {
        if !self.enabled {
            return Err(AiError::Disabled);
        }
        let url = format!("{}/api/tags", self.base_url);
        let resp = self.http.get(url).send().await?;
        if !resp.status().is_success() {
            return Err(AiError::Ollama(format!("status {}", resp.status())));
        }
        let tags: TagsResponse = resp.json().await?;
        Ok(tags.models.into_iter().map(|m| m.name).collect())
    }

    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String, AiError> {
        if !self.enabled {
            return Err(AiError::Disabled);
        }
        let url = format!("{}/api/chat", self.base_url);
        let body = ChatRequest {
            model: self.model.clone(),
            messages,
            stream: false,
        };
        let resp = self.http.post(url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AiError::Ollama(format!("{status}: {text}")));
        }
        let parsed: ChatResponse = resp.json().await?;
        Ok(parsed.message.content)
    }

    /// Stream chat tokens; callback receives each content chunk.
    pub async fn chat_stream<F>(&self, messages: Vec<ChatMessage>, mut on_token: F) -> Result<String, AiError>
    where
        F: FnMut(&str),
    {
        if !self.enabled {
            return Err(AiError::Disabled);
        }
        let url = format!("{}/api/chat", self.base_url);
        let body = ChatRequest {
            model: self.model.clone(),
            messages,
            stream: true,
        };
        let resp = self.http.post(url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AiError::Ollama(format!("{status}: {text}")));
        }
        let mut stream = resp.bytes_stream();
        let mut full = String::new();
        let mut buffer = String::new();
        while let Some(item) = stream.next().await {
            let chunk = item?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer = buffer[pos + 1..].to_string();
                if line.is_empty() {
                    continue;
                }
                match serde_json::from_str::<ChatResponse>(&line) {
                    Ok(msg) => {
                        if !msg.message.content.is_empty() {
                            on_token(&msg.message.content);
                            full.push_str(&msg.message.content);
                        }
                    }
                    Err(err) => debug!("skip stream line: {err} ({line})"),
                }
            }
        }
        Ok(full)
    }

    pub async fn suggest_command(
        &self,
        question: &str,
        os_hint: &str,
        shell_hint: &str,
        system_prompt: Option<&str>,
    ) -> Result<CommandSuggestion, AiError> {
        let system = system_prompt.unwrap_or(
            "You are a command-line assistant. Reply with exactly two lines:\n\
             LINE1: a single shell command for the user's OS (no markdown fences)\n\
             LINE2: one short sentence of explanation\n\
             Never execute anything; only suggest.",
        );
        let user = format!(
            "OS: {os_hint}\nShell: {shell_hint}\nRequest: {question}"
        );
        let content = self
            .chat(vec![
                ChatMessage {
                    role: "system".into(),
                    content: system.into(),
                },
                ChatMessage {
                    role: "user".into(),
                    content: user,
                },
            ])
            .await?;
        Ok(CommandSuggestion::parse(&content))
    }
}

#[derive(Debug, Clone)]
pub struct CommandSuggestion {
    pub command: String,
    pub explanation: String,
    pub raw: String,
}

impl CommandSuggestion {
    pub fn parse(raw: &str) -> Self {
        let cleaned = raw
            .trim()
            .trim_start_matches("```bash")
            .trim_start_matches("```sh")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        let mut lines = cleaned.lines().filter(|l| !l.trim().is_empty());
        let command = lines
            .next()
            .unwrap_or("")
            .trim()
            .trim_start_matches('$')
            .trim()
            .to_string();
        let explanation = lines.collect::<Vec<_>>().join(" ").trim().to_string();
        Self {
            command,
            explanation,
            raw: raw.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    message: ChatMessageWire,
}

#[derive(Deserialize, Default)]
struct ChatMessageWire {
    #[serde(default)]
    content: String,
}

#[derive(Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<ModelTag>,
}

#[derive(Deserialize)]
struct ModelTag {
    name: String,
}

/// In-memory chat transcript for the AI chat pane.
#[derive(Debug, Default, Clone)]
pub struct ChatSession {
    pub messages: Vec<ChatMessage>,
}

impl ChatSession {
    pub fn push_user(&mut self, content: impl Into<String>) {
        self.messages.push(ChatMessage {
            role: "user".into(),
            content: content.into(),
        });
    }

    pub fn push_assistant(&mut self, content: impl Into<String>) {
        self.messages.push(ChatMessage {
            role: "assistant".into(),
            content: content.into(),
        });
    }
}
