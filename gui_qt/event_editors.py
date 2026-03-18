from __future__ import annotations
import logging

from PySide6.QtWidgets import (
    QWidget, QLabel, QVBoxLayout, QHBoxLayout, QGridLayout,
    QPlainTextEdit, QSizePolicy, QFrame,
)
from PySide6.QtCore import Qt, Signal

from gui_qt.components.custom_components import (
    SimpleEntry, SimpleOptionMenu, SimpleButton, AmountEntry, CheckboxLabel,
)
from utils.constants import const
from utils.config_manager import config
from pkmn.gen_factory import current_gen_info
from routing.route_events import (
    BlackoutEventDefinition,
    EventDefinition,
    EvolutionEventDefinition,
    HealEventDefinition,
    HoldItemEventDefinition,
    InventoryEventDefinition,
    LearnMoveEventDefinition,
    RareCandyEventDefinition,
    SaveEventDefinition,
    TrainerEventDefinition,
    VitaminEventDefinition,
    WildPkmnEventDefinition,
)
from routing import full_route_state

logger = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Decorator -- suppresses save callbacks while loading data into the editors
# ---------------------------------------------------------------------------
def ignore_updates(load_fn):
    """Must wrap an instance method from the EventEditorBase class."""
    def wrapper(*args, **kwargs):
        editor: EventEditorBase = args[0]
        editor._ignoring_updates = True
        try:
            load_fn(*args, **kwargs)
        except Exception as e:
            logger.error(f"Trying to run function: {load_fn}, got error: {e}")
            logger.exception(e)
            raise
        finally:
            editor._ignoring_updates = False
    return wrapper


# ---------------------------------------------------------------------------
# EditorParams
# ---------------------------------------------------------------------------
class EditorParams:
    def __init__(self, event_type, cur_defeated_trainers, cur_state):
        self.event_type = event_type
        self.cur_defeated_trainers = cur_defeated_trainers
        self.cur_state = cur_state


# ---------------------------------------------------------------------------
# EventEditorBase
# ---------------------------------------------------------------------------
class EventEditorBase(QWidget):
    """Base class for all event-type editors."""

    def __init__(self, editor_params: EditorParams, notes_visibility_callback=None, parent=None):
        super().__init__(parent)
        self.editor_params = editor_params
        self._save_callback = None
        self._delayed_save_callback = None
        self._ignoring_updates = False
        self._notes_visibility_callback = notes_visibility_callback

        # Grid layout is the default; subclasses add rows to it
        self._layout = QGridLayout(self)
        self._layout.setContentsMargins(4, 4, 4, 4)
        self._layout.setSpacing(4)
        self._cur_row = 0

    # -- public API used by EventEditorFactory / EventDetails ---------------
    def configure(self, editor_params, save_callback=None, delayed_save_callback=None):
        self._save_callback = save_callback
        self._delayed_save_callback = delayed_save_callback
        self.editor_params = editor_params

    def load_event(self, event_def: EventDefinition):
        pass

    def get_event(self) -> EventDefinition:
        return None

    def enable(self):
        pass

    def disable(self):
        pass

    # -- internal helpers ---------------------------------------------------
    def _trigger_save(self, *args, **kwargs):
        if self._ignoring_updates:
            return
        if self._save_callback is not None:
            self._save_callback()

    def _trigger_delayed_save(self, *args, **kwargs):
        if self._ignoring_updates:
            return
        if self._delayed_save_callback is not None:
            self._delayed_save_callback()


# ---------------------------------------------------------------------------
# NotesEditor
# ---------------------------------------------------------------------------
class NotesEditor(EventEditorBase):
    def __init__(self, editor_params: EditorParams, notes_visibility_callback=None, parent=None):
        super().__init__(editor_params, notes_visibility_callback=notes_visibility_callback, parent=parent)

        # Row 0: label + visibility dropdown
        self._notes_label = QLabel("Battle summary notes:")
        self._layout.addWidget(self._notes_label, self._cur_row, 0)

        notes_options = [
            "Show notes in battle summary when space allows",
            "Show notes in battle summary at all times",
            "Never show notes in battle summary",
        ]
        self._notes_mode_map = {
            notes_options[0]: "when_space_allows",
            notes_options[1]: "always",
            notes_options[2]: "never",
        }
        self._notes_mode_reverse = {v: k for k, v in self._notes_mode_map.items()}

        current_mode = config.get_notes_visibility_mode()
        current_option = self._notes_mode_reverse.get(current_mode, notes_options[0])

        self._notes_visibility_dropdown = SimpleOptionMenu(notes_options, default_val=current_option, callback=self._on_notes_visibility_changed)
        self._layout.addWidget(self._notes_visibility_dropdown, self._cur_row, 1)
        self._cur_row += 1

        # Row 1: notes text area
        self._notes = QPlainTextEdit()
        self._notes.setMaximumHeight(160)
        self._notes.setMinimumHeight(80)
        self._notes.textChanged.connect(self._trigger_delayed_save)
        self._layout.addWidget(self._notes, self._cur_row, 0, 1, 4)
        self._cur_row += 1

        self._layout.setColumnStretch(0, 1)
        self._layout.setColumnStretch(2, 1)

    # ---- overrides --------------------------------------------------------
    @ignore_updates
    def configure(self, editor_params, save_callback=None, delayed_save_callback=None):
        super().configure(editor_params, save_callback=save_callback, delayed_save_callback=delayed_save_callback)

    @ignore_updates
    def load_event(self, event_def: EventDefinition):
        self._notes.blockSignals(True)
        self._notes.clear()
        if event_def is not None:
            self._notes.setPlainText(event_def.notes)
        self._notes.blockSignals(False)
        current_mode = config.get_notes_visibility_mode()
        current_option = self._notes_mode_reverse.get(current_mode, list(self._notes_mode_map.keys())[0])
        self._notes_visibility_dropdown.set(current_option)

    def get_event(self) -> EventDefinition:
        return EventDefinition(notes=self._notes.toPlainText().strip())

    def enable(self):
        self._notes.setEnabled(True)

    def disable(self):
        self._notes.setEnabled(False)

    # ---- visibility helpers -----------------------------------------------
    def _on_notes_visibility_changed(self):
        selected = self._notes_visibility_dropdown.get()
        mode = self._notes_mode_map.get(selected, "when_space_allows")
        config.set_notes_visibility_mode(mode)
        if self._notes_visibility_callback is not None:
            self._notes_visibility_callback()


