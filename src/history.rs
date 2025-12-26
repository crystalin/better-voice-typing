use chrono::{DateTime, Local};
use parking_lot::Mutex;
use std::sync::Arc;

const MAX_HISTORY_SIZE: usize = 10;

#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub text: String,
    pub timestamp: DateTime<Local>,
}

pub struct TranscriptionHistory {
    entries: Arc<Mutex<Vec<HistoryEntry>>>,
}

impl TranscriptionHistory {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn add(&self, text: String) {
        let mut entries = self.entries.lock();

        entries.insert(0, HistoryEntry {
            text,
            timestamp: Local::now(),
        });

        if entries.len() > MAX_HISTORY_SIZE {
            entries.truncate(MAX_HISTORY_SIZE);
        }
    }

    pub fn get_recent(&self, count: usize) -> Vec<HistoryEntry> {
        let entries = self.entries.lock();
        entries.iter().take(count).cloned().collect()
    }

    pub fn get_all(&self) -> Vec<HistoryEntry> {
        self.entries.lock().clone()
    }

    pub fn clear(&self) {
        self.entries.lock().clear();
    }

    pub fn is_empty(&self) -> bool {
        self.entries.lock().is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.lock().len()
    }
}

impl Default for TranscriptionHistory {
    fn default() -> Self {
        Self::new()
    }
}
