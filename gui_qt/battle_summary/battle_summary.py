import os
import logging
from datetime import datetime
from typing import List

from PySide6.QtWidgets import (
    QWidget, QLabel, QScrollArea, QGridLayout, QVBoxLayout, QHBoxLayout,
    QFrame, QCompleter, QLineEdit, QSizePolicy, QPushButton, QCheckBox, QComboBox,
)
from PySide6.QtCore import Qt, QTimer, Signal, QRectF, QStringListModel, QEvent, QCoreApplication
from PySide6.QtGui import QFont, QPixmap, QPainter, QPainterPath, QFocusEvent

from controllers.battle_summary_controller import BattleSummaryController, MoveRenderInfo
from gui_qt.components.custom_components import (
    SimpleButton, SimpleOptionMenu, AmountEntry, CheckboxLabel, DisclosureTriangle,
)
from pkmn import universal_data_objects
from pkmn.gen_factory import current_gen_info
from routing import full_route_state
from routing import route_events
from gui_qt import pkmn_icon
from utils.config_manager import config
from utils.constants import const
from utils import io_utils

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Colour utilities
# ---------------------------------------------------------------------------

def _hex_to_rgb(hex_color: str):
    hex_color = hex_color.lstrip("#")
    if len(hex_color) == 3:
        hex_color = "".join(c * 2 for c in hex_color)
    if len(hex_color) != 6:
        return (0, 0, 0)
    return tuple(int(hex_color[i : i + 2], 16) for i in (0, 2, 4))


def _rgb_to_hex(r, g, b):
    return "#{:02x}{:02x}{:02x}".format(int(max(0, min(255, r))), int(max(0, min(255, g))), int(max(0, min(255, b))))


def _blend_color(color1: str, color2: str, alpha: float) -> str:
    """Blend *color1* over *color2* with the given *alpha* (0.0 = fully color2, 1.0 = fully color1)."""
    c1 = str(color1) if color1 else ""
    c2 = str(color2) if color2 else ""
    if not c1 or not c1.startswith("#"):
        return c2 if c2 else "#000000"
    if not c2 or not c2.startswith("#"):
        return c1 if c1 else "#000000"
    try:
        r1, g1, b1 = _hex_to_rgb(c1)
        r2, g2, b2 = _hex_to_rgb(c2)
        return _rgb_to_hex(
            r1 * alpha + r2 * (1 - alpha),
            g1 * alpha + g2 * (1 - alpha),
            b1 * alpha + b2 * (1 - alpha),
        )
    except Exception:
        return c2 if c2 else "#000000"


def _darken(hex_color: str, amount: float) -> str:
    """Darken a hex colour by the given amount (0.0 = no change, 1.0 = black)."""
    try:
        r, g, b = _hex_to_rgb(hex_color)
        r = max(0, r * (1 - amount))
        g = max(0, g * (1 - amount))
        b = max(0, b * (1 - amount))
        return _rgb_to_hex(r, g, b)
    except Exception:
        return hex_color


# ---------------------------------------------------------------------------
# Highlight-state colour map
# ---------------------------------------------------------------------------

_HIGHLIGHT_COLORS = {
    1: "#006400",  # dark green
    2: "#00008B",  # dark blue
    3: "#FF8C00",  # dark orange
}

_HIGHLIGHT_COLORS_IMMEDIATE = {
    1: "#165416",
    2: "#212168",
    3: "#69400f",
}


# ---------------------------------------------------------------------------
# Helper: themed colour accessors (QSS property colours)
# ---------------------------------------------------------------------------

def _primary_bg():
    """Move name header background - slightly lighter than app background."""
    return _lighten_color(config.get_background_color(), 0.12)

def _primary_fg():
    return config.get_text_color()

def _contrast_bg():
    """Damage range area background - slightly lighter than app background."""
    return _lighten_color(config.get_background_color(), 0.06)

def _contrast_fg():
    return config.get_text_color()

def _secondary_bg():
    """Kill info area background - same as app background."""
    return config.get_background_color()

def _secondary_fg():
    return config.get_text_color()

def _header_bg():
    """Mon pair header background - slightly lighter than app background."""
    return _lighten_color(config.get_background_color(), 0.15)

def _lighten_color(hex_color, amount):
    r, g, b = _hex_to_rgb(hex_color)
    r = min(255, r + (255 - r) * amount)
    g = min(255, g + (255 - g) * amount)
    b = min(255, b + (255 - b) * amount)
    return _rgb_to_hex(r, g, b)


# ===================================================================
# BattleSummary -- top-level widget
# ===================================================================

