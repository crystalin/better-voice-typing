use anyhow::Result;
use crossbeam_channel::Sender;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use crate::audio::AudioDevice;
use crate::history::HistoryEntry;
use std::sync::Arc;
use parking_lot::Mutex;

pub enum TrayEvent {
    ToggleSilenceDetection,
    SelectMicrophone(String),
    ToggleFavoriteMicrophone(String),
    RefreshMicrophones,
    CopyHistory(usize),
    ClearHistory,
    Quit,
}

pub struct TrayManager {
    tray_icon: TrayIcon,
    menu_event_sender: Sender<TrayEvent>,
    devices: Arc<Mutex<Vec<AudioDevice>>>,
    favorites: Arc<Mutex<Vec<String>>>,
}

impl TrayManager {
    pub fn new(menu_event_sender: Sender<TrayEvent>) -> Result<Self> {
        let icon = Self::create_icon()?;

        let menu = Self::build_menu(&[], &[], &[], true);

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Voice Typing")
            .with_icon(icon)
            .build()?;

        Ok(Self {
            tray_icon,
            menu_event_sender,
            devices: Arc::new(Mutex::new(Vec::new())),
            favorites: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn update_menu(
        &mut self,
        devices: Vec<AudioDevice>,
        favorites: Vec<String>,
        history: Vec<HistoryEntry>,
        silence_detection_enabled: bool,
    ) {
        *self.devices.lock() = devices.clone();
        *self.favorites.lock() = favorites.clone();

        let menu = Self::build_menu(&devices, &favorites, &history, silence_detection_enabled);
        self.tray_icon.set_menu(Some(Box::new(menu)));
    }

    pub fn set_tooltip(&mut self, text: &str) {
        let _ = self.tray_icon.set_tooltip(Some(text));
    }

    fn build_menu(
        devices: &[AudioDevice],
        favorites: &[String],
        history: &[HistoryEntry],
        silence_detection_enabled: bool,
    ) -> Menu {
        let menu = Menu::new();

        if !history.is_empty() {
            let history_submenu = Submenu::new("History", true);
            for (idx, entry) in history.iter().enumerate().take(10) {
                let preview = if entry.text.len() > 50 {
                    format!("{}...", &entry.text[..50])
                } else {
                    entry.text.clone()
                };
                let item = MenuItem::new(preview, true, None);
                let _ = history_submenu.append(&item);
            }

            let _ = history_submenu.append(&PredefinedMenuItem::separator());
            let clear_item = MenuItem::new("Clear History", true, None);
            let _ = history_submenu.append(&clear_item);

            let _ = menu.append(&history_submenu);
            let _ = menu.append(&PredefinedMenuItem::separator());
        }

        if !devices.is_empty() {
            let mic_submenu = Submenu::new("Microphones", true);

            for device in devices {
                let is_favorite = favorites.contains(&device.name);
                let label = if is_favorite {
                    format!("★ {}", device.name)
                } else {
                    device.name.clone()
                };
                let item = MenuItem::new(label, true, None);
                let _ = mic_submenu.append(&item);
            }

            let _ = mic_submenu.append(&PredefinedMenuItem::separator());
            let refresh_item = MenuItem::new("Refresh", true, None);
            let _ = mic_submenu.append(&refresh_item);

            let _ = menu.append(&mic_submenu);
            let _ = menu.append(&PredefinedMenuItem::separator());
        }

        let silence_label = if silence_detection_enabled {
            "Disable Silence Detection"
        } else {
            "Enable Silence Detection"
        };
        let silence_item = MenuItem::new(silence_label, true, None);
        let _ = menu.append(&silence_item);

        let _ = menu.append(&PredefinedMenuItem::separator());

        let quit_item = MenuItem::new("Quit", true, None);
        let _ = menu.append(&quit_item);

        menu
    }

    fn create_icon() -> Result<Icon> {
        let rgba = vec![255u8; 16 * 16 * 4];
        Ok(Icon::from_rgba(rgba, 16, 16)?)
    }

    pub fn process_menu_events(&self) {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = event.id.0;

            if id.contains("Quit") {
                let _ = self.menu_event_sender.try_send(TrayEvent::Quit);
            } else if id.contains("Silence Detection") {
                let _ = self.menu_event_sender.try_send(TrayEvent::ToggleSilenceDetection);
            } else if id.contains("Clear History") {
                let _ = self.menu_event_sender.try_send(TrayEvent::ClearHistory);
            } else if id.contains("Refresh") {
                let _ = self.menu_event_sender.try_send(TrayEvent::RefreshMicrophones);
            }
        }
    }
}
