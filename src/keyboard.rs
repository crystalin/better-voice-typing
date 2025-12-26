use anyhow::Result;
use crossbeam_channel::Sender;
use log::{debug, error};
use std::sync::Arc;
use parking_lot::Mutex;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_CAPITAL, VK_CONTROL, VK_LCONTROL, VK_RCONTROL,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
};

pub enum KeyboardEvent {
    CapsLockPressed,
}

pub struct KeyboardListener {
    hook: HHOOK,
    ctrl_pressed: Arc<Mutex<bool>>,
}

static mut EVENT_SENDER: Option<Sender<KeyboardEvent>> = None;
static mut CTRL_PRESSED: Option<Arc<Mutex<bool>>> = None;

impl KeyboardListener {
    pub fn new(event_sender: Sender<KeyboardEvent>) -> Result<Self> {
        let ctrl_pressed = Arc::new(Mutex::new(false));

        unsafe {
            EVENT_SENDER = Some(event_sender);
            CTRL_PRESSED = Some(ctrl_pressed.clone());

            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0)?;

            Ok(Self {
                hook,
                ctrl_pressed,
            })
        }
    }

    pub fn is_ctrl_pressed(&self) -> bool {
        *self.ctrl_pressed.lock()
    }
}

impl Drop for KeyboardListener {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWindowsHookEx(self.hook);
            EVENT_SENDER = None;
            CTRL_PRESSED = None;
        }
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kb_struct = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = kb_struct.vkCode as u16;
        let msg = wparam.0 as u32;

        if let Some(ctrl_state) = &CTRL_PRESSED {
            if vk_code == VK_CONTROL.0 || vk_code == VK_LCONTROL.0 || vk_code == VK_RCONTROL.0 {
                let mut ctrl_pressed = ctrl_state.lock();
                if msg == WM_KEYDOWN {
                    *ctrl_pressed = true;
                } else if msg == WM_KEYUP {
                    *ctrl_pressed = false;
                }
            }
        }

        if vk_code == VK_CAPITAL.0 && msg == WM_KEYDOWN {
            let ctrl_pressed = CTRL_PRESSED.as_ref()
                .map(|c| *c.lock())
                .unwrap_or(false);

            if !ctrl_pressed {
                if let Some(sender) = &EVENT_SENDER {
                    if sender.try_send(KeyboardEvent::CapsLockPressed).is_ok() {
                        return LRESULT(1);
                    }
                }
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}
