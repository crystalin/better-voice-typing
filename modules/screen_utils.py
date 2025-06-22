import sys
import ctypes
from typing import Optional, NamedTuple

class MonitorGeometry(NamedTuple):
    left: int
    top: int
    right: int
    bottom: int
    width: int
    height: int

def get_primary_monitor_geometry() -> Optional[MonitorGeometry]:
    """
    Get the geometry of the primary monitor using ctypes on Windows.

    Returns a NamedTuple with left, top, right, bottom, width, and height,
    or None if the platform is not Windows or if the information cannot be retrieved.
    """
    if sys.platform != 'win32':
        return None

    try:
        user32 = ctypes.windll.user32

        # Define necessary Win32 structures and constants
        class RECT(ctypes.Structure):
            _fields_ = [("left", ctypes.c_long),
                        ("top", ctypes.c_long),
                        ("right", ctypes.c_long),
                        ("bottom", ctypes.c_long)]

        class MONITORINFO(ctypes.Structure):
            _fields_ = [("cbSize", ctypes.c_ulong),
                        ("rcMonitor", RECT),
                        ("rcWork", RECT),
                        ("dwFlags", ctypes.c_ulong)]

        MONITOR_DEFAULTTOPRIMARY = 1

        # Retrieve a handle to the primary monitor
        primary_monitor = user32.MonitorFromPoint(0, 0, MONITOR_DEFAULTTOPRIMARY)

        # Get monitor info for the primary monitor
        mi = MONITORINFO()
        mi.cbSize = ctypes.sizeof(MONITORINFO)
        if not user32.GetMonitorInfoW(primary_monitor, ctypes.byref(mi)):
            return None

        left = mi.rcMonitor.left
        top = mi.rcMonitor.top
        right = mi.rcMonitor.right
        bottom = mi.rcMonitor.bottom
        width = right - left
        height = bottom - top

        return MonitorGeometry(left=left, top=top, right=right, bottom=bottom, width=width, height=height)

    except Exception:
        # If any ctypes call fails, return None
        return None

def set_process_dpi_awareness() -> bool:
    """Attempt to set the process DPI awareness on Windows. Returns True if successful."""
    if sys.platform != 'win32':
        return False

    try:
        # Per-monitor DPI awareness (Windows 8.1+)
        ctypes.windll.shcore.SetProcessDpiAwareness(1)  # type: ignore[attr-defined]
        return True
    except Exception:
        return False


def hide_console_window() -> bool:
    """Hide the console window on Windows. Returns True if hidden."""
    if sys.platform != 'win32':
        return False

    try:
        ctypes.windll.user32.ShowWindow(ctypes.windll.kernel32.GetConsoleWindow(), 0)
        return True
    except Exception:
        return False