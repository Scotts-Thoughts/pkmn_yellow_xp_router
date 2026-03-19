import logging
from typing import Tuple

from PySide6.QtWidgets import (
    QWidget, QLabel, QGridLayout, QVBoxLayout,
)
from PySide6.QtCore import Qt

from gui_qt.components.custom_components import AmountEntry, SimpleOptionMenu

from pkmn.pkmn_info import CurrentGen
from pkmn.universal_data_objects import Nature, PokemonSpecies, StatBlock
from pkmn.gen_factory import current_gen_info

logger = logging.getLogger(__name__)


class CustomDVsFrame(QWidget):
    def __init__(
        self,
        target_mon: PokemonSpecies,
        parent=None,
        target_game: CurrentGen = None,
        init_dvs: StatBlock = None,
        init_ability_idx: int = None,
        init_nature: Nature = None,
    ):
        super().__init__(parent)
        # will be overwritten later by config function that runs at the end of constructor
        self._target_game = None

        outer_layout = QVBoxLayout(self)
        outer_layout.setContentsMargins(0, 0, 0, 0)
        outer_layout.setSpacing(0)

        self.controls_frame = QWidget(self)
        self._grid = QGridLayout(self.controls_frame)
        self._grid.setContentsMargins(0, 0, 0, 0)
        self._grid.setHorizontalSpacing(5)
        self._grid.setVerticalSpacing(5)
        outer_layout.addWidget(self.controls_frame)

        self.padx = 5
        self.pady = 5

        self.custom_dvs_hp_label = QLabel("HP DV:")
        self.custom_dvs_hp = AmountEntry(min_val=0, max_val=15, init_val=15, callback=self.recalc_hidden_power)
        self.custom_dvs_atk_label = QLabel("Attack DV:")
        self.custom_dvs_atk = AmountEntry(min_val=0, max_val=15, init_val=15, callback=self.recalc_hidden_power)
        self.custom_dvs_def_label = QLabel("Defense DV:")
        self.custom_dvs_def = AmountEntry(min_val=0, max_val=15, init_val=15, callback=self.recalc_hidden_power)
        self.custom_dvs_spd_label = QLabel("Speed DV:")
        self.custom_dvs_spd = AmountEntry(min_val=0, max_val=15, init_val=15, callback=self.recalc_hidden_power)
        self.custom_dvs_spc_atk_label = QLabel("Special DV:")
        self.custom_dvs_spc_atk = AmountEntry(min_val=0, max_val=15, init_val=15, callback=self.recalc_hidden_power)
        self.custom_dvs_spc_def_label = None
        self.custom_dvs_spc_def = None
        self.nature_label = None
        self._nature_lookup = []
        self.nature_vals = None
        self.ability_label = None
        self.ability_vals = None
        self.hidden_power_label = None
        self.hidden_power = None

        if target_game is None:
            target_game = current_gen_info()

        self.config_for_target_game_and_mon(
            target_game,
            target_mon,
            init_dvs=init_dvs,
            init_ability_idx=init_ability_idx,
            init_nature=init_nature,
        )

    def _remove_widget_from_grid(self, widget):
        """Remove a widget from the grid layout and hide it."""
        if widget is not None:
            self._grid.removeWidget(widget)
            widget.setParent(None)
            widget.deleteLater()

    def config_for_target_game_and_mon(
        self,
        target_game: CurrentGen,
        target_mon: PokemonSpecies,
        init_dvs: StatBlock = None,
        init_ability_idx: int = None,
        init_nature: Nature = None,
    ):
        self._target_game = target_game
        cur_gen = self._target_game.get_generation()

        if init_dvs is None:
            if cur_gen <= 2:
                max_dv = 15
            else:
                max_dv = 31

            init_dvs = self._target_game.make_stat_block(max_dv, max_dv, max_dv, max_dv, max_dv, max_dv)

        if init_ability_idx is None and cur_gen >= 3:
            init_ability_idx = 0

        if init_nature is None and cur_gen >= 3:
            init_nature = Nature.HARDY

        if cur_gen > 2:
            dv_max = 31
            dv_text = "IV"
            self._nature_lookup = [str(x) for x in Nature]

            # these may not exist, so create them if necessary
            if self.custom_dvs_spc_def_label is None:
                self.custom_dvs_spc_def_label = QLabel()
            if self.custom_dvs_spc_def is None:
                self.custom_dvs_spc_def = AmountEntry(min_val=0, max_val=dv_max, callback=self.recalc_hidden_power)
            if self.nature_label is None:
                self.nature_label = QLabel("Nature:")
            if self.nature_vals is None:
                self.nature_vals = SimpleOptionMenu(
                    option_list=self._nature_lookup,
                    default_val=str(init_nature),
                )
            if self.ability_label is None:
                self.ability_label = QLabel("Ability:")
            if self.ability_vals is None:
                ability_list = target_mon.abilities if target_mon is not None else [""]
                self.ability_vals = SimpleOptionMenu(
                    option_list=ability_list,
                    default_val=ability_list[init_ability_idx],
                )
            else:
                ability_list = target_mon.abilities if target_mon is not None else [""]
                self.ability_vals.new_values(ability_list)
            if self.hidden_power_label is None:
                self.hidden_power_label = QLabel("Hidden Power:")
            if self.hidden_power is None:
                self.hidden_power = QLabel("")

            self.custom_dvs_hp_label.setText(f"HP {dv_text}:")
            self.custom_dvs_hp.max_val = dv_max
            self.custom_dvs_hp.set(init_dvs.hp)
            self.custom_dvs_hp.enable()
            self.custom_dvs_atk_label.setText(f"Attack {dv_text}:")
            self.custom_dvs_atk.max_val = dv_max
            self.custom_dvs_atk.set(init_dvs.attack)
            self.custom_dvs_def_label.setText(f"Defense {dv_text}:")
            self.custom_dvs_def.max_val = dv_max
            self.custom_dvs_def.set(init_dvs.defense)
            self.custom_dvs_spd_label.setText(f"Speed {dv_text}:")
            self.custom_dvs_spd.max_val = dv_max
            self.custom_dvs_spd.set(init_dvs.speed)
            self.custom_dvs_spc_atk_label.setText(f"Special Attack {dv_text}:")
            self.custom_dvs_spc_atk.max_val = dv_max
            self.custom_dvs_spc_atk.set(init_dvs.special_attack)
            self.custom_dvs_spc_def_label.setText(f"Special Defense {dv_text}:")
            self.custom_dvs_spc_def.max_val = dv_max
            self.custom_dvs_spc_def.set(init_dvs.special_defense)
            self.ability_vals.set(ability_list[init_ability_idx])
        else:
            dv_text = "DV"
            dv_max = 15
            self._nature_lookup = []

            if self.custom_dvs_spc_def_label is not None:
                self._remove_widget_from_grid(self.custom_dvs_spc_def_label)
                self.custom_dvs_spc_def_label = None
            if self.custom_dvs_spc_def is not None:
                self._remove_widget_from_grid(self.custom_dvs_spc_def)
                self.custom_dvs_spc_def = None
            if self.nature_label is not None:
                self._remove_widget_from_grid(self.nature_label)
                self.nature_label = None
            if self.nature_vals is not None:
                self._remove_widget_from_grid(self.nature_vals)
                self.nature_vals = None
            if self.ability_label is not None:
                self._remove_widget_from_grid(self.ability_label)
                self.ability_label = None
            if self.ability_vals is not None:
                self._remove_widget_from_grid(self.ability_vals)
                self.ability_vals = None
            if cur_gen == 1:
                if self.hidden_power_label is not None:
                    self._remove_widget_from_grid(self.hidden_power_label)
                    self.hidden_power_label = None
                if self.hidden_power is not None:
                    self._remove_widget_from_grid(self.hidden_power)
                    self.hidden_power = None
            elif cur_gen == 2:
                if self.hidden_power_label is None:
                    self.hidden_power_label = QLabel("Hidden Power:")
                if self.hidden_power is None:
                    self.hidden_power = QLabel("")

            self.custom_dvs_atk_label.setText(f"Attack {dv_text}:")
            self.custom_dvs_atk.max_val = dv_max
            self.custom_dvs_atk.set(init_dvs.attack)
            self.custom_dvs_def_label.setText(f"Defense {dv_text}:")
            self.custom_dvs_def.max_val = dv_max
            self.custom_dvs_def.set(init_dvs.defense)
            self.custom_dvs_spd_label.setText(f"Speed {dv_text}:")
            self.custom_dvs_spd.max_val = dv_max
            self.custom_dvs_spd.set(init_dvs.speed)
            self.custom_dvs_spc_atk_label.setText(f"Special {dv_text}:")
            self.custom_dvs_spc_atk.max_val = dv_max
            self.custom_dvs_spc_atk.set(init_dvs.special_attack)

            # Gen 1 & 2: HP DV is derived from the other DVs, not user-editable
            self.custom_dvs_hp_label.setText(f"HP {dv_text}:")
            self.custom_dvs_hp.max_val = dv_max
            self.custom_dvs_hp.disable()
            self._recalc_hp_dv()

        # Place all existing widgets into the grid
        self._grid.addWidget(self.custom_dvs_hp_label, 0, 0)
        self._grid.addWidget(self.custom_dvs_hp, 0, 1)
        self._grid.addWidget(self.custom_dvs_atk_label, 1, 0)
        self._grid.addWidget(self.custom_dvs_atk, 1, 1)
        self._grid.addWidget(self.custom_dvs_def_label, 2, 0)
        self._grid.addWidget(self.custom_dvs_def, 2, 1)
        self._grid.addWidget(self.custom_dvs_spd_label, 3, 0)
        self._grid.addWidget(self.custom_dvs_spd, 3, 1)
        self._grid.addWidget(self.custom_dvs_spc_atk_label, 4, 0)
        self._grid.addWidget(self.custom_dvs_spc_atk, 4, 1)
        if self.custom_dvs_spc_def_label is not None:
            self._grid.addWidget(self.custom_dvs_spc_def_label, 5, 0)
        if self.custom_dvs_spc_def is not None:
            self._grid.addWidget(self.custom_dvs_spc_def, 5, 1)

        if self.hidden_power_label is not None:
            self._grid.addWidget(self.hidden_power_label, 6, 0)
        if self.hidden_power is not None:
            self._grid.addWidget(self.hidden_power, 6, 1)

        if self.nature_label is not None:
            self._grid.addWidget(self.nature_label, 10, 0)
        if self.nature_vals is not None:
            self._grid.addWidget(self.nature_vals, 10, 1)
        if self.ability_label is not None:
            self._grid.addWidget(self.ability_label, 12, 0)
        if self.ability_vals is not None:
            self._grid.addWidget(self.ability_vals, 12, 1)

    def _recalc_hp_dv(self):
        """Auto-calculate HP DV for Gen 1 & 2 from the other DVs."""
        try:
            atk = int(self.custom_dvs_atk.get())
            defense = int(self.custom_dvs_def.get())
            spd = int(self.custom_dvs_spd.get())
            spc = int(self.custom_dvs_spc_atk.get())
            hp_dv = (atk % 2) * 8 + (defense % 2) * 4 + (spd % 2) * 2 + (spc % 2) * 1
            self.custom_dvs_hp.set(hp_dv)
        except (ValueError, TypeError):
            pass

    def recalc_hidden_power(self, *args, **kwargs):
        # For Gen 1 & 2, recalculate the HP DV from the other DVs
        if hasattr(self, '_target_game') and self._target_game is not None:
            if self._target_game.get_generation() <= 2:
                self._recalc_hp_dv()

        if not hasattr(self, 'hidden_power') or self.hidden_power is None:
            return
        try:
            if self.custom_dvs_spc_def is not None:
                spc_def_stat = self.custom_dvs_spc_def.get()
            else:
                spc_def_stat = self.custom_dvs_spc_atk.get()
            hp_type, hp_power = self._target_game.get_hidden_power(
                StatBlock(
                    int(self.custom_dvs_hp.get()),
                    int(self.custom_dvs_atk.get()),
                    int(self.custom_dvs_def.get()),
                    int(self.custom_dvs_spc_atk.get()),
                    int(spc_def_stat),
                    int(self.custom_dvs_spd.get())
                )
            )

            if not hp_type:
                self.hidden_power.setText("Not supported in gen 1")
            else:
                self.hidden_power.setText(f"{hp_type}: {hp_power}")
        except Exception as e:
            self.hidden_power.setText("Failed to calculate, invalid DVs")
            logger.exception("Failed to calculated hidden power")

    def get_dvs(self, *args, **kwargs) -> Tuple[StatBlock, int, Nature]:
        if self.nature_vals is None:
            new_nature = Nature.HARDY
        else:
            new_nature = Nature(self._nature_lookup.index(self.nature_vals.get()))

        if self.ability_vals is None:
            new_ability = 0
        else:
            new_ability = self.ability_vals.cur_options.index(self.ability_vals.get())

        if self.custom_dvs_spc_def is not None:
            spc_def_stat = self.custom_dvs_spc_def.get()
        else:
            spc_def_stat = self.custom_dvs_spc_atk.get()

        return (
            StatBlock(
                int(self.custom_dvs_hp.get()),
                int(self.custom_dvs_atk.get()),
                int(self.custom_dvs_def.get()),
                int(self.custom_dvs_spc_atk.get()),
                int(spc_def_stat),
                int(self.custom_dvs_spd.get())
            ),
            new_ability,
            new_nature
        )
