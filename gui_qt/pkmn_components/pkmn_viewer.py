import logging

from PySide6.QtWidgets import (
    QWidget, QLabel, QGridLayout, QSizePolicy,
)
from PySide6.QtCore import Qt
from PySide6.QtGui import QFont

from gui_qt.pkmn_components.stat_column import StatColumn, tinted_bg_for_style

from pkmn.gen_factory import current_gen_info
from pkmn import universal_data_objects
from utils.config_manager import config

logger = logging.getLogger(__name__)


class PkmnViewer(QWidget):
    def __init__(self, parent=None, stats_only=False, font_size=None):
        super().__init__(parent)

        self.stats_only = stats_only
        self.setMinimumHeight(150)

        font_to_use = None
        if font_size is not None:
            font_to_use = QFont()
            font_to_use.setPointSize(font_size)

        self.stat_width = 4
        self.move_width = 11

        layout = QGridLayout(self)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(2)

        header_bg = tinted_bg_for_style("Header", alpha=0.25)
        header_color = config.get_header_color()

        # Name label (row 0, spans 2 columns)
        self._name_value = QLabel("")
        self._name_value.setStyleSheet(
            f"color: {header_color}; background-color: {header_bg};"
            f" padding: 2px 4px; border-radius: 2px;"
        )
        if font_to_use is not None:
            self._name_value.setFont(font_to_use)
        layout.addWidget(self._name_value, 0, 0, 1, 2)

        # Ability / Nature label (row 2, hidden until set_pkmn)
        self._ability = QLabel("")
        self._ability.setStyleSheet(
            f"color: {header_color}; background-color: {header_bg};"
            f" padding: 2px 4px; border-radius: 2px;"
        )
        if font_to_use is not None:
            self._ability.setFont(font_to_use)
        self._ability.setVisible(False)

        # Held item label (row 3, hidden until set_pkmn)
        self._held_item = QLabel("")
        self._held_item.setStyleSheet(
            f"color: {header_color}; background-color: {header_bg};"
            f" padding: 2px 4px; border-radius: 2px;"
        )
        if font_to_use is not None:
            self._held_item.setFont(font_to_use)
        self._held_item.setVisible(False)

        # Stat column (row 5, column 0)
        self.stat_column = StatColumn(
            parent=self,
            val_width=self.stat_width,
            num_rows=6,
            style_prefix="Secondary",
            font=font_to_use,
        )
        self.stat_column.set_labels(["HP:", "Attack:", "Defense:", "Spc Atk:", "Spc Def:", "Speed:"])
        self.stat_column.set_header("")
        layout.addWidget(self.stat_column, 5, 0)

        # Move column (row 5, column 1)
        self.move_column = StatColumn(
            parent=self,
            val_width=self.move_width,
            num_rows=6,
            style_prefix="Primary",
            font=font_to_use,
        )
        self.move_column.set_labels(["Lv:", "Exp:", "Move 1:", "Move 2:", "Move 3:", "Move 4:"])
        self.move_column.set_header("")

        if not self.stats_only:
            layout.addWidget(self.move_column, 5, 1)
        else:
            self.move_column.setVisible(False)

        # Configure columns to expand horizontally
        layout.setColumnStretch(0, 1)
        layout.setColumnStretch(1, 1)

    def set_pkmn(self, pkmn: universal_data_objects.EnemyPkmn, badges: universal_data_objects.BadgeList = None, speed_style=None):

        self._name_value.setText(pkmn.name)

        self._ability.setText(f"{pkmn.ability} ({pkmn.nature})")
        layout = self.layout()
        if current_gen_info().get_generation() >= 3:
            self._ability.setVisible(True)
            # addWidget is idempotent in terms of display; re-add to ensure it's in the grid
            layout.addWidget(self._ability, 2, 0, 1, 2)
        else:
            self._ability.setVisible(False)

        self._held_item.setText(f"Held Item: {pkmn.held_item}")
        if current_gen_info().get_generation() >= 2:
            self._held_item.setVisible(True)
            layout.addWidget(self._held_item, 3, 0, 1, 2)
        else:
            self._held_item.setVisible(False)

        attack_val = str(pkmn.cur_stats.attack)
        if badges is not None and badges.is_attack_boosted():
            attack_val = "*" + attack_val

        defense_val = str(pkmn.cur_stats.defense)
        if badges is not None and badges.is_defense_boosted():
            defense_val = "*" + defense_val

        spa_val = str(pkmn.cur_stats.special_attack)
        if badges is not None and badges.is_special_attack_boosted():
            spa_val = "*" + spa_val

        spd_val = str(pkmn.cur_stats.special_defense)
        if badges is not None and badges.is_special_defense_boosted():
            if current_gen_info().get_generation() == 2:
                unboosted_spa = pkmn.base_stats.calc_level_stats(
                    pkmn.level,
                    pkmn.dvs,
                    pkmn.stat_xp,
                    current_gen_info().make_badge_list(),
                    pkmn.nature,
                    ""
                ).special_attack
                if not pkmn.cur_stats.should_ignore_spd_badge_boost(unboosted_spa):
                    spd_val = "*" + spd_val
            else:
                spd_val = "*" + spd_val

        speed_val = str(pkmn.cur_stats.speed)
        if badges is not None and badges.is_speed_boosted():
            speed_val = "*" + speed_val

        self.stat_column.set_values(
            [str(pkmn.cur_stats.hp), attack_val, defense_val, spa_val, spd_val, speed_val],
            style_iterable=[None, None, None, None, None, speed_style]
        )

        move_list = [x for x in pkmn.move_list]
        for move_idx in range(len(move_list)):
            if move_list[move_idx] is None:
                move_list[move_idx] = ""
        self.move_column.set_values([str(pkmn.level), str(pkmn.xp)] + move_list)
