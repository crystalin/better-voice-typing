use anyhow::{Context, Result};
use log::{debug, error, info};
use reqwest::blocking::multipart;
use serde_json::Value;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

pub struct SttClient {
    base_url: String,
    model: String,
    client: reqwest::blocking::Client,
}

impl SttClient {
    pub fn new(base_url: String, model: String) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()?;

        info!("Initialized STT client with URL: {}, model: {}", base_url, model);

        Ok(Self {
            base_url,
            model,
            client,
        })
    }

    pub fn transcribe<P: AsRef<Path>>(&self, audio_path: P) -> Result<String> {
        let path = audio_path.as_ref();
        info!("Transcribing audio file: {:?}", path);

        let file = File::open(path)
            .context("Failed to open audio file")?;

        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.wav");

        let form = multipart::Form::new()
            .file("file", path)?;

        let endpoint = format!("{}/transcribe", self.base_url);
        debug!("Sending request to: {}", endpoint);

        let response = self.client
            .post(&endpoint)
            .multipart(form)
            .send()
            .context("Failed to send transcription request")?;

        if response.status().is_success() {
            let json: Value = response.json()
                .context("Failed to parse JSON response")?;

            if let Some(success) = json.get("success") {
                if success == &Value::Bool(false) {
                    let message = json.get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Transcription failed");
                    anyhow::bail!("Server error: {}", message);
                }
            }

            self.parse_response(&json)
        } else {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            anyhow::bail!("HTTP {}: {}", status, text)
        }
    }

    fn parse_response(&self, result: &Value) -> Result<String> {
        if let Value::Object(obj) = result {
            if let Some(Value::Bool(false)) = obj.get("success") {
                let error_msg = obj.get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Transcription failed");
                anyhow::bail!("Server error: {}", error_msg);
            }

            if let Some(Value::Array(segments)) = obj.get("segments") {
                let texts: Vec<String> = segments
                    .iter()
                    .filter_map(|seg| {
                        seg.as_object()
                            .and_then(|s| s.get("text"))
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect();

                if !texts.is_empty() {
                    return Ok(texts.join(" "));
                }
            }

            for field in &["text", "transcription", "result", "transcript", "output", "response", "data"] {
                if let Some(value) = obj.get(field) {
                    if let Some(text) = value.as_str() {
                        return Ok(text.to_string());
                    }
                }
            }

            error!("Unknown response format: {:?}", result);
            Ok(format!("{:?}", result))
        } else if let Value::String(s) = result {
            Ok(s.clone())
        } else {
            Ok(format!("{:?}", result))
        }
    }

    pub fn update_model(&mut self, model: String) {
        info!("Updated STT model to: {}", model);
        self.model = model;
    }

    pub fn update_base_url(&mut self, base_url: String) {
        info!("Updated STT base URL to: {}", base_url);
        self.base_url = base_url;
    }
}
