use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, SampleFormat, StreamConfig};
use crossbeam_channel::{Receiver, Sender};
use hound::{WavSpec, WavWriter};
use log::{debug, error, info, warn};
use parking_lot::Mutex;
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

const MIN_DURATION: f64 = 1.0;
const SMOOTHING_FACTOR: f32 = 0.2;

#[derive(Clone)]
pub struct AudioDevice {
    pub name: String,
    pub channels: u32,
    pub sample_rate: u32,
    pub device: Device,
}

pub struct AudioRecorder {
    host: Host,
    device: Option<Device>,
    config: Option<StreamConfig>,
    silence_threshold: f64,
    silent_start_timeout: Option<f64>,
    level_sender: Option<Sender<f32>>,
}

impl AudioRecorder {
    pub fn new(
        silence_threshold: f64,
        silent_start_timeout: Option<f64>,
    ) -> Result<Self> {
        let host = cpal::default_host();

        Ok(Self {
            host,
            device: None,
            config: None,
            silence_threshold,
            silent_start_timeout,
            level_sender: None,
        })
    }

    pub fn set_level_callback(&mut self, sender: Sender<f32>) {
        self.level_sender = Some(sender);
    }

    pub fn list_input_devices(&self) -> Result<Vec<AudioDevice>> {
        let mut devices = Vec::new();

        for device in self.host.input_devices()? {
            if let Ok(name) = device.name() {
                if let Ok(config) = device.default_input_config() {
                    devices.push(AudioDevice {
                        name,
                        channels: config.channels() as u32,
                        sample_rate: config.sample_rate().0,
                        device: device.clone(),
                    });
                }
            }
        }

        Ok(devices)
    }

    pub fn set_device(&mut self, device: Device) -> Result<()> {
        let config = device.default_input_config()?;
        info!(
            "Selected device: {} ({}Hz, {} channels)",
            device.name()?,
            config.sample_rate().0,
            config.channels()
        );

        self.device = Some(device);
        self.config = Some(config.into());
        Ok(())
    }

    pub fn set_default_device(&mut self) -> Result<()> {
        let device = self.host.default_input_device()
            .context("No default input device found")?;
        self.set_device(device)
    }

    pub fn record(&self, filename: &str) -> Result<RecordingSession> {
        let device = self.device.as_ref()
            .context("No device selected")?;
        let config = self.config.as_ref()
            .context("No config available")?;

        let sample_rate = config.sample_rate.0;
        let channels = config.channels as u16;

        info!("Starting recording: {} ({}Hz, {} channels)", filename, sample_rate, channels);

        let spec = WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let path = PathBuf::from(filename);
        let writer = WavWriter::create(&path, spec)?;
        let writer = Arc::new(Mutex::new(Some(writer)));

        let recording_state = Arc::new(Mutex::new(RecordingState {
            is_recording: true,
            auto_stopped: false,
            smoothed_level: 0.0,
            silence_start: None,
            initial_sound_detected: false,
            recording_start: Instant::now(),
        }));

        let state_clone = recording_state.clone();
        let writer_clone = writer.clone();
        let level_sender = self.level_sender.clone();
        let silence_threshold = self.silence_threshold;
        let silent_start_timeout = self.silent_start_timeout;

        let sample_format = device.default_input_config()?.sample_format();
        let stream = match sample_format {
            SampleFormat::F32 => {
                device.build_input_stream(
                    config,
                    move |data: &[f32], _: &_| {
                        process_audio_f32(
                            data,
                            &writer_clone,
                            &state_clone,
                            &level_sender,
                            silence_threshold,
                            silent_start_timeout,
                        );
                    },
                    |err| error!("Audio stream error: {}", err),
                    None,
                )?
            }
            SampleFormat::I16 => {
                device.build_input_stream(
                    config,
                    move |data: &[i16], _: &_| {
                        process_audio_i16(
                            data,
                            &writer_clone,
                            &state_clone,
                            &level_sender,
                            silence_threshold,
                            silent_start_timeout,
                        );
                    },
                    |err| error!("Audio stream error: {}", err),
                    None,
                )?
            }
            _ => anyhow::bail!("Unsupported sample format"),
        };

        stream.play()?;

        Ok(RecordingSession {
            stream,
            writer,
            recording_state,
            path,
            sample_rate,
        })
    }

    pub fn get_default_sample_rate(&self) -> Result<u32> {
        let device = self.device.as_ref()
            .context("No device selected")?;
        let config = device.default_input_config()?;
        Ok(config.sample_rate().0)
    }
}

struct RecordingState {
    is_recording: bool,
    auto_stopped: bool,
    smoothed_level: f32,
    silence_start: Option<Instant>,
    initial_sound_detected: bool,
    recording_start: Instant,
}

pub struct RecordingSession {
    stream: cpal::Stream,
    writer: Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>,
    recording_state: Arc<Mutex<RecordingState>>,
    path: PathBuf,
    sample_rate: u32,
}

impl RecordingSession {
    pub fn stop(self) -> Result<RecordingResult> {
        {
            let mut state = self.recording_state.lock();
            state.is_recording = false;
        }

        let path = self.path.clone();

        drop(self.stream);

        let writer = self.writer.lock().take();
        if let Some(writer) = writer {
            writer.finalize()?;
        }

        let state = self.recording_state.lock();
        let was_auto_stopped = state.auto_stopped;
        drop(state);

        let (is_valid, reason) = analyze_recording(&path, self.sample_rate)?;

        Ok(RecordingResult {
            path: self.path,
            was_auto_stopped,
            is_valid,
            invalid_reason: reason,
        })
    }

