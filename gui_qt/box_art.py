"""Box art loader for Pokemon game versions.

Loads cover art images from assets/box_art/ and caches them as scaled
QPixmaps keyed by version name + size. Files are expected to follow the
naming pattern ``{gen}-{VersionName}.png`` (e.g. ``2-Crystal.png``), and
callers pass the version name portion (matching ``const.*_VERSION``).
"""

import logging
import os

from PySide6.QtCore import Qt
from PySide6.QtGui import QPixmap

from utils.constants import const

logger = logging.getLogger(__name__)

_BOX_ART_DIR = os.path.join(const.ASSETS_PATH, "box_art")

_name_to_path = None
_pixmap_cache = {}


def _ensure_index():
    global _name_to_path
    if _name_to_path is not None:
        return
    _name_to_path = {}
    try:
        for fname in os.listdir(_BOX_ART_DIR):
            if not fname.lower().endswith(".png"):
                continue
            stem = os.path.splitext(fname)[0]
            # Expected format: "{gen}-{VersionName}" e.g. "2-Crystal"
            if "-" not in stem:
                continue
            version_name = stem.split("-", 1)[1]
            _name_to_path[version_name] = os.path.join(_BOX_ART_DIR, fname)
    except FileNotFoundError:
        logger.warning(f"Box art directory not found: {_BOX_ART_DIR}")
    except Exception as e:
        logger.warning(f"Could not index box art directory: {e}")


def get_box_art(version_name: str, width: int = 72, height: int = 72) -> QPixmap:
    """Return a scaled QPixmap for the given game version, or None if missing.

    The pixmap is scaled to fit within ``width`` x ``height`` while preserving
    aspect ratio. Results are cached per (version, width, height).
    """
    _ensure_index()

    cache_key = (version_name, width, height)
    if cache_key in _pixmap_cache:
        return _pixmap_cache[cache_key]

    path = _name_to_path.get(version_name)
    if path is None or not os.path.exists(path):
        _pixmap_cache[cache_key] = None
        return None

    try:
        pm = QPixmap(path)
        if pm.isNull():
            _pixmap_cache[cache_key] = None
            return None
        scaled = pm.scaled(width, height, Qt.KeepAspectRatio, Qt.SmoothTransformation)
        _pixmap_cache[cache_key] = scaled
        return scaled
    except Exception as e:
        logger.warning(f"Failed to load box art for {version_name}: {e}")
        _pixmap_cache[cache_key] = None
        return None
