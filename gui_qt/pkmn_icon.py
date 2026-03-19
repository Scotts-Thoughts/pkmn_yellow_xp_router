"""Pokemon HOME icon loader for the Qt UI.

Loads the small HOME menu icons from assets/pkmn_icons/ and caches
them as scaled QPixmaps keyed by species name.
"""

import json
import logging
import os

from PySide6.QtCore import Qt
from PySide6.QtGui import QPixmap

from utils.constants import const

logger = logging.getLogger(__name__)

_ICONS_DIR = os.path.join(const.ASSETS_PATH, "pkmn_icons")
_LOOKUP_PATH = os.path.join(_ICONS_DIR, "dex_lookup.json")

_dex_lookup = None
_pixmap_cache = {}


def _ensure_lookup():
    global _dex_lookup
    if _dex_lookup is not None:
        return
    try:
        with open(_LOOKUP_PATH, "r") as f:
            _dex_lookup = json.load(f)
    except Exception as e:
        logger.warning(f"Could not load dex_lookup.json: {e}")
        _dex_lookup = {}


def get_icon(species_name: str, size: int = 28) -> QPixmap:
    """Return a scaled QPixmap for the given Pokemon species, or None."""
    _ensure_lookup()

    cache_key = (species_name, size)
    if cache_key in _pixmap_cache:
        return _pixmap_cache[cache_key]

    dex = _dex_lookup.get(species_name)
    if dex is None:
        _pixmap_cache[cache_key] = None
        return None

    icon_path = os.path.join(_ICONS_DIR, f"{dex}.png")
    if not os.path.exists(icon_path):
        _pixmap_cache[cache_key] = None
        return None

    try:
        pm = QPixmap(icon_path)
        if pm.isNull():
            _pixmap_cache[cache_key] = None
            return None
        scaled = pm.scaled(size, size, Qt.KeepAspectRatio, Qt.SmoothTransformation)
        _pixmap_cache[cache_key] = scaled
        return scaled
    except Exception as e:
        logger.warning(f"Failed to load icon for {species_name}: {e}")
        _pixmap_cache[cache_key] = None
        return None
