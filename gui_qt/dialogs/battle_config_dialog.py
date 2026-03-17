from PySide6.QtWidgets import (
    QVBoxLayout, QGridLayout, QLabel, QWidget, QScrollArea,
)
from PySide6.QtCore import Qt

from gui_qt.dialogs.base_dialog import BaseDialog
from gui_qt.components.custom_components import (
    SimpleButton, SimpleOptionMenu, AmountEntry, CheckboxLabel,
)
from utils.constants import const
from utils.config_manager import config


class BattleConfigDialog(BaseDialog):
    """Dialog for configuring battle calculation settings and highlighting strategies."""

    def __init__(self, parent, battle_controller=None, **kwargs):
        super().__init__(parent, title="Battle Configuration", **kwargs)
        self._battle_controller = battle_controller

        # Use a scroll area for the potentially tall content
        scroll = QScrollArea()
        scroll.setWidgetResizable(True)
        scroll.setHorizontalScrollBarPolicy(Qt.ScrollBarAlwaysOff)

        content = QWidget()
        main_layout = QVBoxLayout(content)

        # === Damage Notes Section ===
        damage_notes_frame = QWidget()
        notes_layout = QVBoxLayout(damage_notes_frame)
        notes_layout.setContentsMargins(5, 20, 5, 10)

        damage_notes_title = QLabel("Battle calcs limitations and edge cases")
        damage_notes_title.setAlignment(Qt.AlignCenter)
        notes_layout.addWidget(damage_notes_title)

        damage_notes = QLabel(
            "Possible kills with less than 0.1% chance are not reported.\n"
            "For each move, a full search for kill percents is done, up to a certain # of turns (configurable below).\n"
            "Up to 3 ranges are reported: 2 fastest kills, if applicable. The number required for a guaranteed kill is always given as the last range\n"
            "For weaker moves, the maximum number of HITS (assumes attack lands every time) needed to guarantee a kill are given instead\n"
            "With respect to accuracy calculations, Gen 1 misses are ignored\n"
            "The crit damage ranges for multi-hit moves in Gen 2 assume exactly one of the multi-hits crit"
        )
        damage_notes.setWordWrap(True)
        notes_layout.addWidget(damage_notes)

        main_layout.addWidget(damage_notes_frame)

        # === Input Configuration Section ===
        input_frame = QWidget()
        input_grid = QGridLayout(input_frame)
        input_grid.setContentsMargins(5, 10, 5, 10)
        input_grid.setHorizontalSpacing(5)
        input_grid.setVerticalSpacing(5)

        # Damage search depth
        search_depth_label = QLabel("Damage Calc Search Depth:")
        self.search_depth_val = AmountEntry(
            init_val=config.get_damage_search_depth(),
            callback=self._update_damage_search_depth,
            min_val=1,
            max_val=99,
        )
        input_grid.addWidget(search_depth_label, 0, 0)
        input_grid.addWidget(self.search_depth_val, 0, 1)

        search_depth_details = QLabel(
            "\n# Of turns to search damage ranges to find kill %'s\n"
            "Larger gets more accurate guaranteed kills, but may take longer, especially on slower computers"
        )
        search_depth_details.setWordWrap(True)
        input_grid.addWidget(search_depth_details, 1, 0, 1, 2)

        # Force full search (psywave)
        self.force_full_search_label = CheckboxLabel(
            text="Fully calculate psywave (Not recommended):",
            toggle_command=self._toggle_force_full_search,
            flip=True,
        )
        self.force_full_search_label.set_checked(config.do_force_full_search())
        input_grid.addWidget(self.force_full_search_label, 10, 0, 1, 2)

        # Ignore accuracy
        self.accuracy_label = CheckboxLabel(
            text="Ignore Accuracy in Kill Ranges:",
            toggle_command=self._toggle_accuracy,
            flip=True,
        )
        self.accuracy_label.set_checked(config.do_ignore_accuracy())
        input_grid.addWidget(self.accuracy_label, 11, 0, 1, 2)

        # Player highlight strategy
        player_strat_label = QLabel("Player Highlight Strategy:")
        self.player_strat_val = SimpleOptionMenu(
            option_list=const.ALL_HIGHLIGHT_STRATS,
            default_val=config.get_player_highlight_strategy(),
            callback=self._update_player_strat,
        )
        input_grid.addWidget(player_strat_label, 12, 0)
        input_grid.addWidget(self.player_strat_val, 12, 1)

        # Enemy highlight strategy
        enemy_strat_label = QLabel("Enemy Highlight Strategy:")
        self.enemy_strat_val = SimpleOptionMenu(
            option_list=const.ALL_HIGHLIGHT_STRATS,
            default_val=config.get_enemy_highlight_strategy(),
            callback=self._update_enemy_strat,
        )
        input_grid.addWidget(enemy_strat_label, 13, 0)
        input_grid.addWidget(self.enemy_strat_val, 13, 1)

        # Consistent threshold
        consistent_threshold_label = QLabel("Consistency Threshold:")
        self.consistent_threshold_val = AmountEntry(
            init_val=config.get_consistent_threshold(),
            callback=self._update_consistent_threshold,
            min_val=1,
            max_val=99,
        )
        input_grid.addWidget(consistent_threshold_label, 14, 0)
        input_grid.addWidget(self.consistent_threshold_val, 14, 1)

        main_layout.addWidget(input_frame)

        # === Explanation Section ===
        explanation_frame = QWidget()
        exp_layout = QVBoxLayout(explanation_frame)
        exp_layout.setContentsMargins(5, 20, 5, 10)

        strat_header = QLabel("Highlighting Strategies")
        strat_header.setAlignment(Qt.AlignCenter)
        exp_layout.addWidget(strat_header)

        guaranteed_kill_title = QLabel("Guaranteed Kill")
        guaranteed_kill_title.setAlignment(Qt.AlignCenter)
        exp_layout.addWidget(guaranteed_kill_title)
        guaranteed_kill_explanation = QLabel(
            "This will highlight the move that has the lowest number of turns for a 'guaranteed' kill.\n"
            "'Guaranteed' is if the move has a 99% chance or higher to kill."
        )
        guaranteed_kill_explanation.setWordWrap(True)
        exp_layout.addWidget(guaranteed_kill_explanation)

        fastest_kill_title = QLabel("Fastest Kill")
        fastest_kill_title.setAlignment(Qt.AlignCenter)
        exp_layout.addWidget(fastest_kill_title)
        fastest_kill_explanation = QLabel(
            "This will highlight the move that has the lowest number of turns for any possible kill.\n"
            "Kills that have a less than 0.1% chance of occuring are ignored by the damage calcs, but if the move has at least a 1% chance of killing,\n"
            "it will be reported"
        )
        fastest_kill_explanation.setWordWrap(True)
        exp_layout.addWidget(fastest_kill_explanation)

        consistent_kill_title = QLabel("Consistent Kill")
        consistent_kill_title.setAlignment(Qt.AlignCenter)
        exp_layout.addWidget(consistent_kill_title)
        consistent_kill_explanation = QLabel(
            "This will highlight the move that has the lowest number of turns for a kill that has at least a chance above the consistency threshold.\n"
            "This can be configured to suit your preferences"
        )
        consistent_kill_explanation.setWordWrap(True)
        exp_layout.addWidget(consistent_kill_explanation)

        ties_explanation = QLabel(
            "Regardless of the strategy, ties between moves with the same # of turns will be broken with successive checks for the following stats: "
            "Highest Accuracy, Punish 2 turn moves (dig/fly), Highest damage"
        )
        ties_explanation.setWordWrap(True)
        exp_layout.addWidget(ties_explanation)

        hyper_beam_explanation = QLabel(
            "Hyper Beam is also special cased to not be highlighted if it cannot kill with a single non-crit hit, due to the need to recharge"
        )
        hyper_beam_explanation.setWordWrap(True)
        exp_layout.addWidget(hyper_beam_explanation)

        main_layout.addWidget(explanation_frame)

        scroll.setWidget(content)

        outer_layout = QVBoxLayout(self)
        outer_layout.addWidget(scroll)

        # Close button
        self.close_button = SimpleButton("Close")
        self.close_button.clicked.connect(self._final_cleanup)
        outer_layout.addWidget(self.close_button, alignment=Qt.AlignCenter)

        self.resize(600, 500)

    def keyPressEvent(self, event):
        if event.key() == Qt.Key_Return or event.key() == Qt.Key_Enter:
            self._final_cleanup()
        else:
            super().keyPressEvent(event)

    def _final_cleanup(self, *args, **kwargs):
        if self._battle_controller is not None:
            self._battle_controller._full_refresh()
        self.close()

    def _toggle_accuracy(self, *args, **kwargs):
        config.set_ignore_accuracy(self.accuracy_label.is_checked())

    def _toggle_force_full_search(self, *args, **kwargs):
        config.set_force_full_search(self.force_full_search_label.is_checked())

    def _update_player_strat(self, *args, **kwargs):
        config.set_player_highlight_strategy(self.player_strat_val.get())

    def _update_enemy_strat(self, *args, **kwargs):
        config.set_enemy_highlight_strategy(self.enemy_strat_val.get())

    def _update_consistent_threshold(self, *args, **kwargs):
        result = config.DEFAULT_CONSISTENT_THRESHOLD
        try:
            result = int(self.consistent_threshold_val.get())
        except Exception:
            pass
        config.set_consistent_threshold(result)

    def _update_damage_search_depth(self, *args, **kwargs):
        result = config.DEFAULT_DAMAGE_SEARCH_DEPTH
        try:
            result = int(self.search_depth_val.get())
        except Exception:
            pass
        config.set_damage_search_depth(result)
