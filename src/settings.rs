use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceIdentifier {
    pub name: String,
    pub channels: u32,
    pub default_samplerate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_silent_start_timeout")]
    pub silent_start_timeout: Option<f64>,

    #[serde(default = "default_silence_threshold")]
    pub silence_threshold: f64,

    #[serde(default = "default_stt_base_url")]
    pub stt_base_url: String,

    #[serde(default = "default_stt_model")]
    pub stt_model: String,

    #[serde(default)]
    pub selected_microphone: Option<DeviceIdentifier>,

    #[serde(default)]
    pub favorite_microphones: Vec<DeviceIdentifier>,

    #[serde(default = "default_ui_position")]
    pub ui_indicator_position: String,

    #[serde(default = "default_ui_size")]
    pub ui_indicator_size: String,

    #[serde(default = "default_log_retention")]
    pub log_retention_days: u32,
}

fn default_silent_start_timeout() -> Option<f64> { Some(4.0) }
fn default_silence_threshold() -> f64 { 0.01 }
fn default_stt_base_url() -> String { "https://parakeet.kaki.dev".to_string() }
fn default_stt_model() -> String { "parakeet-tdt-0.6b-v3".to_string() }
fn default_ui_position() -> String { "top-right".to_string() }
fn default_ui_size() -> String { "normal".to_string() }
fn default_log_retention() -> u32 { 60 }

impl Default for Settings {
    fn default() -> Self {
        Self {
            silent_start_timeout: Some(4.0),
            silence_threshold: default_silence_threshold(),
            stt_base_url: default_stt_base_url(),
            stt_model: default_stt_model(),
            selected_microphone: None,
            favorite_microphones: Vec::new(),
            ui_indicator_position: default_ui_position(),
            ui_indicator_size: default_ui_size(),
            log_retention_days: default_log_retention(),
        }
    }
}

impl Settings {
    pub fn load() -> Result<Self> {
        let settings_path = Self::get_settings_path();

        if settings_path.exists() {
            let contents = fs::read_to_string(&settings_path)?;
            let settings: Settings = serde_json::from_str(&contents)?;
            Ok(settings)
        } else {
            let settings = Settings::default();
            settings.save()?;
            Ok(settings)
        }
    }

    pub fn save(&self) -> Result<()> {
        let settings_path = Self::get_settings_path();
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&settings_path, json)?;
        Ok(())
    }

    fn get_settings_path() -> PathBuf {
        let exe_path = std::env::current_exe()
            .unwrap_or_else(|_| PathBuf::from("."));
        let exe_dir = exe_path.parent().unwrap_or(std::path::Path::new("."));
        exe_dir.join("settings.json")
    }
}