class BattleSummary(QWidget):
    """PySide6 port of the Tkinter BattleSummary widget.

    Displays per-pokemon matchup grids with damage calculations,
    kill ranges, move highlighting, setup-move / weather / candy
    configuration, and screenshot capability.
    """

    def __init__(self, controller: BattleSummaryController, parent=None):
        super().__init__(parent)
        self._controller = controller
        self._loading = False

        # ---- outer layout with scroll area --------------------------------
        outer_layout = QVBoxLayout(self)
        outer_layout.setContentsMargins(0, 0, 0, 0)
        outer_layout.setSpacing(0)

        self._scroll_area = QScrollArea()
        self._scroll_area.setWidgetResizable(True)
        self._scroll_area.setFrameShape(QFrame.NoFrame)
        outer_layout.addWidget(self._scroll_area)

        # The scrollable content widget
        self._base_frame = QWidget()
        self._base_layout = QVBoxLayout(self._base_frame)
        self._base_layout.setContentsMargins(2, 2, 2, 2)
        self._base_layout.setSpacing(6)
        self._scroll_area.setWidget(self._base_frame)

        # ---- new controls bar (above legacy controls) ---------------------
        self._controls_bar = QWidget()
        controls_layout = QHBoxLayout(self._controls_bar)
        controls_layout.setContentsMargins(6, 2, 6, 2)
        controls_layout.setSpacing(10)

        # Unified "stepper" appearance: each [-] [center] [+] group renders as
        # a single rounded control. Visual structure is built from background
        # contrast (no outer border, since 1px QSS borders alias badly):
        # - center area uses a subtle dark fill
        # - buttons use a slightly lighter fill, giving them a clear affordance
        #   and acting as their own dividers from the center area
        self._controls_bar.setStyleSheet(
            """
            QWidget[class~="stepper-group"] {
                background-color: rgba(255, 255, 255, 0.04);
                border-radius: 6px;
            }
            QPushButton[class~="stepper-btn"] {
                background-color: rgba(255, 255, 255, 0.11);
                border: none;
                border-radius: 0px;
                padding: 0px;
                margin: 0px;
                font-weight: bold;
                font-size: 13pt;
                min-height: 0px;
                min-width: 0px;
                color: #cccccc;
            }
            QPushButton[class~="stepper-btn"]:hover {
                background-color: rgba(120, 170, 255, 0.45);
                color: #ffffff;
            }
            QPushButton[class~="stepper-btn"]:pressed {
                background-color: rgba(80, 130, 220, 0.60);
                color: #ffffff;
            }
            QPushButton[class~="stepper-btn-left"] {
                border-top-left-radius: 6px;
                border-bottom-left-radius: 6px;
            }
            QPushButton[class~="stepper-btn-right"] {
                border-top-right-radius: 6px;
                border-bottom-right-radius: 6px;
            }
            QWidget[class~="stepper-group"] QLabel {
                background-color: transparent;
                border: none;
            }
            """
        )

        # Prefight rare candy control: [-] [icon] [count] [+]
        candy_group = QWidget()
        candy_group.setProperty("class", "stepper-group")
        candy_group_layout = QHBoxLayout(candy_group)
        candy_group_layout.setContentsMargins(0, 0, 0, 0)
        candy_group_layout.setSpacing(0)

        # ---- candy debounce state ----------------------------------------
        # Mirrors the vitamin debounce pattern: clicks update the visible
        # label immediately, the actual (expensive) route mutation is
        # deferred until the user pauses.
        self._candy_displayed_count = 0
        self._candy_pending_target = None  # int when an apply is pending, else None
        self._candy_debounce_timer = QTimer(self)
        self._candy_debounce_timer.setSingleShot(True)
        self._candy_debounce_timer.setInterval(250)
        self._candy_debounce_timer.timeout.connect(self._flush_candy_adjustment)

        self._candy_minus_btn = QPushButton("\u2212")
        self._candy_minus_btn.setProperty("class", "stepper-btn stepper-btn-left")
        self._candy_minus_btn.setFixedSize(32, 26)
        self._candy_minus_btn.setFocusPolicy(Qt.NoFocus)
        self._candy_minus_btn.setToolTip("Decrement Pre-Fight Candies")
        self._candy_minus_btn.clicked.connect(lambda: self._on_candy_adjust(-1))
        candy_group_layout.addWidget(self._candy_minus_btn)

        self._candy_icon_label = QLabel()
        _candy_icon_path = os.path.join(
            const.SOURCE_ROOT_PATH, "icons", "filter icons", "TASK_RARE_CANDY.png"
        )
        if os.path.isfile(_candy_icon_path):
            _pix = QPixmap(_candy_icon_path)
            if not _pix.isNull():
                self._candy_icon_label.setPixmap(
                    _pix.scaled(20, 20, Qt.KeepAspectRatio, Qt.SmoothTransformation)
                )
        self._candy_icon_label.setToolTip("Pre-Fight Rare Candies")
        self._candy_icon_label.setContentsMargins(8, 0, 4, 0)
        candy_group_layout.addWidget(self._candy_icon_label)

        self._candy_count_label = QLabel("0")
        self._candy_count_label.setAlignment(Qt.AlignCenter)
        self._candy_count_label.setMinimumWidth(20)
        self._candy_count_label.setStyleSheet(
            "QLabel { color: #ffffff; font-weight: bold; font-size: 10pt; padding: 0px 8px 0px 4px; }"
        )
        candy_group_layout.addWidget(self._candy_count_label)

        self._candy_plus_btn = QPushButton("+")
        self._candy_plus_btn.setProperty("class", "stepper-btn stepper-btn-right")
        self._candy_plus_btn.setFixedSize(32, 26)
        self._candy_plus_btn.setFocusPolicy(Qt.NoFocus)
        self._candy_plus_btn.setToolTip("Increment Pre-Fight Candies")
        self._candy_plus_btn.clicked.connect(lambda: self._on_candy_adjust(+1))
        candy_group_layout.addWidget(self._candy_plus_btn)

        controls_layout.addWidget(candy_group)

        # Vitamin-per-stat indicators: [-] [label] [+] per stat.
        # Shows how many vitamins boosting that stat have been used before
        # the currently-loaded battle.
        self._vitamin_stat_widgets = {}  # stat_key -> (minus_btn, lbl, plus_btn, stat_label)
        self._vitamin_pending_deltas = {}  # stat_key -> accumulated delta (int)
        self._vitamin_debounce_timer = QTimer(self)
        self._vitamin_debounce_timer.setSingleShot(True)
        self._vitamin_debounce_timer.setInterval(300)
        self._vitamin_debounce_timer.timeout.connect(self._flush_vitamin_adjustments)

        _stat_display_order = [
            (const.HP,  "HP"),
            (const.ATK, "Atk"),
            (const.DEF, "Def"),
            (const.SPA, "SpA"),
            (const.SPD, "SpD"),
            (const.SPE, "Spe"),
        ]
        for stat_key, stat_label in _stat_display_order:
            stat_group = QWidget()
            stat_group.setProperty("class", "stepper-group")
            stat_group_layout = QHBoxLayout(stat_group)
            stat_group_layout.setContentsMargins(0, 0, 0, 0)
            stat_group_layout.setSpacing(0)

            minus_btn = QPushButton("\u2212")
            minus_btn.setProperty("class", "stepper-btn stepper-btn-left")
            minus_btn.setFixedSize(30, 26)
            minus_btn.setFocusPolicy(Qt.NoFocus)
            minus_btn.setToolTip(f"Remove a vitamin for {stat_label}")
            minus_btn.clicked.connect(
                lambda checked, s=stat_key: self._on_vitamin_adjust(s, -1)
            )
            stat_group_layout.addWidget(minus_btn)

            lbl = QLabel()
            lbl.setTextFormat(Qt.RichText)
            lbl.setText(self._format_vitamin_label(stat_label, 0))
            lbl.setProperty("count", 0)
            lbl.setToolTip(f"Vitamins boosting {stat_label} used before this battle")
            lbl.setAlignment(Qt.AlignCenter)
            lbl.setMinimumWidth(46)
            lbl.setStyleSheet("QLabel { padding: 0px 8px; }")
            stat_group_layout.addWidget(lbl)

            plus_btn = QPushButton("+")
            plus_btn.setProperty("class", "stepper-btn stepper-btn-right")
            plus_btn.setFixedSize(30, 26)
            plus_btn.setFocusPolicy(Qt.NoFocus)
            plus_btn.setToolTip(f"Add a vitamin for {stat_label}")
            plus_btn.clicked.connect(
                lambda checked, s=stat_key: self._on_vitamin_adjust(s, +1)
            )
            stat_group_layout.addWidget(plus_btn)

            controls_layout.addWidget(stat_group)

            self._vitamin_stat_widgets[stat_key] = (minus_btn, lbl, plus_btn, stat_label)

        # Small gap before held item / stat readouts
        _spacer2 = QLabel("")
        _spacer2.setFixedWidth(10)
        controls_layout.addWidget(_spacer2)

        # Typable held-item dropdown. Selecting (or typing + Enter / focus-out)
        # adds/updates a Hold event right before the battle.
        held_lbl = QLabel("Held:")
        held_lbl.setStyleSheet("QLabel { border: none; }")
        controls_layout.addWidget(held_lbl)

        self._held_item_combo = QComboBox()
        self._held_item_combo.setEditable(True)
        self._held_item_combo.setInsertPolicy(QComboBox.NoInsert)
        self._held_item_combo.setMinimumContentsLength(14)
        self._held_item_combo.setSizeAdjustPolicy(QComboBox.AdjustToMinimumContentsLengthWithIcon)
        self._held_item_combo.setToolTip(
            "Item the player is holding entering this battle. "
            "Changing this adds/updates a Hold event right before the battle."
        )
        self._held_item_completer = QCompleter([])
        self._held_item_completer.setCaseSensitivity(Qt.CaseInsensitive)
        self._held_item_completer.setFilterMode(Qt.MatchContains)
        self._held_item_combo.setCompleter(self._held_item_completer)
        self._held_item_combo.activated.connect(self._on_held_item_activated)
        self._held_item_combo.lineEdit().editingFinished.connect(self._on_held_item_editing_finished)
        self._held_item_options_cache = []
        controls_layout.addWidget(self._held_item_combo)

        # Player HP / Speed readouts (going into the battle)
        self._player_hp_label = QLabel("HP -")
        self._player_hp_label.setToolTip("Player HP entering this battle")
        self._player_hp_label.setStyleSheet(
            "QLabel { border: 1px solid rgba(255, 255, 255, 0.15);"
            " border-radius: 3px; padding: 1px 3px; }"
        )
        controls_layout.addWidget(self._player_hp_label)

        self._player_speed_label = QLabel("Spe -")
        self._player_speed_label.setToolTip("Player Speed entering this battle")
        self._player_speed_label.setStyleSheet(
            "QLabel { border: 1px solid rgba(255, 255, 255, 0.15);"
            " border-radius: 3px; padding: 1px 3px; }"
        )
        controls_layout.addWidget(self._player_speed_label)

        controls_layout.addStretch(1)

        self._base_layout.addWidget(self._controls_bar)

        # ---- legacy controls (collapsible, hidden by default) -------------
        self._legacy_section = QWidget()
        legacy_section_layout = QVBoxLayout(self._legacy_section)
        legacy_section_layout.setContentsMargins(0, 0, 0, 0)
        legacy_section_layout.setSpacing(0)

        self._legacy_header = QWidget()
        self._legacy_header.setCursor(Qt.PointingHandCursor)
        self._legacy_header.mousePressEvent = lambda e: self._toggle_legacy_controls()
        legacy_header_row = QHBoxLayout(self._legacy_header)
        legacy_header_row.setContentsMargins(6, 3, 6, 3)
        legacy_header_row.setSpacing(4)

        self._legacy_disclosure = DisclosureTriangle(size=14, color="#cccccc", parent=self._legacy_header)
        legacy_header_row.addWidget(self._legacy_disclosure)

        legacy_label = QLabel("Legacy Controls")
        legacy_label.setStyleSheet("QLabel { font-weight: bold; border: none; }")
        legacy_header_row.addWidget(legacy_label)
        legacy_header_row.addStretch(1)

        legacy_section_layout.addWidget(self._legacy_header)

        # ---- top bar (setup moves, weather, candy, config) ----------------
        self._top_bar = QWidget()
        top_bar_layout = QHBoxLayout(self._top_bar)
        top_bar_layout.setContentsMargins(0, 0, 0, 0)
        top_bar_layout.setSpacing(4)

        # Left half: setup moves + transform
        setup_half = QWidget()
        setup_layout = QVBoxLayout(setup_half)
        setup_layout.setContentsMargins(0, 0, 0, 0)
        setup_layout.setSpacing(2)

        # Player setup row
        player_setup_row = QWidget()
        player_setup_layout = QHBoxLayout(player_setup_row)
        player_setup_layout.setContentsMargins(0, 0, 0, 0)
        player_setup_layout.setSpacing(2)

        self.setup_moves = SetupMovesSummary(callback=self._player_setup_move_callback, is_player=True, parent=player_setup_row)
        player_setup_layout.addWidget(self.setup_moves, 1)

        self.transform_checkbox = CheckboxLabel(
            text="Transform:",
            toggle_command=self._player_transform_callback,
            flip=True,
            parent=player_setup_row,
        )
        player_setup_layout.addWidget(self.transform_checkbox, 0)

        self.held_item_label = QLabel("")
        self.held_item_label.setStyleSheet("QLabel { color: #aaaaaa; border: none; }")
        player_setup_layout.addWidget(self.held_item_label, 0)

        setup_layout.addWidget(player_setup_row)

        # Enemy setup row
        self.enemy_setup_moves = SetupMovesSummary(callback=self._enemy_setup_move_callback, is_player=False, parent=setup_half)
        setup_layout.addWidget(self.enemy_setup_moves)

        top_bar_layout.addWidget(setup_half, 1)

        # Right half: config button, double label, weather, candy
        weather_half = QWidget()
        weather_layout = QGridLayout(weather_half)
        weather_layout.setContentsMargins(0, 0, 0, 0)
        weather_layout.setSpacing(2)

        self.config_button = SimpleButton("Configure/Help", parent=weather_half)
        self.config_button.clicked.connect(self._launch_config_popup)
        weather_layout.addWidget(self.config_button, 0, 0)

        self.double_label = QLabel("Single Battle")
        weather_layout.addWidget(self.double_label, 1, 0)

        self.weather_status = WeatherSummary(callback=self._weather_callback, parent=weather_half)
        weather_layout.addWidget(self.weather_status, 0, 1)

        self.candy_summary = PrefightCandySummary(callback=self._candy_callback, parent=weather_half)
        weather_layout.addWidget(self.candy_summary, 1, 1)

        top_bar_layout.addWidget(weather_half, 0)

        legacy_section_layout.addWidget(self._top_bar)

        # Collapsed by default; top-bar starts hidden.
        self._legacy_expanded = False
        self._top_bar.setVisible(False)
        self._legacy_disclosure.set_expanded(False)

        self._base_layout.addWidget(self._legacy_section)

        # Honor the "Show Legacy Controls" toggle from config.
        self._legacy_section.setVisible(config.get_show_legacy_controls())

        # ---- mon pair slots (up to 6) ------------------------------------
        self._mon_pairs: List[MonPairSummary] = []
        self._did_draw_mon_pairs: List[bool] = []

        for idx in range(6):
            mp = MonPairSummary(self._controller, idx, parent=self._base_frame)
            mp.setVisible(False)
            self._mon_pairs.append(mp)
            self._base_layout.addWidget(mp)
            self._did_draw_mon_pairs.append(False)

        self._base_layout.addStretch(1)

        # ---- state --------------------------------------------------------
        self.should_render = False

        # Register for refresh callbacks from the controller
        self._unsubscribe_refresh = self._controller.register_refresh(self._on_full_refresh)

        # Initial load
        self.set_team(None)

    # ------------------------------------------------------------------
    # Public API called by main_window / event_details
    # ------------------------------------------------------------------

    def configure_weather(self, possible_weather_vals):
        self.weather_status.configure_weather(possible_weather_vals)

    def configure_setup_moves(self, possible_setup_moves):
        self.setup_moves.configure_moves(possible_setup_moves)
        self.enemy_setup_moves.configure_moves(possible_setup_moves)

    def refresh_test_move_options(self):
        all_moves = current_gen_info().move_db().get_filtered_names()
        all_moves.insert(0, "")
        for mp in self._mon_pairs:
            for slot in mp.test_move_slots:
                slot.set_test_move_options(all_moves)

    def _toggle_legacy_controls(self):
        self._legacy_expanded = not self._legacy_expanded
        self._top_bar.setVisible(self._legacy_expanded)
        self._legacy_disclosure.set_expanded(self._legacy_expanded)

    def set_legacy_controls_visible(self, visible: bool):
        """Show or completely remove the Legacy Controls section."""
        self._legacy_section.setVisible(bool(visible))

    # ------------------------------------------------------------------
    # Vitamin +/- with debounce
    # ------------------------------------------------------------------

    @staticmethod
    def _format_vitamin_label(stat_label: str, count: int) -> str:
        """Render the vitamin label as faded stat name + bright value."""
        return (
            f"<span style='color:#888888;'>{stat_label}</span>"
            f"&nbsp;&nbsp;<span style='color:#ffffff;font-weight:bold;'>{int(count)}</span>"
        )

    def _on_vitamin_adjust(self, stat, delta):
        """Called on each +/- click. Updates the label immediately and
        accumulates the delta.  The actual route modification is deferred
        until the debounce timer fires."""
        self._vitamin_pending_deltas[stat] = self._vitamin_pending_deltas.get(stat, 0) + delta

        # Instantly update the label so the UI feels responsive.
        widgets = self._vitamin_stat_widgets.get(stat)
        if widgets:
            _minus_btn, lbl, _plus_btn, stat_label = widgets
            cur_count = lbl.property("count") or 0
            new_count = max(0, int(cur_count) + delta)
            lbl.setProperty("count", new_count)
            lbl.setText(self._format_vitamin_label(stat_label, new_count))

        # Restart the debounce timer.
        self._vitamin_debounce_timer.start()

    def _flush_vitamin_adjustments(self):
        """Apply all accumulated vitamin deltas to the route in one batch."""
        pending = self._vitamin_pending_deltas.copy()
        self._vitamin_pending_deltas.clear()
        items = [(s, d) for s, d in pending.items() if d != 0]
        if not items:
            return
        for stat, delta in items:
            self._controller.adjust_vitamin_for_stat(stat, delta, _skip_refresh=True)
        # Single refresh after all adjustments are applied.
        self._controller._full_refresh()

    def hide_contents(self):
        self.should_render = False

    def show_contents(self):
        self.should_render = True
        self._on_full_refresh()

    def set_team(
        self,
        enemy_pkmn: List[universal_data_objects.EnemyPkmn],
        cur_state: full_route_state.RouteState = None,
        event_group: route_events.EventGroup = None,
        is_wild: bool = False,
    ):
        if event_group is not None:
            self._controller.load_from_event(event_group)
        elif cur_state is not None and enemy_pkmn is not None:
            self._controller.load_from_state(cur_state, enemy_pkmn, is_wild=is_wild)
        else:
            self._controller.load_empty()

    # ------------------------------------------------------------------
    # Screenshot helpers
    # ------------------------------------------------------------------

    def _save_pixmap(self, pixmap: QPixmap, suffix: str):
        date_prefix = datetime.now().strftime("%Y%m%d%H%M%S")
        save_dir = config.get_images_dir()
        try:
            from controllers.main_controller import MainController
            route_name = self._controller._main_controller.get_current_route_name()
        except Exception:
            route_name = "battle_summary"
        out_path = io_utils.get_safe_path_no_collision(
            save_dir,
            f"{date_prefix}-{route_name}_{suffix}",
            ext=".png",
        )
        os.makedirs(os.path.dirname(out_path), exist_ok=True)
        pixmap.save(out_path)
        try:
            self._controller._main_controller.send_message(f"Saved screenshot to: {out_path}")
        except Exception:
            pass

    def _grab_transparent(self, widget):
        """Render *widget* into a QPixmap with a transparent background.

        The widget and _base_frame backgrounds are temporarily made
        transparent so that individual move/mon-pair frames keep their
        own backgrounds while the overall canvas is see-through — ideal
        for compositing into video footage.
        """
        saved_widget = widget.styleSheet()
        saved_base = self._base_frame.styleSheet()
        widget.setStyleSheet("background: transparent;")
        self._base_frame.setStyleSheet("background: transparent;")

        pixmap = QPixmap(widget.size())
        pixmap.fill(Qt.transparent)
        widget.render(pixmap)

        widget.setStyleSheet(saved_widget)
        self._base_frame.setStyleSheet(saved_base)
        return pixmap

    def _hide_defaults_for_screenshot(self):
        """Hide default-value dropdowns and unchecked toggles in all visible
        damage summaries. Returns a list of widgets to re-show afterwards.

        "Default" means the first option (index 0) — e.g., "No Bonus" for
        custom-data dropdowns or "0" for stat-stage dropdowns. Forces a
        synchronous relayout (on each affected header layout, and the base
        frame) so the rendered pixmap reflects the tighter spacing — the
        move name re-centers within the freed space.
        """
        restore = []
        for idx, mp in enumerate(self._mon_pairs):
            if not (self._did_draw_mon_pairs[idx] and mp.isVisible()):
                continue
            for ds in list(mp.move_list) + list(mp.test_move_slots):
                if not ds.isVisible():
                    continue
                candidates = (
                    (ds.custom_data_dropdown,
                     ds.custom_data_dropdown.isVisible() and ds.custom_data_dropdown.currentIndex() == 0),
                    (ds.stat_stage_dropdown,
                     ds.stat_stage_dropdown.isVisible() and ds.stat_stage_dropdown.currentIndex() == 0),
                    (ds.weather_checkbox,
                     ds.weather_checkbox.isVisible() and not ds.weather_checkbox.isChecked()),
                    (ds.screen_checkbox,
                     ds.screen_checkbox.isVisible() and not ds.screen_checkbox.isChecked()),
                )
                for widget, should_hide in candidates:
                    if should_hide:
                        widget.setVisible(False)
                        restore.append(widget)
        if restore:
            self._reactivate_layouts_for(restore)
        return restore

    def _restore_after_screenshot(self, restore):
        if not restore:
            return
        for widget in restore:
            widget.setVisible(True)
        self._reactivate_layouts_for(restore)

    def _reactivate_layouts_for(self, widgets):
        """Synchronously re-run the header layouts that own *widgets*, then the
        base frame layout, so a subsequent render() reflects the new geometry.
        """
        headers = {w.parent() for w in widgets if w.parent() is not None}
        for header in headers:
            layout = header.layout()
            if layout is not None:
                layout.activate()
        self._base_frame.layout().activate()

    def take_battle_summary_screenshot(self):
        restore = self._hide_defaults_for_screenshot()
        try:
            pixmap = self._grab_transparent(self._base_frame)
            # Crop tightly to just the mon-pair grids (damage ranges only); exclude
            # the controls bar (rare candy, vitamins, held item, HP/Spe) above.
            mon_rect = self._get_mon_pairs_rect()
            if mon_rect[0] is not None:
                y_top, y_bottom = mon_rect
                cropped = pixmap.copy(0, y_top, pixmap.width(), y_bottom - y_top)
                self._save_pixmap(cropped, "battle_summary")
            else:
                self._save_pixmap(pixmap, "battle_summary")
        finally:
            self._restore_after_screenshot(restore)

    def _get_divider_x_in_base_frame(self):
        """Return (left_edge_x, right_edge_x) of the divider relative to _base_frame.

        Uses the first visible MonPairSummary's divider widget to determine the
        actual split point.  Returns None if no visible mon-pair is found.
        """
        for idx, mp in enumerate(self._mon_pairs):
            if self._did_draw_mon_pairs[idx] and mp.isVisible():
                divider = mp.divider
                pos = divider.mapTo(self._base_frame, divider.rect().topLeft())
                left_x = pos.x()
                right_x = left_x + divider.width()
                return (left_x, right_x)
        return None

    def _get_mon_pairs_rect(self):
        """Return (y_top, y_bottom) of the visible mon-pair area relative to _base_frame.

        Excludes the top bar and any bottom stretch, matching the reference
        implementation which crops tightly to just the mon-pair grids.
        """
        top = None
        bottom = None
        for idx in range(6):
            if self._did_draw_mon_pairs[idx] and self._mon_pairs[idx].isVisible():
                mp = self._mon_pairs[idx]
                pos = mp.mapTo(self._base_frame, mp.rect().topLeft())
                mp_top = pos.y()
                mp_bottom = mp_top + mp.height()
                if top is None or mp_top < top:
                    top = mp_top
                if bottom is None or mp_bottom > bottom:
                    bottom = mp_bottom
        return (top, bottom)

    def _get_visible_mon_pair_rects(self):
        """Return [(x, y, w, h), ...] for each visible MonPairSummary in _base_frame coords."""
        rects = []
        for idx in range(6):
            if self._did_draw_mon_pairs[idx] and self._mon_pairs[idx].isVisible():
                mp = self._mon_pairs[idx]
                pos = mp.mapTo(self._base_frame, mp.rect().topLeft())
                rects.append((pos.x(), pos.y(), mp.width(), mp.height()))
        return rects

    def _round_container_corners(self, pixmap: QPixmap, container_rects, radius: int = 6):
        """Erase the four corner triangles of each container so the cut edge of a
        cropped half-screenshot picks up the same border-radius the full widget had.
        Corners that were already rounded (left side) are unaffected since those
        pixels are already transparent."""
        if not container_rects:
            return pixmap
        result = QPixmap(pixmap.size())
        result.fill(Qt.transparent)
        painter = QPainter(result)
        painter.setRenderHint(QPainter.Antialiasing)
        painter.drawPixmap(0, 0, pixmap)
        painter.setCompositionMode(QPainter.CompositionMode_DestinationOut)
        for x, y, w, h in container_rects:
            full_path = QPainterPath()
            full_path.addRect(QRectF(x, y, w, h))
            rounded_path = QPainterPath()
            rounded_path.addRoundedRect(QRectF(x, y, w, h), radius, radius)
            painter.fillPath(full_path.subtracted(rounded_path), Qt.black)
        painter.end()
        return result

    def take_player_ranges_screenshot(self):
        """Capture only the left (player) half of the mon-pair grids."""
        restore = self._hide_defaults_for_screenshot()
        try:
            pixmap = self._grab_transparent(self._base_frame)
            divider_pos = self._get_divider_x_in_base_frame()
            mon_rect = self._get_mon_pairs_rect()
            if divider_pos is not None:
                split_x = divider_pos[0]
            else:
                split_x = pixmap.width() // 2
            if mon_rect[0] is not None:
                y_top, y_bottom = mon_rect
            else:
                y_top, y_bottom = 0, pixmap.height()
            cropped = pixmap.copy(0, y_top, split_x, y_bottom - y_top)
            cropped_rects = []
            for x, y, w, h in self._get_visible_mon_pair_rects():
                cw = min(w, split_x - x)
                if cw > 0:
                    cropped_rects.append((x, y - y_top, cw, h))
            cropped = self._round_container_corners(cropped, cropped_rects)
            self._save_pixmap(cropped, "player_ranges")
        finally:
            self._restore_after_screenshot(restore)

    def take_enemy_ranges_screenshot(self):
        """Capture only the right (enemy) half of the mon-pair grids."""
        restore = self._hide_defaults_for_screenshot()
        try:
            pixmap = self._grab_transparent(self._base_frame)
            w = pixmap.width()
            divider_pos = self._get_divider_x_in_base_frame()
            mon_rect = self._get_mon_pairs_rect()
            if divider_pos is not None:
                split_x = divider_pos[1]
            else:
                split_x = w // 2
            if mon_rect[0] is not None:
                y_top, y_bottom = mon_rect
            else:
                y_top, y_bottom = 0, pixmap.height()
            cropped = pixmap.copy(split_x, y_top, w - split_x, y_bottom - y_top)
            cropped_rects = []
            for mp_x, mp_y, mp_w, mp_h in self._get_visible_mon_pair_rects():
                container_right = mp_x + mp_w
                if container_right <= split_x:
                    continue
                new_x = max(0, mp_x - split_x)
                new_w = container_right - split_x - new_x
                if new_w > 0:
                    cropped_rects.append((new_x, mp_y - y_top, new_w, mp_h))
            cropped = self._round_container_corners(cropped, cropped_rects)
            self._save_pixmap(cropped, "enemy_ranges")
        finally:
            self._restore_after_screenshot(restore)

    def _increment_prefight_candies(self):
        if self._candy_plus_btn.isEnabled():
            self._on_candy_adjust(+1)

    def _decrement_prefight_candies(self):
        if self._candy_minus_btn.isEnabled() and self._candy_displayed_count > 0:
            self._on_candy_adjust(-1)

    # ------------------------------------------------------------------
    # Candy +/- with debounce (mirrors vitamin pattern)
    # ------------------------------------------------------------------

    def _on_candy_adjust(self, delta):
        """Called on each candy +/- click. Updates the label immediately and
        accumulates the target value. The actual route mutation is deferred
        until the debounce timer fires so rapid clicks coalesce into one
        expensive recalculation."""
        # Seed the displayed count from the controller on first click of a
        # new burst — keeps us in sync if the route was changed externally.
        if self._candy_pending_target is None:
            try:
                self._candy_displayed_count = int(self._controller.get_prefight_candy_count())
            except Exception:
                self._candy_displayed_count = 0

        new_count = max(0, self._candy_displayed_count + delta)
        if new_count == self._candy_displayed_count and self._candy_pending_target is None:
            return

        self._candy_displayed_count = new_count
        self._candy_pending_target = new_count

        # Instant UI feedback.
        self._candy_count_label.setText(str(new_count))
        self._candy_minus_btn.setEnabled(new_count > 0)
        # Mirror to the legacy widget so the two stay consistent if visible.
        try:
            self.candy_summary.set_candy_count(new_count)
        except Exception:
            pass

        # Restart the debounce timer.
        self._candy_debounce_timer.start()

    def _flush_candy_adjustment(self):
        """Apply the pending candy target to the route in one batch."""
        target = self._candy_pending_target
        self._candy_pending_target = None
        if target is None:
            return
        self._controller.update_prefight_candies(target)

    def _update_notes_visibility_in_battle_summary(self):
        # placeholder -- notes visibility handled at main_window level in Qt
        pass

    # ------------------------------------------------------------------
    # Internal callbacks
    # ------------------------------------------------------------------

    def _launch_config_popup(self, *args, **kwargs):
        from gui_qt.dialogs import BattleConfigDialog
        dlg = BattleConfigDialog(self)
        dlg.exec()
        self._on_full_refresh()

    def _weather_callback(self, *args, **kwargs):
        self._controller.update_weather(self.weather_status.get_weather())

    def _candy_callback(self, *args, **kwargs):
        self._controller.update_prefight_candies(self.candy_summary.get_prefight_candy_count())

    def _player_setup_move_callback(self, *args, **kwargs):
        self._controller.update_player_setup_moves(self.setup_moves._move_list.copy())

    def _player_transform_callback(self, *args, **kwargs):
        if not self._loading:
            self._controller.update_player_transform(self.transform_checkbox.is_checked())

    def _enemy_setup_move_callback(self, *args, **kwargs):
        self._controller.update_enemy_setup_moves(self.enemy_setup_moves._move_list.copy())

    def _on_held_item_activated(self, *args, **kwargs):
        # Fired when the user picks an item from the dropdown list.
        if self._loading:
            return
        self._apply_held_item_change()

    def _on_held_item_editing_finished(self, *args, **kwargs):
        # Fired when the line edit loses focus or the user presses Enter.
        if self._loading:
            return
        self._apply_held_item_change()

    def _apply_held_item_change(self):
        # Re-entry guard: a single dropdown selection can fire both `activated`
        # and `editingFinished`, and the second may arrive while the first is
        # still inside the route-change cascade.
        if getattr(self, "_held_item_apply_in_flight", False):
            return

        new_text = self._held_item_combo.currentText().strip()
        if new_text and new_text not in self._held_item_options_cache:
            # Reject anything that isn't a real item; revert the visible value
            # to whatever the controller currently believes is held.
            self._loading = True
            try:
                cur = self._controller.get_player_held_item() or ""
                self._held_item_combo.setEditText(cur)
            finally:
                self._loading = False
            return

        # Skip if the value already matches what the controller reports.
        cur_held = self._controller.get_player_held_item() or ""
        if cur_held == new_text:
            return

        self._held_item_apply_in_flight = True
        try:
            self._controller.update_player_held_item(new_text)
        finally:
            self._held_item_apply_in_flight = False

    # ------------------------------------------------------------------
    # Full refresh (called by controller)
    # ------------------------------------------------------------------

    def _on_full_refresh(self, *args, **kwargs):
        if not self.should_render:
            return

        # Suppress intermediate repaints while updating many child widgets.
        self._base_frame.setUpdatesEnabled(False)
        try:
            self._loading = True
            cur_candies = self._controller.get_prefight_candy_count()
            can_candies = self._controller.can_support_prefight_candies()
            if not can_candies:
                self.candy_summary.disable()
            else:
                self.candy_summary.enable()
            # Mirror into the controls-bar candy control. If a debounced
            # candy update is in flight, keep showing the user's intended
            # value so the label doesn't snap back during the cascade.
            if self._candy_pending_target is None:
                self.candy_summary.set_candy_count(cur_candies)
                self._candy_count_label.setText(str(cur_candies))
                self._candy_displayed_count = cur_candies
                self._candy_minus_btn.setEnabled(can_candies and cur_candies > 0)
            else:
                self._candy_minus_btn.setEnabled(can_candies and self._candy_displayed_count > 0)
            self._candy_plus_btn.setEnabled(can_candies)
            # Vitamin-per-stat indicators
            vit_counts = self._controller.get_vitamins_used_per_stat()
            for stat_key, (minus_btn, lbl, plus_btn, stat_label) in self._vitamin_stat_widgets.items():
                count = vit_counts.get(stat_key, 0)
                lbl.setProperty("count", count)
                lbl.setText(self._format_vitamin_label(stat_label, count))

            if self._controller.is_double_battle():
                self.double_label.setText("Double Battle")
            else:
                self.double_label.setText("Single Battle")

            self.transform_checkbox.set_checked(self._controller.is_player_transformed())
            held = self._controller.get_player_held_item()
            self.held_item_label.setText(f"Held: {held}" if held else "")

            # Controls-bar held item dropdown + HP/Speed readouts
            options = self._controller.get_held_item_options()
            if options != self._held_item_options_cache:
                self._held_item_options_cache = options
                self._held_item_combo.blockSignals(True)
                try:
                    self._held_item_combo.clear()
                    self._held_item_combo.addItems(options)
                finally:
                    self._held_item_combo.blockSignals(False)
                self._held_item_completer.setModel(self._held_item_combo.model())
            self._held_item_combo.blockSignals(True)
            try:
                self._held_item_combo.setEditText(held or "")
            finally:
                self._held_item_combo.blockSignals(False)
            self._held_item_combo.setEnabled(can_candies)

            self._player_hp_label.setText(f"HP {self._controller.get_player_battle_hp()}")
            self._player_speed_label.setText(f"Spe {self._controller.get_player_battle_speed()}")
            self.weather_status.set_weather(self._controller.get_weather())
            self.setup_moves.set_move_list(self._controller.get_player_setup_moves())
            self.enemy_setup_moves.set_move_list(self._controller.get_enemy_setup_moves())

            for idx in range(6):
                player_info = self._controller.get_pkmn_info(idx, True)
                enemy_info = self._controller.get_pkmn_info(idx, False)

                if player_info is None and enemy_info is None:
                    if self._did_draw_mon_pairs[idx]:
                        self._mon_pairs[idx].setVisible(False)
                        self._did_draw_mon_pairs[idx] = False
                else:
                    if not self._did_draw_mon_pairs[idx]:
                        self._mon_pairs[idx].setVisible(True)
                        self._did_draw_mon_pairs[idx] = True
                    self._mon_pairs[idx].update_rendering()

            self._loading = False
        finally:
            self._base_frame.setUpdatesEnabled(True)

    # ------------------------------------------------------------------
    # Bounding-box helpers (used by screenshot cropping in main_window)
    # ------------------------------------------------------------------

    def get_content_bounding_box(self):
        rect = self._base_frame.rect()
        top_left = self._base_frame.mapToGlobal(rect.topLeft())
        bottom_right = self._base_frame.mapToGlobal(rect.bottomRight())
        return (top_left.x(), top_left.y(), bottom_right.x(), bottom_right.y())

    def get_player_ranges_bounding_box(self):
        rect = self._base_frame.rect()
        top_left = self._base_frame.mapToGlobal(rect.topLeft())
        bottom_right = self._base_frame.mapToGlobal(rect.bottomRight())
        # Use actual divider position when available
        divider_x = self._get_divider_global_x()
        if divider_x is not None:
            return (top_left.x(), top_left.y(), divider_x[0], bottom_right.y())
        mid_x = (top_left.x() + bottom_right.x()) // 2
        return (top_left.x(), top_left.y(), mid_x, bottom_right.y())

    def get_enemy_ranges_bounding_box(self):
        rect = self._base_frame.rect()
        top_left = self._base_frame.mapToGlobal(rect.topLeft())
        bottom_right = self._base_frame.mapToGlobal(rect.bottomRight())
        # Use actual divider position when available
        divider_x = self._get_divider_global_x()
        if divider_x is not None:
            return (divider_x[1], top_left.y(), bottom_right.x(), bottom_right.y())
        mid_x = (top_left.x() + bottom_right.x()) // 2
        return (mid_x, top_left.y(), bottom_right.x(), bottom_right.y())

    def _get_divider_global_x(self):
        """Return (left_edge_global_x, right_edge_global_x) of the divider in global coords.

        Uses the first visible MonPairSummary's divider widget.
        Returns None if no visible mon-pair is found.
        """
        for idx, mp in enumerate(self._mon_pairs):
            if self._did_draw_mon_pairs[idx] and mp.isVisible():
                divider = mp.divider
                top_left = divider.mapToGlobal(divider.rect().topLeft())
                top_right = divider.mapToGlobal(divider.rect().topRight())
                return (top_left.x(), top_right.x())
        return None


