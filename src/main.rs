mod audio;
mod history;
mod keyboard;
mod settings;
mod stt;
mod text_inserter;
mod tray;
mod ui;

use anyhow::Result;
use crossbeam_channel::{unbounded, Receiver, Sender};
use log::{error, info, warn};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use audio::{AudioRecorder, RecordingSession};
use history::TranscriptionHistory;
use keyboard::{KeyboardEvent, KeyboardListener};
use settings::Settings;
use stt::SttClient;
use text_inserter::TextInserter;
use tray::{TrayEvent, TrayManager};
use ui::{AppStatus, UiCommand, UiFeedback};

const TEMP_AUDIO_FILE: &str = "temp_audio.wav";

struct VoiceTypingApp {
    settings: Arc<Mutex<Settings>>,
    audio_recorder: Arc<Mutex<AudioRecorder>>,
    stt_client: Arc<Mutex<SttClient>>,
    text_inserter: Arc<Mutex<TextInserter>>,
    history: Arc<TranscriptionHistory>,
    ui_feedback: Arc<UiFeedback>,
    tray_manager: Arc<Mutex<TrayManager>>,
    recording_session: Arc<Mutex<Option<RecordingSession>>>,
    last_recording_path: Arc<Mutex<Option<PathBuf>>>,
}

impl VoiceTypingApp {
    fn new() -> Result<Self> {
        env_logger::init();
        info!("Starting Voice Typing application");

        let settings = Settings::load()?;

        let mut audio_recorder = AudioRecorder::new(
            settings.silence_threshold,
            Some(settings.silent_start_timeout),
        )?;
        audio_recorder.set_default_device()?;

        let stt_client = SttClient::new(
            settings.stt_base_url.clone(),
            settings.stt_model.clone(),
        )?;

        let text_inserter = TextInserter::new()?;
        let history = TranscriptionHistory::new();

        let ui_feedback = UiFeedback::new(
            &settings.ui_indicator_position,
            &settings.ui_indicator_size,
        )?;

        let (tray_event_sender, tray_event_receiver) = unbounded();
        let tray_manager = TrayManager::new(tray_event_sender)?;

        Ok(Self {
            settings: Arc::new(Mutex::new(settings)),
            audio_recorder: Arc::new(Mutex::new(audio_recorder)),
            stt_client: Arc::new(Mutex::new(stt_client)),
            text_inserter: Arc::new(Mutex::new(text_inserter)),
            history: Arc::new(history),
            ui_feedback: Arc::new(ui_feedback),
            tray_manager: Arc::new(Mutex::new(tray_manager)),
            recording_session: Arc::new(Mutex::new(None)),
            last_recording_path: Arc::new(Mutex::new(None)),
        })
    }

    fn run(self) -> Result<()> {
        let (keyboard_event_sender, keyboard_event_receiver) = unbounded();
        let _keyboard_listener = KeyboardListener::new(keyboard_event_sender)?;

        let app = Arc::new(self);

        let app_clone = app.clone();
        thread::spawn(move || {
            app_clone.event_loop(keyboard_event_receiver);
        });

        UiFeedback::run_message_loop();

        Ok(())
    }

    fn event_loop(&self, keyboard_receiver: Receiver<KeyboardEvent>) {
        loop {
            if let Ok(event) = keyboard_receiver.recv_timeout(Duration::from_millis(100)) {
                match event {
                    KeyboardEvent::CapsLockPressed => {
                        self.toggle_recording();
                    }
                }
            }

            self.ui_feedback.process_commands();
            self.tray_manager.lock().process_menu_events();

            thread::sleep(Duration::from_millis(10));
        }
    }

    fn toggle_recording(&self) {
        let mut session = self.recording_session.lock();

        if session.is_none() {
            info!("Starting recording");
            *self.last_recording_path.lock() = None;

            let recorder = self.audio_recorder.lock();
            match recorder.record(TEMP_AUDIO_FILE) {
                Ok(new_session) => {
                    *session = Some(new_session);
                    self.ui_feedback.update_status(
                        AppStatus::Recording,
                        "Recording...".to_string(),
                    );
                }
                Err(e) => {
                    error!("Failed to start recording: {}", e);
                    self.ui_feedback.show_error("Failed to start recording".to_string());
                }
            }
        } else {
            info!("Stopping recording");
            if let Some(recording_session) = session.take() {
                drop(session);

                let app = self.clone_arc();
                thread::spawn(move || {
                    app.process_recording(recording_session);
                });
            }
        }
    }

    fn process_recording(&self, session: RecordingSession) {
        self.ui_feedback.update_status(
            AppStatus::Processing,
            "Processing...".to_string(),
        );

        match session.stop() {
            Ok(result) => {
                if result.was_auto_stopped {
                    warn!("Recording auto-stopped due to silence");
                    self.ui_feedback.update_status(
                        AppStatus::Error,
                        "No audio detected".to_string(),
                    );
                    return;
                }

                if !result.is_valid {
                    warn!("Recording invalid: {:?}", result.invalid_reason);
                    self.ui_feedback.update_status(
                        AppStatus::Error,
                        result.invalid_reason.unwrap_or_else(|| "Invalid recording".to_string()),
                    );
                    return;
                }

                *self.last_recording_path.lock() = Some(result.path.clone());

                self.transcribe_audio(&result.path);
            }
            Err(e) => {
                error!("Error stopping recording: {}", e);
                self.ui_feedback.show_error("Recording failed".to_string());
            }
        }
    }

    fn transcribe_audio(&self, audio_path: &PathBuf) {
        self.ui_feedback.update_status(
            AppStatus::Transcribing,
            "Transcribing...".to_string(),
        );

        let stt_client = self.stt_client.lock();
        match stt_client.transcribe(audio_path) {
            Ok(text) => {
                info!("Transcription successful: {} chars", text.len());
                self.history.add(text.clone());

                let mut inserter = self.text_inserter.lock();
                if let Err(e) = inserter.insert_text(&text) {
                    error!("Failed to insert text: {}", e);
                    let _ = inserter.copy_to_clipboard(&text);
                    self.ui_feedback.show_warning("Copied to clipboard".to_string());
                }

                self.ui_feedback.update_status(
                    AppStatus::Idle,
                    "Ready".to_string(),
                );

                self.update_tray_menu();
            }
            Err(e) => {
                error!("Transcription failed: {}", e);
                self.ui_feedback.show_error("Transcription failed".to_string());
            }
        }
    }

    fn update_tray_menu(&self) {
        let devices = self.audio_recorder.lock().list_input_devices().unwrap_or_default();
        let history = self.history.get_recent(10);
        let settings = self.settings.lock();
        let silence_enabled = settings.silent_start_timeout.is_some();
        let favorites: Vec<String> = settings.favorite_microphones.iter()
            .map(|d| d.name.clone())
            .collect();

        self.tray_manager.lock().update_menu(
            devices,
            favorites,
            history,
            silence_enabled,
        );
    }

    fn clone_arc(&self) -> Arc<Self> {
        Arc::new(Self {
            settings: self.settings.clone(),
            audio_recorder: self.audio_recorder.clone(),
            stt_client: self.stt_client.clone(),
            text_inserter: self.text_inserter.clone(),
            history: self.history.clone(),
            ui_feedback: self.ui_feedback.clone(),
            tray_manager: self.tray_manager.clone(),
            recording_session: self.recording_session.clone(),
            last_recording_path: self.last_recording_path.clone(),
        })
    }
}

fn main() -> Result<()> {
    let app = VoiceTypingApp::new()?;
    app.run()
}
