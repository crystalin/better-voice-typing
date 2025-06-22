import os
import sys
import threading
import traceback
from typing import Any, Callable, Optional, Tuple
import logging
from datetime import datetime
from pathlib import Path
import httpx

from pynput import keyboard
import pyperclip

from modules.clean_text import clean_transcription
from modules.history import TranscriptionHistory
from modules.recorder import AudioRecorder, DEFAULT_SILENT_START_TIMEOUT
from modules.settings import Settings
from modules.transcribe import transcribe_audio
from modules.tray import setup_tray_icon
from modules.ui import UIFeedback
from modules.audio_manager import set_input_device, get_default_device_id, DeviceIdentifier, find_device_by_identifier
from modules.status_manager import StatusManager, AppStatus

def setup_logging() -> logging.Logger:
    """Configure application logging"""
    # Create logs directory in user's documents folder
    # Ex. "C:\Users\{name}\Documents\VoiceTyping\logs\voice_typing_20241120.log"
    log_dir = Path.home() / "Documents" / "VoiceTyping" / "logs"
    log_dir.mkdir(parents=True, exist_ok=True)

    # Create log file with timestamp
    log_file = log_dir / f"voice_typing_{datetime.now().strftime('%Y%m%d')}.log"

    # Configure logging
    logger = logging.getLogger('voice_typing')
    logger.setLevel(logging.DEBUG)

    # File handler
    file_handler = logging.FileHandler(log_file)
    file_handler.setLevel(logging.INFO)
    formatter = logging.Formatter('%(asctime)s - %(levelname)s - %(message)s')
    file_handler.setFormatter(formatter)

    # Add console handler for real-time debugging
    console_handler = logging.StreamHandler(sys.stdout)
    console_handler.setLevel(logging.DEBUG)
    console_formatter = logging.Formatter('%(levelname)s: %(message)s')  # Simpler format for console
    console_handler.setFormatter(console_formatter)

    logger.addHandler(file_handler)
    logger.addHandler(console_handler)

    # Log system info at startup
    logger.info(f"Python version: {sys.version}")
    logger.info(f"Platform: {sys.platform}")

    return logger