# ===================================================================
# SetupMovesSummary
# ===================================================================

class SetupMovesSummary(QWidget):
    def __init__(self, callback=None, is_player=True, parent=None):
        super().__init__(parent)
        self._callback = callback
        self._move_list: List[str] = []

        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(2)

        self.reset_button = SimpleButton("Reset Setup", parent=self)
        self.reset_button.clicked.connect(self._reset)
        layout.addWidget(self.reset_button)

        self.setup_label = QLabel("Move:")
        layout.addWidget(self.setup_label)

        self.setup_moves = SimpleOptionMenu(option_list=["N/A"], parent=self)
        layout.addWidget(self.setup_moves)

        self.add_button = SimpleButton("Apply Move", parent=self)
        self.add_button.clicked.connect(self._add_setup_move)
        layout.addWidget(self.add_button)

        label_text = "Player Setup:" if is_player else "Enemy Setup:"
        self.extra_label = QLabel(label_text)
        layout.addWidget(self.extra_label)

        self.move_list_label = QLabel("")
        layout.addWidget(self.move_list_label)

        layout.addStretch(1)

    def _reset(self, *args, **kwargs):
        self._move_list = []
        self._move_list_updated()

    def _add_setup_move(self, *args, **kwargs):
        self._move_list.append(self.setup_moves.get())
        self._move_list_updated()

    def configure_moves(self, new_moves):
        self.setup_moves.new_values(new_moves)

    def set_move_list(self, new_moves, trigger_update=False):
        self._move_list = new_moves
        self._move_list_updated(trigger_update=trigger_update)

    def get_stage_modifiers(self):
        result = universal_data_objects.StageModifiers()
        for cur_move in self._move_list:
            result = result.apply_stat_mod(current_gen_info().move_db().get_stat_mod(cur_move))
        return result

    def _move_list_updated(self, trigger_update=True):
        to_display = ", ".join(self._move_list)
        if not to_display:
            to_display = "None"
        self.move_list_label.setText(to_display)
        if self._callback is not None and trigger_update:
            self._callback()


