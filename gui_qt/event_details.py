"""EventDetails -- PySide6 port of gui/event_details.py.

Manages the right-side panel: Pre-event State tab (StateViewer + event editor),
Battle Summary tab, auto-switch, and the notes footer.  Handles event-selection
changes and coordinates delayed saves.
"""
from __future__ import annotations

import logging
import time

from PySide6.QtWidgets import (
    QWidget, QVBoxLayout, QHBoxLayout, QTabWidget, QLabel,
    QPlainTextEdit, QSizePolicy, QScrollArea, QFrame,
)
from PySide6.QtCore import Qt, QTimer, Signal

from controllers.main_controller import MainController
from controllers.battle_summary_controller import BattleSummaryController
from gui_qt.components.custom_components import CheckboxLabel
from gui_qt.pkmn_components.state_viewer import StateViewer
from gui_qt.battle_summary import BattleSummary
from gui_qt.event_editors import EventEditorFactory, EditorParams, NotesEditor
from routing.route_events import EventDefinition, EventFolder, EventGroup, EventItem
from utils.constants import const
from utils.config_manager import config
from pkmn.gen_factory import current_gen_info

logger = logging.getLogger(__name__)


class EventDetails(QWidget):
    """Right-panel widget: tabbed state/battle-summary + notes footer."""

    # Emitted when switching to battle summary (True) or pre-state (False)
    battle_summary_visible = Signal(bool)

    SAVE_DELAY_MS = 2000

    def __init__(self, controller: MainController, parent=None):
        super().__init__(parent)
        self._controller = controller
        self._battle_summary_controller = BattleSummaryController(self._controller)
        self._ignore_tab_switching = False
        self._in_selection_handler = False
        self._cur_delayed_event_id = None
        self._cur_delayed_event_start = None
        self._unsubscribers: list = []

        # ---- layout -------------------------------------------------------
        root_layout = QVBoxLayout(self)
        root_layout.setContentsMargins(4, 4, 4, 4)
        root_layout.setSpacing(2)

        # ---- tab widget ---------------------------------------------------
        self._tab_widget = QTabWidget()
        root_layout.addWidget(self._tab_widget, 1)

        # Pre-state tab -- wrapped in a QScrollArea so its minimum size hint
        # does not force the surrounding layout to grow. Without this, the
        # combined min-heights of StateViewer + event editor would propagate
        # up through the splitter to the central widget; when the docked Run
        # Summary is visible in a maximized window, that overflow pushes the
        # docked panel and status bar below the visible area.
        pre_state_scroll = QScrollArea()
        pre_state_scroll.setWidgetResizable(True)
        pre_state_scroll.setFrameShape(QFrame.NoFrame)
        pre_state_scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarAsNeeded)
        pre_state_scroll.setVerticalScrollBarPolicy(Qt.ScrollBarAsNeeded)

        pre_state_tab = QWidget()
        # Exposed so MainWindow can query its sizeHint for default splitter sizing.
        self._pre_state_tab = pre_state_tab
        ps_layout = QVBoxLayout(pre_state_tab)
        ps_layout.setContentsMargins(4, 4, 4, 4)
        ps_layout.setSpacing(4)

        self.state_viewer = StateViewer(pre_state_tab)
        ps_layout.addWidget(self.state_viewer)

        # Event-editor placeholder area -- editors are swapped in/out here
        self._editor_container = QWidget(pre_state_tab)
        self._editor_layout = QVBoxLayout(self._editor_container)
        self._editor_layout.setContentsMargins(0, 0, 0, 0)
        self._editor_layout.setSpacing(0)
        ps_layout.addWidget(self._editor_container, 1)

        pre_state_scroll.setWidget(pre_state_tab)
        self._tab_widget.addTab(pre_state_scroll, "Pre-Event State")
        self.pre_state_tab_index = 0

        # Battle-summary tab
        self.battle_summary = BattleSummary(self._battle_summary_controller)
        self._tab_widget.addTab(self.battle_summary, "Battle Summary")
        self.battle_summary_tab_index = 1
        self.battle_summary.show_contents()

        # Auto-switch checkbox (inline with tabs)
        self.auto_switch_checkbox = CheckboxLabel(
            text="Switch tabs automatically",
            toggle_command=self._handle_auto_switch_toggle,
            flip=True,
        )
        self.auto_switch_checkbox.set_checked(config.do_auto_switch())
        root_layout.addWidget(self.auto_switch_checkbox)

        # Event editor factory and footer notes
        self._event_editor_lookup = EventEditorFactory(self._editor_container)
        self._current_event_editor = None

        self._footer_editor_factory = EventEditorFactory(self)
        self._trainer_notes = self._footer_editor_factory.get_editor(
            EditorParams(const.TASK_NOTES_ONLY, None, None),
            save_callback=self.update_existing_event,
            delayed_save_callback=self.update_existing_event_after_delay,
            notes_visibility_callback=self._update_notes_visibility,
        )
        root_layout.addWidget(self._trainer_notes)

        # ---- controller callbacks -----------------------------------------
        self._tab_widget.currentChanged.connect(self._tab_changed_callback)

        unsub = self._controller.register_event_selection(self._handle_selection)
        self._unsubscribers.append(unsub)

        unsub = self._controller.register_record_mode_change(self._handle_selection)
        self._unsubscribers.append(unsub)

        unsub = self._controller.register_route_change(self._handle_route_change)
        self._unsubscribers.append(unsub)

        unsub = self._controller.register_version_change(self._handle_version_change)
        self._unsubscribers.append(unsub)

        self._controller.register_pre_save_hook(self.force_and_clear_event_update)

        # Battle summary controller callbacks: save on non-load changes,
        # update notes visibility when matchups refresh.
        unsub = self._battle_summary_controller.register_nonload_change(
            self.update_existing_event_after_delay
        )
        self._unsubscribers.append(unsub)

        unsub = self._battle_summary_controller.register_refresh(
            self._on_battle_summary_refresh
        )
        self._unsubscribers.append(unsub)

        # Initial state
        self._tab_changed_callback()

    # ------------------------------------------------------------------
    # Cleanup
    # ------------------------------------------------------------------
    def cleanup(self):
        for unsub in self._unsubscribers:
            try:
                unsub()
            except Exception:
                pass

    # ------------------------------------------------------------------
    # Tab switching
    # ------------------------------------------------------------------
    def _tab_changed_callback(self, index=None):
        if index is None:
            index = self._tab_widget.currentIndex()
        is_battle = index == self.battle_summary_tab_index
        self.battle_summary_visible.emit(is_battle)
        if is_battle:
            QTimer.singleShot(300, self._deferred_show_battle_summary)
        else:
            self.battle_summary.hide_contents()
            self._update_notes_visibility()

    def _deferred_show_battle_summary(self):
        self.battle_summary.show_contents()
        self._update_notes_visibility()

    def change_tabs(self):
        idx = self._tab_widget.currentIndex()
        self._tab_widget.setCurrentIndex(1 - idx)

    def _handle_auto_switch_toggle(self):
        config.set_auto_switch(self.auto_switch_checkbox.is_checked())

    def _on_battle_summary_refresh(self):
        """Called when battle summary refreshes -- update notes visibility."""
        QTimer.singleShot(0, self._update_notes_visibility)

    # ------------------------------------------------------------------
    # Controller event handlers
    # ------------------------------------------------------------------
    def _handle_version_change(self):
        self._battle_summary_controller.load_empty()
        try:
            self.battle_summary.configure_weather(current_gen_info().get_valid_weather())
            self.battle_summary.configure_setup_moves(current_gen_info().get_stat_modifer_moves())
            self.battle_summary.refresh_test_move_options()
        except Exception:
            pass

    def _handle_route_change(self):
        if self._in_selection_handler:
            return  # _handle_selection_inner will update everything
        try:
            event_group = self._controller.get_single_selected_event_obj()
            if event_group is None:
                self.state_viewer.set_state(self._controller.get_init_state())
                self.battle_summary.set_team(None)
            else:
                self.state_viewer.set_state(event_group.init_state)
                if event_group.event_definition.trainer_def is not None:
                    self.battle_summary.set_team(
                        event_group.event_definition.get_pokemon_list(),
                        cur_state=event_group.init_state,
                        event_group=event_group,
                    )
                elif event_group.event_definition.wild_pkmn_info is not None:
                    wild_pkmn = event_group.event_definition.get_pokemon_list()
                    if wild_pkmn and event_group.init_state is not None:
                        self.battle_summary.set_team(
                            wild_pkmn,
                            cur_state=event_group.init_state,
                            is_wild=True,
                        )
                    else:
                        self.battle_summary.set_team(None)
                else:
                    self.battle_summary.set_team(None)
        except Exception as e:
            # Must not propagate — _safely_invoke_callbacks permanently
            # removes any callback that raises, which would break all
            # future battle-summary updates for the rest of the session.
            logger.exception(f"Error handling route change: {e}")

    def _handle_selection(self):
        if self._in_selection_handler:
            return
        # Suppress all intermediate repaints during the entire selection
        # transition (tab switch + data load + battle summary refresh).
        self._in_selection_handler = True
        self.setUpdatesEnabled(False)
        try:
            self._handle_selection_inner()
        except Exception as e:
            # Must not propagate — _safely_invoke_callbacks permanently
            # removes any callback that raises, which would break all
            # future battle-summary updates for the rest of the session.
            logger.exception(f"Error handling selection change: {e}")
        finally:
            self.setUpdatesEnabled(True)
            self._in_selection_handler = False

    def set_suppress_battle_summary(self, suppress):
        """While *suppress* is True, auto-switching to the Battle Summary
        tab is blocked and the Pre-Event State tab is forced instead."""
        self._suppress_battle_summary = suppress
        if suppress:
            self._tab_widget.setCurrentIndex(self.pre_state_tab_index)

    def _handle_selection_inner(self):
        force_pre = getattr(self, "_suppress_battle_summary", False)

        # When recording starts, immediately show the battle summary tab
        # so the right side stays stable throughout the recording session.
        if self._controller.is_record_mode_active():
            self._tab_widget.setCurrentIndex(self.battle_summary_tab_index)

        event_group = self._controller.get_single_selected_event_obj()

        if event_group is None:
            self.show_event_details(
                None,
                self._controller.get_init_state(),
                self._controller.get_final_state(),
                allow_updates=False,
            )
        elif isinstance(event_group, EventFolder):
            self.show_event_details(
                event_group.event_definition,
                event_group.init_state,
                event_group.final_state,
            )
        else:
            do_allow_updates = (
                isinstance(event_group, EventGroup)
                or event_group.event_definition.get_event_type() == const.TASK_LEARN_MOVE_LEVELUP
            )
            trainer_event_group = event_group
            if isinstance(trainer_event_group, EventItem) and event_group.event_definition.learn_move is None:
                trainer_event_group = trainer_event_group.parent

            has_battle = (
                trainer_event_group.event_definition.trainer_def is not None
                or trainer_event_group.event_definition.wild_pkmn_info is not None
            )
            if force_pre:
                # Inline editing requested pre-event view — honour it
                # regardless of auto-switch or battle presence.
                self._tab_widget.setCurrentIndex(self.pre_state_tab_index)
            elif self._ignore_tab_switching or self.auto_switch_checkbox.is_checked():
                if self._controller.is_record_mode_active():
                    # During recording, only switch TO battle summary, never
                    # away from it.  Constantly flipping to Pre-Event State
                    # on every non-battle event is visually disorienting.
                    if has_battle:
                        self._tab_widget.setCurrentIndex(self.battle_summary_tab_index)
                elif has_battle:
                    self._tab_widget.setCurrentIndex(self.battle_summary_tab_index)
                else:
                    self._tab_widget.setCurrentIndex(self.pre_state_tab_index)

            self.show_event_details(
                event_group.event_definition,
                event_group.init_state,
                event_group.final_state,
                do_allow_updates,
                event_group=trainer_event_group,
            )

    # ------------------------------------------------------------------
    # Show event details
    # ------------------------------------------------------------------
    def show_event_details(
        self,
        event_def: EventDefinition,
        init_state,
        final_state,
        allow_updates=True,
        event_group: EventGroup = None,
    ):
        self.force_and_clear_event_update()
        if self._controller.is_record_mode_active():
            allow_updates = False

        self.state_viewer.set_state(init_state)

        # Remove previous editor
        if self._current_event_editor is not None:
            self._current_event_editor.setVisible(False)
            self._editor_layout.removeWidget(self._current_event_editor)
            self._current_event_editor = None

        if event_def is None:
            self._trainer_notes.load_event(None)
            self.battle_summary.set_team(None)
        else:
            self._trainer_notes.load_event(event_def)
            if event_def.trainer_def is not None:
                # Ensure battle summary will render when set_team triggers
                # the controller refresh callback (must be True before set_team)
                self.battle_summary.should_render = True
                self.battle_summary.set_team(
                    event_def.get_pokemon_list(),
                    cur_state=init_state,
                    event_group=event_group,
                )
            elif event_def.wild_pkmn_info is not None:
                self.battle_summary.should_render = True
                wild_pkmn = event_def.get_pokemon_list()
                if wild_pkmn and init_state is not None:
                    self.battle_summary.set_team(
                        wild_pkmn,
                        cur_state=init_state,
                        is_wild=True,
                    )
                else:
                    self.battle_summary.set_team(None)
            else:
                self.battle_summary.set_team(None)

            if event_def.get_event_type() != const.TASK_NOTES_ONLY:
                self._current_event_editor = self._event_editor_lookup.get_editor(
                    EditorParams(event_def.get_event_type(), None, init_state),
                    save_callback=self.update_existing_event,
                    delayed_save_callback=self.update_existing_event_after_delay,
                    is_enabled=allow_updates,
                )
                self._current_event_editor.load_event(event_def)
                self._editor_layout.addWidget(self._current_event_editor)
                self._current_event_editor.setVisible(True)

    # ------------------------------------------------------------------
    # Event saving
    # ------------------------------------------------------------------
    def update_existing_event(self):
        self._event_update_helper(self._controller.get_single_selected_event_id())

    def update_existing_event_after_delay(self):
        to_save = self._controller.get_single_selected_event_id()
        if self._cur_delayed_event_id is not None and self._cur_delayed_event_id != to_save:
            logger.error(
                f"Unexpected switch of event id from {self._cur_delayed_event_id} to {to_save}"
            )

        self._cur_delayed_event_id = to_save
        self._cur_delayed_event_start = time.time() + self.SAVE_DELAY_MS / 1000.0
        QTimer.singleShot(self.SAVE_DELAY_MS, self._delayed_event_update)

    def force_and_clear_event_update(self):
        if self._cur_delayed_event_id is None:
            return
        self._event_update_helper(self._cur_delayed_event_id)

    def _delayed_event_update(self):
        if self._cur_delayed_event_id is None or self._cur_delayed_event_start is None:
            return
        if self._cur_delayed_event_start - time.time() > 0:
            return
        self._event_update_helper(self._cur_delayed_event_id)

    def _event_update_helper(self, event_to_update):
        if event_to_update is None:
            return

        if self._cur_delayed_event_id is not None and self._cur_delayed_event_id != event_to_update:
            logger.error(
                f"Found delayed update for: {self._cur_delayed_event_id} "
                f"which is different from the current update for {event_to_update}"
            )

        self._cur_delayed_event_id = None
        self._cur_delayed_event_start = None
        try:
            if self._current_event_editor is None:
                new_event = EventDefinition()
            else:
                new_event = self._current_event_editor.get_event()

            if new_event.get_event_type() == const.TASK_TRAINER_BATTLE:
                new_trainer_def = self._battle_summary_controller.get_partial_trainer_definition()
                if new_trainer_def is None:
                    logger.error(
                        "Expected to get updated trainer def from battle summary controller, but got None"
                    )
                else:
                    new_trainer_def.exp_split = new_event.trainer_def.exp_split
                    new_trainer_def.pay_day_amount = new_event.trainer_def.pay_day_amount
                    new_trainer_def.mon_order = new_event.trainer_def.mon_order
                    new_event.trainer_def = new_trainer_def

            new_event.notes = self._trainer_notes.get_event().notes
        except Exception as e:
            logger.error("Exception occurred trying to update current event")
            logger.exception(e)
            self._controller.trigger_exception("Exception occurred trying to update current event")
            return

        self._controller.update_existing_event(event_to_update, new_event)

    # ------------------------------------------------------------------
    # Notes visibility
    # ------------------------------------------------------------------
    def _update_notes_visibility(self):
        idx = self._tab_widget.currentIndex()
        if idx == self.battle_summary_tab_index and not config.are_notes_visible_in_battle_summary():
            self._trainer_notes.force_collapsed(True)
        else:
            self._trainer_notes.force_collapsed(False)

    # ------------------------------------------------------------------
    # Screenshots / candy (delegated to battle_summary)
    # ------------------------------------------------------------------
    def take_battle_summary_screenshot(self):
        if self._tab_widget.currentIndex() == self.battle_summary_tab_index:
            self.battle_summary.take_battle_summary_screenshot()

    def take_player_ranges_screenshot(self):
        if self._tab_widget.currentIndex() == self.battle_summary_tab_index:
            self.battle_summary.take_player_ranges_screenshot()

    def take_enemy_ranges_screenshot(self):
        if self._tab_widget.currentIndex() == self.battle_summary_tab_index:
            self.battle_summary.take_enemy_ranges_screenshot()

    def _increment_prefight_candies(self):
        self.battle_summary._increment_prefight_candies()

    def _decrement_prefight_candies(self):
        self.battle_summary._decrement_prefight_candies()

    def _update_notes_visibility_in_battle_summary(self):
        self._update_notes_visibility()
