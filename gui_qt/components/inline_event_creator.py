"""Inline event creation widget overlaid on the route list."""
import logging

from PySide6.QtWidgets import (
    QFrame, QHBoxLayout, QComboBox, QPushButton, QWidget,
    QLabel, QLineEdit, QCompleter,
)
from PySide6.QtCore import Qt, Signal, QEvent, QObject, QTimer

from gui_qt.components.custom_components import AmountEntry
from pkmn.gen_factory import current_gen_info
from routing.route_events import (
    EventDefinition, TrainerEventDefinition, InventoryEventDefinition,
    WildPkmnEventDefinition, VitaminEventDefinition, RareCandyEventDefinition,
    HoldItemEventDefinition, LearnMoveEventDefinition, SaveEventDefinition,
    HealEventDefinition, BlackoutEventDefinition, EvolutionEventDefinition,
)
from utils.constants import const

logger = logging.getLogger(__name__)

# (display name, internal key)
_EVENT_TYPES = [
    ("Fight Trainer",  "trainer"),
    ("Get Item",       "get_item"),
    ("Buy Item",       "buy_item"),
    ("Sell Item",      "sell_item"),
    ("Use/Drop Item",  "use_item"),
    ("Hold Item",      "hold_item"),
    ("Wild Pkmn",      "wild_pkmn"),
    ("Rare Candy",     "rare_candy"),
    ("Vitamin",        "vitamin"),
    ("TM/HM Move",    "tm_move"),
    ("Save",           "save"),
    ("Heal",           "heal"),
    ("Blackout",       "blackout"),
    ("Evolve",         "evolve"),
    ("Notes",          "notes"),
]

# Map event-type constant → inline key
_EVENT_TYPE_TO_KEY = {
    const.TASK_TRAINER_BATTLE:   "trainer",
    const.TASK_GET_FREE_ITEM:    "get_item",
    const.TASK_PURCHASE_ITEM:    "buy_item",
    const.TASK_SELL_ITEM:        "sell_item",
    const.TASK_USE_ITEM:         "use_item",
    const.TASK_HOLD_ITEM:        "hold_item",
    const.TASK_FIGHT_WILD_PKMN:  "wild_pkmn",
    const.TASK_RARE_CANDY:       "rare_candy",
    const.TASK_VITAMIN:          "vitamin",
    const.TASK_LEARN_MOVE_TM:    "tm_move",
    const.TASK_SAVE:             "save",
    const.TASK_HEAL:             "heal",
    const.TASK_BLACKOUT:         "blackout",
    const.TASK_EVOLUTION:        "evolve",
    const.TASK_NOTES_ONLY:       "notes",
}

# Inline key → index in _EVENT_TYPES
_KEY_TO_INDEX = {key: idx for idx, (_, key) in enumerate(_EVENT_TYPES)}


class _SelectAllOnFocusFilter(QObject):
    """Event filter that selects all text in a QLineEdit when it gains focus
    or is clicked, so the user can immediately start typing to filter.

    Qt runs the default mouse/focus handlers after the filter returns False,
    and those handlers can clear the selection (e.g. mouse press positions
    the cursor), so the ``selectAll`` call is deferred via a zero-delay
    QTimer to run after the default behavior.
    """

    def eventFilter(self, obj, event):
        etype = event.type()
        if etype == QEvent.FocusIn or etype == QEvent.MouseButtonPress:
            QTimer.singleShot(0, obj.selectAll)
        return False


def _make_searchable(combo):
    """Configure a QComboBox for type-to-filter searching."""
    combo.setEditable(True)
    combo.setInsertPolicy(QComboBox.NoInsert)
    line_edit = combo.lineEdit()
    if line_edit is not None:
        # Parent the filter to the combo so it's cleaned up with it.
        filt = _SelectAllOnFocusFilter(combo)
        line_edit.installEventFilter(filt)
    completer = combo.completer()
    if completer:
        completer.setCompletionMode(QCompleter.PopupCompletion)
        completer.setFilterMode(Qt.MatchContains)