# ===================================================================
# WeatherSummary
# ===================================================================

class WeatherSummary(QWidget):
    def __init__(self, callback=None, parent=None):
        super().__init__(parent)
        self._outer_callback = callback
        self._loading = False

        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(2)

        self.label = QLabel("Weather:")
        layout.addWidget(self.label)

        self.weather_dropdown = SimpleOptionMenu(
            option_list=[const.WEATHER_NONE],
            callback=self._callback,
            parent=self,
        )
        layout.addWidget(self.weather_dropdown)

    def _callback(self, *args, **kwargs):
        if self._loading:
            return
        if self._outer_callback is not None:
            self._outer_callback()

    def set_weather(self, new_weather):
        self._loading = True
        self.weather_dropdown.set(new_weather)
        self._loading = False

    def configure_weather(self, weather_vals):
        self.weather_dropdown.new_values(weather_vals)

    def get_weather(self):
        return self.weather_dropdown.get()


# ===================================================================
# PrefightCandySummary
# ===================================================================

class PrefightCandySummary(QWidget):
    def __init__(self, callback=None, parent=None):
        super().__init__(parent)
        self._outer_callback = callback
        self._loading = False
        self._candy_callback_timer = None

        layout = QHBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(2)

        self.label = QLabel("Prefight Candies:")
        layout.addWidget(self.label)

        self.candy_count = AmountEntry(
            callback=self._callback,
            min_val=0,
            init_val=0,
            width=5,
            parent=self,
        )
        layout.addWidget(self.candy_count)

    def _callback(self, *args, **kwargs):
        if self._loading:
            return
        # Debounce: schedule the callback after a short delay
        if self._candy_callback_timer is not None:
            self._candy_callback_timer.stop()
        if self._outer_callback is not None:
            self._candy_callback_timer = QTimer(self)
            self._candy_callback_timer.setSingleShot(True)
            self._candy_callback_timer.timeout.connect(self._delayed_candy_callback)
            self._candy_callback_timer.start(150)

    def _delayed_candy_callback(self):
        self._candy_callback_timer = None
        # NOTE: do NOT call QApplication.processEvents() here. Re-entering
        # the Qt event loop while the parent BattleSummary is mid-rebuild
        # can momentarily flash a child widget as a top-level window
        # (mirrors the QCompleter popup-flash issue handled in
        # EditableOptionMenu.set). The 150ms debounce above is sufficient
        # to coalesce rapid clicks on its own.
        if self._outer_callback is not None:
            self._outer_callback()

    def _increment_candy(self, event=None):
        if not self._loading:
            self._loading = True
            self.candy_count._raise_amt()
            self._loading = False
            self._callback()

    def _decrement_candy(self, event=None):
        if not self._loading:
            self._loading = True
            self.candy_count._lower_amt()
            self._loading = False
            self._callback()

    def _fire_candy_callback(self):
        """Cancel any pending debounce and fire the callback immediately."""
        if self._candy_callback_timer is not None:
            self._candy_callback_timer.stop()
            self._candy_callback_timer = None
        if self._outer_callback is not None:
            self._outer_callback()

    def set_candy_count(self, new_amount):
        self._loading = True
        self.candy_count.set(new_amount)
        self._loading = False

    def get_prefight_candy_count(self):
        try:
            return int(self.candy_count.get())
        except Exception:
            return 0

    def disable(self):
        self.candy_count.disable()

    def enable(self):
        self.candy_count.enable()



