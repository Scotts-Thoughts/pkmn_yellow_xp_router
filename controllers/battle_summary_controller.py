from __future__ import annotations
from dataclasses import dataclass
import copy
import logging
from typing import Dict, List, Tuple
from controllers.main_controller import MainController
from pkmn.damage_calc import DamageRange, find_kill
from pkmn.universal_data_objects import EnemyPkmn, FieldStatus, StageModifiers
from routing.full_route_state import RouteState
from utils.config_manager import config

from utils.constants import const
from routing.route_events import EventDefinition, EventGroup, RareCandyEventDefinition, TrainerEventDefinition
from pkmn.gen_factory import current_gen_info

logger = logging.getLogger(__name__)


@dataclass
class MoveRenderInfo:
    name:str
    attack_flavor:List[str]
    damage_ranges:DamageRange
    crit_damage_ranges:DamageRange
    defending_mon_hp:int
    kill_ranges:List[Tuple[int, float]]
    mimic_data:str
    mimic_options:List[str]
    custom_data_options:List[str]
    custom_data_selection:str
    is_best_move:bool=False
    # Stat stage dropdown options (None if this move doesn't have stat effects)
    stat_stage_options:List[str]=None
    stat_stage_selection:str="0"
    # Info about the stat effect for UI/logic purposes
    stat_stage_info:dict=None

@dataclass
class PkmnRenderInfo:
    attacking_mon_name:str
    attacking_mon_level:int
    attacking_mon_speed:int
    defending_mon_name:str
    defending_mon_level:int
    defending_mon_speed:int
    defending_mon_hp:int

    def __str__(self) -> str:
        if self.attacking_mon_speed > self.defending_mon_speed:
            verb = "outspeeds"
        elif self.defending_mon_speed > self.attacking_mon_speed:
            verb = "underspeeds"
        else:
            verb = "speed-ties"
        
        return f"Lv {self.attacking_mon_level}: {self.attacking_mon_name} {verb} Lv {self.defending_mon_level}: {self.defending_mon_name} ({self.defending_mon_hp} HP)"


