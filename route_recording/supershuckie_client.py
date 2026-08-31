from __future__ import annotations
import logging
import json
import threading
import time
import urllib.request

from utils.constants import const

logger = logging.getLogger(__name__)


def format_time_ms(time_ms:int) -> str:
    """
    Render a millisecond timer value as H:MM:SS.CC, which is how the stream overlays
    display Super Shuckie's timer.

    Centiseconds are truncated rather than rounded, so 679 milliseconds becomes .67
    """
    total_seconds, milliseconds = divmod(int(time_ms), 1000)
    total_minutes, seconds = divmod(total_seconds, 60)
    hours, minutes = divmod(total_minutes, 60)

    return f"{hours}:{minutes:02d}:{seconds:02d}.{milliseconds // 10:02d}"


class SuperShuckieClient:
    """
    Keeps a cached copy of Super Shuckie's run timer, so that recording code can read
    the current time without making a request itself.

    Super Shuckie answers requests from its emulator thread, which means a request can
    hang for a long time whenever the emulator is minimized or showing a dialog. The
    recorder reacts to GameHook events that are time sensitive, so it must never block
    on that; it reads the cached value instead.
    """

    POLL_INTERVAL = 0.1
    # how long a cached value stays usable after Super Shuckie stops responding
    STALE_THRESHOLD = 2
    REQUEST_TIMEOUT = 1
    ERROR_LOG_INTERVAL = 60

    def __init__(self, connection_string:str=None) -> None:
        if connection_string is None:
            connection_string = const.SUPER_SHUCKIE_URL
        self._connection_string = connection_string

        self._active = False
        # incremented on every start, so that a poll loop left over from a previous
        # start can tell that it has been replaced and exit on its own
        self._generation = 0

        self._lock = threading.Lock()
        self._time_ms = None
        self._last_success = None
        self._last_error_logged = 0

    def start(self):
        if self._active:
            return

        self._active = True
        self._generation += 1
        threading.Thread(target=self._poll_loop, args=(self._generation,), daemon=True).start()

    def stop(self):
        self._active = False
        with self._lock:
            self._time_ms = None
            self._last_success = None

    def get_current_time(self):
        """
        The most recent timer value from Super Shuckie, formatted as H:MM:SS.CC, or None
        when no value is available. See get_current_time_ms for when that happens.
        """
        time_ms = self.get_current_time_ms()
        if time_ms is None:
            return None

        return format_time_ms(time_ms)

    def get_current_time_ms(self):
        """
        The most recent timer value from Super Shuckie, in milliseconds.

        Returns None when no value is available: Super Shuckie isn't running, the timer
        hasn't been started yet (nothing has called mark-start), or the cached value has
        gone stale because Super Shuckie stopped responding.
        """
        with self._lock:
            if self._last_success is None:
                return None
            if time.time() - self._last_success > self.STALE_THRESHOLD:
                return None
            return self._time_ms

    def _poll_loop(self, generation:int):
        while self._active and self._generation == generation:
            self._refresh_stats()
            time.sleep(self.POLL_INTERVAL)

    def _refresh_stats(self):
        try:
            with urllib.request.urlopen(f"{self._connection_string}/stats", timeout=self.REQUEST_TIMEOUT) as response:
                stats = json.load(response)
            # null until a run has been started with mark-start. Read it in here too, so
            # that an unexpected response shape can't take the polling thread down
            time_current = stats.get("time_current")
        except Exception as e:
            self._log_failure(e)
            return

        if not isinstance(time_current, int) or isinstance(time_current, bool):
            time_current = None

        with self._lock:
            self._time_ms = time_current
            self._last_success = time.time()

    def _log_failure(self, e:Exception):
        # Super Shuckie is entirely optional, so it being closed is not an error worth
        # filling the log with. Report it rarely, just often enough to be diagnosable
        now = time.time()
        if now - self._last_error_logged < self.ERROR_LOG_INTERVAL:
            return

        self._last_error_logged = now
        logger.info(f"Could not read Super Shuckie stats from {self._connection_string}: {type(e).__name__}: {e}")


supershuckie_client = SuperShuckieClient()