# ===================================================================
# MonPairSummary -- one row per enemy pokemon matchup
# ===================================================================

class MonPairSummary(QWidget):
    def __init__(self, controller: BattleSummaryController, mon_idx: int, parent=None):
        super().__init__(parent)
        self._controller = controller
        self._mon_idx = mon_idx
        self._expanded = True

        # Outer frame with a darker grey fill to separate each matchup visually.
        # WA_StyledBackground is required for QSS backgrounds to paint on a QWidget subclass.
        self.setAttribute(Qt.WA_StyledBackground, True)
        _matchup_bg = _darken(config.get_background_color(), 0.35)
        self.setStyleSheet(
            f"MonPairSummary {{ background-color: {_matchup_bg}; border-radius: 6px; }}"
            "QWidget#matchupHeader { background-color: transparent; border: none; }"
            "QWidget#matchupContent { background-color: transparent; }"
        )

        outer_layout = QVBoxLayout(self)
        outer_layout.setContentsMargins(2, 2, 2, 2)
        outer_layout.setSpacing(0)

        # ---- clickable header row -----------------------------------------
        self._header_widget = QWidget()
        self._header_widget.setObjectName("matchupHeader")
        self._header_widget.setCursor(Qt.PointingHandCursor)
        self._header_widget.mousePressEvent = lambda e: self._toggle_expanded()
        header_grid = QGridLayout(self._header_widget)
        header_grid.setContentsMargins(6, 3, 6, 3)
        header_grid.setHorizontalSpacing(0)
        header_grid.setVerticalSpacing(0)

        # Mirror the content's column structure so headers center over their move columns
        for col in range(4):
            header_grid.setColumnStretch(col, 1)
        header_grid.setColumnStretch(4, 0)
        header_grid.setColumnMinimumWidth(4, 12)
        for col in range(5, 9):
            header_grid.setColumnStretch(col, 1)

        header_font = QFont()
        header_font.setBold(True)

        self._player_header = QLabel("")
        self._player_header.setAlignment(Qt.AlignVCenter)
        self._player_header.setStyleSheet(
            "QLabel { font-weight: bold; border: none; }"
        )
        self._player_header.setFont(header_font)

        self._enemy_header = QLabel("")
        self._enemy_header.setAlignment(Qt.AlignVCenter)
        self._enemy_header.setStyleSheet(
            "QLabel { font-weight: bold; border: none; }"
        )
        self._enemy_header.setFont(header_font)

        self._player_icon = QLabel()
        self._player_icon.setFixedSize(28, 28)
        self._player_icon.setStyleSheet("border: none;")

        self._enemy_icon = QLabel()
        self._enemy_icon.setFixedSize(28, 28)
        self._enemy_icon.setStyleSheet("border: none;")

        # Player icon+title centered over cols 0-3
        player_section = QWidget()
        player_section.setStyleSheet("background: transparent; border: none;")
        player_section_layout = QHBoxLayout(player_section)
        player_section_layout.setContentsMargins(0, 0, 0, 0)
        player_section_layout.setSpacing(4)
        player_section_layout.addWidget(self._player_icon)
        player_section_layout.addWidget(self._player_header)
        header_grid.addWidget(player_section, 0, 0, 1, 4, Qt.AlignCenter)

        # Enemy icon+title centered over cols 5-8
        enemy_section = QWidget()
        enemy_section.setStyleSheet("background: transparent; border: none;")
        enemy_section_layout = QHBoxLayout(enemy_section)
        enemy_section_layout.setContentsMargins(0, 0, 0, 0)
        enemy_section_layout.setSpacing(4)
        enemy_section_layout.addWidget(self._enemy_icon)
        enemy_section_layout.addWidget(self._enemy_header)
        header_grid.addWidget(enemy_section, 0, 5, 1, 4, Qt.AlignCenter)

        # Chevron pinned to the top-left corner so it doesn't push the title off-center
        self._disclosure = DisclosureTriangle(size=14, color="#cccccc", parent=self._header_widget)
        header_grid.addWidget(self._disclosure, 0, 0, Qt.AlignLeft | Qt.AlignVCenter)
        self._disclosure.raise_()

        outer_layout.addWidget(self._header_widget)

        # ---- collapsible content area -------------------------------------
        self._content = QWidget()
        self._content.setObjectName("matchupContent")
        content_layout = QGridLayout(self._content)
        content_layout.setContentsMargins(1, 1, 1, 1)
        content_layout.setSpacing(0)

        # Configure column stretches so both halves share equal space
        for col in range(4):
            content_layout.setColumnStretch(col, 1)
        content_layout.setColumnStretch(4, 0)  # divider column
        content_layout.setColumnMinimumWidth(4, 12)
        for col in range(5, 9):
            content_layout.setColumnStretch(col, 1)

        # Divider widget between player (cols 0-3) and enemy (cols 5-8)
        self.divider = QFrame(self._content)
        self.divider.setFixedWidth(2)
        self.divider.setStyleSheet(
            f"background-color: {_darken(config.get_divider_color(), 0.3)};"
        )
        self.divider.setSizePolicy(QSizePolicy.Fixed, QSizePolicy.Expanding)
        content_layout.addWidget(self.divider, 0, 4, -1, 1, Qt.AlignCenter)

        outer_layout.addWidget(self._content)

        # ---- move slots ---------------------------------------------------
        # 8 regular: 4 player (cols 0-3) + 4 enemy (cols 5-8)
        self.move_list: List[DamageSummary] = []
        self._did_draw: List[bool] = []
        for cur_idx in range(8):
            ds = DamageSummary(
                self._controller,
                self._mon_idx,
                cur_idx % 4,
                cur_idx < 4,
                parent=self._content,
            )
            ds.setVisible(False)
            self.move_list.append(ds)
            self._did_draw.append(False)

        # 4 test-move slots (player moves 5-8, displayed in cols 5-8)
        self.test_move_slots: List[DamageSummary] = []
        self._did_draw_test_moves: List[bool] = []
        for slot_idx in range(4):
            ds = DamageSummary(
                self._controller,
                self._mon_idx,
                4 + slot_idx,
                True,
                parent=self._content,
                is_test_move=True,
            )
            ds.setVisible(False)
            self.test_move_slots.append(ds)
            self._did_draw_test_moves.append(False)

    def _toggle_expanded(self):
        self._expanded = not self._expanded
        self._content.setVisible(self._expanded)
        self._disclosure.set_expanded(self._expanded)
        if hasattr(self._controller, "update_mon_collapsed"):
            self._controller.update_mon_collapsed(self._mon_idx, not self._expanded)

    def _sync_expanded_from_controller(self):
        if not hasattr(self._controller, "is_mon_collapsed"):
            return
        expanded = not self._controller.is_mon_collapsed(self._mon_idx)
        if expanded == self._expanded:
            return
        self._expanded = expanded
        self._content.setVisible(self._expanded)
        self._disclosure.set_expanded(self._expanded)

    def _update_header_text(self):
        player_info = self._controller.get_pkmn_info(self._mon_idx, True)
        enemy_info = self._controller.get_pkmn_info(self._mon_idx, False)

        if player_info is None or enemy_info is None:
            self._player_header.setText("")
            self._enemy_header.setText("")
            self._player_icon.clear()
            self._enemy_icon.clear()
            return

        # Set player Pokemon icon
        player_icon_pm = pkmn_icon.get_icon(player_info.attacking_mon_name, size=28)
        if player_icon_pm is not None:
            self._player_icon.setPixmap(player_icon_pm)
            self._player_icon.setVisible(True)
        else:
            self._player_icon.setVisible(False)

        # Set enemy Pokemon icon
        icon_pm = pkmn_icon.get_icon(enemy_info.attacking_mon_name, size=28)
        if icon_pm is not None:
            self._enemy_icon.setPixmap(icon_pm)
            self._enemy_icon.setVisible(True)
        else:
            self._enemy_icon.setVisible(False)

        self._disclosure.set_expanded(self._expanded)

        player_text = f"{player_info.attacking_mon_name} Lv{player_info.attacking_mon_level} Damage Ranges"
        enemy_text = f"{enemy_info.attacking_mon_name} Lv{enemy_info.attacking_mon_level} Damage Ranges"

        # Color matchup text based on speed comparison
        if player_info.attacking_mon_speed > player_info.defending_mon_speed:
            text_color = "#3498db"  # Blue - player outspeeds
        elif player_info.attacking_mon_speed == player_info.defending_mon_speed:
            text_color = "#f1c40f"  # Yellow - speed tie
        else:
            text_color = "#e74c3c"  # Red - player underspeeds

        self._player_header.setTextFormat(Qt.RichText)
        self._player_header.setText(
            f'<span style="color:{text_color}">{player_text}</span>'
        )
        self._enemy_header.setTextFormat(Qt.RichText)
        self._enemy_header.setText(
            f'<span style="color:{text_color}">{enemy_text}</span>'
        )

    def update_rendering(self):
        self._sync_expanded_from_controller()
        self._update_header_text()

        test_moves_enabled = False
        if hasattr(self._controller, 'get_test_moves_enabled'):
            test_moves_enabled = self._controller.get_test_moves_enabled()
        grid = self._content.layout()

        # Regular moves: player cols 0-3, enemy cols 5-8
        for cur_idx, cur_move in enumerate(self.move_list):
            column_idx = cur_idx
            if column_idx >= 4:
                column_idx += 1  # skip divider column

            # Hide enemy moves when test moves enabled
            if cur_idx >= 4 and test_moves_enabled:
                if self._did_draw[cur_idx]:
                    cur_move.setVisible(False)
                    self._did_draw[cur_idx] = False
            elif self._controller.get_move_info(cur_move._mon_idx, cur_move._move_idx, cur_move._is_player_mon) is not None:
                if not self._did_draw[cur_idx]:
                    grid.addWidget(cur_move, 0, column_idx)
                    cur_move.setVisible(True)
                    self._did_draw[cur_idx] = True
                cur_move.update_rendering()
            else:
                if self._did_draw[cur_idx]:
                    cur_move.setVisible(False)
                    self._did_draw[cur_idx] = False

        # Test move slots: cols 5-8
        if test_moves_enabled:
            for slot_idx, test_move in enumerate(self.test_move_slots):
                column_idx = 5 + slot_idx
                if not self._did_draw_test_moves[slot_idx]:
                    grid.addWidget(test_move, 0, column_idx)
                    test_move.setVisible(True)
                    self._did_draw_test_moves[slot_idx] = True
                test_move.update_rendering()
        else:
            for slot_idx, test_move in enumerate(self.test_move_slots):
                if self._did_draw_test_moves[slot_idx]:
                    test_move.setVisible(False)
                    self._did_draw_test_moves[slot_idx] = False


# ===================================================================
# AutocompleteEntry -- text entry with filtered listbox dropdown
# ===================================================================

