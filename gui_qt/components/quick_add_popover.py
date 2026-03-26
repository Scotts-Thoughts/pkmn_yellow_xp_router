import logging

from PySide6.QtWidgets import (
    QWidget, QPushButton, QHBoxLayout, QVBoxLayout,
    QStackedWidget, QGroupBox, QFrame,
)
from PySide6.QtCore import Qt
from PySide6.QtGui import QFont

from gui_qt.components.quick_trainer_add import QuickTrainerAdd
from gui_qt.components.quick_item_add import QuickItemAdd
from gui_qt.components.quick_wild_pkmn import QuickWildPkmn
from gui_qt.components.quick_misc import QuickMiscEvents
from pkmn.gen_factory import current_gen_info
from routing.route_events import EventDefinition, LearnMoveEventDefinition
from utils.constants import const

logger = logging.getLogger(__name__)


# (icon, tooltip, shortcut key)
_CATEGORIES = [
    ("\u2694", "Trainer (Q)",   Qt.Key_Q),
    ("\u25C6", "Item (W)",      Qt.Key_W),
    ("\u2605", "Move (E)",      Qt.Key_E),
    ("\u2663", "Wild Pkmn (R)", Qt.Key_R),
    ("\u2630", "Misc (T)",      Qt.Key_T),
]