class BattleSummaryController:
    def __init__(self, main_controller:MainController):
        self._main_controller = main_controller
        self._refresh_events = []
        self._nonload_change_events = []

        # trainer object data that we don't actually use, but need to hang on to to properly re-create events
        self._trainer_name = None
        self._second_trainer_name = None
        self._event_group_id = None

        # actual state used to calculate battle stats
        self._original_player_mon_list:List[EnemyPkmn] = []
        self._player_setup_move_list:List[str] = []
        self._is_player_transformed:bool = False
        self._transformed_mon_list:List[EnemyPkmn] = []
        self._original_enemy_mon_list:List[EnemyPkmn] = []
        self._enemy_setup_move_list:List[str] = []

        self._mimic_options:List[str] = []
        self._player_mimic_selection:str = ""
        self._mimic_selection:str = ""
        self._custom_move_data:List[Dict[str, Dict[str, str]]] = []
        self._move_highlights:List[Dict[str, Dict[str, int]]] = []
        self._cached_definition_order = []
        self._weather = None
        self._double_battle_flag = False
        
        # Per-move stat stage setup: tracks how many times each stat-altering move is applied
        # Structure: List[Dict[player/enemy, Dict[move_name, count_str]]]
        # where count_str is "0", "1", "2", etc.
        self._stat_stage_setup:List[Dict[str, Dict[str, str]]] = []

        # NOTE: all of the state above this comment is considered the "true" state
        # The below state is all calculated based on values from the above state
        self._player_stage_modifier:StageModifiers = None
        self._enemy_stage_modifier:StageModifiers = None
        self._player_field_status:FieldStatus = None
        self._enemy_field_status:FieldStatus = None
        
        # Per-matchup stage modifiers (calculated from per-move stat stage setup)
        self._per_matchup_player_modifiers:List[StageModifiers] = []
        self._per_matchup_enemy_modifiers:List[StageModifiers] = []

        # NOTE: and finally, the actual display information
        # first idx: idx of pkmn in team
        # second idx: idx of move for pkmn pair
        self._player_move_data:List[List[MoveRenderInfo]] = []
        self._enemy_move_data:List[List[MoveRenderInfo]] = []
        # first idx: idx of pkmn in team
        self._player_pkmn_matchup_data:List[PkmnRenderInfo] = []
        self._enemy_pkmn_matchup_data:List[PkmnRenderInfo] = []

        self.load_empty()

    
    #####
    # Registration methods
    #####

    def register_nonload_change(self, tk_obj):
        new_event_name = const.EVENT_BATTLE_SUMMARY_NONLOAD_CHANGE.format(len(self._nonload_change_events))
        self._nonload_change_events.append((tk_obj, new_event_name))
        return new_event_name

    def register_refresh(self, tk_obj):
        new_event_name = const.EVENT_BATTLE_SUMMARY_REFRESH.format(len(self._refresh_events))
        self._refresh_events.append((tk_obj, new_event_name))
        return new_event_name

    #####
    # Event callbacks
    #####
    
    def _on_refresh(self):
        for tk_obj, cur_event_name in self._refresh_events:
            tk_obj.event_generate(cur_event_name, when="tail")
    
    def _on_nonload_change(self):
        for tk_obj, cur_event_name in self._nonload_change_events:
            tk_obj.event_generate(cur_event_name, when="tail")
    
    ######
    # Methods that induce a state change
    ######

    def update_mimic_selection(self, new_value):
        self._mimic_selection = new_value
        target_found = False
        for mon_idx in range(len(self._original_enemy_mon_list)):
            if not target_found:
                if new_value in self._original_enemy_mon_list[mon_idx].move_list:
                    target_found = True
            
            if not target_found:
                move_name = "Leer"
            else:
                move_name = self._mimic_selection
            
            if const.MIMIC_MOVE_NAME in self._original_player_mon_list[mon_idx].move_list:
                cur_mimic_idx = self._original_player_mon_list[mon_idx].move_list.index(const.MIMIC_MOVE_NAME)
                self._player_move_data[mon_idx][cur_mimic_idx] = self._recalculate_single_move(mon_idx, True, move_name, move_display_name=const.MIMIC_MOVE_NAME)
                self._update_best_move_inplace(mon_idx, True)

        self._on_refresh()
        self._on_nonload_change()

    def update_custom_move_data(self, pkmn_idx, move_idx, is_player_mon, new_value):
        try:
            if is_player_mon:
                move_data = self._player_move_data
                lookup_key = const.PLAYER_KEY
            else:
                move_data = self._enemy_move_data
                lookup_key = const.ENEMY_KEY

            move_name = move_data[pkmn_idx][move_idx].name
            self._custom_move_data[pkmn_idx][lookup_key][move_name] = new_value

            move_data[pkmn_idx][move_idx] = self._recalculate_single_move(pkmn_idx, is_player_mon, move_name)
            self._update_best_move_inplace(pkmn_idx, is_player_mon)

            self._on_refresh()
            self._on_nonload_change()
        except Exception as e:
            logger.error(f"encountered error updating custom move data: {pkmn_idx, move_idx, is_player_mon, new_value}")

    def _get_stat_stage_selection(self, pkmn_idx:int, is_player_mon:bool, move_name:str) -> str:
        """Get the current stat stage selection for a move."""
        if pkmn_idx < 0 or pkmn_idx >= len(self._stat_stage_setup):
            return "0"
        
        lookup_key = const.PLAYER_KEY if is_player_mon else const.ENEMY_KEY
        return self._stat_stage_setup[pkmn_idx].get(lookup_key, {}).get(move_name, "0")
    
    def update_stat_stage_setup(self, pkmn_idx:int, move_idx:int, is_player_mon:bool, new_value:str):
        """Update the stat stage setup for a specific move.
        
        This triggers a recalculation of damage for this matchup and potentially
        subsequent matchups depending on the move's targeting.
        """
        try:
            if is_player_mon:
                move_data = self._player_move_data
                lookup_key = const.PLAYER_KEY
            else:
                move_data = self._enemy_move_data
                lookup_key = const.ENEMY_KEY
            
            if pkmn_idx < 0 or pkmn_idx >= len(move_data) or move_idx < 0 or move_idx >= len(move_data[pkmn_idx]):
                return
            
            move_info = move_data[pkmn_idx][move_idx]
            if move_info is None:
                return
            
            move_name = move_info.name
            
            # Ensure stat_stage_setup structure exists
            while len(self._stat_stage_setup) <= pkmn_idx:
                self._stat_stage_setup.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
            
            if lookup_key not in self._stat_stage_setup[pkmn_idx]:
                self._stat_stage_setup[pkmn_idx][lookup_key] = {}
            
            self._stat_stage_setup[pkmn_idx][lookup_key][move_name] = new_value
            
            # Determine the scope of recalculation based on move properties
            # For now, we do a full refresh to ensure correctness
            # TODO: Optimize to only recalculate affected matchups
            self._full_refresh()
        except Exception as e:
            logger.error(f"encountered error updating stat stage setup: {pkmn_idx, move_idx, is_player_mon, new_value}")
            logger.exception(e)
    
    def has_global_setup(self) -> bool:
        """Check if any global setup moves are configured (player or enemy)."""
        return len(self._player_setup_move_list) > 0 or len(self._enemy_setup_move_list) > 0
    
    def clear_per_move_stat_setup(self):
        """Clear all per-move stat stage setup selections."""
        for pkmn_setup in self._stat_stage_setup:
            pkmn_setup[const.PLAYER_KEY] = {}
            pkmn_setup[const.ENEMY_KEY] = {}

    def update_weather(self, new_weather):
        self._weather = new_weather
        self._full_refresh()

    def update_enemy_setup_moves(self, new_setup_moves):
        self._enemy_setup_move_list = new_setup_moves
        # Clear per-move stat setup for enemy when global setup is used
        if new_setup_moves:
            self._clear_per_move_stat_setup_for_side(is_player=False)
        self._full_refresh()

    def update_player_setup_moves(self, new_setup_moves):
        self._player_setup_move_list = new_setup_moves
        # Clear per-move stat setup for player when global setup is used
        if new_setup_moves:
            self._clear_per_move_stat_setup_for_side(is_player=True)
        self._full_refresh()
    
    def _clear_per_move_stat_setup_for_side(self, is_player:bool):
        """Clear per-move stat stage setup for one side (player or enemy)."""
        lookup_key = const.PLAYER_KEY if is_player else const.ENEMY_KEY
        for pkmn_setup in self._stat_stage_setup:
            if lookup_key in pkmn_setup:
                pkmn_setup[lookup_key] = {}

    def update_player_transform(self, is_transformed):
        self._is_player_transformed = is_transformed
        self._full_refresh()

    def update_prefight_candies(self, num_candies):
        if self._event_group_id is None:
            return
        
        prev_event = self._main_controller.get_previous_event(self._event_group_id, enabled_only=True)
        if prev_event is None or prev_event.event_definition is None or prev_event.event_definition.rare_candy is None:
            # If num_candies is 0 and we don't have a number to update, don't create a pointless "use 0 candies" event
            if num_candies <= 0:
                return
            self._main_controller.new_event(
                EventDefinition(rare_candy=RareCandyEventDefinition(amount=num_candies)),
                insert_before=self._event_group_id,
                do_select=False
            )
        else:
            # If num_candies is 0, delete the existing candy event
            if num_candies <= 0:
                self._main_controller.delete_events([prev_event.group_id])
            else:
                self._main_controller.update_existing_event(
                    prev_event.group_id,
                    EventDefinition(rare_candy=RareCandyEventDefinition(amount=num_candies)),
                )
        
        self._full_refresh()

    def get_show_move_highlights(self) -> bool:
        return config.get_show_move_highlights()

    def update_move_highlight(self, pkmn_idx:int, move_idx:int, is_player_mon:bool, reset=False):
        """Cycle through highlight states: 0 (default) -> 1 (dark green) -> 2 (dark blue) -> 3 (dark orange) -> 0
        If reset=True, immediately set to 0 (default)"""
        if self._event_group_id is None:
            return
        
        # Get the move name
        if is_player_mon:
            move_data = self._player_move_data
        else:
            move_data = self._enemy_move_data
        
        if pkmn_idx < 0 or pkmn_idx >= len(move_data) or move_idx < 0 or move_idx >= len(move_data[pkmn_idx]):
            return
        
        move_info = move_data[pkmn_idx][move_idx]
        if move_info is None:
            return
        
        move_name = move_info.name
        
        # Ensure move_highlights structure exists
        while len(self._move_highlights) <= pkmn_idx:
            self._move_highlights.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
        
        lookup_key = const.PLAYER_KEY if is_player_mon else const.ENEMY_KEY
        current_state = self._move_highlights[pkmn_idx][lookup_key].get(move_name, 0)
        
        if reset:
            # Reset to default immediately
            new_state = 0
        else:
            # Cycle: 0 -> 1 -> 2 -> 3 -> 0
            new_state = (current_state + 1) % 4
        
        self._move_highlights[pkmn_idx][lookup_key][move_name] = new_state
        
        # Trigger nonload change for delayed save (don't refresh immediately - UI updates directly)
        self._on_nonload_change()

    def get_move_highlight_state(self, pkmn_idx:int, move_idx:int, is_player_mon:bool) -> int:
        """Get highlight state for a move: 0 (default), 1 (dark green), 2 (dark blue), 3 (dark orange)"""
        if pkmn_idx < 0 or pkmn_idx >= len(self._move_highlights):
            return 0
        
        # Get the move name
        if is_player_mon:
            move_data = self._player_move_data
        else:
            move_data = self._enemy_move_data
        
        if move_idx < 0 or move_idx >= len(move_data[pkmn_idx]) or move_data[pkmn_idx][move_idx] is None:
            return 0
        
        move_name = move_data[pkmn_idx][move_idx].name
        lookup_key = const.PLAYER_KEY if is_player_mon else const.ENEMY_KEY
        
        return self._move_highlights[pkmn_idx][lookup_key].get(move_name, 0)

    def _save_move_highlights_to_event(self):
        """Save move highlights to the current event - called via nonload_change event (delayed)"""
        if self._event_group_id is None:
            return
        
        event_group = self._main_controller.get_event_by_id(self._event_group_id)
        if event_group is None or event_group.event_definition is None or event_group.event_definition.trainer_def is None:
            return
        
        # Reorder highlights according to cached definition order
        if self._move_highlights and self._cached_definition_order:
            reordered_highlights = [self._move_highlights[x] for x in self._cached_definition_order]
        else:
            reordered_highlights = self._move_highlights if self._move_highlights else None
        
        # Check if highlights are empty (all states are 0 or missing)
        is_empty = True
        if reordered_highlights:
            for pkmn_highlights in reordered_highlights:
                for key in [const.PLAYER_KEY, const.ENEMY_KEY]:
                    if pkmn_highlights.get(key):
                        for move_name, state in pkmn_highlights[key].items():
                            if state != 0:
                                is_empty = False
                                break
                        if not is_empty:
                            break
                    if not is_empty:
                        break
                if not is_empty:
                    break
        
        if is_empty:
            reordered_highlights = None
        
        # Create updated trainer definition
        trainer_def = event_group.event_definition.trainer_def
        updated_trainer_def = TrainerEventDefinition(
            trainer_def.trainer_name,
            second_trainer_name=trainer_def.second_trainer_name,
            verbose_export=trainer_def.verbose_export,
            setup_moves=trainer_def.setup_moves,
            mimic_selection=trainer_def.mimic_selection,
            custom_move_data=trainer_def.custom_move_data,
            enemy_setup_moves=trainer_def.enemy_setup_moves,
            player_field_moves=trainer_def.player_field_moves,
            enemy_field_moves=trainer_def.enemy_field_moves,
            exp_split=trainer_def.exp_split,
            weather=trainer_def.weather,
            pay_day_amount=trainer_def.pay_day_amount,
            mon_order=trainer_def.mon_order,
            transformed=trainer_def.transformed,
            move_highlights=reordered_highlights,
        )
        
        self._main_controller.update_existing_event(
            self._event_group_id,
            EventDefinition(trainer_def=updated_trainer_def),
        )
    
    def update_player_strategy(self, strat):
        config.set_player_highlight_strategy(strat)
        self._full_refresh()
    
    def update_enemy_strategy(self, strat):
        config.set_enemy_highlight_strategy(strat)
        self._full_refresh()
    
    def update_consistent_threshold(self, threshold:int):
        config.set_consistent_threshold(threshold)
        self._full_refresh()
    
    def _update_best_move_inplace(self, pkmn_idx, is_player_mon):
        # NOTE: this is a helper function that induces a state change, but does not directly (or indirectly) trigger any events
        # If you call this function, it is your responsibility to properly trigger an appropriate event independently
        if is_player_mon:
            move_data = self._player_move_data
            mon_data = self._player_pkmn_matchup_data[pkmn_idx]
        else:
            move_data = self._enemy_move_data
            mon_data = self._enemy_pkmn_matchup_data[pkmn_idx]

        best_move = None
        best_move_idx = None
        for idx, cur_move in enumerate(move_data[pkmn_idx]):
            if cur_move is None or cur_move.name == const.STRUGGLE_MOVE_NAME:
                continue

            if self._is_move_better(
                cur_move,
                best_move,
                config.get_player_highlight_strategy() if is_player_mon else config.get_enemy_highlight_strategy(),
                mon_data
            ):
                best_move = cur_move
                best_move_idx = idx
        
        for idx, cur_move in enumerate(move_data[pkmn_idx]):
            if cur_move is None:
                continue

            cur_move.is_best_move = idx == best_move_idx


    def _full_refresh(self, is_load=False):
        # Once the "true" state of the current battle has been updated, recalculate all the derived properties
        
        # Check if we're using global setup (which overrides per-move setup)
        using_global_player_setup = len(self._player_setup_move_list) > 0
        using_global_enemy_setup = len(self._enemy_setup_move_list) > 0
        
        # Calculate global stage modifiers (used when global setup is enabled)
        self._player_stage_modifier = self._calc_stage_modifier(self._player_setup_move_list)
        self._player_field_status = self._calc_field_status(self._player_setup_move_list)
        self._enemy_stage_modifier = self._calc_stage_modifier(self._enemy_setup_move_list)
        self._enemy_field_status = self._calc_field_status(self._enemy_setup_move_list)
        
        # Calculate per-matchup stage modifiers based on stat_stage_setup
        # This is separate from global setup - if global setup is used, per-move setup is disabled
        self._per_matchup_player_modifiers = []
        self._per_matchup_enemy_modifiers = []
        
        # Calculate per-move stat stage modifiers for all generations
        # Gen 1 badge boost bug is already handled by StageModifiers.apply_stat_mod()
        if not using_global_player_setup and not using_global_enemy_setup:
            self._per_matchup_player_modifiers, self._per_matchup_enemy_modifiers = self._calc_per_matchup_stage_modifiers()
        
        self._player_pkmn_matchup_data = []
        self._enemy_pkmn_matchup_data = []
        self._player_move_data = []
        self._enemy_move_data = []
        self._mimic_options = []

        can_mimic_yet = False
        for mon_idx in range(len(self._original_player_mon_list)):
            # Determine which stage modifiers to use for this matchup
            if mon_idx < len(self._per_matchup_player_modifiers):
                cur_player_stage_mod = self._per_matchup_player_modifiers[mon_idx]
            else:
                cur_player_stage_mod = self._player_stage_modifier
            
            if mon_idx < len(self._per_matchup_enemy_modifiers):
                cur_enemy_stage_mod = self._per_matchup_enemy_modifiers[mon_idx]
            else:
                cur_enemy_stage_mod = self._enemy_stage_modifier
            
            if self._is_player_transformed:
                player_mon = self._transformed_mon_list[mon_idx]
                player_stats = player_mon.cur_stats
            else:
                player_mon = self._original_player_mon_list[mon_idx]
                player_stats = player_mon.get_battle_stats(cur_player_stage_mod)

            enemy_mon = self._original_enemy_mon_list[mon_idx]
            enemy_stats = enemy_mon.get_battle_stats(cur_enemy_stage_mod)

            self._player_pkmn_matchup_data.append(
                PkmnRenderInfo(player_mon.name, player_mon.level, player_stats.speed, enemy_mon.name, enemy_mon.level, enemy_stats.speed, enemy_mon.cur_stats.hp)
            )
            self._enemy_pkmn_matchup_data.append(
                PkmnRenderInfo(enemy_mon.name, enemy_mon.level, enemy_stats.speed, player_mon.name, player_mon.level, player_stats.speed, player_mon.cur_stats.hp)
            )
            self._player_move_data.append([])
            self._enemy_move_data.append([])

            struggle_set = False
            for move_idx in range(4):
                # Handle the player move calculation
                if move_idx < len(player_mon.move_list):
                    move_name = player_mon.move_list[move_idx]
                    move_display_name = move_name
                    if move_name == const.MIMIC_MOVE_NAME:
                        if can_mimic_yet:
                            move_name = self._mimic_selection
                        elif self._mimic_selection and (self._mimic_selection in enemy_mon.move_list):
                            move_name = self._mimic_selection
                            can_mimic_yet = True
                        else:
                            move_name = "Leer"
                    
                    if not move_name and not struggle_set:
                        struggle_set = True
                        move_name = const.STRUGGLE_MOVE_NAME
                    
                    cur_player_move_data = self._recalculate_single_move(mon_idx, True, move_name, move_display_name=move_display_name)
                else:
                    cur_player_move_data = None

                # Now handle the enemy move calculation
                if move_idx < len(enemy_mon.move_list):
                    move_name = enemy_mon.move_list[move_idx]
                    if move_name and move_name not in self._mimic_options:
                        self._mimic_options.append(move_name)
                    cur_enemy_move_data = self._recalculate_single_move(mon_idx, False, move_name)
                else:
                    cur_enemy_move_data = None
                

                #####
                # Now the info has been generated for both sides of the fight. Add to the data structure, if appropriate
                #####
                self._player_move_data[mon_idx].append(cur_player_move_data)
                self._enemy_move_data[mon_idx].append(cur_enemy_move_data)

            #####
            # Add test moves (player moves 5-8) if enabled
            #####
            if config.get_test_moves_enabled():
                test_moves = self._main_controller.get_raw_route().test_moves
                for test_idx in range(4):
                    # Only process if test move is defined (not empty or None)
                    if test_idx < len(test_moves) and test_moves[test_idx] and test_moves[test_idx].strip():
                        test_move_name = test_moves[test_idx].strip()
                        test_move_data = self._recalculate_single_move(mon_idx, True, test_move_name, move_idx=4 + test_idx)
                        self._player_move_data[mon_idx].append(test_move_data)
                    else:
                        self._player_move_data[mon_idx].append(None)

            #####
            # Finally out of move data loop. Update best moves
            #####
            self._update_best_move_inplace(mon_idx, True)
            self._update_best_move_inplace(mon_idx, False)
        
        # finally done calculating everything. Refresh and exit
        self._on_refresh()
        if not is_load:
            self._on_nonload_change()

    def _recalculate_single_move(
        self,
        mon_idx:int,
        is_player_mon:bool,
        move_name:str,
        move_display_name:str=None,
    ):
        if is_player_mon:
            # TODO: gross hacky transform support. Somehow we should figure out how to offload some of this logic back into the generation objects...
            # but I'm not sure how, currently...
            if self._is_player_transformed:
                attacking_mon = self._transformed_mon_list[mon_idx]
                attacking_mon_stats = attacking_mon.cur_stats
                if current_gen_info().get_generation() == 1:
                    if attacking_mon.level > self._transformed_mon_list[0].level:
                        attacking_mon.badges = copy.deepcopy(self._original_player_mon_list[0].badges)
                        # Use per-matchup modifiers for Gen 1 transform case
                        player_stage_mod = self._per_matchup_player_modifiers[mon_idx] if mon_idx < len(self._per_matchup_player_modifiers) else self._player_stage_modifier
                        attacking_mon_stats = attacking_mon.get_battle_stats(player_stage_mod)
                    orig_player_mon = self._original_player_mon_list[mon_idx]
                    crit_mon = copy.deepcopy(attacking_mon)
                    crit_mon.level = orig_player_mon.level
                    crit_mon.base_stats = orig_player_mon.base_stats
                    crit_mon.stat_xp = orig_player_mon.stat_xp
                    crit_mon.dvs = orig_player_mon.dvs
                    crit_mon.badges = None
                    crit_mon_stats = None
                elif current_gen_info().get_generation() == 2:
                    crit_mon = attacking_mon
                    crit_mon_stats = attacking_mon_stats

                    if attacking_mon.level > self._transformed_mon_list[0].level:
                        # Use per-matchup modifiers if available
                        player_stage_mod = self._per_matchup_player_modifiers[mon_idx] if mon_idx < len(self._per_matchup_player_modifiers) else self._player_stage_modifier
                        attacking_mon_stats = self._original_player_mon_list[mon_idx].get_battle_stats(player_stage_mod)
                        crit_mon_stats = self._original_player_mon_list[mon_idx].get_battle_stats(player_stage_mod, is_crit=True)
                else:
                    crit_mon = attacking_mon
                    crit_mon_stats = attacking_mon_stats
            else:
                attacking_mon = self._original_player_mon_list[mon_idx]
                attacking_mon_stats = None
                crit_mon = attacking_mon
                crit_mon_stats = attacking_mon_stats
            
            # Use per-matchup modifiers if available, otherwise use global modifiers
            if mon_idx < len(self._per_matchup_player_modifiers):
                attacking_stage_modifiers = self._per_matchup_player_modifiers[mon_idx]
            else:
                attacking_stage_modifiers = self._player_stage_modifier
            attacking_field_status = self._player_field_status
            defending_mon = self._original_enemy_mon_list[mon_idx]
            defending_mon_stats = None
            if mon_idx < len(self._per_matchup_enemy_modifiers):
                defending_stage_modifiers = self._per_matchup_enemy_modifiers[mon_idx]
            else:
                defending_stage_modifiers = self._enemy_stage_modifier
            defending_field_status = self._enemy_field_status
            custom_lookup_key = const.PLAYER_KEY
        else:
            attacking_mon = self._original_enemy_mon_list[mon_idx]
            attacking_mon_stats = None
            crit_mon = attacking_mon
            crit_mon_stats = attacking_mon_stats
            # Use per-matchup modifiers if available
            if mon_idx < len(self._per_matchup_enemy_modifiers):
                attacking_stage_modifiers = self._per_matchup_enemy_modifiers[mon_idx]
            else:
                attacking_stage_modifiers = self._enemy_stage_modifier
            attacking_field_status = self._enemy_field_status
            if self._is_player_transformed:
                defending_mon = self._transformed_mon_list[mon_idx]
                defending_mon_stats = defending_mon.cur_stats
            else:
                defending_mon = self._original_player_mon_list[mon_idx]
                defending_mon_stats = None
            if mon_idx < len(self._per_matchup_player_modifiers):
                defending_stage_modifiers = self._per_matchup_player_modifiers[mon_idx]
            else:
                defending_stage_modifiers = self._player_stage_modifier
            defending_field_status = self._player_field_status
            custom_lookup_key = const.ENEMY_KEY

        if not move_name:
            return None
        move = current_gen_info().move_db().get_move(move_name)
        if move is None:
            logger.error(f"invalid move encountered during battle summary calculations: {move_name}")
            return None
        if move.name == const.HIDDEN_POWER_MOVE_NAME:
            hidden_power_type, hidden_power_base_power = current_gen_info().get_hidden_power(attacking_mon.dvs)
            move_display_name = f"{move.name} ({hidden_power_type}: {hidden_power_base_power})"
        
        if move_display_name is None:
            move_display_name = move.name

        custom_data_selection = self._custom_move_data[mon_idx][custom_lookup_key].get(move_name)
        custom_data_options = current_gen_info().get_move_custom_data(move.name, attacking_pkmn=attacking_mon, move=move)
        if custom_data_options is None and const.FLAVOR_MULTI_HIT in move.attack_flavor:
            custom_data_options = const.MULTI_HIT_CUSTOM_DATA
        
        if custom_data_options is None:
            custom_data_selection = None
        elif custom_data_selection not in custom_data_options:
            custom_data_selection = custom_data_options[0]

        # Ensure custom_move_data is always a string (empty string if None)
        custom_move_data_str = custom_data_selection if custom_data_selection is not None else ""

        normal_ranges = current_gen_info().calculate_damage(
            attacking_mon,
            move,
            defending_mon,
            attacking_stage_modifiers=attacking_stage_modifiers,
            defending_stage_modifiers=defending_stage_modifiers,
            attacking_field=attacking_field_status,
            defending_field=defending_field_status,
            custom_move_data=custom_move_data_str,
            weather=self._weather,
            is_double_battle=self._double_battle_flag,
            attacking_battle_stats=attacking_mon_stats,
            defending_battle_stats=defending_mon_stats,
        )
        crit_ranges = current_gen_info().calculate_damage(
            crit_mon,
            move,
            defending_mon,
            attacking_stage_modifiers=attacking_stage_modifiers,
            defending_stage_modifiers=defending_stage_modifiers,
            attacking_field=attacking_field_status,
            defending_field=defending_field_status,
            custom_move_data=custom_move_data_str,
            is_crit=True,
            weather=self._weather,
            is_double_battle=self._double_battle_flag,
            attacking_battle_stats=crit_mon_stats,
            defending_battle_stats=defending_mon_stats,
        )
        if normal_ranges is not None and crit_ranges is not None:
            if config.do_ignore_accuracy():
                accuracy = 100
            else:
                accuracy = current_gen_info().get_move_accuracy(
                    attacking_mon, move, custom_data_selection, defending_mon, self._weather,
                    attacking_stage_modifiers=attacking_stage_modifiers,
                    defending_stage_modifiers=defending_stage_modifiers
                )
                if accuracy is None:
                    accuracy = 100

            accuracy = float(accuracy) / 100.0

            kill_ranges = find_kill(
                normal_ranges,
                crit_ranges,
                current_gen_info().get_crit_rate(attacking_mon, move, custom_data_selection),
                accuracy,
                defending_mon.cur_stats.hp,
                attack_depth=config.get_damage_search_depth(),
                force_full_search=config.do_force_full_search()
            )
        else:
            kill_ranges = []

        # Get stat stage dropdown options for this move
        stat_stage_options = current_gen_info().move_db().get_stat_stage_dropdown_options(move.name)
        stat_stage_info = current_gen_info().move_db().get_stat_stage_info(move.name)
        
        # Get the current stat stage selection for this move
        stat_stage_selection = "0"
        if stat_stage_options is not None:
            stat_stage_selection = self._get_stat_stage_selection(mon_idx, is_player_mon, move.name)
        
        return MoveRenderInfo(
            move_display_name,
            move.attack_flavor,
            normal_ranges,
            crit_ranges,
            defending_mon.cur_stats.hp,
            kill_ranges,
            self._mimic_selection,
            self._mimic_options,
            custom_data_options,
            custom_data_selection,
            stat_stage_options=stat_stage_options,
            stat_stage_selection=stat_stage_selection,
            stat_stage_info=stat_stage_info,
        )

    def load_from_event(self, event_group:EventGroup):
        if event_group is None or event_group.event_definition is None or event_group.event_definition.trainer_def is None:
            self.load_empty()
            return

        self._event_group_id = event_group.group_id
        trainer_def = event_group.event_definition.trainer_def
        trainer_obj = event_group.event_definition.get_first_trainer_obj()
        second_trainer_obj = event_group.event_definition.get_second_trainer_obj()

        self._trainer_name = trainer_def.trainer_name
        self._second_trainer_name = trainer_def.second_trainer_name
        self._weather = trainer_def.weather
        self._double_battle_flag = trainer_obj.double_battle or second_trainer_obj is not None
        self._mimic_selection = trainer_def.mimic_selection
        self._is_player_transformed = trainer_def.transformed
        self._player_setup_move_list = trainer_def.setup_moves.copy()
        self._player_stage_modifier = self._calc_stage_modifier(self._player_setup_move_list)
        self._player_field_status = self._calc_field_status(self._player_setup_move_list)
        self._enemy_setup_move_list = trainer_def.enemy_setup_moves.copy()
        self._enemy_stage_modifier = self._calc_stage_modifier(self._enemy_setup_move_list)
        self._enemy_field_status = self._calc_field_status(self._enemy_setup_move_list)
        self._cached_definition_order = [x.mon_order - 1 for x in event_group.event_definition.get_pokemon_list(definition_order=True)]
        if not trainer_def.custom_move_data:
            self._custom_move_data = []
            for _ in range(len(event_group.event_definition.get_pokemon_list())):
                self._custom_move_data.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
        else:
            self._custom_move_data = [copy.deepcopy(x.custom_move_data) for x in event_group.event_definition.get_pokemon_list()]

        # Load move highlights (backward compatible - handle missing data)
        # move_highlights are stored in definition order (mon_order), need to convert to display order
        if trainer_def.move_highlights:
            # _cached_definition_order maps: display_idx -> definition_idx
            # So for each display index, we need to get the definition index and use that to index into move_highlights
            self._move_highlights = []
            num_pokemon = len(event_group.event_definition.get_pokemon_list())
            for display_idx in range(num_pokemon):
                if display_idx < len(self._cached_definition_order):
                    def_idx = self._cached_definition_order[display_idx]
                    if def_idx < len(trainer_def.move_highlights):
                        self._move_highlights.append(copy.deepcopy(trainer_def.move_highlights[def_idx]))
                    else:
                        self._move_highlights.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
                else:
                    self._move_highlights.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
        else:
            self._move_highlights = []
            for _ in range(len(event_group.event_definition.get_pokemon_list())):
                self._move_highlights.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})

        # Load per-move stat stage setup (backward compatible - handle missing data)
        if trainer_def.stat_stage_setup:
            self._stat_stage_setup = []
            num_pokemon = len(event_group.event_definition.get_pokemon_list())
            for display_idx in range(num_pokemon):
                if display_idx < len(self._cached_definition_order):
                    def_idx = self._cached_definition_order[display_idx]
                    if def_idx < len(trainer_def.stat_stage_setup):
                        self._stat_stage_setup.append(copy.deepcopy(trainer_def.stat_stage_setup[def_idx]))
                    else:
                        self._stat_stage_setup.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
                else:
                    self._stat_stage_setup.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
        else:
            self._stat_stage_setup = []
            for _ in range(len(event_group.event_definition.get_pokemon_list())):
                self._stat_stage_setup.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})

        self._original_player_mon_list = []
        self._transformed_mon_list = []
        self._original_enemy_mon_list = []

        # NOTE: kind of weird, but basically we want to iterate over all the pokemon we want to fight, and then get the appropriate
        # event item for fighting that pokemon. This allows us to pull learned moves/levelups/etc automatically
        cur_item_idx = 0
        for cur_pkmn in event_group.event_definition.get_pokemon_list():
            while cur_item_idx < len(event_group.event_items):
                cur_event_item = event_group.event_items[cur_item_idx]
                cur_item_idx += 1
                # skip level-up events mid-fight
                if cur_event_item.event_definition.trainer_def is None:
                    continue

                if cur_event_item.to_defeat_mon == cur_pkmn:
                    self._original_enemy_mon_list.append(cur_pkmn)
                    self._transformed_mon_list.append(
                        copy.deepcopy(event_group.event_definition.get_pokemon_list()[0])
                    )
                    self._original_player_mon_list.append(
                        cur_event_item.init_state.solo_pkmn.get_pkmn_obj(
                            cur_event_item.init_state.badges,
                            stage_modifiers=self._player_stage_modifier
                        )
                    )
                    self._transformed_mon_list[-1].level = self._original_player_mon_list[-1].level
                    self._transformed_mon_list[-1].cur_stats.hp = self._original_player_mon_list[-1].cur_stats.hp
                    break

        self._full_refresh(is_load=True)

    def load_from_state(self, init_state:RouteState, enemy_mons:List[EnemyPkmn], trainer_name:str=None):
        if init_state is None or not enemy_mons:
            self.load_empty()
            return

        self._event_group_id = None
        self._trainer_name = trainer_name
        self._second_trainer_name = ""
        self._weather = const.WEATHER_NONE
        self._mimic_selection = ""
        self._is_player_transformed = False
        self._player_setup_move_list = []
        self._enemy_setup_move_list = []
        self._custom_move_data = []
        self._move_highlights = []
        self._stat_stage_setup = []
        self._cached_definition_order = list(range(len(enemy_mons)))
        for _ in range(len(enemy_mons)):
            self._custom_move_data.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
            self._move_highlights.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
            self._stat_stage_setup.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})

        self._original_player_mon_list = []
        self._original_enemy_mon_list = []

        cur_state = init_state
        for cur_enemy in enemy_mons:
            self._original_enemy_mon_list.append(cur_enemy)
            self._transformed_mon_list.append(
                copy.deepcopy(enemy_mons[0])
            )
            self._original_player_mon_list.append(cur_state.solo_pkmn.get_pkmn_obj(cur_state.badges))
            self._transformed_mon_list[-1].level = self._original_player_mon_list[-1].level
            self._transformed_mon_list[-1].cur_stats.hp = self._original_player_mon_list[-1].cur_stats.hp

            cur_state = cur_state.defeat_pkmn(cur_enemy)[0]
        
        trainer_obj = current_gen_info().trainer_db().get_trainer(trainer_name)
        if trainer_obj is None:
            self._double_battle_flag = False
        else:
            self._double_battle_flag = trainer_obj.double_battle

        self._full_refresh(is_load=True)
    
    def load_empty(self):
        self._event_group_id = None
        self._trainer_name = ""
        self._second_trainer_name = ""
        self._weather = const.WEATHER_NONE
        self._double_battle_flag = False
        self._mimic_selection = ""
        self._is_player_transformed = False
        self._player_setup_move_list = []
        self._enemy_setup_move_list = []
        self._custom_move_data = []
        self._move_highlights = []
        self._stat_stage_setup = []
        self._show_move_highlights = False
        self._original_player_mon_list = []
        self._transformed_mon_list = []
        self._original_enemy_mon_list = []
        self._cached_definition_order = []
        self._full_refresh(is_load=True)

    ######
    # Methods that do not induce a state change
    ######

    def get_partial_trainer_definition(self) -> TrainerEventDefinition:
        if not self._trainer_name:
            return None

        is_custom_move_data_present = False
        for cur_test in self._custom_move_data:
            if len(cur_test[const.PLAYER_KEY]) > 0 or len(cur_test[const.ENEMY_KEY]) > 0:
                is_custom_move_data_present = True
                break
        
        if is_custom_move_data_present:
            final_custom_move_data = [self._custom_move_data[x] for x in self._cached_definition_order]
        else:
            final_custom_move_data = None

        # NOTE: somewhat gross, but we are intentionally ignoring the extra fields here (exp_split, mon_order, etc)
        # Instead, we expect that the place this is called is smart enough to fill in the extra pieces that we don't
        # have full info to replicate here
        # Check if move highlights are present
        is_move_highlights_present = False
        if self._move_highlights:
            for cur_test in self._move_highlights:
                if len(cur_test.get(const.PLAYER_KEY, {})) > 0 or len(cur_test.get(const.ENEMY_KEY, {})) > 0:
                    # Check if any highlight state is non-zero
                    for key in [const.PLAYER_KEY, const.ENEMY_KEY]:
                        for move_name, state in cur_test.get(key, {}).items():
                            if state != 0:
                                is_move_highlights_present = True
                                break
                        if is_move_highlights_present:
                            break
                    if is_move_highlights_present:
                        break
        
        if is_move_highlights_present:
            # Convert from display order back to definition order
            # _cached_definition_order maps display_idx -> definition_idx
            # We need to create definition_idx -> display_idx mapping
            def_to_display = {}
            for display_idx, def_idx in enumerate(self._cached_definition_order):
                def_to_display[def_idx] = display_idx
            
            # Now reorder: for each definition index, get the corresponding display index data
            final_move_highlights = []
            for def_idx in sorted(def_to_display.keys()):
                display_idx = def_to_display[def_idx]
                if display_idx < len(self._move_highlights):
                    final_move_highlights.append(copy.deepcopy(self._move_highlights[display_idx]))
                else:
                    final_move_highlights.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
        else:
            final_move_highlights = None

        # Check if stat_stage_setup is present
        is_stat_stage_setup_present = False
        if self._stat_stage_setup:
            for cur_test in self._stat_stage_setup:
                if len(cur_test.get(const.PLAYER_KEY, {})) > 0 or len(cur_test.get(const.ENEMY_KEY, {})) > 0:
                    for key in [const.PLAYER_KEY, const.ENEMY_KEY]:
                        for move_name, stage in cur_test.get(key, {}).items():
                            if stage != "0":
                                is_stat_stage_setup_present = True
                                break
                        if is_stat_stage_setup_present:
                            break
                    if is_stat_stage_setup_present:
                        break
        
        if is_stat_stage_setup_present:
            # Convert from display order back to definition order
            def_to_display = {}
            for display_idx, def_idx in enumerate(self._cached_definition_order):
                def_to_display[def_idx] = display_idx
            
            final_stat_stage_setup = []
            for def_idx in sorted(def_to_display.keys()):
                display_idx = def_to_display[def_idx]
                if display_idx < len(self._stat_stage_setup):
                    final_stat_stage_setup.append(copy.deepcopy(self._stat_stage_setup[display_idx]))
                else:
                    final_stat_stage_setup.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
        else:
            final_stat_stage_setup = None

        return TrainerEventDefinition(
            self._trainer_name,
            second_trainer_name=self._second_trainer_name,
            setup_moves=self._player_setup_move_list,
            enemy_setup_moves=self._enemy_setup_move_list,
            mimic_selection=self._mimic_selection,
            custom_move_data=final_custom_move_data,
            weather=self._weather,
            transformed=self._is_player_transformed,
            move_highlights=final_move_highlights,
            stat_stage_setup=final_stat_stage_setup,
        )

    def get_pkmn_info(self, pkmn_idx, is_player_mon) -> PkmnRenderInfo:
        if is_player_mon:
            cur_data = self._player_pkmn_matchup_data
        else:
            cur_data = self._enemy_pkmn_matchup_data
        
        if pkmn_idx < 0 or pkmn_idx >= len(cur_data):
            return None
        
        return cur_data[pkmn_idx]

    def get_move_info(self, pkmn_idx, move_idx, is_player_mon) -> MoveRenderInfo:
        if is_player_mon:
            cur_move_data = self._player_move_data
        else:
            cur_move_data = self._enemy_move_data

        if pkmn_idx < 0 or pkmn_idx >= len(cur_move_data) or move_idx < 0:
            return None
        
        # For player moves, allow indices 0-7 when test moves are enabled (0-3 regular, 4-7 test)
        # For enemy moves, only allow 0-3
        if is_player_mon:
            max_idx = 7 if config.get_test_moves_enabled() else 3
        else:
            max_idx = 3
        
        if move_idx > max_idx:
            return None
        
        # Check if the inner list has enough elements
        if move_idx >= len(cur_move_data[pkmn_idx]):
            return None
        
        return cur_move_data[pkmn_idx][move_idx]

    def get_weather(self) -> str:
        return self._weather

    def get_player_setup_moves(self) -> List[str]:
        return self._player_setup_move_list

    def get_enemy_setup_moves(self) -> List[str]:
        return self._enemy_setup_move_list

    def is_double_battle(self) -> bool:
        return self._double_battle_flag

    def is_player_transformed(self) -> bool:
        return self._is_player_transformed
    
    def get_test_moves_enabled(self) -> bool:
        return config.get_test_moves_enabled()
    
    def get_test_moves(self) -> List[str]:
        test_moves = self._main_controller.get_raw_route().test_moves.copy()
        # Ensure we always have exactly 4 slots, defaulting to empty strings
        while len(test_moves) < 4:
            test_moves.append("")
        return test_moves[:4]
    
    def update_test_move(self, slot_idx:int, move_name:str):
        """Update a test move slot (0-3) with a new move name"""
        if slot_idx < 0 or slot_idx >= 4:
            return
        
        # Get current test moves from router
        test_moves = self._main_controller.get_raw_route().test_moves
        
        # Ensure we have 4 slots
        while len(test_moves) < 4:
            test_moves.append("")
        
        # Update the specific slot
        test_moves[slot_idx] = move_name
        
        # Save back to router (this will trigger save when route is saved)
        self._main_controller.get_raw_route().test_moves = test_moves
        
        # Trigger refresh and mark as changed (so route file gets saved)
        self._full_refresh()
        self._on_nonload_change()

    @staticmethod
    def _is_move_better(new_move:MoveRenderInfo, prev_move:MoveRenderInfo, strat:str, other_mon:PkmnRenderInfo) -> bool:
        if (
            strat is None or
            not isinstance(strat, str) or
            strat == const.HIGHLIGHT_NONE or
            strat not in const.ALL_HIGHLIGHT_STRATS
        ):
            return False

        if new_move is None or new_move.damage_ranges is None:
            return False

        if prev_move is None or prev_move.damage_ranges is None:
            return True

        # special case for recharge moves (e.g. Hyper Beam)
        # If a one-hit is not possible without a crit, always prefer other damage dealing moves
        if const.FLAVOR_RECHARGE in new_move.attack_flavor and new_move.damage_ranges.max_damage < other_mon.defending_mon_hp:
            return False
        elif const.FLAVOR_RECHARGE in prev_move.attack_flavor and prev_move.damage_ranges.max_damage < other_mon.defending_mon_hp:
            return True
        
        new_fastest_kill = 1000000
        new_accuracy = -2
        if len(new_move.kill_ranges) > 0:
            if strat == const.HIGHLIGHT_GUARANTEED_KILL:
                # if the last slot has a -1 % to kill, then that means it was auto-calculated
                if config.do_ignore_accuracy() or new_move.kill_ranges[-1][1] != -1:
                    new_fastest_kill, new_accuracy = new_move.kill_ranges[-1]

            elif strat == const.HIGHLIGHT_FASTEST_KILL:
                new_fastest_kill, new_accuracy = new_move.kill_ranges[0]
            elif strat == const.HIGHLIGHT_CONSISTENT_KILL:
                for test_kill in new_move.kill_ranges:
                    if test_kill[1] >= config.get_consistent_threshold():
                        new_fastest_kill, new_accuracy = test_kill
                        break
        
        prev_fastest_kill = new_fastest_kill + 1
        prev_accuracy = -2
        if len(prev_move.kill_ranges) > 0:
            if strat == const.HIGHLIGHT_GUARANTEED_KILL:
                prev_fastest_kill, prev_accuracy = prev_move.kill_ranges[-1]
            elif strat == const.HIGHLIGHT_FASTEST_KILL:
                prev_fastest_kill, prev_accuracy = prev_move.kill_ranges[0]
            elif strat == const.HIGHLIGHT_CONSISTENT_KILL:
                for test_kill in prev_move.kill_ranges:
                    if test_kill[1] >= config.get_consistent_threshold():
                        prev_fastest_kill, prev_accuracy = test_kill
                        break

        
        # always prefer lower number of turns
        if new_fastest_kill < prev_fastest_kill:
            return True
        elif prev_fastest_kill < new_fastest_kill:
            return False
        
        # if number of turns is tied, then prefer higher accuracy
        # note that accuracy might be "-1" in the case of auto-calculated kills
        # but that's fine, because we want higher accuracy, so it will always "lose", which is desirable
        if new_accuracy > prev_accuracy:
            return True
        elif prev_accuracy > new_accuracy:
            return False

        # if number of turns and accuracy is tied, punish moves that take more than one turn
        if (
            const.FLAVOR_TWO_TURN in prev_move.attack_flavor or
            const.FLAVOR_TWO_TURN_INVULN in prev_move.attack_flavor
        ):
            return True
        elif (
            const.FLAVOR_TWO_TURN in new_move.attack_flavor or
            const.FLAVOR_TWO_TURN_INVULN in new_move.attack_flavor
        ):
            return False
        
        # Only rely on damage for tie breakers
        return new_move.damage_ranges.max_damage > prev_move.damage_ranges.max_damage

    @staticmethod
    def _calc_stage_modifier(move_list) -> StageModifiers:
        result = StageModifiers()

        for cur_move in move_list:
            result = result.apply_stat_mod(current_gen_info().move_db().get_stat_mod(cur_move))
        
        return result

    @staticmethod
    def _calc_field_status(move_list) -> FieldStatus:
        result = FieldStatus()

        for cur_move in move_list:
            if cur_move:  # Only process non-empty moves
                result = result.apply_move(cur_move)
        
        return result
    
    def _calc_per_matchup_stage_modifiers(self) -> Tuple[List[StageModifiers], List[StageModifiers]]:
        """Calculate stage modifiers for each matchup based on per-move stat stage setup.
        
        Returns:
            Tuple of (player_modifiers, enemy_modifiers) where each is a list of StageModifiers,
            one per matchup.
        
        The logic is:
        - Player self-targeting moves: persist for rest of battle
        - Player opponent-targeting moves: apply to current matchup only
        - Enemy self-targeting moves: apply to current matchup only  
        - Enemy opponent-targeting moves: persist for rest of battle (affects player)
        - Damage-dealing moves with stat effects: apply order matters for current matchup
        
        Order: Player stage modifiers are applied before opponent stage modifiers.
        
        Gen 1 specific:
        - Badge boost bug is handled by StageModifiers.apply_stat_mod()
        - Badge boosts reset when the player levels up between matchups
        """
        num_matchups = len(self._original_player_mon_list)
        player_modifiers = []
        enemy_modifiers = []
        
        # Track persistent modifiers that carry across matchups
        # For player: self-targeting moves persist
        # For enemy (affecting player): enemy's opponent-targeting moves persist
        persistent_player_modifier = StageModifiers()
        
        move_db = current_gen_info().move_db()
        gen = current_gen_info().get_generation()
        prev_player_level = None
        
        for mon_idx in range(num_matchups):
            # Gen 1 specific: Check if player leveled up - if so, reset badge boosts
            # Badge boost counters reset when the Pokemon levels up
            if gen == 1 and mon_idx < len(self._original_player_mon_list):
                cur_player_level = self._original_player_mon_list[mon_idx].level
                if prev_player_level is not None and cur_player_level > prev_player_level:
                    # Player leveled up - reset badge boosts but keep stage modifiers
                    persistent_player_modifier = persistent_player_modifier.clear_badge_boosts()
                prev_player_level = cur_player_level
            
            # Start with persistent modifier from previous matchups
            cur_player_modifier = persistent_player_modifier._copy_constructor()
            cur_enemy_modifier = StageModifiers()
            
            # Get stat stage setup for this matchup
            if mon_idx < len(self._stat_stage_setup):
                matchup_setup = self._stat_stage_setup[mon_idx]
            else:
                matchup_setup = {const.PLAYER_KEY: {}, const.ENEMY_KEY: {}}
            
            player_setup = matchup_setup.get(const.PLAYER_KEY, {})
            enemy_setup = matchup_setup.get(const.ENEMY_KEY, {})
            
            # Step 1: Apply player's stage modifiers FIRST (order matters)
            # Player self-targeting moves persist for rest of battle
            if mon_idx < len(self._original_player_mon_list):
                player_mon = self._original_player_mon_list[mon_idx]
                for move_name in player_mon.move_list:
                    if not move_name:
                        continue
                    
                    count_str = player_setup.get(move_name, "0")
                    try:
                        count = int(count_str)
                    except ValueError:
                        count = 0
                    
                    if count <= 0:
                        continue
                    
                    stat_info = move_db.get_stat_stage_info(move_name)
                    if not stat_info.get('has_stat_effect', False):
                        continue
                    
                    is_guaranteed = stat_info.get('is_guaranteed', False)
                    targets_self = stat_info.get('targets_self', True)
                    is_damaging = stat_info.get('is_damaging', False)
                    
                    # Skip non-guaranteed effects for non-damaging moves
                    # But allow damage-dealing moves with chance-based effects (Category 3)
                    if not is_guaranteed and not is_damaging:
                        continue
                    
                    if targets_self and not is_damaging:
                        # Category 1: Self-targeting status move - apply to player, persists
                        stat_mods = move_db.get_stat_mod_for_target(move_name, target_self=True)
                        for _ in range(count):
                            persistent_player_modifier = persistent_player_modifier.apply_stat_mod(stat_mods)
                            cur_player_modifier = cur_player_modifier.apply_stat_mod(stat_mods)
                    elif not targets_self and not is_damaging:
                        # Category 2: Opponent-targeting status move - apply to enemy, current matchup only
                        stat_mods = move_db.get_stat_mod_for_target(move_name, target_self=False)
                        for _ in range(count):
                            cur_enemy_modifier = cur_enemy_modifier.apply_stat_mod(stat_mods)
                    elif is_damaging:
                        # Category 3: Damage-dealing move with stat effect - apply to enemy, current matchup
                        stat_mods = move_db.get_stat_mod_for_target(move_name, target_self=False)
                        for _ in range(count):
                            cur_enemy_modifier = cur_enemy_modifier.apply_stat_mod(stat_mods)
            
            # Step 2: Apply enemy's stage modifiers AFTER player's
            if mon_idx < len(self._original_enemy_mon_list):
                enemy_mon = self._original_enemy_mon_list[mon_idx]
                for move_name in enemy_mon.move_list:
                    if not move_name:
                        continue
                    
                    count_str = enemy_setup.get(move_name, "0")
                    try:
                        count = int(count_str)
                    except ValueError:
                        count = 0
                    
                    if count <= 0:
                        continue
                    
                    stat_info = move_db.get_stat_stage_info(move_name)
                    if not stat_info.get('has_stat_effect', False):
                        continue
                    
                    is_guaranteed = stat_info.get('is_guaranteed', False)
                    targets_self = stat_info.get('targets_self', True)
                    is_damaging = stat_info.get('is_damaging', False)
                    
                    # Skip non-guaranteed effects for non-damaging moves
                    # But allow damage-dealing moves with chance-based effects
                    if not is_guaranteed and not is_damaging:
                        continue
                    
                    if targets_self and not is_damaging:
                        # Enemy self-targeting: only affects current matchup
                        stat_mods = move_db.get_stat_mod_for_target(move_name, target_self=True)
                        for _ in range(count):
                            cur_enemy_modifier = cur_enemy_modifier.apply_stat_mod(stat_mods)
                    elif not targets_self and not is_damaging:
                        # Enemy opponent-targeting: affects player, persists for rest of battle
                        stat_mods = move_db.get_stat_mod_for_target(move_name, target_self=False)
                        for _ in range(count):
                            persistent_player_modifier = persistent_player_modifier.apply_stat_mod(stat_mods)
                            cur_player_modifier = cur_player_modifier.apply_stat_mod(stat_mods)
                    elif is_damaging:
                        # Enemy damage-dealing with stat effect: affects player, persists
                        stat_mods = move_db.get_stat_mod_for_target(move_name, target_self=False)
                        for _ in range(count):
                            persistent_player_modifier = persistent_player_modifier.apply_stat_mod(stat_mods)
                            cur_player_modifier = cur_player_modifier.apply_stat_mod(stat_mods)
            
            player_modifiers.append(cur_player_modifier)
            enemy_modifiers.append(cur_enemy_modifier)
        
        return player_modifiers, enemy_modifiers
    
    def can_support_prefight_candies(self):
        return self._event_group_id is not None
    
    def get_prefight_candy_count(self):
        if self._event_group_id is None:
            return 0
        
        prev_event = self._main_controller.get_previous_event(self._event_group_id, enabled_only=True)
        if prev_event is None or prev_event.event_definition is None or prev_event.event_definition.rare_candy is None:
            return 0
        
        return prev_event.event_definition.rare_candy.amount
    
    def take_screenshot(self, bbox, suffix=""):
        if not self._trainer_name:
            self._main_controller.send_message(f"No active battle to screenshot")
            return
        
        image_name = self._trainer_name.replace(" ", "_")
        if suffix:
            image_name = f"{image_name}{suffix}"
        
        self._main_controller.take_screenshot(
            image_name,
            bbox
        )
        