class AutocompleteEntry(QWidget):
    """A text entry with autocomplete dropdown that filters as you type."""

    selection_made = Signal()

    def __init__(self, values, callback=None, width=20, parent=None):
        super().__init__(parent)
        self._all_values = values
        self._callback = callback
        self._original_value = ""

        layout = QVBoxLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)

        self._entry = QLineEdit()
        self._entry.setMinimumWidth(width * 7)
        layout.addWidget(self._entry)

        # Use a QCompleter for autocomplete behaviour
        self._completer = QCompleter(values, self)
        self._completer.setCaseSensitivity(Qt.CaseInsensitive)
        self._completer.setFilterMode(Qt.MatchContains)
        self._completer.setMaxVisibleItems(15)
        self._completer.activated.connect(self._on_completer_activated)
        self._entry.setCompleter(self._completer)

        self._entry.editingFinished.connect(self._on_editing_finished)

    def _on_completer_activated(self, text):
        self._original_value = text
        if self._callback:
            self._callback()
        # Defer focus teardown so the popup-close machinery finishes
        # before we move focus. If we do this inline, Qt re-asserts
        # focus on the entry as the popup unwinds.
        QTimer.singleShot(0, self._finalize_activation)

    def _finalize_activation(self):
        popup = self._completer.popup()
        if popup is not None and popup.isVisible():
            popup.hide()
        # Move focus off the entry. Required so keyboard input no
        # longer goes here, but on its own this leaves a stuck blink
        # cursor when the activation came from a mouse click on the
        # popup -- QLineEdit's cursor-blink timer is started by the
        # popup re-routing focus and never stopped because the
        # FocusOut path was short-circuited.
        win = self._entry.window()
        if win is not None:
            win.setFocus(Qt.OtherFocusReason)
        self._entry.clearFocus()
        # Synthesise a FocusOut so QLineEdit's blink timer is forced
        # to stop even if Qt's own focus events did not deliver one.
        if not self._entry.hasFocus():
            QCoreApplication.sendEvent(
                self._entry, QFocusEvent(QEvent.FocusOut, Qt.OtherFocusReason)
            )

    def _on_editing_finished(self):
        current = self._entry.text()
        if current not in self._all_values:
            self._entry.setText(self._original_value)

    def get(self):
        return self._entry.text()

    def set(self, value):
        # Temporarily detach the completer so setText doesn't flash its popup.
        self._entry.setCompleter(None)
        self._entry.setText(value)
        self._entry.setCompleter(self._completer)
        self._original_value = value

    def set_values(self, values):
        self._all_values = values
        self._completer.setModel(QStringListModel(values, self._completer))

    def enable(self):
        self._entry.setEnabled(True)

    def disable(self):
        self._entry.setEnabled(False)


# ===================================================================
# DamageSummary -- a single move's damage info
# ===================================================================

