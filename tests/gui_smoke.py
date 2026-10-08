"""Black-box X11 test: real keyboard input, autosave/restart, and desktop capture.

Run under xvfb-run. Requires libX11, libXtst, and the built native binaries.
All data goes into a TemporaryDirectory, never the user's Oxide profile.
"""
import ctypes
import ctypes.util
import os
import re
import resource
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import time


def wait_for(check, timeout=12):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        result = check()
        if result:
            return result
        time.sleep(0.1)
    raise AssertionError("Native workflow did not reach the expected state")


def main():
    binary = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/oxide-app").resolve()
    x11 = ctypes.CDLL(ctypes.util.find_library("X11"))
    xtest = ctypes.CDLL(ctypes.util.find_library("Xtst"))
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XStringToKeysym.restype = ctypes.c_ulong
    x11.XStringToKeysym.argtypes = [ctypes.c_char_p]
    x11.XKeysymToKeycode.restype = ctypes.c_uint
    x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
    x11.XFlush.argtypes = [ctypes.c_void_p]
    x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
    x11.XSetInputFocus.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
    xtest.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
    xtest.XTestFakeMotionEvent.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_ulong]
    xtest.XTestFakeButtonEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
    display = x11.XOpenDisplay(None)
    assert display, "No X11 display; run with xvfb-run"

    def key(name, down=None):
        code = x11.XKeysymToKeycode(display, x11.XStringToKeysym(name.encode()))
        assert code, f"No native key mapping for {name}"
        for pressed in ([1, 0] if down is None else [int(down)]):
            xtest.XTestFakeKeyEvent(display, code, pressed, 0)
        x11.XFlush(display)

    def chord(*names):
        for name in names[:-1]:
            key(name, True)
        key(names[-1])
        for name in reversed(names[:-1]):
            key(name, False)

    def click(x, y):
        xtest.XTestFakeMotionEvent(display, -1, x, y, 0)
        xtest.XTestFakeButtonEvent(display, 1, 1, 0)
        xtest.XTestFakeButtonEvent(display, 1, 0, 0)
        x11.XFlush(display)

    def text(value):
        for char in value:
            if char.isupper():
                key("Shift_L", True)
            key("space" if char == " " else char.lower())
            if char.isupper():
                key("Shift_L", False)
            time.sleep(0.07)

    with tempfile.TemporaryDirectory(prefix="oxide-gui-") as root:
        root = Path(root)
        subprocess.run(["xsetroot", "-solid", "#123456"], check=True)
        process = subprocess.Popen([str(binary), "--data-dir", str(root), "--no-worker"])
        try:
            db = root / "oxide.db"

            def rows(sql):
                if not db.exists():
                    return []
                try:
                    with sqlite3.connect(db) as connection:
                        return connection.execute(sql).fetchall()
                except sqlite3.OperationalError:
                    return []

            wait_for(lambda: rows("SELECT count(*) FROM notes") == [(0,)])
            time.sleep(1)
            tree = subprocess.check_output(["xwininfo", "-root", "-tree"], text=True)
            window = re.search(r'(0x[0-9a-f]+) "Oxide"', tree)
            assert window, f"Native Oxide window not found: {tree}"
            x11.XSetInputFocus(display, int(window.group(1), 16), 1, 0)
            x11.XFlush(display)
            key("Escape")  # close first-run settings
            chord("Control_L", "n")
            wait_for(lambda: rows("SELECT count(*) FROM notes") == [(1,)])
            time.sleep(0.5)
            click(480, 240)
            time.sleep(0.5)
            text("Persisted native note body")
            wait_for(lambda: any("Persisted native note body" in row[0] for row in rows("SELECT body FROM notes")))
            chord("Alt_L", "Shift_L", "d")
            wait_for(lambda: rows("SELECT count(*) FROM screenshots") == [(1,)])
            wait_for(lambda: rows("SELECT count(*) FROM note_screenshots") == [(1,)])
            assert list((root / "captures").glob("*.png")), "Native capture original is missing"
            original = rows("SELECT path FROM screenshots")[0][0]
            assert Path(original).is_file()
            pixels = subprocess.check_output(["ffmpeg", "-loglevel", "error", "-i", original, "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"])
            assert pixels[(240 * 1280 + 480) * 3: (240 * 1280 + 480) * 3 + 3] == bytes([18, 52, 86]), "Oxide's editor leaked into its own screenshot"
            chord("Alt_L", "Shift_L", "h")
            wait_for(lambda: "Map State: IsUnMapped" in subprocess.check_output(["xwininfo", "-id", window.group(1)], text=True))
            wake = subprocess.run([str(binary), "--data-dir", str(root), "--no-worker"], timeout=10, check=False)
            assert wake.returncode == 0 and process.poll() is None, "Resident relaunch did not summon the running app"
            wait_for(lambda: "Map State: IsViewable" in subprocess.check_output(["xwininfo", "-id", window.group(1)], text=True))
            process.terminate()
            process.wait(timeout=10)
            process = subprocess.Popen([str(binary), "--data-dir", str(root), "--no-worker"])
            time.sleep(2)
            assert process.poll() is None, "Application failed to restart"
            assert rows("SELECT body FROM notes")[0][0] == "Persisted native note body"
            assert rows("SELECT count(*) FROM note_screenshots") == [(1,)]
            print("PASS: native typing → acknowledged autosave → own-UI-free capture → attachment → hide/resident wake → restart")
            print(f"Peak native child RSS (Linux, OCR child disabled): {resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss / 1024:.1f} MiB")
        except Exception:
            subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-f", "x11grab", "-video_size", "1280x800", "-i", os.environ["DISPLAY"], "-frames:v", "1", "/tmp/omnirush/oxide-gui-failure.png"], check=False)
            raise
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)
            x11.XCloseDisplay(display)


if __name__ == "__main__":
    main()