# ---------------------------------------------------------------------------
# TrainerFightEditor
# ---------------------------------------------------------------------------
class TrainerFightEditor(EventEditorBase):
    VAR_COUNTER = 0
    EXP_PER_SEC_TEXT = "Optimal exp per second (4x speed): "

    def __init__(self, editor_params: EditorParams, notes_visibility_callback=None, parent=None):
        super().__init__(editor_params, notes_visibility_callback=notes_visibility_callback, parent=parent)
        self._cur_trainer = None
        self._second_trainer = None
        self._num_pkmn = 0

        # Header frame --------------------------------------------------
        header = QWidget()
        header_lay = QHBoxLayout(header)
        header_lay.setContentsMargins(0, 0, 0, 0)

        self._exp_per_sec_label = QLabel(self.EXP_PER_SEC_TEXT)
        self._pay_day_label = QLabel("Pay Day Amount: ")
        self._pay_day_value = SimpleEntry(callback=self._trigger_save, parent=self)
        self._pay_day_value.setFixedWidth(60)

        header_lay.addStretch(1)
        header_lay.addWidget(self._pay_day_label)
        header_lay.addWidget(self._pay_day_value)
        header_lay.addStretch(1)
        header_lay.addWidget(self._exp_per_sec_label)
        header_lay.addStretch(1)

        self._layout.addWidget(header, self._cur_row, 0, 1, 4)
        self._cur_row += 1

        # Info grid for up to 6 pokemon ---------------------------------
        self._info_frame = QWidget()
        self._info_layout = QGridLayout(self._info_frame)
        self._info_layout.setContentsMargins(0, 0, 0, 0)
        self._info_layout.setSpacing(4)
        self._layout.addWidget(self._info_frame, self._cur_row, 0, 1, 4)
        self._cur_row += 1

        # Pre-create labels for up to 6 pokemon
        self._all_pkmn_labels: list[QLabel] = []
        self._all_exp_labels: list[QLabel] = []
        self._all_exp_splits: list[SimpleOptionMenu] = []
        self._all_order_labels: list[QLabel] = []
        self._all_order_menus: list[SimpleOptionMenu] = []

        for idx in range(6):
            pkmn_lbl = QLabel()
            pkmn_lbl.setWordWrap(True)
            pkmn_lbl.setMinimumWidth(160)
            self._all_pkmn_labels.append(pkmn_lbl)

            exp_lbl = QLabel("Exp Split:")
            self._all_exp_labels.append(exp_lbl)

            exp_menu = SimpleOptionMenu(
                option_list=["1", "2", "3", "4", "5", "6"],
                callback=self._trigger_save,
                parent=self,
            )
            exp_menu.setFixedWidth(55)
            self._all_exp_splits.append(exp_menu)

            order_lbl = QLabel("Mon Order")
            self._all_order_labels.append(order_lbl)

            order_menu = SimpleOptionMenu(
                option_list=["1", "2", "3", "4", "5", "6"],
                callback=lambda _idx=idx: self._reorder_mons(_idx),
                parent=self,
            )
            order_menu.setFixedWidth(55)
            self._all_order_menus.append(order_menu)

        self._cached_definition_order = []

    # ---- overrides --------------------------------------------------------
    @ignore_updates
    def configure(self, editor_params, save_callback=None, delayed_save_callback=None):
        super().configure(editor_params, save_callback=save_callback, delayed_save_callback=delayed_save_callback)

    @ignore_updates
    def load_event(self, event_def):
        self._cur_trainer = event_def.trainer_def.trainer_name
        self._second_trainer = event_def.trainer_def.second_trainer_name
        try:
            pay_day_val = int(event_def.trainer_def.pay_day_amount)
        except Exception:
            pay_day_val = 0

        self._exp_per_sec_label.setText(f"{self.EXP_PER_SEC_TEXT} {event_def.experience_per_second()}")
        self._pay_day_value.set(pay_day_val)

        enemy_pkmn_ordered = event_def.get_pokemon_list()
        self._num_pkmn = len(enemy_pkmn_ordered)
        order_values = [str(v) for v in range(1, len(enemy_pkmn_ordered) + 1)]

        self._cached_definition_order = [x.mon_order - 1 for x in event_def.get_pokemon_list(definition_order=True)]

        cur_state: full_route_state.RouteState = self.editor_params.cur_state

        idx = -1
        for idx, cur_pkmn in enumerate(enemy_pkmn_ordered):
            if cur_state is not None:
                if cur_state.solo_pkmn.cur_stats.speed > cur_pkmn.cur_stats.speed:
                    speed_class = "success"
                elif cur_state.solo_pkmn.cur_stats.speed == cur_pkmn.cur_stats.speed:
                    speed_class = "warning"
                else:
                    speed_class = "failure"
                cur_state = cur_state.defeat_pkmn(cur_pkmn)[0]
            else:
                speed_class = "contrast"

            # Build a table-style HTML summary for each enemy pokemon
            stats = cur_pkmn.cur_stats
            xp_val = cur_pkmn.xp if hasattr(cur_pkmn, 'xp') and cur_pkmn.xp else ""
            spe_color = {"success": "#4ec97a", "warning": "#e8b730", "failure": "#e05555"}.get(speed_class, "#d4d4d4")

            moves = []
            if hasattr(cur_pkmn, 'move_list') and cur_pkmn.move_list:
                moves = [m for m in cur_pkmn.move_list if m]

            rows = [
                ("HP:", stats.hp, f"Lv: {cur_pkmn.level}"),
                ("Atk:", stats.attack, f"Exp: {xp_val}" if xp_val else ""),
                ("Def:", stats.defense, f"Move 1: {moves[0]}" if len(moves) > 0 else ""),
                ("SpA:", stats.special_attack, f"Move 2: {moves[1]}" if len(moves) > 1 else ""),
                ("SpD:", stats.special_defense, f"Move 3: {moves[2]}" if len(moves) > 2 else ""),
                ("Spe:", stats.speed, f"Move 4: {moves[3]}" if len(moves) > 3 else ""),
            ]

            html = '<table cellspacing="0" cellpadding="1">'
            html += f'<tr><td colspan="3"><b>{cur_pkmn.name}</b></td></tr>'

            if hasattr(cur_pkmn, 'ability') and cur_pkmn.ability:
                nature_str = ""
                if hasattr(cur_pkmn, 'nature') and cur_pkmn.nature and str(cur_pkmn.nature) != "Hardy":
                    nature_str = f" ({cur_pkmn.nature})"
                html += f'<tr><td colspan="3">{cur_pkmn.ability}{nature_str}</td></tr>'
            if hasattr(cur_pkmn, 'held_item') and cur_pkmn.held_item:
                html += f'<tr><td colspan="3">Item: {cur_pkmn.held_item}</td></tr>'

            for sn, sv, right_col in rows:
                if sn == "Spe:":
                    style_l = f'style="color:{spe_color}; padding-right:4px;"'
                    style_r = f'style="color:{spe_color}; padding-right:12px; text-align:right;"'
                else:
                    style_l = 'style="padding-right:4px;"'
                    style_r = 'style="padding-right:12px; text-align:right;"'
                html += (
                    f'<tr>'
                    f'<td {style_l}>{sn}</td>'
                    f'<td {style_r}>{sv}</td>'
                    f'<td>{right_col}</td>'
                    f'</tr>'
                )

            html += '</table>'
            self._all_pkmn_labels[idx].setText(html)

            row_idx = (2 * (idx // 3))
            col_idx = 4 * (idx % 3)

            self._all_pkmn_labels[idx].setVisible(True)
            self._info_layout.addWidget(self._all_pkmn_labels[idx], row_idx, col_idx, 1, 4)

            self._all_order_labels[idx].setVisible(True)
            self._info_layout.addWidget(self._all_order_labels[idx], row_idx + 1, col_idx)
            self._all_order_menus[idx].new_values(order_values)
            self._all_order_menus[idx].set(str(cur_pkmn.mon_order))
            self._all_order_menus[idx].setVisible(True)
            self._info_layout.addWidget(self._all_order_menus[idx], row_idx + 1, col_idx + 1)

            self._all_exp_labels[idx].setVisible(True)
            self._info_layout.addWidget(self._all_exp_labels[idx], row_idx + 1, col_idx + 2)
            self._all_exp_splits[idx].set(str(cur_pkmn.exp_split))
            self._all_exp_splits[idx].setVisible(True)
            self._info_layout.addWidget(self._all_exp_splits[idx], row_idx + 1, col_idx + 3)

        for missing_idx in range(idx + 1, 6):
            self._all_pkmn_labels[missing_idx].setVisible(False)
            self._all_exp_labels[missing_idx].setVisible(False)
            self._all_exp_splits[missing_idx].setVisible(False)
            self._all_order_labels[missing_idx].setVisible(False)
            self._all_order_menus[missing_idx].setVisible(False)
            self._all_order_menus[missing_idx].blockSignals(True)
            self._all_order_menus[missing_idx].set("-1")
            self._all_order_menus[missing_idx].blockSignals(False)

    def _reorder_mons(self, updated_idx):
        if self._ignoring_updates:
            return

        self._ignoring_updates = True
        try:
            adjusted_val = int(self._all_order_menus[updated_idx].get())

            # Collect other menus sorted by their current value
            ordered_indices = []
            for i in range(self._num_pkmn):
                if i == updated_idx:
                    continue
                val = int(self._all_order_menus[i].get())
                if val == -1:
                    continue
                ordered_indices.append((val, i))
            ordered_indices.sort()

            new_order_idx = 1
            oi_pos = 0
            while new_order_idx <= self._num_pkmn:
                if new_order_idx == adjusted_val:
                    new_order_idx += 1
                    continue
                if oi_pos < len(ordered_indices):
                    self._all_order_menus[ordered_indices[oi_pos][1]].blockSignals(True)
                    self._all_order_menus[ordered_indices[oi_pos][1]].set(str(new_order_idx))
                    self._all_order_menus[ordered_indices[oi_pos][1]].blockSignals(False)
                    oi_pos += 1
                new_order_idx += 1
        finally:
            self._ignoring_updates = False

        self._trigger_save()
        self.load_event(self.get_event())

    def get_event(self):
        exp_split = [int(self._all_exp_splits[x].get()) for x in self._cached_definition_order]
        mon_order = [int(self._all_order_menus[x].get()) for x in self._cached_definition_order]
        try:
            pay_day_amount = int(self._pay_day_value.get())
        except Exception:
            pay_day_amount = 0
        return EventDefinition(
            trainer_def=TrainerEventDefinition(
                self._cur_trainer,
                second_trainer_name=self._second_trainer,
                exp_split=exp_split,
                pay_day_amount=pay_day_amount,
                mon_order=mon_order,
            )
        )

    def enable(self):
        self._pay_day_value.enable()
        for s in self._all_exp_splits:
            s.enable()
        for o in self._all_order_menus:
            o.enable()

    def disable(self):
        self._pay_day_value.disable()
        for s in self._all_exp_splits:
            s.disable()
        for o in self._all_order_menus:
            o.disable()


# ---------------------------------------------------------------------------
# VitaminEditor
# ---------------------------------------------------------------------------
class VitaminEditor(EventEditorBase):
    def __init__(self, editor_params: EditorParams, notes_visibility_callback=None, parent=None):
        super().__init__(editor_params, notes_visibility_callback=notes_visibility_callback, parent=parent)

        self._vitamin_label = QLabel("Vitamin Type:")
        self._vitamin_types = SimpleOptionMenu(option_list=[const.NO_ITEM], callback=self._trigger_save, parent=self)
        self._layout.addWidget(self._vitamin_label, self._cur_row, 0)
        self._layout.addWidget(self._vitamin_types, self._cur_row, 1)
        self._cur_row += 1

        self._item_amount_label = QLabel("Num Vitamins:")
        self._item_amount = AmountEntry(min_val=1, callback=self._amount_update, width=5, parent=self)
        self._layout.addWidget(self._item_amount_label, self._cur_row, 0)
        self._layout.addWidget(self._item_amount, self._cur_row, 1)
        self._cur_row += 1

    def _amount_update(self):
        try:
            if int(self._item_amount.get()) > 0:
                self._trigger_save()
        except Exception:
            pass

    @ignore_updates
    def configure(self, editor_params, save_callback=None, delayed_save_callback=None):
        self._vitamin_types.new_values(current_gen_info().get_valid_vitamins())
        super().configure(editor_params, save_callback=save_callback, delayed_save_callback=delayed_save_callback)

    @ignore_updates
    def load_event(self, event_def):
        self._vitamin_types.set(event_def.vitamin.vitamin)
        self._item_amount.set(str(event_def.vitamin.amount))

    def get_event(self):
        return EventDefinition(vitamin=VitaminEventDefinition(self._vitamin_types.get(), int(self._item_amount.get())))

    def enable(self):
        self._vitamin_types.enable()
        self._item_amount.enable()

    def disable(self):
        self._vitamin_types.disable()
        self._item_amount.disable()


# ---------------------------------------------------------------------------
# RareCandyEditor
# ---------------------------------------------------------------------------
class RareCandyEditor(EventEditorBase):
    def __init__(self, editor_params: EditorParams, notes_visibility_callback=None, parent=None):
        super().__init__(editor_params, notes_visibility_callback=notes_visibility_callback, parent=parent)

        self._item_amount_label = QLabel("Num Rare Candies:")
        self._item_amount = AmountEntry(min_val=1, callback=self._amount_update, width=5, parent=self)
        self._layout.addWidget(self._item_amount_label, self._cur_row, 0)
        self._layout.addWidget(self._item_amount, self._cur_row, 1)
        self._cur_row += 1

    def _amount_update(self):
        try:
            if int(self._item_amount.get()) > 0:
                self._trigger_save()
        except Exception:
            pass

    @ignore_updates
    def load_event(self, event_def):
        self._item_amount.set(str(event_def.rare_candy.amount))

    def get_event(self):
        return EventDefinition(rare_candy=RareCandyEventDefinition(int(self._item_amount.get())))

    def enable(self):
        self._item_amount.enable()

    def disable(self):
        self._item_amount.disable()


# ---------------------------------------------------------------------------
# LearnMoveEditor
# ---------------------------------------------------------------------------
class LearnMoveEditor(EventEditorBase):
    def __init__(self, editor_params: EditorParams, notes_visibility_callback=None, parent=None):
        super().__init__(editor_params, notes_visibility_callback=notes_visibility_callback, parent=parent)

        val_width = 180

        self._move_name_label = QLabel()
        self._layout.addWidget(self._move_name_label, self._cur_row, 0, 1, 2)
        self._cur_row += 1

        self._destination_label = QLabel("Move Destination:")
        self._destination = SimpleOptionMenu(option_list=[""], callback=self._trigger_save, parent=self)
        self._destination.setMinimumWidth(val_width)
        self._layout.addWidget(self._destination_label, self._cur_row, 0)
        self._layout.addWidget(self._destination, self._cur_row, 1)
        self._cur_row += 1

        self._move = None
        self._level = const.LEVEL_ANY
        self._mon = None

        self._source_label = QLabel("Move Source")
        self._source = SimpleOptionMenu(option_list=[""], callback=self._move_source_callback, parent=self)
        self._source.setMinimumWidth(val_width)
        self._layout.addWidget(self._source_label, self._cur_row, 0)
        self._layout.addWidget(self._source, self._cur_row, 1)
        self._cur_row += 1

        # TM/HM filter + selector
        self._item_filter_label = QLabel("Item Name Filter:")
        self._item_filter = SimpleEntry(callback=self._item_filter_callback, parent=self)
        self._item_filter.setMinimumWidth(val_width)
        self._item_filter_row = self._cur_row
        self._cur_row += 1

        self._item_selector_label = QLabel("Move:")
        self._item_selector = SimpleOptionMenu(option_list=[""], callback=self._move_selected_callback, parent=self)
        self._item_selector.setMinimumWidth(val_width)
        self._item_selector_row = self._cur_row
        self._cur_row += 1

        # Tutor filter + selector
        self._move_filter_label = QLabel("Move Filter:")
        self._move_filter = SimpleEntry(callback=self._move_filter_callback, parent=self)
        self._move_filter.setMinimumWidth(val_width)
        self._move_filter_row = self._cur_row
        self._cur_row += 1

        self._move_selector_label = QLabel("Move:")
        self._move_selector = SimpleOptionMenu(option_list=[""], callback=self._move_selected_callback, parent=self)
        self._move_selector.setMinimumWidth(val_width)
        self._move_selector_row = self._cur_row
        self._cur_row += 1

        # Initially hide the dynamic rows
        self._hide_tm_widgets()
        self._hide_tutor_widgets()

    def _hide_tm_widgets(self):
        self._item_filter_label.setVisible(False)
        self._item_filter.setVisible(False)
        self._item_selector_label.setVisible(False)
        self._item_selector.setVisible(False)

    def _show_tm_widgets(self):
        self._layout.addWidget(self._item_filter_label, self._item_filter_row, 0)
        self._layout.addWidget(self._item_filter, self._item_filter_row, 1)
        self._layout.addWidget(self._item_selector_label, self._item_selector_row, 0)
        self._layout.addWidget(self._item_selector, self._item_selector_row, 1)
        self._item_filter_label.setVisible(True)
        self._item_filter.setVisible(True)
        self._item_selector_label.setVisible(True)
        self._item_selector.setVisible(True)

    def _hide_tutor_widgets(self):
        self._move_filter_label.setVisible(False)
        self._move_filter.setVisible(False)
        self._move_selector_label.setVisible(False)
        self._move_selector.setVisible(False)

    def _show_tutor_widgets(self):
        self._layout.addWidget(self._move_filter_label, self._move_filter_row, 0)
        self._layout.addWidget(self._move_filter, self._move_filter_row, 1)
        self._layout.addWidget(self._move_selector_label, self._move_selector_row, 0)
        self._layout.addWidget(self._move_selector, self._move_selector_row, 1)
        self._move_filter_label.setVisible(True)
        self._move_filter.setVisible(True)
        self._move_selector_label.setVisible(True)
        self._move_selector.setVisible(True)

    def _move_source_callback(self):
        new_source = self._source.get()
        if new_source == const.MOVE_SOURCE_LEVELUP:
            return

        if new_source == const.MOVE_SOURCE_TM_HM:
            self._show_tm_widgets()
            self._hide_tutor_widgets()
            self._item_filter_callback()
        else:
            self._hide_tm_widgets()
            self._show_tutor_widgets()
            self._move_filter_callback()

        self._trigger_save()

    def _item_filter_callback(self):
        new_vals = current_gen_info().item_db().get_filtered_names(item_type=const.ITEM_TYPE_TM)
        item_filter_val = self._item_filter.get().strip().lower()
        if item_filter_val:
            new_vals = [x for x in new_vals if item_filter_val in x.lower()]
        if not new_vals:
            new_vals = [const.NO_ITEM]
        self._item_selector.new_values(new_vals)

    def _move_filter_callback(self):
        self._move_selector.new_values(
            current_gen_info().move_db().get_filtered_names(filter=self._move_filter.get(), include_delete_move=True)
        )

    def _move_selected_callback(self):
        if self._source.get() == const.MOVE_SOURCE_TM_HM:
            item_obj = current_gen_info().item_db().get_item(self._item_selector.get())
            if item_obj is not None:
                self._move = item_obj.move_name
            else:
                self._move = None
            self._move_name_label.setText(f"Move: {self._move}")
        elif self._source.get() == const.MOVE_SOURCE_TUTOR:
            self._move = self._move_selector.get()
            if self._move == const.DELETE_MOVE:
                self._move = None
            self._move_name_label.setText(f"Move: {self._move}")

        learn_move_info = self.editor_params.cur_state.solo_pkmn.get_move_destination(self._move, None)
        if not learn_move_info[1]:
            if learn_move_info[0] is None:
                self._destination.set(const.MOVE_DONT_LEARN)
            else:
                self._destination.set(const.MOVE_SLOT_TEMPLATE.format(learn_move_info[0] + 1, None))
            self._destination.disable()
        else:
            self._destination.enable()

        self._trigger_save()

    @ignore_updates
    def configure(self, editor_params, save_callback=None, delayed_save_callback=None):
        super().configure(editor_params, save_callback=save_callback, delayed_save_callback=delayed_save_callback)
        self._destination.new_values(
            [const.MOVE_DONT_LEARN] +
            [
                const.MOVE_SLOT_TEMPLATE.format(idx + 1, x) for idx, x in
                enumerate(self.editor_params.cur_state.solo_pkmn.move_list)
            ]
        )

        self._hide_tm_widgets()
        self._hide_tutor_widgets()

        if self.editor_params.event_type == const.TASK_LEARN_MOVE_LEVELUP:
            self._source.new_values([const.MOVE_SOURCE_LEVELUP])
            self._source.disable()
        else:
            self._source.new_values([const.MOVE_SOURCE_TUTOR, const.MOVE_SOURCE_TM_HM])
            self._source.enable()

        self._item_filter_callback()
        self._move_selected_callback()

    @ignore_updates
    def load_event(self, event_def):
        if self.editor_params.event_type == const.TASK_LEARN_MOVE_LEVELUP:
            self._move = event_def.learn_move.move_to_learn
            self._move_name_label.setText(f"Move: {self._move}")
            self._level = event_def.learn_move.level
            self._mon = event_def.learn_move.mon
        else:
            if event_def.learn_move.source == const.MOVE_SOURCE_TUTOR:
                self._source.set(const.MOVE_SOURCE_TUTOR)
                self._move_filter.set("")
                move = event_def.learn_move.move_to_learn
                if move is None:
                    move = const.DELETE_MOVE
                self._move_selector.set(move)
                self._level = const.LEVEL_ANY
                self._mon = None
            else:
                self._source.set(const.MOVE_SOURCE_TM_HM)
                self._item_filter.set("")
                self._item_selector.set(event_def.learn_move.source)
                self._level = const.LEVEL_ANY
                self._mon = None

        self._move_selected_callback()
        if event_def.learn_move.destination is None:
            self._destination.set(const.MOVE_DONT_LEARN)
        else:
            self._destination.set(self._destination.cur_options[event_def.learn_move.destination + 1])

    def get_event(self):
        dest = self._destination.get()
        if dest == const.MOVE_DONT_LEARN:
            dest = None
        else:
            try:
                dest = int(dest.split('#')[1][0]) - 1
            except Exception:
                raise ValueError(f"Failed to extract slot destination from string '{dest}'")

        if self.editor_params.event_type == const.TASK_LEARN_MOVE_LEVELUP:
            source = const.MOVE_SOURCE_LEVELUP
        elif self._source.get() == const.MOVE_SOURCE_TUTOR:
            source = const.MOVE_SOURCE_TUTOR
        else:
            source = self._item_selector.get()

        return EventDefinition(learn_move=LearnMoveEventDefinition(self._move, dest, source, level=self._level, mon=self._mon))

    def enable(self):
        self._item_filter.enable()
        self._item_selector.enable()
        self._move_filter.enable()
        self._move_selector.enable()
        self._destination.enable()
        # Re-run callback to correctly disable destination if needed
        ignore_updates(self._move_selected_callback)()

    def disable(self):
        self._item_filter.disable()
        self._item_selector.disable()
        self._move_filter.disable()
        self._move_selector.disable()
        self._destination.disable()


# ---------------------------------------------------------------------------
# WildPkmnEditor
# ---------------------------------------------------------------------------
class WildPkmnEditor(EventEditorBase):
    def __init__(self, editor_params: EditorParams, notes_visibility_callback=None, parent=None):
        super().__init__(editor_params, notes_visibility_callback=notes_visibility_callback, parent=parent)

        self._pkmn_label = QLabel("Wild Pokemon Type:")
        self._pkmn_types = SimpleOptionMenu(
            option_list=current_gen_info().pkmn_db().get_all_names(),
            callback=self._trigger_save,
            parent=self,
        )
        self._pkmn_types.setMinimumWidth(140)
        self._layout.addWidget(self._pkmn_label, self._cur_row, 0)
        self._layout.addWidget(self._pkmn_types, self._cur_row, 1)
        self._cur_row += 1

        self._pkmn_filter_label = QLabel("Wild Pokemon Type Filter:")
        self._pkmn_filter = SimpleEntry(callback=self._pkmn_filter_callback, parent=self)
        self._pkmn_filter.setMinimumWidth(140)
        self._layout.addWidget(self._pkmn_filter_label, self._cur_row, 0)
        self._layout.addWidget(self._pkmn_filter, self._cur_row, 1)
        self._cur_row += 1

        self._pkmn_level_label = QLabel("Wild Pokemon Level:")
        self._pkmn_level = AmountEntry(min_val=2, max_val=100, callback=self._update_button_status, width=5, parent=self)
        self._layout.addWidget(self._pkmn_level_label, self._cur_row, 0)
        self._layout.addWidget(self._pkmn_level, self._cur_row, 1)
        self._cur_row += 1

        self._quantity_label = QLabel("Num Pkmn:")
        self._quantity = AmountEntry(min_val=1, callback=self._update_button_status, width=5, parent=self)
        self._layout.addWidget(self._quantity_label, self._cur_row, 0)
        self._layout.addWidget(self._quantity, self._cur_row, 1)
        self._cur_row += 1

        self._pkmn_trainer_flag = CheckboxLabel(text="Is Trainer Pkmn?", flip=True, toggle_command=self._trigger_save)
        self._layout.addWidget(self._pkmn_trainer_flag, self._cur_row, 0, 1, 2)
        self._cur_row += 1

    def _update_button_status(self):
        valid = True
        try:
            pkmn_level = int(self._pkmn_level.get().strip())
            if pkmn_level < 2 or pkmn_level > 100:
                raise ValueError
        except Exception:
            valid = False

        if self._pkmn_types.get().strip().startswith(const.NO_POKEMON):
            valid = False

        try:
            quantity = int(self._quantity.get().strip())
            if quantity < 1:
                raise ValueError
        except Exception:
            valid = False

        if valid:
            self._trigger_save()

    def _pkmn_filter_callback(self):
        self._pkmn_types.new_values(
            current_gen_info().pkmn_db().get_filtered_names(filter_val=self._pkmn_filter.get().strip())
        )
        self._update_button_status()

    @ignore_updates
    def configure(self, editor_params, save_callback=None, delayed_save_callback=None):
        super().configure(editor_params, save_callback=save_callback, delayed_save_callback=delayed_save_callback)
        self._pkmn_filter.set("")
        self._pkmn_level.set("1")
        self._quantity.set("1")
        self._pkmn_trainer_flag.set_checked(False)

    @ignore_updates
    def load_event(self, event_def):
        self._pkmn_filter.set("")
        self._pkmn_level.set(str(event_def.wild_pkmn_info.level))
        self._pkmn_types.set(event_def.wild_pkmn_info.name)
        self._quantity.set(str(event_def.wild_pkmn_info.quantity))
        self._pkmn_trainer_flag.set_checked(event_def.wild_pkmn_info.trainer_pkmn)

    def get_event(self):
        return EventDefinition(
            wild_pkmn_info=WildPkmnEventDefinition(
                self._pkmn_types.get(),
                int(self._pkmn_level.get().strip()),
                quantity=int(self._quantity.get().strip()),
                trainer_pkmn=self._pkmn_trainer_flag.is_checked(),
            )
        )

    def enable(self):
        self._pkmn_types.enable()
        self._pkmn_filter.enable()
        self._pkmn_level.enable()
        self._quantity.enable()
        self._pkmn_trainer_flag.enable()

    def disable(self):
        self._pkmn_types.disable()
        self._pkmn_filter.disable()
        self._pkmn_level.disable()
        self._quantity.disable()
        self._pkmn_trainer_flag.disable()


# ---------------------------------------------------------------------------
# InventoryEventEditor (handles Acquire / Purchase / Use / Sell / Hold Item)
# ---------------------------------------------------------------------------
class InventoryEventEditor(EventEditorBase):
    def __init__(self, editor_params: EditorParams, notes_visibility_callback=None, parent=None):
        super().__init__(editor_params, notes_visibility_callback=notes_visibility_callback, parent=parent)
        self._allow_none_item = False
        self.event_type = editor_params.event_type

        val_width = 180

        self._item_type_label = QLabel("Item Type:")
        self._item_type_selector = SimpleOptionMenu(option_list=const.ITEM_TYPES, callback=self._item_filter_callback, parent=self)
        self._item_type_selector.setMinimumWidth(val_width)
        self._item_type_row = self._cur_row
        self._layout.addWidget(self._item_type_label, self._cur_row, 0)
        self._layout.addWidget(self._item_type_selector, self._cur_row, 1)
        self._cur_row += 1

        self._item_mart_label = QLabel("Mart:")
        self._item_mart_selector = SimpleOptionMenu(
            option_list=[const.ITEM_TYPE_ALL_ITEMS] + sorted(list(current_gen_info().item_db().mart_items.keys())),
            callback=self._item_filter_callback,
            parent=self,
        )
        self._item_mart_selector.setMinimumWidth(val_width)
        self._item_mart_row = self._cur_row
        self._layout.addWidget(self._item_mart_label, self._cur_row, 0)
        self._layout.addWidget(self._item_mart_selector, self._cur_row, 1)
        self._cur_row += 1

        self._item_filter_label = QLabel("Item Name Filter:")
        self._item_filter = SimpleEntry(callback=self._item_filter_callback, parent=self)
        self._item_filter.setMinimumWidth(val_width)
        self._item_filter_row = self._cur_row
        self._layout.addWidget(self._item_filter_label, self._cur_row, 0)
        self._layout.addWidget(self._item_filter, self._cur_row, 1)
        self._cur_row += 1

        self._item_selector_label = QLabel("Item:")
        self._item_selector = SimpleOptionMenu(
            option_list=current_gen_info().item_db().get_filtered_names(),
            callback=self._item_selector_callback,
            parent=self,
        )
        self._item_selector.setMinimumWidth(val_width)
        self._item_selector_row = self._cur_row
        self._layout.addWidget(self._item_selector_label, self._cur_row, 0)
        self._layout.addWidget(self._item_selector, self._cur_row, 1)
        self._cur_row += 1

        self._item_amount_label = QLabel("Num Items:")
        self._item_amount = AmountEntry(min_val=1, callback=self._item_selector_callback, width=5, parent=self)
        self._item_amount_row = self._cur_row
        self._layout.addWidget(self._item_amount_label, self._cur_row, 0)
        self._layout.addWidget(self._item_amount, self._cur_row, 1)
        self._cur_row += 1

        self._item_cost_label = QLabel("Total Cost:")
        self._item_cost_row = self._cur_row
        self._layout.addWidget(self._item_cost_label, self._cur_row, 0, 1, 2)
        self._cur_row += 1

        self._consume_held_item = CheckboxLabel(text="Consume previously held item?", toggle_command=self._trigger_save, flip=True)
        self._consume_held_item_row = self._cur_row
        self._layout.addWidget(self._consume_held_item, self._cur_row, 0, 1, 2)
        self._cur_row += 1

        self._hold_nothing = CheckboxLabel(text="Hold nothing?", toggle_command=self._trigger_save, flip=True)
        self._hold_nothing_row = self._cur_row
        self._layout.addWidget(self._hold_nothing, self._cur_row, 0, 1, 2)
        self._cur_row += 1

        # Start hidden, set_event_type will reveal the right set
        self._hide_all_item_obj()

    def _hide_all_item_obj(self):
        for w in (
            self._item_type_label, self._item_type_selector,
            self._item_mart_label, self._item_mart_selector,
            self._item_filter_label, self._item_filter,
            self._item_selector_label, self._item_selector,
            self._item_amount_label, self._item_amount,
            self._item_cost_label,
            self._consume_held_item,
            self._hold_nothing,
        ):
            w.setVisible(False)

    def _show_acquire_item(self):
        for w in (self._item_type_label, self._item_type_selector,
                  self._item_filter_label, self._item_filter,
                  self._item_selector_label, self._item_selector,
                  self._item_amount_label, self._item_amount):
            w.setVisible(True)

    def _show_purchase_item(self):
        for w in (self._item_type_label, self._item_type_selector,
                  self._item_mart_label, self._item_mart_selector,
                  self._item_filter_label, self._item_filter,
                  self._item_selector_label, self._item_selector,
                  self._item_amount_label, self._item_amount,
                  self._item_cost_label):
            w.setVisible(True)

    def _show_use_item(self):
        for w in (self._item_type_label, self._item_type_selector,
                  self._item_filter_label, self._item_filter,
                  self._item_selector_label, self._item_selector,
                  self._item_amount_label, self._item_amount):
            w.setVisible(True)

    def _show_sell_item(self):
        for w in (self._item_type_label, self._item_type_selector,
                  self._item_filter_label, self._item_filter,
                  self._item_selector_label, self._item_selector,
                  self._item_amount_label, self._item_amount,
                  self._item_cost_label):
            w.setVisible(True)

    def _show_hold_item(self):
        for w in (self._item_type_label, self._item_type_selector,
                  self._item_filter_label, self._item_filter,
                  self._item_selector_label, self._item_selector,
                  self._consume_held_item,
                  self._hold_nothing):
            w.setVisible(True)

    def _item_filter_callback(self):
        item_type = self._item_type_selector.get()
        backpack_filter = False
        if item_type == const.ITEM_TYPE_BACKPACK_ITEMS:
            item_type = const.ITEM_TYPE_ALL_ITEMS
            backpack_filter = True

        new_vals = current_gen_info().item_db().get_filtered_names(
            item_type=item_type,
            source_mart=self._item_mart_selector.get(),
        )

        if backpack_filter:
            backpack_items = [x.base_item.name for x in self.editor_params.cur_state.inventory.cur_items]
            new_vals = [x for x in new_vals if x in backpack_items]

        item_filter_val = self._item_filter.get().strip().lower()
        if item_filter_val:
            new_vals = [x for x in new_vals if item_filter_val in x.lower()]

        if not new_vals:
            new_vals = [None] if self._allow_none_item else [const.NO_ITEM]

        self._item_selector.new_values(new_vals)

    def _item_selector_callback(self):
        try:
            item_amt = int(self._item_amount.get())
            cur_item = current_gen_info().item_db().get_item(self._item_selector.get())
            if self.event_type == const.TASK_PURCHASE_ITEM:
                cost = cur_item.purchase_price * item_amt
                self._item_cost_label.setText(f"Total Cost: {cost}")
            elif self.event_type == const.TASK_SELL_ITEM:
                cost = cur_item.sell_price * item_amt
                self._item_cost_label.setText(f"Total Profit: {cost}")

            if cur_item is None:
                return
            self._trigger_save()
        except Exception:
            pass

    def set_event_type(self, event_type):
        self._hide_all_item_obj()
        if event_type == const.TASK_GET_FREE_ITEM:
            self.event_type = event_type
            self._allow_none_item = False
            self._show_acquire_item()
            return True
        elif event_type == const.TASK_PURCHASE_ITEM:
            self.event_type = event_type
            self._allow_none_item = False
            self._show_purchase_item()
            return True
        elif event_type == const.TASK_USE_ITEM:
            self.event_type = event_type
            self._allow_none_item = False
            self._show_use_item()
            return True
        elif event_type == const.TASK_SELL_ITEM:
            self.event_type = event_type
            self._allow_none_item = False
            self._show_sell_item()
            return True
        elif event_type == const.TASK_HOLD_ITEM:
            self.event_type = event_type
            self._allow_none_item = True
            self._show_hold_item()
            return True
        return False

    @ignore_updates
    def configure(self, editor_params, save_callback=None, delayed_save_callback=None):
        super().configure(editor_params, save_callback=save_callback, delayed_save_callback=delayed_save_callback)
        self._item_filter.set("")
        self._item_mart_selector.set(const.ITEM_TYPE_ALL_ITEMS)
        self._item_type_selector.set(const.ITEM_TYPE_ALL_ITEMS)
        self._item_amount.set("1")
        self.set_event_type(self.editor_params.event_type)

    @ignore_updates
    def load_event(self, event_def):
        self._item_filter.set("")
        self._item_mart_selector.set(const.ITEM_TYPE_ALL_ITEMS)
        self._item_type_selector.set(const.ITEM_TYPE_ALL_ITEMS)
        self.set_event_type(event_def.get_event_type())

        if self.event_type != const.TASK_HOLD_ITEM:
            self._item_selector.set(event_def.item_event_def.item_name)
            self._item_amount.set(event_def.item_event_def.item_amount)
        else:
            self._item_selector.set(event_def.hold_item.item_name)
            self._consume_held_item.set_checked(event_def.hold_item.consumed)
            self._hold_nothing.set_checked(event_def.hold_item.item_name is None)

    def get_event(self):
        if self.event_type == const.TASK_GET_FREE_ITEM:
            return EventDefinition(
                item_event_def=InventoryEventDefinition(self._item_selector.get(), int(self._item_amount.get()), True, False)
            )
        elif self.event_type == const.TASK_PURCHASE_ITEM:
            return EventDefinition(
                item_event_def=InventoryEventDefinition(self._item_selector.get(), int(self._item_amount.get()), True, True)
            )
        elif self.event_type == const.TASK_USE_ITEM:
            return EventDefinition(
                item_event_def=InventoryEventDefinition(self._item_selector.get(), int(self._item_amount.get()), False, False)
            )
        elif self.event_type == const.TASK_SELL_ITEM:
            return EventDefinition(
                item_event_def=InventoryEventDefinition(self._item_selector.get(), int(self._item_amount.get()), False, True)
            )
        elif self.event_type == const.TASK_HOLD_ITEM:
            held_item = None if self._hold_nothing.is_checked() else self._item_selector.get()
            return EventDefinition(
                hold_item=HoldItemEventDefinition(held_item, consumed=self._consume_held_item.is_checked())
            )
        raise ValueError(f"Cannot generate inventory event for event type: {self.event_type}")

    def enable(self):
        self._item_type_selector.enable()
        self._item_mart_selector.enable()
        self._item_filter.enable()
        self._item_selector.enable()
        self._item_amount.enable()

    def disable(self):
        self._item_type_selector.disable()
        self._item_mart_selector.disable()
        self._item_filter.disable()
        self._item_selector.disable()
        self._item_amount.disable()


# ---------------------------------------------------------------------------
# SaveEventEditor
# ---------------------------------------------------------------------------
class SaveEventEditor(EventEditorBase):
    def __init__(self, editor_params: EditorParams, notes_visibility_callback=None, parent=None):
        super().__init__(editor_params, notes_visibility_callback=notes_visibility_callback, parent=parent)
        self._location_label = QLabel("Save Location")
        self._location_value = SimpleEntry(callback=self._trigger_delayed_save, parent=self)
        self._location_value.setMinimumWidth(160)
        self._layout.addWidget(self._location_label, self._cur_row, 0)
        self._layout.addWidget(self._location_value, self._cur_row, 1)
        self._cur_row += 1

    @ignore_updates
    def load_event(self, event_def):
        self._location_value.set(str(event_def.save.location))

    def get_event(self):
        return EventDefinition(save=SaveEventDefinition(location=self._location_value.get()))

    def enable(self):
        self._location_value.enable()

    def disable(self):
        self._location_value.disable()


# ---------------------------------------------------------------------------
# HealEventEditor
# ---------------------------------------------------------------------------
class HealEventEditor(EventEditorBase):
    def __init__(self, editor_params: EditorParams, notes_visibility_callback=None, parent=None):
        super().__init__(editor_params, notes_visibility_callback=notes_visibility_callback, parent=parent)
        self._location_label = QLabel("Heal Location")
        self._location_value = SimpleEntry(callback=self._trigger_delayed_save, parent=self)
        self._location_value.setMinimumWidth(160)
        self._layout.addWidget(self._location_label, self._cur_row, 0)
        self._layout.addWidget(self._location_value, self._cur_row, 1)
        self._cur_row += 1

    @ignore_updates
    def load_event(self, event_def):
        self._location_value.set(str(event_def.heal.location))

    def get_event(self):
        return EventDefinition(heal=HealEventDefinition(location=self._location_value.get()))

    def enable(self):
        self._location_value.enable()

    def disable(self):
        self._location_value.disable()


# ---------------------------------------------------------------------------
# BlackoutEventEditor
# ---------------------------------------------------------------------------
class BlackoutEventEditor(EventEditorBase):
    def __init__(self, editor_params: EditorParams, notes_visibility_callback=None, parent=None):
        super().__init__(editor_params, notes_visibility_callback=notes_visibility_callback, parent=parent)
        self._location_label = QLabel("Black Out back to:")
        self._location_value = SimpleEntry(callback=self._trigger_delayed_save, parent=self)
        self._location_value.setMinimumWidth(160)
        self._layout.addWidget(self._location_label, self._cur_row, 0)
        self._layout.addWidget(self._location_value, self._cur_row, 1)
        self._cur_row += 1

    @ignore_updates
    def load_event(self, event_def):
        self._location_value.set(str(event_def.blackout.location))

    def get_event(self):
        return EventDefinition(blackout=BlackoutEventDefinition(location=self._location_value.get()))

    def enable(self):
        self._location_value.enable()

    def disable(self):
        self._location_value.disable()


# ---------------------------------------------------------------------------
# EvolutionEditor
# ---------------------------------------------------------------------------
class EvolutionEditor(EventEditorBase):
    def __init__(self, editor_params: EditorParams, notes_visibility_callback=None, parent=None):
        super().__init__(editor_params, notes_visibility_callback=notes_visibility_callback, parent=parent)
        self._growth_rate = ""

        self._pkmn_label = QLabel("Pokemon Species:")
        self._pkmn_types = SimpleOptionMenu(option_list=[const.NO_POKEMON], callback=self._trigger_save, parent=self)
        self._pkmn_types.setMinimumWidth(140)
        self._layout.addWidget(self._pkmn_label, self._cur_row, 0)
        self._layout.addWidget(self._pkmn_types, self._cur_row, 1)
        self._cur_row += 1

        self._pkmn_filter_label = QLabel("Pokemon Species Filter:")
        self._pkmn_filter = SimpleEntry(callback=self._pkmn_filter_callback, parent=self)
        self._pkmn_filter.setMinimumWidth(140)
        self._layout.addWidget(self._pkmn_filter_label, self._cur_row, 0)
        self._layout.addWidget(self._pkmn_filter, self._cur_row, 1)
        self._cur_row += 1

        self._item_selector_label = QLabel("By Stone:")
        self._item_selector = SimpleOptionMenu(option_list=[const.NO_ITEM], callback=self._trigger_save, parent=self)
        self._item_selector.setMinimumWidth(140)
        self._layout.addWidget(self._item_selector_label, self._cur_row, 0)
        self._layout.addWidget(self._item_selector, self._cur_row, 1)
        self._cur_row += 1

    @ignore_updates
    def configure(self, editor_params, save_callback=None, delayed_save_callback=None):
        self._item_selector.new_values([const.NO_ITEM] + current_gen_info().item_db().get_filtered_names(name_filter="stone"))
        super().configure(editor_params, save_callback=save_callback, delayed_save_callback=delayed_save_callback)

    @ignore_updates
    def load_event(self, event_def):
        self._growth_rate = current_gen_info().pkmn_db().get_pkmn(event_def.evolution.evolved_species).growth_rate
        self._pkmn_types.new_values(
            current_gen_info().pkmn_db().get_all_names(growth_rate=self._growth_rate),
            default_val=event_def.evolution.evolved_species,
        )
        self._pkmn_filter.set("")
        if event_def.evolution.by_stone:
            self._item_selector.set(event_def.evolution.by_stone)
        else:
            self._item_selector.set(const.NO_ITEM)

    def get_event(self):
        return EventDefinition(
            evolution=EvolutionEventDefinition(
                self._pkmn_types.get(),
                by_stone=None if self._item_selector.get() == const.NO_ITEM else self._item_selector.get(),
            )
        )

    def enable(self):
        self._pkmn_types.enable()
        self._pkmn_filter.enable()
        self._item_selector.enable()

    def disable(self):
        self._pkmn_types.disable()
        self._pkmn_filter.disable()
        self._item_selector.disable()

    def _pkmn_filter_callback(self):
        self._pkmn_types.new_values(
            current_gen_info().pkmn_db().get_filtered_names(
                filter_val=self._pkmn_filter.get().strip(),
                growth_rate=self._growth_rate,
            )
        )


# ---------------------------------------------------------------------------
# EventEditorFactory  -- caches editors and maps event types to classes
# ---------------------------------------------------------------------------
class EventEditorFactory:
    TYPE_MAP = {
        const.TASK_TRAINER_BATTLE: TrainerFightEditor,
        const.TASK_RARE_CANDY: RareCandyEditor,
        const.TASK_VITAMIN: VitaminEditor,
        const.TASK_FIGHT_WILD_PKMN: WildPkmnEditor,
        const.TASK_GET_FREE_ITEM: InventoryEventEditor,
        const.TASK_PURCHASE_ITEM: InventoryEventEditor,
        const.TASK_USE_ITEM: InventoryEventEditor,
        const.TASK_SELL_ITEM: InventoryEventEditor,
        const.TASK_HOLD_ITEM: InventoryEventEditor,
        const.TASK_LEARN_MOVE_LEVELUP: LearnMoveEditor,
        const.TASK_LEARN_MOVE_TM: LearnMoveEditor,
        const.TASK_SAVE: SaveEventEditor,
        const.TASK_HEAL: HealEventEditor,
        const.TASK_BLACKOUT: BlackoutEventEditor,
        const.TASK_NOTES_ONLY: NotesEditor,
        const.TASK_EVOLUTION: EvolutionEditor,
    }

    def __init__(self, parent_widget):
        self._lookup = {}
        self._parent_widget = parent_widget

    def get_editor(
        self,
        editor_params: EditorParams,
        save_callback=None,
        delayed_save_callback=None,
        is_enabled=True,
        notes_visibility_callback=None,
    ) -> EventEditorBase:
        if editor_params.event_type in self._lookup:
            result: EventEditorBase = self._lookup[editor_params.event_type]
            result.configure(editor_params, save_callback=save_callback, delayed_save_callback=delayed_save_callback)
            if is_enabled:
                result.enable()
            else:
                result.disable()
            return result

        editor_type = self.TYPE_MAP.get(editor_params.event_type)
        if editor_type is None:
            raise ValueError(f"Could not find visual editor for event type: {editor_params.event_type}")

        result = editor_type(
            editor_params,
            notes_visibility_callback=notes_visibility_callback,
            parent=self._parent_widget,
        )
        result.configure(
            editor_params,
            save_callback=save_callback,
            delayed_save_callback=delayed_save_callback,
        )
        self._lookup[editor_params.event_type] = result

        # Share the same editor instance across all item event types
        if editor_params.event_type in const.ITEM_ROUTE_EVENT_TYPES:
            for other_type in const.ITEM_ROUTE_EVENT_TYPES:
                self._lookup[other_type] = result

        return result