    pub fn was_auto_stopped(&self) -> bool {
        self.recording_state.lock().auto_stopped
    }
}

fn analyze_recording(path: &PathBuf, sample_rate: u32) -> Result<(bool, Option<String>)> {
        let mut reader = hound::WavReader::open(path)?;

        let duration = reader.duration() as f64 / sample_rate as f64;
        if duration < MIN_DURATION {
            return Ok((false, Some(format!("Recording too short ({:.1}s < {:.1}s)", duration, MIN_DURATION))));
        }

        let samples: Vec<i16> = reader.samples::<i16>().filter_map(Result::ok).collect();
        if samples.is_empty() {
            return Ok((false, Some("No audio data recorded".to_string())));
        }

        let sum_squares: f64 = samples.iter().map(|&s| {
            let normalized = s as f64 / i16::MAX as f64;
            normalized * normalized
        }).sum();

        let rms = (sum_squares / samples.len() as f64).sqrt();

        if rms < 0.01 {
            let db_value = 20.0 * rms.max(1e-10).log10();
            return Ok((false, Some(format!("Recording contains mostly silence (RMS: {:.4} / {:.1}dB)", rms, db_value))));
        }

        Ok((true, None))
    }

pub struct RecordingResult {
    pub path: PathBuf,
    pub was_auto_stopped: bool,
    pub is_valid: bool,
    pub invalid_reason: Option<String>,
}

fn process_audio_f32(
    data: &[f32],
    writer: &Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>,
    state: &Arc<Mutex<RecordingState>>,
    level_sender: &Option<Sender<f32>>,
    silence_threshold: f64,
    silent_start_timeout: Option<f64>,
) {
    let mut state_lock = state.lock();

    if !state_lock.is_recording {
        return;
    }

    let sum_squares: f32 = data.iter().map(|&s| s * s).sum();
    let rms = (sum_squares / data.len() as f32).sqrt();

    let db = 20.0 * rms.max(1e-10).log10();
    let normalized = ((db + 60.0) / 60.0).max(0.0).min(1.0);

    if let Some(timeout) = silent_start_timeout {
        if !state_lock.initial_sound_detected {
            if rms < silence_threshold as f32 {
                if state_lock.silence_start.is_none() {
                    state_lock.silence_start = Some(Instant::now());
                } else if state_lock.silence_start.unwrap().elapsed().as_secs_f64() >= timeout {
                    warn!("Auto-stopping due to {}s of initial silence", timeout);
                    state_lock.auto_stopped = true;
                    state_lock.is_recording = false;
                    return;
                }
            } else {
                state_lock.initial_sound_detected = true;
                state_lock.silence_start = None;
            }
        }
    }

    state_lock.smoothed_level = SMOOTHING_FACTOR * normalized + (1.0 - SMOOTHING_FACTOR) * state_lock.smoothed_level;

    if let Some(sender) = level_sender {
        let _ = sender.try_send(state_lock.smoothed_level);
    }

    drop(state_lock);

    if let Some(writer) = writer.lock().as_mut() {
        for &sample in data {
            let amplitude = (sample * i16::MAX as f32) as i16;
            let _ = writer.write_sample(amplitude);
        }
    }
}

fn process_audio_i16(
    data: &[i16],
    writer: &Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>,
    state: &Arc<Mutex<RecordingState>>,
    level_sender: &Option<Sender<f32>>,
    silence_threshold: f64,
    silent_start_timeout: Option<f64>,
) {
    let mut state_lock = state.lock();

    if !state_lock.is_recording {
        return;
    }

    let sum_squares: f64 = data.iter().map(|&s| {
        let normalized = s as f64 / i16::MAX as f64;
        normalized * normalized
    }).sum();

    let rms = (sum_squares / data.len() as f64).sqrt();

    let db = 20.0 * rms.max(1e-10).log10();
    let normalized = ((db + 60.0) / 60.0).max(0.0).min(1.0) as f32;

    if let Some(timeout) = silent_start_timeout {
        if !state_lock.initial_sound_detected {
            if rms < silence_threshold {
                if state_lock.silence_start.is_none() {
                    state_lock.silence_start = Some(Instant::now());
                } else if state_lock.silence_start.unwrap().elapsed().as_secs_f64() >= timeout {
                    warn!("Auto-stopping due to {}s of initial silence", timeout);
                    state_lock.auto_stopped = true;
                    state_lock.is_recording = false;
                    return;
                }
            } else {
                state_lock.initial_sound_detected = true;
                state_lock.silence_start = None;
            }
        }
    }

    state_lock.smoothed_level = SMOOTHING_FACTOR * normalized + (1.0 - SMOOTHING_FACTOR) * state_lock.smoothed_level;

    if let Some(sender) = level_sender {
        let _ = sender.try_send(state_lock.smoothed_level);
    }

    drop(state_lock);

    if let Some(writer) = writer.lock().as_mut() {
        for &sample in data {
            let _ = writer.write_sample(sample);
        }
    }
}