class VoiceTypingApp:
    def __init__(self) -> None:
        # Setup logging first
        self.logger = setup_logging()
        self.logger.info("Starting Voice Typing application")

        # Hide console in Windows if running as .pyw
        if os.name == 'nt':
            import ctypes
            ctypes.windll.user32.ShowWindow(
                ctypes.windll.kernel32.GetConsoleWindow(), 0)

        # Initialize these as None first
        self.update_tray_tooltip = None
        self.update_icon_menu = None
        # Initialize last_recording before tray setup
        self.last_recording: Optional[str] = None

        self.settings = Settings()
        silent_start_timeout = self.settings.get('silent_start_timeout')
        self.ui_feedback = UIFeedback()
        self.recorder = AudioRecorder(
            level_callback=self.ui_feedback.update_audio_level,
            silent_start_timeout=silent_start_timeout
        )
        self.ui_feedback.set_click_callback(self.handle_ui_click)
        self.recording = False
        self.ctrl_pressed = False
        self.clean_transcription_enabled = self.settings.get('clean_transcription')
        self.history = TranscriptionHistory()

        # Add a flag for canceling processing
        self.processing_thread: Optional[threading.Thread] = None
        self.cancel_flag = threading.Event()

        # Log settings information
        self.logger.info(f"Text cleaning: {'enabled' if self.clean_transcription_enabled else 'disabled'}")
        self.logger.info(f"LLM model: {self.settings.get('llm_model')}")

        # Initialize microphone
        self._initialize_microphone()

        # Initialize status manager first
        self.status_manager = StatusManager()

        # Setup single tray icon instance
        setup_tray_icon(self)

        # Now set the callbacks
        self.status_manager.set_callbacks(
            ui_callback=self.ui_feedback.update_status,
            tray_callback=self.update_tray_tooltip
        )

        # Set initial status
        self.status_manager.set_status(AppStatus.IDLE)

        # Store last recording for retry functionality
        self.ui_feedback.set_retry_callback(self.retry_transcription)

        def win32_event_filter(msg: int, data: Any) -> bool:
            # Key codes and messages
            VK_CONTROL = 0x11
            VK_LCONTROL = 0xA2
            VK_RCONTROL = 0xA3
            VK_CAPITAL = 0x14

            WM_KEYDOWN = 0x0100
            WM_KEYUP = 0x0101

            if data.vkCode in (VK_CONTROL, VK_LCONTROL, VK_RCONTROL):
                if msg == WM_KEYDOWN:
                    self.ctrl_pressed = True
                elif msg == WM_KEYUP:
                    self.ctrl_pressed = False
                return True

            # Handle Caps Lock
            if data.vkCode == VK_CAPITAL and msg == WM_KEYDOWN:
                if self.ctrl_pressed:
                    # Allow normal Caps Lock behavior when Ctrl is pressed
                    return True
                else:
                    # Toggle recording and suppress default Caps Lock behavior
                    self.toggle_recording()
                    # self.listener.suppress_event()
                    return False

            return True

        self.listener = keyboard.Listener(
            win32_event_filter=win32_event_filter,
            suppress=False
        )

    def _initialize_microphone(self) -> None:
        """Initialize microphone device from settings or default"""
        try:
            saved_identifier = self.settings.get('selected_microphone')
            if saved_identifier is not None:
                try:
                    # Convert dictionary back to DeviceIdentifier
                    identifier = DeviceIdentifier(**saved_identifier)
                    device = find_device_by_identifier(identifier)
                    if device:
                        set_input_device(device['id'])
                    else:
                        # Fallback to default if saved device not found
                        self.settings.set('selected_microphone', None)
                        set_input_device(get_default_device_id())
                except Exception as e:
                    print(f"Error setting saved microphone: {e}")
                    # Fallback to default
                    self.settings.set('selected_microphone', None)
                    set_input_device(get_default_device_id())
        except Exception as e:
            self.logger.error(f"Error setting saved microphone: {e}", exc_info=True)
            # Fallback to default
            self.settings.set('selected_microphone', None)
            set_input_device(get_default_device_id())

    def set_microphone(self, device_id: int) -> None:
        """Change the active microphone device"""
        try:
            set_input_device(device_id)
            self.settings.set('selected_microphone', device_id)
            # Stop any ongoing recording when changing microphone
            if self.recording:
                self.handle_ui_click()
        except Exception as e:
            self.logger.error(f"Error setting microphone: {e}", exc_info=True)
            self.logger.debug(f"Failed device_id: {device_id}")
            self.ui_feedback.show_warning("⚠️ Error changing microphone")

    def refresh_microphones(self) -> None:
        """Refresh the microphone list and update the tray menu"""
        if self.update_icon_menu:
            self.update_icon_menu()

    def toggle_recording(self) -> None:
        if not self.recording:
            print("🎙️ Starting recording...")
            self.recording = True
            self.recorder.start()
            self.status_manager.set_status(AppStatus.RECORDING)
            # Start periodic status checks
            self._check_recorder_status()
        else:
            self._stop_recording()

    def _stop_recording(self) -> None:
        """Helper method to handle recording stop logic"""
        self.recording = False
        self.recorder.stop()
        self.logger.info("Recording stopped via keyboard shortcut")

        if self.recorder.was_auto_stopped():
            self.status_manager.set_status(
                AppStatus.ERROR,
                "⚠️ Recording stopped: No audio detected"
            )
            self.logger.warning("Recording auto-stopped due to initial silence")
            # Clear the auto-stopped flag
            self.recorder.auto_stopped = False
        else:
            self.status_manager.set_status(AppStatus.PROCESSING)
            self.process_audio()

    # Add this method to check recorder status periodically
    def _check_recorder_status(self) -> None:
        """Periodically check if recorder has auto-stopped"""
        if self.recording and self.recorder.was_auto_stopped():
            self._stop_recording()

        if self.recording:
            # Schedule next check in 100ms
            self.ui_feedback.root.after(100, self._check_recorder_status)

    def process_audio(self) -> None:
        try:
            self.cancel_flag.clear()  # Reset flag before starting
            self.processing_thread = threading.Thread(target=self._process_audio_thread)
            self.processing_thread.start()
        except Exception as e:
            self.logger.error("Failed to start processing thread", exc_info=True)
            self.logger.debug(f"Thread state: {threading.current_thread().name}")
            self.ui_feedback.insert_text(f"Error: {str(e)[:50]}...")

    def _process_audio_thread(self) -> None:
        try:
            self.logger.info("Starting audio processing")
            print("Analyzing audio...")
            is_valid, reason = self.recorder.analyze_recording()

            if self.cancel_flag.is_set():
                self.logger.info("Processing cancelled before transcription.")
                self.status_manager.set_status(AppStatus.IDLE)
                return

            if not is_valid:
                self.logger.warning(f"Skipping transcription: {reason}")
                self.status_manager.set_status(
                    AppStatus.ERROR,
                    "⛔ Skipped: " + ("too short" if "short" in reason.lower() else "mostly silence")
                )
                return

            # Store recording path for retry functionality
            self.last_recording = self.recorder.filename

            self.logger.info("Starting transcription")
            success, result = self._attempt_transcription()
            self.logger.info("Finished transcription")

            if self.cancel_flag.is_set():
                self.logger.info("Processing cancelled after transcription.")
                self.status_manager.set_status(AppStatus.IDLE)
                return

            if not success:
                # Check if it was a timeout error
                if result == "timeout":
                    self.ui_feedback.show_error_with_retry("⏱️ Request timed out - try again")
                    self.status_manager.set_status(AppStatus.ERROR, "⏱️ Request timed out")
                else:
                    self.ui_feedback.show_error_with_retry("⚠️ Transcription failed")
                    self.status_manager.set_status(AppStatus.ERROR, "⚠️ Error processing audio")
            elif result:
                self.last_recording = None  # Clear on success
                self.history.add(result)
                self.ui_feedback.insert_text(result)
                if self.update_icon_menu:
                    self.update_icon_menu()
                self.status_manager.set_status(AppStatus.IDLE)
                print("Transcription completed and inserted")

        except Exception as e:
            self.logger.error("Error in _process_audio_thread:", exc_info=True)
            # Check if it's a timeout exception
            if 'timeout' in str(e).lower():
                self.ui_feedback.show_error_with_retry("⏱️ Request timed out - try again")
                self.status_manager.set_status(AppStatus.ERROR, "⏱️ Request timed out")
            else:
                self.ui_feedback.show_error_with_retry("⚠️ Transcription failed")
                self.status_manager.set_status(AppStatus.ERROR, "⚠️ Error processing audio")

    def _attempt_transcription(self) -> Tuple[bool, Optional[str]]:
        """Attempt transcription and return (success, result or error_type)"""
        try:
            if not self.last_recording:
                self.logger.error("Attempted transcription with no recording available.")
                return False, "no_recording"

            # Update status to show we're transcribing
            self.status_manager.set_status(AppStatus.TRANSCRIBING)
            text = transcribe_audio(self.last_recording)

            if self.cancel_flag.is_set():
                return False, "cancelled"

            if self.clean_transcription_enabled:
                try:
                    # Update status to show we're cleaning
                    self.status_manager.set_status(AppStatus.CLEANING)

                    # Get the configured LLM model and timeout from settings
                    llm_model = self.settings.get('llm_model')
                    cleaning_timeout = self.settings.get('cleaning_timeout')

                    cleaned_text = clean_transcription(text, model=llm_model, timeout=cleaning_timeout)
                    self.logger.info("Transcription cleaned successfully")
                    return True, cleaned_text
                except Exception as e:
                    self.logger.warning(f"LLM cleaning failed, falling back to raw transcription. Error: {e}")
                    # Show a brief warning that we're using the fallback
                    self.ui_feedback.show_warning("⚠️ Using raw transcript (cleaning failed)", 2000)
                    return True, text  # Fallback to original text

            self.logger.info("Transcription completed successfully")
            return True, text
        except Exception as e:
            # Check if it's a timeout exception
            if 'timeout' in str(e).lower():
                self.logger.error(f"Transcription timeout: Request took too long", exc_info=True)
                return False, "timeout"
            else:
                self.logger.error(f"Transcription error: {e}", exc_info=True)
                return False, None

    def retry_transcription(self) -> None:
        """Retry transcription of last failed recording"""
        if not self.last_recording:
            return

        def retry_thread():
            self.status_manager.set_status(AppStatus.PROCESSING)
            success, result = self._attempt_transcription()

            if success and result:
                self.last_recording = None
                self.history.add(result)
                pyperclip.copy(result)  # Copy to clipboard instead of direct insertion
                self.status_manager.set_status(AppStatus.IDLE)
                self.ui_feedback.show_warning("✅ Transcription copied to clipboard", 3000)
            else:
                self.ui_feedback.show_error_with_retry("⚠️ Retry failed")
                self.status_manager.set_status(AppStatus.ERROR)

        threading.Thread(target=retry_thread).start()

    def toggle_clean_transcription(self) -> None:
        self.clean_transcription_enabled = not self.clean_transcription_enabled
        self.settings.set('clean_transcription', self.clean_transcription_enabled)
        print(f"Clean transcription {'enabled' if self.clean_transcription_enabled else 'disabled'}")

    def run(self) -> None:
        # Start keyboard listener
        self.listener.start()

        # Start the UI feedback's tkinter mainloop in the main thread
        try:
            self.ui_feedback.root.mainloop()
        finally:
            self.cleanup()
            sys.exit(0)

    def cleanup(self) -> None:
        """Ensure proper cleanup of all resources"""
        self.logger.info("Cleaning up application resources")
        self.listener.stop()
        if self.recording:
            self.recorder.stop()
        self.ui_feedback.cleanup()

    def handle_ui_click(self) -> None:
        """Handle clicks on the UI feedback window."""
        status = self.status_manager.current_status
        if status == AppStatus.RECORDING:
            print("Canceling recording...")
            self.recording = False
            threading.Thread(target=self._stop_recorder).start()
            self.status_manager.set_status(AppStatus.IDLE)
        elif status in (AppStatus.PROCESSING, AppStatus.TRANSCRIBING, AppStatus.CLEANING):
            print("Canceling processing...")
            if self.processing_thread and self.processing_thread.is_alive():
                self.cancel_flag.set()
                # The processing thread will set the status to IDLE upon graceful exit

    def _stop_recorder(self) -> None:
        """Helper method to stop recorder in a separate thread"""
        try:
            self.recorder.stop()
        except Exception as e:
            self.logger.error("Error stopping recorder", exc_info=True)
            self.logger.debug(f"Recorder state: recording={self.recording}")

    def toggle_favorite_microphone(self, device_id: int) -> None:
        """Toggle favorite status for a microphone device"""
        favorites = self.settings.get('favorite_microphones')
        if device_id in favorites:
            favorites.remove(device_id)
        else:
            favorites.append(device_id)
        self.settings.set('favorite_microphones', favorites)

    def toggle_silence_detection(self) -> None:
        """Toggle silence detection on/off"""
        current_timeout = self.settings.get('silent_start_timeout')
        # Toggle between None and default timeout
        new_timeout = None if current_timeout is not None else DEFAULT_SILENT_START_TIMEOUT
        self.settings.set('silent_start_timeout', new_timeout)

        # Update recorder's silence timeout
        self.recorder.silent_start_timeout = new_timeout

        status = "enabled" if new_timeout is not None else "disabled"
        print(f"Silence detection {status}")
        self.logger.info(f"Silence detection {status}")

    def restart_app(self) -> None:
        """Restart the application by launching a new instance and closing the current one"""
        self.logger.info("Restarting application...")
        try:
            # Start new instance of the app
            os.startfile(sys.argv[0])
            # Exit current instance
            os._exit(0)
        except Exception as e:
            self.logger.error(f"Failed to restart application: {e}", exc_info=True)
            self.status_manager.set_status(AppStatus.ERROR, "⚠️ Failed to restart")

if __name__ == "__main__":
    app = VoiceTypingApp()
    app.run()