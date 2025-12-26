use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use log::{debug, error};
use parking_lot::Mutex;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, EndPaint, FillRect, ReleaseDC, HBRUSH, HDC, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwareness, PROCESS_PER_MONITOR_DPI_AWARE};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetClientRect,
    GetMessageW, LoadCursorW, PostQuitMessage, RegisterClassW, ShowWindow, TranslateMessage,
    UpdateWindow, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW, MSG, SW_HIDE, SW_SHOW,
    WINDOW_EX_STYLE, WM_CLOSE, WM_DESTROY, WM_PAINT, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_OVERLAPPED,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppStatus {
    Idle,
    Recording,
    Processing,
    Transcribing,
    Error,
}

pub enum UiCommand {
    UpdateStatus(AppStatus, String),
    UpdateAudioLevel(f32),
    ShowWarning(String),
    ShowError(String),
    Hide,
    Show,
}

pub struct UiFeedback {
    hwnd: HWND,
    status: Arc<Mutex<AppStatus>>,
    message: Arc<Mutex<String>>,
    audio_level: Arc<Mutex<f32>>,
    command_sender: Sender<UiCommand>,
    command_receiver: Receiver<UiCommand>,
}

static mut UI_STATE: Option<Arc<Mutex<UiState>>> = None;

struct UiState {
    status: AppStatus,
    message: String,
    audio_level: f32,
}

impl UiFeedback {
    pub fn new(position: &str, size: &str) -> Result<Self> {
        let _ = unsafe { SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE) };

        let (command_sender, command_receiver) = crossbeam_channel::unbounded();

        let status = Arc::new(Mutex::new(AppStatus::Idle));
        let message = Arc::new(Mutex::new(String::from("Ready")));
        let audio_level = Arc::new(Mutex::new(0.0f32));

        let ui_state = Arc::new(Mutex::new(UiState {
            status: AppStatus::Idle,
            message: String::from("Ready"),
            audio_level: 0.0,
        }));

        unsafe {
            UI_STATE = Some(ui_state.clone());
        }

        let hwnd = Self::create_window()?;

        Ok(Self {
            hwnd,
            status,
            message,
            audio_level,
            command_sender,
            command_receiver,
        })
    }

    fn create_window() -> Result<HWND> {
        unsafe {
            let instance = GetModuleHandleW(None)?;

            let class_name = windows::core::w!("VoiceTypingOverlay");

            let wc = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                style: CS_HREDRAW | CS_VREDRAW,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                ..Default::default()
            };

            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_NOACTIVATE,
                class_name,
                windows::core::w!("Voice Typing"),
                WS_OVERLAPPED,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                300,
                80,
                None,
                None,
                instance,
                None,
            )?;

            ShowWindow(hwnd, SW_HIDE);
            UpdateWindow(hwnd)?;

            Ok(hwnd)
        }
    }

    pub fn update_status(&self, status: AppStatus, message: String) {
        let _ = self.command_sender.try_send(UiCommand::UpdateStatus(status, message));
    }

    pub fn update_audio_level(&self, level: f32) {
        let _ = self.command_sender.try_send(UiCommand::UpdateAudioLevel(level));
    }

    pub fn show_warning(&self, message: String) {
        let _ = self.command_sender.try_send(UiCommand::ShowWarning(message));
    }

    pub fn show_error(&self, message: String) {
        let _ = self.command_sender.try_send(UiCommand::ShowError(message));
    }

    pub fn process_commands(&self) {
        while let Ok(cmd) = self.command_receiver.try_recv() {
            match cmd {
                UiCommand::UpdateStatus(status, msg) => {
                    if let Some(state) = unsafe { &UI_STATE } {
                        let mut s = state.lock();
                        s.status = status;
                        s.message = msg;
                    }
                    unsafe {
                        let _ = ShowWindow(self.hwnd, SW_SHOW);
                    }
                }
                UiCommand::UpdateAudioLevel(level) => {
                    if let Some(state) = unsafe { &UI_STATE } {
                        state.lock().audio_level = level;
                    }
                }
                UiCommand::ShowWarning(msg) | UiCommand::ShowError(msg) => {
                    if let Some(state) = unsafe { &UI_STATE } {
                        let mut s = state.lock();
                        s.status = AppStatus::Error;
                        s.message = msg;
                    }
                }
                UiCommand::Hide => {
                    unsafe {
                        let _ = ShowWindow(self.hwnd, SW_HIDE);
                    }
                }
                UiCommand::Show => {
                    unsafe {
                        let _ = ShowWindow(self.hwnd, SW_SHOW);
                    }
                }
            }
        }
    }

    pub fn run_message_loop() {
        unsafe {
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rect = RECT::default();
            let _ = GetClientRect(hwnd, &mut rect);

            let brush = CreateSolidBrush(COLORREF(0x00F0F0F0));
            FillRect(hdc, &rect, brush);

            EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