class QuickAddPopover(QWidget):
    """Floating two-phase popover for quickly adding route events.

    Phase 1 – the user sees five category icon buttons (Trainer / Item /
    Move / Wild Pkmn / Misc), reachable via Q / W / E / R / T.

    Phase 2 – after choosing a category, the full configuration panel for
    that event type appears below the icon bar so the user can fill in the
    details before the event is created.

    The popover auto-closes when an event is added (detected via the
    controller's route-change callback) or when the user presses Escape /
    Space.
    """

    def __init__(self, controller, parent=None):
        super().__init__(parent, Qt.Popup | Qt.FramelessWindowHint)
        self._controller = controller
        self._anchor = None
        self.setAttribute(Qt.WA_DeleteOnClose, False)

        self.setObjectName("quickAddPopover")
        self.setStyleSheet(
            "#quickAddPopover { border: 1px solid rgba(255, 255, 255, 0.15);"
            " border-radius: 4px; }"
        )

        main_layout = QVBoxLayout(self)
        main_layout.setContentsMargins(6, 6, 6, 6)
        main_layout.setSpacing(4)

        # ---- Category icon bar ------------------------------------------
        cat_widget = QWidget()
        cat_layout = QHBoxLayout(cat_widget)
        cat_layout.setContentsMargins(0, 0, 0, 0)
        cat_layout.setSpacing(4)

        icon_font = QFont()
        icon_font.setPointSize(14)

        self._cat_buttons = []
        self._key_to_idx = {}
        for idx, (icon, tooltip, key) in enumerate(_CATEGORIES):
            btn = QPushButton(icon)
            btn.setCheckable(True)
            btn.setFixedSize(36, 36)
            btn.setFont(icon_font)
            btn.setToolTip(tooltip)
            btn.setFocusPolicy(Qt.NoFocus)
            btn.clicked.connect(lambda _checked, i=idx: self._select_category(i))
            cat_layout.addWidget(btn)
            self._cat_buttons.append(btn)
            self._key_to_idx[key] = idx

        main_layout.addWidget(cat_widget)

        # ---- Separator (hidden until a category is chosen) ---------------
        self._separator = QFrame()
        self._separator.setFrameShape(QFrame.HLine)
        self._separator.setVisible(False)
        main_layout.addWidget(self._separator)

        # ---- Stacked detail panels ---------------------------------------
        self._detail_stack = QStackedWidget()
        self._detail_stack.setVisible(False)

        self._trainer_page = QuickTrainerAdd(controller)
        self._item_page = QuickItemAdd(controller)
        self._move_page = self._build_move_page()
        self._wild_pkmn_page = QuickWildPkmn(controller)
        self._misc_page = QuickMiscEvents(controller)

        for page in (
            self._trainer_page,
            self._item_page,
            self._move_page,
            self._wild_pkmn_page,
            self._misc_page,
        ):
            self._detail_stack.addWidget(page)

        main_layout.addWidget(self._detail_stack)

        # ---- Callbacks ---------------------------------------------------
        self._controller.register_route_change(self._on_route_change)

        # Eagerly populate the embedded widgets with current game data.
        # The popover is created lazily (on first Space press) so the game
        # version is guaranteed to be loaded by that point.
        try:
            self._trainer_page.update_pkmn_version()
            self._item_page.update_pkmn_version()
            self._wild_pkmn_page.update_pkmn_version()
            self._misc_page.update_pkmn_version()
            # update_pkmn_version clears the _uninitialized flag but does not
            # always re-evaluate button states, so do it explicitly.
            self._trainer_page.update_button_status()
            self._item_page.update_button_status()
            self._wild_pkmn_page.update_button_status()
            self._misc_page.update_button_status()
        except Exception:
            pass  # Will be initialised via version_change callback later.

    # ------------------------------------------------------------------
    # Move detail page (lightweight – the other pages reuse existing widgets)
    # ------------------------------------------------------------------

    def _build_move_page(self):
        page = QGroupBox("Moves")
        layout = QVBoxLayout(page)
        layout.setContentsMargins(5, 5, 5, 5)

        btn = QPushButton("Add Tutor Move")
        btn.clicked.connect(self._add_tutor_move)
        layout.addWidget(btn)

        layout.addStretch()
        return page

    def _add_tutor_move(self):
        try:
            cur_state = self._controller.get_active_state()
            if cur_state is None or cur_state.solo_pkmn is None:
                return
            dest = cur_state.solo_pkmn.get_move_destination(None, None)[0]
            self._controller.new_event(
                EventDefinition(
                    learn_move=LearnMoveEventDefinition(
                        None, dest, const.MOVE_SOURCE_TUTOR
                    )
                ),
                insert_after=self._controller.get_single_selected_event_id(),
            )
        except Exception as e:
            logger.error(f"Quick-add tutor move failed: {e}")

    # ------------------------------------------------------------------
    # Positioning
    # ------------------------------------------------------------------

    def show_above(self, global_pos, category_idx=None):
        """Reset to category-only view, position above *global_pos*, show.

        If *category_idx* is given (0-4), jump straight into that category's
        detail panel (phase 2).
        """
        self._anchor = global_pos

        # Reset: uncheck all category buttons, hide detail panel.
        for btn in self._cat_buttons:
            btn.setChecked(False)
        self._separator.setVisible(False)
        self._detail_stack.setVisible(False)

        if category_idx is not None:
            self._select_category(category_idx)

        self._reposition()
        self.show()

    def _reposition(self):
        """(Re)compute window position so the bottom edge sits above the anchor."""
        self.adjustSize()
        if self._anchor is None:
            return
        x = self._anchor.x() - self.width() // 2
        y = self._anchor.y() - self.height() - 4
        if y < 0:
            y = self._anchor.y() + 22
        self.move(x, y)

    # ------------------------------------------------------------------
    # Category selection
    # ------------------------------------------------------------------

    def _select_category(self, idx):
        for i, btn in enumerate(self._cat_buttons):
            btn.setChecked(i == idx)
        self._separator.setVisible(True)
        self._detail_stack.setCurrentIndex(idx)
        self._detail_stack.setVisible(True)
        if self.isVisible():
            # Popup widgets need a hide/show cycle for size changes to
            # take effect.
            self.hide()
            self._reposition()
            self.show()
        else:
            self._reposition()

    # ------------------------------------------------------------------
    # Keyboard handling
    # ------------------------------------------------------------------

    def keyPressEvent(self, event):
        idx = self._key_to_idx.get(event.key())
        if idx is not None:
            self._select_category(idx)
            return
        if event.key() in (Qt.Key_Escape, Qt.Key_Space):
            self.close()
            return
        super().keyPressEvent(event)

    # ------------------------------------------------------------------
    # Auto-close on route change
    # ------------------------------------------------------------------

    def _on_route_change(self):
        if self.isVisible():
            self.close()
