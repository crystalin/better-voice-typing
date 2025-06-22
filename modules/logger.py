import logging
import sys
from datetime import datetime
from pathlib import Path

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

    # Prevent duplicate handlers
    if logger.hasHandlers():
        logger.handlers.clear()

    # File handler with explicit UTF-8 encoding
    file_handler = logging.FileHandler(log_file, encoding='utf-8')
    file_handler.setLevel(logging.INFO)
    formatter = logging.Formatter('%(asctime)s - %(levelname)s - %(message)s')
    file_handler.setFormatter(formatter)

    # Custom stream handler for UTF-8 console output on Windows
    class Utf8ConsoleHandler(logging.StreamHandler):
        def emit(self, record):
            try:
                msg = self.format(record)
                # On Windows, write directly to buffer with UTF-8 encoding
                if hasattr(self.stream, 'buffer'):
                    self.stream.buffer.write((msg + self.terminator).encode('utf-8'))
                    self.stream.buffer.flush()
                else:
                    # Fallback for non-buffer streams
                    self.stream.write(msg + self.terminator)
                    self.flush()
            except Exception:
                self.handleError(record)

    # Use the custom handler for console output
    console_handler = Utf8ConsoleHandler(sys.stdout)
    console_handler.setLevel(logging.DEBUG)
    console_formatter = logging.Formatter('%(levelname)s: %(message)s')
    console_handler.setFormatter(console_formatter)

    logger.addHandler(file_handler)
    logger.addHandler(console_handler)

    # Log system info at startup
    logger.info(f"Python version: {sys.version}")
    logger.info(f"Platform: {sys.platform}")

    return logger