class InlineEventCreator(QFrame):
    """Compact inline widget for creating or editing events in the route list.

    **Create mode** (default): builds a new event and inserts it after the
    target row via ``controller.new_event()``.

    **Edit mode** (when *editing_group_id* is set): pre-populates from an
    existing ``EventDefinition``.  On confirm the built definition is stored
    in ``result_event_def`` and ``event_created`` is emitted — the caller
    (RouteList) is responsible for calling ``update_existing_event``.
    """

    event_created = Signal()
    discarded = Signal()

    def __init__(self, controller, insert_after_id, parent=None,
                 editing_group_id=None, editing_event_def=None):
        super().__init__(parent)
        self._controller = controller
        self._insert_after_id = insert_after_id
        self._current_type_key = None
        self._event_builder = None
        self._create_validator = None
        self._config_refs = {}

        # Edit-mode state
        self._editing_group_id = editing_group_id
        self._editing_event_def = editing_event_def
        self.result_event_def = None

        self.setObjectName("inlineEventCreator")
        self.setStyleSheet(
            "#inlineEventCreator {"
            "  background: #1e2d3d;"
            "  border: 1px solid #0078d4;"
            "  border-radius: 3px;"
            "}"
        )

        self.setMinimumHeight(34)

        self._layout = QHBoxLayout(self)
        self._layout.setContentsMargins(6, 3, 6, 3)
        self._layout.setSpacing(6)

        # --- Event type dropdown (searchable) -----------------------------
        self._type_combo = QComboBox()
        _make_searchable(self._type_combo)
        self._type_combo.setPlaceholderText("Event type...")
        self._type_combo.setMinimumWidth(110)
        self._type_combo.setMaximumWidth(140)
        for name, _ in _EVENT_TYPES:
            self._type_combo.addItem(name)
        self._type_combo.setCurrentIndex(-1)
        self._type_combo.currentIndexChanged.connect(self._on_type_changed)
        self._layout.addWidget(self._type_combo)

        # --- Dynamic config container -------------------------------------
        self._config_container = QWidget()
        self._config_layout = QHBoxLayout(self._config_container)
        self._config_layout.setContentsMargins(0, 0, 0, 0)
        self._config_layout.setSpacing(4)
        self._layout.addWidget(self._config_container)

        # --- Confirm / Discard buttons ------------------------------------
        self._create_btn = QPushButton("Confirm" if editing_group_id else "Create")
        self._create_btn.setFixedWidth(60)
        self._create_btn.setEnabled(False)
        self._create_btn.clicked.connect(self._on_create)
        self._layout.addWidget(self._create_btn)

        # Only relevant for trainer events with a real location selected.
        self._add_all_btn = QPushButton("Add all trainers")
        self._add_all_btn.setFixedWidth(110)
        self._add_all_btn.setVisible(False)
        self._add_all_btn.clicked.connect(self._on_add_all_trainers)
        self._layout.addWidget(self._add_all_btn)

        self._discard_btn = QPushButton("Discard")
        self._discard_btn.setFixedWidth(60)
        self._discard_btn.clicked.connect(lambda: self.discarded.emit())
        self._layout.addWidget(self._discard_btn)

        # Push everything to the left; empty space stays on the right.
        self._layout.addStretch(1)

        # --- If editing, pre-populate from existing definition ------------
        if self._editing_event_def is not None:
            self._init_from_event(self._editing_event_def)

    # ------------------------------------------------------------------
    # Public helpers
    # ------------------------------------------------------------------

    def focus_type_combo(self):
        """Focus the event-type combo so the user can start typing."""
        self._type_combo.setFocus()
        if self._type_combo.lineEdit():
            self._type_combo.lineEdit().selectAll()

    # Map from internal type key → the config-ref key for the "primary" field
    # that the user most likely wants to change when double-click-editing.
    _PRIMARY_FIELD_FOR_TYPE = {
        "trainer":   "trainer",
        "get_item":  "item",
        "buy_item":  "item",
        "sell_item": "item",
        "use_item":  "item",
        "hold_item": "item",
        "wild_pkmn": "pkmn",
        "vitamin":   "vitamin",
        "rare_candy": "qty",
        "tm_move":   "tm",
        "notes":     "note",
    }

    def focus_primary_field(self):
        """Focus the most relevant config field for the current event type.

        For an item event this selects the item dropdown, for a trainer event
        the trainer dropdown, etc.  Falls back to the type combo when no
        specific field is identified (simple types like Save/Heal)."""
        ref_key = self._PRIMARY_FIELD_FOR_TYPE.get(self._current_type_key)
        widget = self._config_refs.get(ref_key) if ref_key else None
        if widget is not None:
            widget.setFocus()
            # For QComboBox / searchable combos, selectAll is handled by the
            # _SelectAllOnFocusFilter already installed. For QLineEdit we do
            # it explicitly so the user can immediately start typing.
            if hasattr(widget, 'selectAll'):
                QTimer.singleShot(0, widget.selectAll)
        else:
            self.focus_type_combo()

    # ------------------------------------------------------------------
    # Type selection
    # ------------------------------------------------------------------

    def _on_type_changed(self, index):
        if index < 0 or index >= len(_EVENT_TYPES):
            self._create_btn.setEnabled(False)
            return
        _, key = _EVENT_TYPES[index]
        if key == self._current_type_key:
            return
        self._current_type_key = key
        self._clear_config()
        self._build_config(key)
        self._update_create_state()

    # ------------------------------------------------------------------
    # Edit-mode initialisation
    # ------------------------------------------------------------------

    def _init_from_event(self, event_def):
        """Select the correct type and populate config from *event_def*."""
        event_type = event_def.get_event_type()
        key = _EVENT_TYPE_TO_KEY.get(event_type)
        if key is None:
            return
        idx = _KEY_TO_INDEX.get(key)
        if idx is None:
            return
        # Setting the index triggers _on_type_changed → _build_config
        self._type_combo.setCurrentIndex(idx)
        # Now fill the config widgets with the existing values
        self._populate_config(event_def)

    def _populate_config(self, event_def):
        """Fill config widgets with values from an existing *event_def*."""
        refs = self._config_refs
        key = self._current_type_key

        if key == "trainer" and event_def.trainer_def:
            td = event_def.trainer_def
            trainer_obj = current_gen_info().trainer_db().get_trainer(td.trainer_name)
            if trainer_obj and trainer_obj.location and "loc" in refs:
                refs["loc"].setCurrentText(trainer_obj.location)
            if "trainer" in refs:
                refs["trainer"].setCurrentText(td.trainer_name)

        elif key in ("get_item", "buy_item", "sell_item", "use_item"):
            if event_def.item_event_def:
                ie = event_def.item_event_def
                if "item" in refs:
                    refs["item"].setCurrentText(ie.item_name)
                if "qty" in refs:
                    refs["qty"].set(ie.item_amount)

        elif key == "hold_item" and event_def.hold_item:
            if "item" in refs:
                refs["item"].setCurrentText(event_def.hold_item.item_name)

        elif key == "wild_pkmn" and event_def.wild_pkmn_info:
            wp = event_def.wild_pkmn_info
            if "pkmn" in refs:
                refs["pkmn"].setCurrentText(wp.name)
            if "level" in refs:
                refs["level"].set(wp.level)
            if "qty" in refs:
                refs["qty"].set(wp.quantity)

        elif key == "rare_candy" and event_def.rare_candy:
            if "qty" in refs:
                refs["qty"].set(event_def.rare_candy.amount)

        elif key == "vitamin" and event_def.vitamin:
            if "vitamin" in refs:
                refs["vitamin"].setCurrentText(event_def.vitamin.vitamin)
            if "qty" in refs:
                refs["qty"].set(event_def.vitamin.amount)

        elif key == "tm_move" and event_def.learn_move:
            if "tm" in refs:
                refs["tm"].setCurrentText(event_def.learn_move.source)
            if "dest" in refs and event_def.learn_move.destination is not None:
                idx = event_def.learn_move.destination + 1  # +1 for "Don't Learn"
                if idx < refs["dest"].count():
                    refs["dest"].setCurrentIndex(idx)

        elif key == "notes":
            if "note" in refs:
                refs["note"].setText(event_def.notes or "")

        self._update_create_state()

    # ------------------------------------------------------------------
    # Config area management
    # ------------------------------------------------------------------

    def _clear_config(self):
        while self._config_layout.count():
            child = self._config_layout.takeAt(0)
            w = child.widget()
            if w:
                w.setParent(None)
                w.deleteLater()
        self._event_builder = None
        self._create_validator = None
        self._config_refs = {}
        self._add_all_btn.setVisible(False)

    def _lbl(self, text):
        lbl = QLabel(text)
        lbl.setStyleSheet("background: transparent;")
        self._config_layout.addWidget(lbl)
        return lbl

    def _combo(self, items, min_w=120, max_w=200):
        c = QComboBox()
        _make_searchable(c)
        c.addItems(items)
        c.setMinimumWidth(min_w)
        c.setMaximumWidth(max_w)
        self._config_layout.addWidget(c)
        return c

    def _spin(self, lo=1, hi=999, val=1):
        s = AmountEntry(min_val=lo, max_val=hi, init_val=val)
        self._config_layout.addWidget(s)
        return s

    def _build_config(self, key):
        {
            "trainer":    self._cfg_trainer,
            "get_item":   lambda: self._cfg_item("get_item"),
            "buy_item":   lambda: self._cfg_item("buy_item"),
            "sell_item":  lambda: self._cfg_item("sell_item"),
            "use_item":   lambda: self._cfg_item("use_item"),
            "hold_item":  self._cfg_hold_item,
            "wild_pkmn":  self._cfg_wild_pkmn,
            "rare_candy": self._cfg_rare_candy,
            "vitamin":    self._cfg_vitamin,
            "tm_move":    self._cfg_tm_move,
            "notes":      self._cfg_notes,
        }.get(key, self._cfg_simple)()

    # ------------------------------------------------------------------
    # Validation & creation
    # ------------------------------------------------------------------

    def _update_create_state(self):
        if self._event_builder is None:
            self._create_btn.setEnabled(False)
        elif self._create_validator is not None:
            self._create_btn.setEnabled(self._create_validator())
        else:
            self._create_btn.setEnabled(True)

    def _on_create(self):
        if self._event_builder is None:
            return
        try:
            ev = self._event_builder()
            if ev is None:
                return
            if self._editing_group_id is not None:
                # Edit mode — store result; RouteList handles the update.
                self.result_event_def = ev
            else:
                # Create mode — insert directly.
                self._controller.new_event(ev, insert_after=self._insert_after_id)
            self.event_created.emit()
        except Exception as e:
            logger.error(f"Inline event creation failed: {e}")

    def _on_add_all_trainers(self):
        """Add every trainer at the currently-selected location to the route."""
        loc_widget = self._config_refs.get("loc")
        if loc_widget is None:
            return
        location = loc_widget.currentText()
        if not location or location == const.ALL_TRAINERS:
            return
        try:
            self._controller.add_area(location, False, self._insert_after_id)
            self.discarded.emit()
        except Exception as e:
            logger.error(f"Add all trainers failed: {e}")

    # ==================================================================
    # Config builders – one per event category
    # ==================================================================

    # ---- Trainer -----------------------------------------------------

    def _cfg_trainer(self):
        self._lbl("Loc:")
        locs = [const.ALL_TRAINERS] + sorted(
            current_gen_info().trainer_db().get_all_locations()
        )
        loc_c = self._combo(locs, 110)

        self._lbl("Class:")
        classes = [const.ALL_TRAINERS] + sorted(
            current_gen_info().trainer_db().get_all_classes()
        )
        cls_c = self._combo(classes, 100)

        self._lbl("Trainer:")
        tr_c = self._combo([], 160)

        def refresh():
            defeated = self._controller.get_defeated_trainers()
            trainers = current_gen_info().trainer_db().get_valid_trainers(
                trainer_loc=loc_c.currentText(),
                trainer_class=cls_c.currentText(),
                defeated_trainers=defeated,
            )
            if not trainers:
                trainers = [const.NO_TRAINERS]
            tr_c.blockSignals(True)
            tr_c.clear()
            tr_c.addItems(trainers)
            tr_c.blockSignals(False)
            self._add_all_btn.setVisible(
                self._editing_group_id is None
                and loc_c.currentText() != const.ALL_TRAINERS
            )
            self._update_create_state()

        loc_c.currentIndexChanged.connect(refresh)
        cls_c.currentIndexChanged.connect(refresh)
        tr_c.currentIndexChanged.connect(lambda: self._update_create_state())
        refresh()

        self._config_refs = {"loc": loc_c, "cls": cls_c, "trainer": tr_c}
        self._event_builder = lambda: (
            EventDefinition(trainer_def=TrainerEventDefinition(tr_c.currentText()))
            if tr_c.currentText() != const.NO_TRAINERS else None
        )
        self._create_validator = lambda: tr_c.currentText() != const.NO_TRAINERS

    # ---- Item (get / buy / sell / use-drop) --------------------------

    def _cfg_item(self, subtype):
        self._lbl("Item:")
        items = current_gen_info().item_db().get_filtered_names()
        if not items:
            items = [const.NO_ITEM]
        item_c = self._combo(items, 130)

        self._lbl("Qty:")
        qty = self._spin()

        item_c.currentIndexChanged.connect(lambda: self._update_create_state())

        self._config_refs = {"item": item_c, "qty": qty}

        def builder():
            n = item_c.currentText()
            q = int(qty.get())
            if n == const.NO_ITEM:
                return None
            if subtype == "get_item":
                return EventDefinition(
                    item_event_def=InventoryEventDefinition(n, q, True, False)
                )
            if subtype == "buy_item":
                return EventDefinition(
                    item_event_def=InventoryEventDefinition(n, q, True, True)
                )
            if subtype == "sell_item":
                return EventDefinition(
                    item_event_def=InventoryEventDefinition(n, q, False, True)
                )
            # use_item
            if n in current_gen_info().get_valid_vitamins():
                return EventDefinition(
                    vitamin=VitaminEventDefinition(n, q)
                )
            if n == const.RARE_CANDY:
                return EventDefinition(
                    rare_candy=RareCandyEventDefinition(q)
                )
            return EventDefinition(
                item_event_def=InventoryEventDefinition(n, q, False, False)
            )

        self._event_builder = builder
        self._create_validator = lambda: item_c.currentText() != const.NO_ITEM

    # ---- Hold item ---------------------------------------------------

    def _cfg_hold_item(self):
        self._lbl("Item:")
        items = current_gen_info().item_db().get_filtered_names()
        if not items:
            items = [const.NO_ITEM]
        item_c = self._combo(items, 130)
        item_c.currentIndexChanged.connect(lambda: self._update_create_state())

        self._config_refs = {"item": item_c}
        self._event_builder = lambda: (
            EventDefinition(hold_item=HoldItemEventDefinition(item_c.currentText()))
            if item_c.currentText() != const.NO_ITEM else None
        )
        self._create_validator = lambda: item_c.currentText() != const.NO_ITEM

    # ---- Wild Pkmn ---------------------------------------------------

    def _cfg_wild_pkmn(self):
        self._lbl("Pkmn:")
        names = current_gen_info().pkmn_db().get_all_names()
        pkmn_c = self._combo(names, 120)

        self._lbl("Lv:")
        lv = self._spin(2, 100, 5)

        self._lbl("Qty:")
        qty = self._spin(1, 999, 1)

        self._config_refs = {"pkmn": pkmn_c, "level": lv, "qty": qty}
        self._event_builder = lambda: EventDefinition(
            wild_pkmn_info=WildPkmnEventDefinition(
                pkmn_c.currentText(), int(lv.get()), quantity=int(qty.get()),
            )
        )

    # ---- Rare Candy --------------------------------------------------

    def _cfg_rare_candy(self):
        self._lbl("Qty:")
        qty = self._spin(1, 999, 1)
        self._config_refs = {"qty": qty}
        self._event_builder = lambda: EventDefinition(
            rare_candy=RareCandyEventDefinition(int(qty.get()))
        )

    # ---- Vitamin -----------------------------------------------------

    def _cfg_vitamin(self):
        self._lbl("Vitamin:")
        vits = current_gen_info().get_valid_vitamins()
        vit_c = self._combo(vits, 100)

        self._lbl("Qty:")
        qty = self._spin(1, 999, 1)

        self._config_refs = {"vitamin": vit_c, "qty": qty}
        self._event_builder = lambda: EventDefinition(
            vitamin=VitaminEventDefinition(vit_c.currentText(), int(qty.get()))
        )

    # ---- TM / HM Move -----------------------------------------------

    def _cfg_tm_move(self):
        self._lbl("TM/HM:")
        tms = current_gen_info().item_db().get_filtered_names(
            item_type=const.ITEM_TYPE_TM,
        )
        if not tms or tms[0] == const.NO_ITEM:
            tms = [const.NO_ITEM]
        tm_c = self._combo(tms, 130)

        # Move-slot destination dropdown
        self._lbl("Over:")
        dest_c = self._combo([], 140)

        def _refresh_dest():
            """Rebuild the destination dropdown from the current moveset."""
            st = self._controller.get_active_state()
            move_list = st.solo_pkmn.move_list if st and st.solo_pkmn else []
            options = [const.MOVE_DONT_LEARN] + [
                const.MOVE_SLOT_TEMPLATE.format(idx + 1, m)
                for idx, m in enumerate(move_list)
            ]
            dest_c.blockSignals(True)
            dest_c.clear()
            dest_c.addItems(options)

            # Auto-select the best slot for the chosen TM's move.
            tm = tm_c.currentText()
            if tm != const.NO_ITEM:
                item_obj = current_gen_info().item_db().get_item(tm)
                if item_obj and st and st.solo_pkmn:
                    info = st.solo_pkmn.get_move_destination(
                        item_obj.move_name, None,
                    )
                    if info[0] is not None and info[0] + 1 < len(options):
                        dest_c.setCurrentIndex(info[0] + 1)

            dest_c.blockSignals(False)
            self._update_create_state()

        tm_c.currentIndexChanged.connect(_refresh_dest)
        tm_c.currentIndexChanged.connect(lambda: self._update_create_state())
        _refresh_dest()

        self._config_refs = {"tm": tm_c, "dest": dest_c}

        def builder():
            tm = tm_c.currentText()
            if tm == const.NO_ITEM:
                return None
            item_obj = current_gen_info().item_db().get_item(tm)
            if item_obj is None:
                return None
            move = item_obj.move_name

            # Parse destination from the dropdown selection.
            dest_text = dest_c.currentText()
            if dest_text == const.MOVE_DONT_LEARN:
                dest = None
            else:
                try:
                    dest = int(dest_text.split("#")[1][0]) - 1
                except Exception:
                    dest = 0

            return EventDefinition(
                learn_move=LearnMoveEventDefinition(move, dest, tm)
            )

        self._event_builder = builder
        self._create_validator = lambda: tm_c.currentText() != const.NO_ITEM

    # ---- Notes -------------------------------------------------------

    def _cfg_notes(self):
        self._lbl("Note:")
        entry = QLineEdit()
        entry.setMinimumWidth(200)
        entry.setPlaceholderText("Enter note...")
        self._config_layout.addWidget(entry)

        self._config_refs = {"note": entry}
        self._event_builder = lambda: EventDefinition(notes=entry.text())

    # ---- No-config events (save / heal / blackout / evolve) ----------

    def _cfg_simple(self):
        key = self._current_type_key

        def builder():
            if key == "save":
                return EventDefinition(save=SaveEventDefinition())
            if key == "heal":
                return EventDefinition(heal=HealEventDefinition())
            if key == "blackout":
                return EventDefinition(blackout=BlackoutEventDefinition())
            if key == "evolve":
                st = self._controller.get_active_state()
                species = st.solo_pkmn.name if st and st.solo_pkmn else ""
                return EventDefinition(
                    evolution=EvolutionEventDefinition(species)
                )
            return None

        self._event_builder = builder
