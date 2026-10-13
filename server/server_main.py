"""Entry point used when the HR server is packaged into hr-server.exe for the desktop app.
Environment: HR_DATA = data folder, HR_PORT = port (default 5000)."""
import os
import sys

if getattr(sys, "frozen", False):            # running inside the packaged .exe
    os.chdir(os.path.dirname(sys.executable))
if sys.stdout is None or sys.stderr is None:  # windowed exe has no console: log to a file in the data folder
    _d = os.environ.get("HR_DATA") or os.path.dirname(os.path.abspath(sys.argv[0]))
    os.makedirs(_d, exist_ok=True)
    _log = open(os.path.join(_d, "server.log"), "a", encoding="utf-8", buffering=1)
    sys.stdout = sys.stdout or _log
    sys.stderr = sys.stderr or _log

import app as hr                               # noqa: E402
import threading                               # noqa: E402
import time                                    # noqa: E402


def _pid_alive(pid):
    if os.name == "nt":
        import ctypes
        k = ctypes.windll.kernel32
        h = k.OpenProcess(0x00100000, False, pid)          # SYNCHRONIZE
        if not h:
            return False
        try:
            return k.WaitForSingleObject(h, 0) == 0x102     # WAIT_TIMEOUT = still running
        finally:
            k.CloseHandle(h)
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def _watch_parent(pid):
    """Stop this server when the desktop app that started it is gone (quit, crash or ended in Task Manager)."""
    while True:
        time.sleep(2)
        try:
            alive = _pid_alive(pid)
        except Exception:                      # noqa: BLE001 - never let the watchdog die
            alive = True
        if not alive:
            try:
                print("Desktop app closed - stopping HR server.", flush=True)
            except Exception:                  # noqa: BLE001 - the output pipe is gone too
                pass
            os._exit(0)


if __name__ == "__main__":
    port = int(os.environ.get("HR_PORT", "5000"))
    print(f"NB Sons HR server {hr.APP_VERSION} - data: {hr.DATA_DIR} - port {port}", flush=True)
    parent = os.environ.get("HR_PARENT_PID", "")
    if parent.isdigit():
        threading.Thread(target=_watch_parent, args=(int(parent),), daemon=True).start()
    from waitress import serve
    serve(hr.app, host="0.0.0.0", port=port, threads=12, ident="NB Sons HR")