class DamageSummary(QWidget):
    """Renders one move slot: move name, damage range, crit range, and KO info.

    Styled to match the original Tkinter version with coloured section
    backgrounds:
      - Header (move name): primary colour background
      - Damage ranges: contrast-tinted background
      - Kill info: secondary-tinted background
    """

    def __init__(
        self,
        controller: BattleSummaryController,
        mon_idx: int,
        move_idx: int,
        is_player_mon: bool,
        parent=None,
        is_test_move: bool = False,
    ):
        super().__init__(parent)
        self._controller = controller
        self._mon_idx = mon_idx
        self._move_idx = move_idx
        self._is_player_mon = is_player_mon
        self._is_test_move = is_test_move
        self._move_name = None
        self._is_loading = False

        # Use Ignored horizontal policy so column stretch factors control widths
        self.setSizePolicy(QSizePolicy.Ignored, QSizePolicy.Preferred)

        outer_layout = QVBoxLayout(self)
        outer_layout.setContentsMargins(2, 0, 2, 2)
        outer_layout.setSpacing(0)

        # Precompute section colours
        primary_bg = _primary_bg()
        primary_fg = _primary_fg()
        contrast_color = config.get_contrast_color()
        secondary_color = config.get_secondary_color()
        base_bg = config.get_background_color()

        # Tinted backgrounds for range and kill sections (like original Tkinter)
        range_bg = _blend_color(contrast_color, base_bg, 0.10)
        kill_bg = _blend_color(secondary_color, base_bg, 0.08)

        # ---- header row (move name + optional dropdowns) ------------------
        self.header = QFrame()
        self.header.setFixedHeight(26)
        self.header.setStyleSheet(
            f"QFrame {{ background-color: {primary_bg}; border: none; border-top-left-radius: 6px; border-top-right-radius: 6px; }}"
        )
        header_layout = QHBoxLayout(self.header)
        header_layout.setContentsMargins(3, 0, 3, 0)
        header_layout.setSpacing(2)

        # Test-move dropdown (only for first pokemon)
        self.test_move_dropdown = None
        if self._is_test_move and self._mon_idx == 0:
            all_moves = current_gen_info().move_db().get_filtered_names()
            all_moves.insert(0, "")
            self.test_move_dropdown = AutocompleteEntry(
                all_moves,
                callback=self._on_test_move_changed,
                width=18,
                parent=self.header,
            )
            self.test_move_dropdown.setVisible(False)
            header_layout.addWidget(self.test_move_dropdown, 1)

        self.move_name_label = QLabel("")
        self.move_name_label.setAlignment(Qt.AlignCenter)
        move_name_font = QFont()
        move_name_font.setBold(True)
        self.move_name_label.setFont(move_name_font)
        self.move_name_label.setStyleSheet(
            f"background-color: {primary_bg}; color: {primary_fg}; padding: 2px 4px; border: none;"
        )
        # Click handlers for move highlighting
        if self._is_player_mon and not self._is_test_move:
            self.move_name_label.mousePressEvent = self._on_move_name_mouse_press

        header_layout.addWidget(self.move_name_label, 1)

        # Weather toggle: shown only when this move is a weather-inducing move
        # (Rain Dance / Sunny Day / Sandstorm / Hail) and the current generation
        # supports that weather. Toggling it sets the active weather for damage
        # calculations, scoped from the current matchup onward.
        self.weather_checkbox = QCheckBox(parent=self.header)
        self.weather_checkbox.setFocusPolicy(Qt.NoFocus)
        self.weather_checkbox.setStyleSheet("QCheckBox { background: transparent; border: none; }")
        self.weather_checkbox.stateChanged.connect(self._weather_checkbox_callback)
        self.weather_checkbox.setVisible(False)
        header_layout.addWidget(self.weather_checkbox)

        # Screen toggle: shown only when this move is Reflect or Light Screen.
        # Toggling applies the screen to the side that owns the move (player or
        # enemy) from the current matchup onward.
        self.screen_checkbox = QCheckBox(parent=self.header)
        self.screen_checkbox.setFocusPolicy(Qt.NoFocus)
        self.screen_checkbox.setStyleSheet("QCheckBox { background: transparent; border: none; }")
        self.screen_checkbox.stateChanged.connect(self._screen_checkbox_callback)
        self.screen_checkbox.setVisible(False)
        header_layout.addWidget(self.screen_checkbox)

        self.custom_data_dropdown = SimpleOptionMenu(option_list=[""], callback=self._custom_data_callback, parent=self.header)
        self.custom_data_dropdown.setSizeAdjustPolicy(QComboBox.AdjustToContents)
        self.custom_data_dropdown.setVisible(False)
        header_layout.addWidget(self.custom_data_dropdown)

        self.stat_stage_dropdown = SimpleOptionMenu(option_list=["0"], callback=self._stat_stage_callback, parent=self.header)
        self.stat_stage_dropdown.setSizeAdjustPolicy(QComboBox.AdjustToContents)
        self.stat_stage_dropdown.setVisible(False)
        header_layout.addWidget(self.stat_stage_dropdown)

        outer_layout.addWidget(self.header)

        # ---- damage range rows -------------------------------------------
        self.range_frame = QFrame()
        self.range_frame.setFixedHeight(34)
        self.range_frame.setStyleSheet(
            f"QFrame {{ background-color: {range_bg}; border: none; }}"
        )
        range_layout = QGridLayout(self.range_frame)
        range_layout.setContentsMargins(4, 0, 4, 0)
        range_layout.setSpacing(0)
        range_layout.setColumnStretch(0, 1)
        range_layout.setColumnStretch(1, 1)

        range_label_style = f"color: {contrast_color}; background: transparent; border: none;"

        self.damage_range = QLabel("")
        self.damage_range.setStyleSheet(range_label_style)
        self.damage_range.setAlignment(Qt.AlignLeft)
        range_layout.addWidget(self.damage_range, 0, 0)

        self.pct_damage_range = QLabel("")
        self.pct_damage_range.setStyleSheet(range_label_style)
        self.pct_damage_range.setAlignment(Qt.AlignRight)
        range_layout.addWidget(self.pct_damage_range, 0, 1)

        self.crit_damage_range = QLabel("")
        self.crit_damage_range.setStyleSheet(range_label_style)
        self.crit_damage_range.setAlignment(Qt.AlignLeft)
        range_layout.addWidget(self.crit_damage_range, 1, 0)

        self.crit_pct_damage_range = QLabel("")
        self.crit_pct_damage_range.setStyleSheet(range_label_style)
        self.crit_pct_damage_range.setAlignment(Qt.AlignRight)
        range_layout.addWidget(self.crit_pct_damage_range, 1, 1)

        outer_layout.addWidget(self.range_frame)

        # ---- kill info row ------------------------------------------------
        self.kill_frame = QFrame()
        self.kill_frame.setMinimumHeight(52)
        self.kill_frame.setStyleSheet(
            f"QFrame {{ background-color: {kill_bg}; border: none; border-bottom-left-radius: 6px; border-bottom-right-radius: 6px; }}"
        )
        kill_layout = QHBoxLayout(self.kill_frame)
        kill_layout.setContentsMargins(4, 1, 4, 1)
        kill_layout.setSpacing(0)

        kill_label_style = f"color: {secondary_color}; background: transparent; border: none;"
        self.num_to_kill = QLabel("")
        self.num_to_kill.setAlignment(Qt.AlignLeft | Qt.AlignTop)
        self.num_to_kill.setStyleSheet(kill_label_style)
        kill_layout.addWidget(self.num_to_kill)

        self.kill_pct = QLabel("")
        self.kill_pct.setAlignment(Qt.AlignRight | Qt.AlignTop)
        self.kill_pct.setStyleSheet(kill_label_style)
        kill_layout.addWidget(self.kill_pct)

        outer_layout.addWidget(self.kill_frame, 1)

    # ------------------------------------------------------------------
    # Best-move highlighting (kill frame)
    # ------------------------------------------------------------------

    def flag_as_best_move(self):
        if self._is_player_mon:
            flag_color = config.get_success_color()
        else:
            flag_color = config.get_failure_color()
        flag_bg = _blend_color(flag_color, config.get_background_color(), 0.20)
        self.kill_frame.setStyleSheet(
            f"QFrame {{ background-color: {flag_bg}; border: none; border-bottom-left-radius: 6px; border-bottom-right-radius: 6px; }}"
        )
        kill_style = f"color: {flag_color}; background: transparent; border: none; font-weight: bold;"
        self.num_to_kill.setStyleSheet(kill_style)
        self.kill_pct.setStyleSheet(kill_style)

    def unflag_as_best_move(self):
        secondary_color = config.get_secondary_color()
        kill_bg = _blend_color(secondary_color, config.get_background_color(), 0.08)
        self.kill_frame.setStyleSheet(
            f"QFrame {{ background-color: {kill_bg}; border: none; border-bottom-left-radius: 6px; border-bottom-right-radius: 6px; }}"
        )
        kill_style = f"color: {secondary_color}; background: transparent; border: none;"
        self.num_to_kill.setStyleSheet(kill_style)
        self.kill_pct.setStyleSheet(kill_style)

    # ------------------------------------------------------------------
    # Callbacks
    # ------------------------------------------------------------------

    def _custom_data_callback(self, *args, **kwargs):
        if self._is_loading:
            return
        if self._move_name == const.MIMIC_MOVE_NAME:
            self._controller.update_mimic_selection(self.custom_data_dropdown.get())
        else:
            self._controller.update_custom_move_data(
                self._mon_idx, self._move_idx, self._is_player_mon,
                self.custom_data_dropdown.get(),
            )

    def _stat_stage_callback(self, *args, **kwargs):
        if self._is_loading:
            return
        new_value = self.stat_stage_dropdown.get()
        self._controller.update_stat_stage_setup(
            self._mon_idx, self._move_idx, self._is_player_mon, new_value,
        )

    def _weather_checkbox_callback(self, *args, **kwargs):
        if self._is_loading:
            return
        if self._move_name is None:
            return
        self._controller.toggle_weather_from_move(
            self._move_name, self.weather_checkbox.isChecked(),
            mon_idx=self._mon_idx,
        )

    def _screen_checkbox_callback(self, *args, **kwargs):
        if self._is_loading:
            return
        if self._move_name is None:
            return
        self._controller.toggle_screen_from_move(
            self._move_name, self.screen_checkbox.isChecked(),
            mon_idx=self._mon_idx, is_player=self._is_player_mon,
        )

    def _on_test_move_changed(self, *args, **kwargs):
        if self._is_loading:
            return
        if self.test_move_dropdown is None:
            return
        selected_move = self.test_move_dropdown.get()
        slot_idx = self._move_idx - 4
        self._controller.update_test_move(slot_idx, selected_move)

    def _on_move_name_mouse_press(self, event):
        """Handle click on move name to cycle or reset highlight state."""
        if not self._controller.get_show_move_highlights():
            return
        if not self._is_player_mon:
            return
        if event.button() == Qt.LeftButton:
            self._controller.update_move_highlight(
                self._mon_idx, self._move_idx, self._is_player_mon, reset=False,
            )
        elif event.button() == Qt.RightButton:
            self._controller.update_move_highlight(
                self._mon_idx, self._move_idx, self._is_player_mon, reset=True,
            )

    # ------------------------------------------------------------------
    # Highlight colour helpers
    # ------------------------------------------------------------------

    def _get_highlight_state(self):
        if not self._is_player_mon or not self._controller.get_show_move_highlights():
            return 0
        return self._controller.get_move_highlight_state(
            self._mon_idx, self._move_idx, self._is_player_mon,
        )

    def _update_highlight_colors(self):
        """Immediately update colours after a highlight click."""
        if not self._controller.get_show_move_highlights() or not self._is_player_mon:
            return
        move = self._controller.get_move_info(self._mon_idx, self._move_idx, self._is_player_mon)
        if move is None:
            return

        highlight_state = self._get_highlight_state()
        default_bg = _primary_bg()
        default_fg = _primary_fg()

        fade_enabled = config.get_fade_moves_without_highlight() and config.get_show_move_highlights()
        should_fade = fade_enabled and highlight_state == 0

        if highlight_state in _HIGHLIGHT_COLORS_IMMEDIATE:
            bg = _HIGHLIGHT_COLORS_IMMEDIATE[highlight_state]
            self.header.setStyleSheet(
                f"QFrame {{ background-color: {bg}; border: none; border-top-left-radius: 6px; border-top-right-radius: 6px; }}"
            )
            self.move_name_label.setStyleSheet(
                f"background-color: {bg}; color: white; padding: 2px 4px; border: none; font-weight: bold;"
            )
            self._reset_fade_elements()
            self._show_dropdowns_if_needed()
        elif should_fade:
            faded_fg = _blend_color(default_fg, default_bg, 0.1)
            self.header.setStyleSheet(
                f"QFrame {{ background-color: {default_bg}; border: none; border-top-left-radius: 6px; border-top-right-radius: 6px; }}"
            )
            self.move_name_label.setStyleSheet(
                f"background-color: {default_bg}; color: {faded_fg}; padding: 2px 4px; border: none;"
            )
            self._apply_fade_to_all_elements(faded_fg, default_bg)
        else:
            self.header.setStyleSheet(
                f"QFrame {{ background-color: {default_bg}; border: none; border-top-left-radius: 6px; border-top-right-radius: 6px; }}"
            )
            self.move_name_label.setStyleSheet(
                f"background-color: {default_bg}; color: {default_fg}; padding: 2px 4px; border: none; font-weight: bold;"
            )
            self._reset_fade_elements()
            self._show_dropdowns_if_needed()

    def _apply_fade_to_all_elements(self, faded_fg, default_bg):
        """Apply faded colours to damage ranges, kill text, and hide dropdowns."""
        try:
            contrast_fg = config.get_contrast_color()
            contrast_bg = config.get_background_color()
            faded_contrast = _blend_color(contrast_fg, contrast_bg, 0.1)
            faded_range_style = f"color: {faded_contrast}; background: transparent; border: none;"
            self.damage_range.setStyleSheet(faded_range_style)
            self.pct_damage_range.setStyleSheet(faded_range_style)
            self.crit_damage_range.setStyleSheet(faded_range_style)
            self.crit_pct_damage_range.setStyleSheet(faded_range_style)

            # Fade the range frame background too
            faded_range_bg = _blend_color(contrast_fg, contrast_bg, 0.04)
            self.range_frame.setStyleSheet(
                f"QFrame {{ background-color: {faded_range_bg}; border: none; }}"
            )

            secondary_fg = config.get_secondary_color()
            secondary_bg = config.get_background_color()
            faded_secondary = _blend_color(secondary_fg, secondary_bg, 0.1)
            faded_kill_style = f"color: {faded_secondary}; background: transparent; border: none;"
            self.num_to_kill.setStyleSheet(faded_kill_style)
            self.kill_pct.setStyleSheet(faded_kill_style)

            faded_kill_bg = _blend_color(secondary_fg, secondary_bg, 0.03)
            self.kill_frame.setStyleSheet(
                f"QFrame {{ background-color: {faded_kill_bg}; border: none; border-bottom-left-radius: 6px; border-bottom-right-radius: 6px; }}"
            )

            self.custom_data_dropdown.setVisible(False)
            if self.test_move_dropdown is not None:
                self.test_move_dropdown.disable()
        except Exception:
            pass

    def _reset_fade_elements(self):
        """Reset damage ranges and kill text to normal colours."""
        try:
            contrast_fg = config.get_contrast_color()
            base_bg = config.get_background_color()
            range_label_style = f"color: {contrast_fg}; background: transparent; border: none;"
            self.damage_range.setStyleSheet(range_label_style)
            self.pct_damage_range.setStyleSheet(range_label_style)
            self.crit_damage_range.setStyleSheet(range_label_style)
            self.crit_pct_damage_range.setStyleSheet(range_label_style)

            range_bg = _blend_color(contrast_fg, base_bg, 0.10)
            self.range_frame.setStyleSheet(
                f"QFrame {{ background-color: {range_bg}; border: none; }}"
            )

            secondary_fg = config.get_secondary_color()
            kill_style = f"color: {secondary_fg}; background: transparent; border: none;"
            self.num_to_kill.setStyleSheet(kill_style)
            self.kill_pct.setStyleSheet(kill_style)

            kill_bg = _blend_color(secondary_fg, base_bg, 0.08)
            self.kill_frame.setStyleSheet(
                f"QFrame {{ background-color: {kill_bg}; border: none; border-bottom-left-radius: 6px; border-bottom-right-radius: 6px; }}"
            )

            if self.test_move_dropdown is not None:
                self.test_move_dropdown.enable()
        except Exception:
            pass

    def _show_dropdowns_if_needed(self):
        """Restore dropdown visibility based on current move data."""
        try:
            move = self._controller.get_move_info(self._mon_idx, self._move_idx, self._is_player_mon)
            if move is None:
                return
            custom_data_options = move.custom_data_options
            if self._move_name == const.MIMIC_MOVE_NAME:
                custom_data_options = move.mimic_options
            if custom_data_options:
                self.custom_data_dropdown.setVisible(True)
        except Exception:
            pass

    # ------------------------------------------------------------------
    # Static formatting helper
    # ------------------------------------------------------------------

    @staticmethod
    def format_message(kill_info):
        """Return (description, percentage) tuple for a kill range entry."""
        kill_pct = kill_info[1]
        if kill_pct == -1:
            if config.do_ignore_accuracy():
                return (f"{kill_info[0]}-hit kill:", "100 %")
            else:
                return (f"{kill_info[0]}-hit kill, IGNORING ACC", "")

        if round(kill_pct, 1) == int(kill_pct):
            rendered_kill_pct = f"{int(kill_pct)}"
        else:
            rendered_kill_pct = f"{kill_pct:.1f}"
        if config.do_ignore_accuracy():
            return (f"{kill_info[0]}-hit kill:", f"{rendered_kill_pct} %")
        return (f"{kill_info[0]}-turn kill:", f"{rendered_kill_pct} %")

    @staticmethod
    def _format_recoil_line(move):
        """Return ('Recoil: X - Y', 'P - Q %') for recoil moves, or None."""
        if move is None or move.min_damage == -1:
            return None
        attacker_hp = getattr(move, "attacking_mon_hp", 0) or 0

        max_hp_divisor = None
        damage_divisor = None
        for flavor in move.attack_flavor:
            if flavor in const.RECOIL_MAX_HP_FLAVOR_DIVISORS:
                max_hp_divisor = const.RECOIL_MAX_HP_FLAVOR_DIVISORS[flavor]
                break
            if flavor in const.RECOIL_FLAVOR_DIVISORS:
                damage_divisor = const.RECOIL_FLAVOR_DIVISORS[flavor]
                break

        if max_hp_divisor is not None and attacker_hp > 0:
            recoil_min = max(1, attacker_hp // max_hp_divisor)
            recoil_max = recoil_min
        elif damage_divisor is not None:
            hp = move.defending_mon_hp
            capped_min = min(move.min_damage, hp)
            capped_max = min(move.max_damage, hp)
            recoil_min = max(1, capped_min // damage_divisor)
            recoil_max = max(1, capped_max // damage_divisor)
        else:
            return None

        if recoil_min == recoil_max:
            desc = f"Recoil: {recoil_min}"
            pct_text = f"{round(recoil_min / attacker_hp * 100)} %" if attacker_hp > 0 else ""
        else:
            desc = f"Recoil: {recoil_min} - {recoil_max}"
            if attacker_hp > 0:
                pct_min = round(recoil_min / attacker_hp * 100)
                pct_max = round(recoil_max / attacker_hp * 100)
                pct_text = f"{pct_min} - {pct_max} %"
            else:
                pct_text = ""
        return (desc, pct_text)

    def set_test_move_options(self, values):
        if self.test_move_dropdown is not None:
            self.test_move_dropdown.set_values(values)

    # ------------------------------------------------------------------
    # Main rendering method
    # ------------------------------------------------------------------

    def update_rendering(self):
        move = self._controller.get_move_info(self._mon_idx, self._move_idx, self._is_player_mon)
        self._move_name = None if move is None else move.name

        self._is_loading = True

        # ---- highlight / fade state -----------------------------------
        fade_enabled = config.get_fade_moves_without_highlight() and config.get_show_move_highlights()
        highlight_state = 0
        if self._is_player_mon and self._controller.get_show_move_highlights():
            highlight_state = self._controller.get_move_highlight_state(
                self._mon_idx, self._move_idx, self._is_player_mon,
            )

        # Best-move flagging is applied later, after damage range styling

        # ---- custom data / stat-stage dropdowns -----------------------
        custom_data_options = None
        custom_data_selection = None
        if move is not None:
            custom_data_options = move.custom_data_options
            custom_data_selection = move.custom_data_selection
        if self._move_name == const.MIMIC_MOVE_NAME:
            custom_data_options = move.mimic_options
            custom_data_selection = move.mimic_data

        stat_stage_options = None
        stat_stage_selection = "0"
        has_global_setup_for_this_side = (
            (self._is_player_mon and self._controller.get_player_setup_moves())
            or (not self._is_player_mon and self._controller.get_enemy_setup_moves())
        )
        if move is not None and not has_global_setup_for_this_side:
            stat_stage_options = move.stat_stage_options
            stat_stage_selection = move.stat_stage_selection if move.stat_stage_selection else "0"

        # ---- layout header components ---------------------------------
        if self._is_test_move:
            test_moves = self._controller.get_test_moves() if hasattr(self._controller, 'get_test_moves') else []
            slot_idx = self._move_idx - 4
            if 0 <= slot_idx < len(test_moves):
                current_test_move = test_moves[slot_idx]
                if self._mon_idx == 0 and self.test_move_dropdown is not None:
                    current_val = current_test_move if current_test_move else ""
                    if self.test_move_dropdown.get() != current_val:
                        self.test_move_dropdown.set(current_val)
                    self.test_move_dropdown.setVisible(True)
                    self.move_name_label.setVisible(False)
                else:
                    self.move_name_label.setText(current_test_move if current_test_move else "")
                    self.move_name_label.setVisible(True)
                    if self.test_move_dropdown is not None:
                        self.test_move_dropdown.setVisible(False)
            self.custom_data_dropdown.setVisible(False)
            self.stat_stage_dropdown.setVisible(False)
        elif custom_data_options and stat_stage_options:
            if self.test_move_dropdown is not None:
                self.test_move_dropdown.setVisible(False)
            self.move_name_label.setVisible(True)
            self.custom_data_dropdown.setVisible(True)
            self.custom_data_dropdown.new_values(custom_data_options, default_val=custom_data_selection)
            self.stat_stage_dropdown.setVisible(True)
            self.stat_stage_dropdown.new_values(stat_stage_options, default_val=stat_stage_selection)
        elif custom_data_options:
            if self.test_move_dropdown is not None:
                self.test_move_dropdown.setVisible(False)
            self.move_name_label.setVisible(True)
            self.custom_data_dropdown.setVisible(True)
            self.custom_data_dropdown.new_values(custom_data_options, default_val=custom_data_selection)
            self.stat_stage_dropdown.setVisible(False)
        elif stat_stage_options:
            if self.test_move_dropdown is not None:
                self.test_move_dropdown.setVisible(False)
            self.move_name_label.setVisible(True)
            self.custom_data_dropdown.setVisible(False)
            self.stat_stage_dropdown.setVisible(True)
            self.stat_stage_dropdown.new_values(stat_stage_options, default_val=stat_stage_selection)
        else:
            if self.test_move_dropdown is not None:
                self.test_move_dropdown.setVisible(False)
            self.move_name_label.setVisible(True)
            self.custom_data_dropdown.setVisible(False)
            self.stat_stage_dropdown.setVisible(False)

        # ---- weather-move toggle --------------------------------------
        weather_for_move = None
        if move is not None:
            weather_for_move = self._controller.get_weather_for_move(move.name)
        if weather_for_move is not None:
            cur_weather = self._controller.get_weather()
            source_mon_idx = self._controller.get_weather_source_mon_idx()
            is_active = (
                cur_weather == weather_for_move
                and source_mon_idx == self._mon_idx
            )
            self.weather_checkbox.setChecked(is_active)
            self.weather_checkbox.setToolTip(
                f"Set weather to {weather_for_move} for this matchup and later"
            )
            self.weather_checkbox.setVisible(True)
        else:
            self.weather_checkbox.setChecked(False)
            self.weather_checkbox.setVisible(False)

        # ---- screen-move toggle (Reflect / Light Screen) ---------------
        screen_for_move = None
        if move is not None:
            screen_for_move = self._controller.get_screen_for_move(move.name)
        if screen_for_move is not None:
            screen_src = self._controller.get_screen_source_mon_idx(
                self._is_player_mon, screen_for_move,
            )
            self.screen_checkbox.setChecked(screen_src == self._mon_idx)
            self.screen_checkbox.setToolTip(
                f"Apply {move.name} for this matchup and later"
            )
            self.screen_checkbox.setVisible(True)
        else:
            self.screen_checkbox.setChecked(False)
            self.screen_checkbox.setVisible(False)

        # ---- populate values ------------------------------------------
        if move is None:
            self.move_name_label.setText("")
            self.damage_range.setText("")
            self.pct_damage_range.setText("")
            self.crit_damage_range.setText("")
            self.crit_pct_damage_range.setText("")
            self.num_to_kill.setText("")
            self.kill_pct.setText("")
        else:
            self.move_name_label.setText(move.name)

            default_bg = _primary_bg()
            default_fg = _primary_fg()
            header_frame_style = (
                f"QFrame {{ background-color: {default_bg}; border: none;"
                f" border-top-left-radius: 6px; border-top-right-radius: 6px; }}"
            )
            move_label_style = (
                f"background-color: {default_bg}; color: {default_fg};"
                f" padding: 2px 4px; border: none; font-weight: bold;"
            )

            # ---- player move highlight colouring ----------------------
            if self._is_player_mon and self._controller.get_show_move_highlights():
                highlight_state = self._controller.get_move_highlight_state(
                    self._mon_idx, self._move_idx, self._is_player_mon,
                )
                self.move_name_label.setCursor(Qt.PointingHandCursor)

                should_fade = fade_enabled and highlight_state == 0

                if highlight_state in _HIGHLIGHT_COLORS:
                    bg = _HIGHLIGHT_COLORS[highlight_state]
                    header_frame_style = (
                        f"QFrame {{ background-color: {bg}; border: none;"
                        f" border-top-left-radius: 6px; border-top-right-radius: 6px; }}"
                    )
                    move_label_style = (
                        f"background-color: {bg}; color: white;"
                        f" padding: 2px 4px; border: none; font-weight: bold;"
                    )
                    self.header.setStyleSheet(header_frame_style)
                    self.move_name_label.setStyleSheet(move_label_style)
                    self._reset_fade_elements()
                elif should_fade:
                    faded_fg = _blend_color(default_fg, default_bg, 0.1)
                    move_label_style = (
                        f"background-color: {default_bg}; color: {faded_fg};"
                        f" padding: 2px 4px; border: none;"
                    )
                    self.header.setStyleSheet(header_frame_style)
                    self.move_name_label.setStyleSheet(move_label_style)
                    self._apply_fade_to_all_elements(faded_fg, default_bg)
                else:
                    self.header.setStyleSheet(header_frame_style)
                    self.move_name_label.setStyleSheet(move_label_style)
                    self._reset_fade_elements()
            else:
                # Highlights disabled -- restore defaults
                if self._is_player_mon:
                    self.move_name_label.setCursor(Qt.ArrowCursor)
                self.header.setStyleSheet(header_frame_style)
                self.move_name_label.setStyleSheet(move_label_style)
                self._reset_fade_elements()

            # ---- enemy-move fading ------------------------------------
            if not self._is_player_mon and fade_enabled:
                if move is not None and not move.is_best_move:
                    try:
                        faded_fg = _blend_color(default_fg, default_bg, 0.1)
                        faded_label_style = (
                            f"background-color: {default_bg}; color: {faded_fg};"
                            f" padding: 2px 4px; border: none;"
                        )
                        self.header.setStyleSheet(header_frame_style)
                        self.move_name_label.setStyleSheet(faded_label_style)
                        self._apply_fade_to_all_elements(faded_fg, default_bg)
                        self.custom_data_dropdown.setEnabled(False)
                    except Exception:
                        pass
                else:
                    try:
                        self.header.setStyleSheet(header_frame_style)
                        self.move_name_label.setStyleSheet(move_label_style)
                        self._reset_fade_elements()
                        self.custom_data_dropdown.setEnabled(True)
                    except Exception:
                        pass

            # ---- damage range text ------------------------------------
            if move.min_damage == -1:
                self.damage_range.setText("")
                self.pct_damage_range.setText("")
                self.crit_damage_range.setText("")
                self.crit_pct_damage_range.setText("")
            else:
                # Determine if we should fade the damage-range labels
                should_fade_range = False
                if fade_enabled:
                    if self._is_player_mon:
                        hl = self._controller.get_move_highlight_state(
                            self._mon_idx, self._move_idx, self._is_player_mon,
                        )
                        should_fade_range = hl == 0
                    else:
                        should_fade_range = not move.is_best_move

                contrast_fg = config.get_contrast_color()
                contrast_bg = config.get_background_color()

                if should_fade_range:
                    faded_contrast = _blend_color(contrast_fg, contrast_bg, 0.1)
                    range_style = f"color: {faded_contrast}; background: transparent; border: none;"

                    faded_range_bg = _blend_color(contrast_fg, contrast_bg, 0.04)
                    self.range_frame.setStyleSheet(
                        f"QFrame {{ background-color: {faded_range_bg}; border: none; }}"
                    )

                    secondary_fg = config.get_secondary_color()
                    faded_secondary = _blend_color(secondary_fg, contrast_bg, 0.1)
                    faded_kill_style = f"color: {faded_secondary}; background: transparent; border: none;"
                    self.num_to_kill.setStyleSheet(faded_kill_style)
                    self.kill_pct.setStyleSheet(faded_kill_style)
                    faded_kill_bg = _blend_color(secondary_fg, contrast_bg, 0.03)
                    self.kill_frame.setStyleSheet(
                        f"QFrame {{ background-color: {faded_kill_bg}; border: none;"
                        f" border-bottom-left-radius: 6px; border-bottom-right-radius: 6px; }}"
                    )

                    self.custom_data_dropdown.setVisible(False)
                    if self.test_move_dropdown is not None:
                        self.test_move_dropdown.disable()
                else:
                    range_style = f"color: {contrast_fg}; background: transparent; border: none;"

                    range_bg = _blend_color(contrast_fg, contrast_bg, 0.10)
                    self.range_frame.setStyleSheet(
                        f"QFrame {{ background-color: {range_bg}; border: none; }}"
                    )

                    secondary_fg = config.get_secondary_color()
                    kill_style = f"color: {secondary_fg}; background: transparent; border: none;"
                    self.num_to_kill.setStyleSheet(kill_style)
                    self.kill_pct.setStyleSheet(kill_style)
                    kill_bg = _blend_color(secondary_fg, contrast_bg, 0.08)
                    self.kill_frame.setStyleSheet(
                        f"QFrame {{ background-color: {kill_bg}; border: none;"
                        f" border-bottom-left-radius: 6px; border-bottom-right-radius: 6px; }}"
                    )

                    if self.test_move_dropdown is not None:
                        self.test_move_dropdown.enable()

                self.damage_range.setStyleSheet(range_style)
                self.pct_damage_range.setStyleSheet(range_style)
                self.crit_damage_range.setStyleSheet(range_style)
                self.crit_pct_damage_range.setStyleSheet(range_style)

                self.damage_range.setText(
                    f"{move.min_damage} - {move.max_damage}"
                )
                pct_min = round(move.min_damage / move.defending_mon_hp * 100)
                pct_max = round(move.max_damage / move.defending_mon_hp * 100)
                self.pct_damage_range.setText(f"{pct_min} - {pct_max}%")

                self.crit_damage_range.setText(
                    f"{move.crit_min_damage} - {move.crit_max_damage}"
                )
                crit_pct_min = round(move.crit_min_damage / move.defending_mon_hp * 100)
                crit_pct_max = round(move.crit_max_damage / move.defending_mon_hp * 100)
                self.crit_pct_damage_range.setText(f"{crit_pct_min} - {crit_pct_max}%")

            # ---- kill ranges ------------------------------------------
            max_num_messages = 3
            kill_ranges = move.kill_ranges
            if len(kill_ranges) > max_num_messages:
                kill_ranges = kill_ranges[: max_num_messages - 1] + [kill_ranges[-1]]
            formatted = [self.format_message(x) for x in kill_ranges]

            recoil_line = self._format_recoil_line(move)
            if recoil_line is not None:
                recoil_color = config.get_failure_color()
                desc_lines = [desc for desc, _ in formatted]
                pct_lines = [pct for _, pct in formatted]
                desc_lines.append(
                    f'<span style="color: {recoil_color};">{recoil_line[0]}</span>'
                )
                pct_lines.append(
                    f'<span style="color: {recoil_color};">{recoil_line[1]}</span>'
                )
                self.num_to_kill.setText("<br>".join(desc_lines))
                self.kill_pct.setText("<br>".join(pct_lines))
            else:
                self.num_to_kill.setText("\n".join(desc for desc, _ in formatted))
                self.kill_pct.setText("\n".join(pct for _, pct in formatted))

        # Best-move flagging — must come AFTER damage range styling
        # so that it overrides the neutral kill_frame colors
        if fade_enabled and self._is_player_mon:
            self.unflag_as_best_move()
        elif move is None or not move.is_best_move:
            self.unflag_as_best_move()
        else:
            self.flag_as_best_move()

        self._is_loading = False
