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
        # Track per-move setup usage: _move_setup_usage[mon_idx][is_player_key][move_idx] = count
        # where is_player_key is const.PLAYER_KEY or const.ENEMY_KEY
        self._move_setup_usage:List[Dict[str, Dict[int, int]]] = []
        # Track which Pokemon have leveled up during battle (for Gen 1 badge boost clearing)
        # _leveled_up_pokemon[mon_idx] = True if Pokemon at mon_idx leveled up
        self._leveled_up_pokemon:List[bool] = []
        self._cached_definition_order = []
        self._weather = None
        self._double_battle_flag = False

        # NOTE: all of the state above this comment is considered the "true" state
        # The below state is all calculated based on values from the above state
        self._player_stage_modifier:StageModifiers = None
        self._enemy_stage_modifier:StageModifiers = None
        self._player_field_status:FieldStatus = None
        self._enemy_field_status:FieldStatus = None

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
                self._player_move_data[mon_idx][cur_mimic_idx] = self._recalculate_single_move(mon_idx, True, move_name, move_display_name=const.MIMIC_MOVE_NAME, move_idx=cur_mimic_idx)
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

            move_data[pkmn_idx][move_idx] = self._recalculate_single_move(pkmn_idx, is_player_mon, move_name, move_idx=move_idx)
            self._update_best_move_inplace(pkmn_idx, is_player_mon)

            self._on_refresh()
            self._on_nonload_change()
        except Exception as e:
            logger.error(f"encountered error updating custom move data: {pkmn_idx, move_idx, is_player_mon, new_value}")

    def update_weather(self, new_weather):
        self._weather = new_weather
        self._full_refresh()

    def update_enemy_setup_moves(self, new_setup_moves):
        self._enemy_setup_move_list = new_setup_moves
        self._full_refresh()

    def update_player_setup_moves(self, new_setup_moves):
        self._player_setup_move_list = new_setup_moves
        self._full_refresh()

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
        self._player_stage_modifier = self._calc_stage_modifier(self._player_setup_move_list)
        self._player_field_status = self._calc_field_status(self._player_setup_move_list)
        self._enemy_stage_modifier = self._calc_stage_modifier(self._enemy_setup_move_list)
        self._enemy_field_status = self._calc_field_status(self._enemy_setup_move_list)
        self._player_pkmn_matchup_data = []
        self._enemy_pkmn_matchup_data = []
        self._player_move_data = []
        self._enemy_move_data = []
        self._mimic_options = []

        can_mimic_yet = False
        for mon_idx in range(len(self._original_player_mon_list)):
            if self._is_player_transformed:
                player_mon = self._transformed_mon_list[mon_idx]
                player_stats = player_mon.cur_stats
            else:
                player_mon = self._original_player_mon_list[mon_idx]
                # Calculate accumulated setup modifiers for this Pokemon (all moves)
                # Use a high move_idx to include all moves for this Pokemon
                accumulated_modifiers = self._calc_accumulated_setup_modifiers(mon_idx, 4, True)
                # Combine with base modifier for speed calculation
                if current_gen_info().get_generation() == 1:
                    # Check if the solo Pokemon leveled up before or during this Pokemon's battle
                    level_up_mon_idx = None
                    for idx in range(len(self._leveled_up_pokemon)):
                        if self._leveled_up_pokemon[idx]:
                            level_up_mon_idx = idx
                            break
                    
                    # If a level-up occurred, clear badge boosts from base_modifier for Pokemon AFTER the level-up
                    # (level-up removes all erroneously applied badge boosts)
                    # Note: Badge boosts still apply to the Pokemon where the level-up occurs
                    base_attack_bb = 0
                    base_defense_bb = 0
                    base_speed_bb = 0
                    base_special_bb = 0
                    if level_up_mon_idx is None or mon_idx <= level_up_mon_idx:
                        # No level-up yet, or calculating for Pokemon up to and including the level-up - preserve base badge boosts
                        base_attack_bb = self._player_stage_modifier.attack_badge_boosts
                        base_defense_bb = self._player_stage_modifier.defense_badge_boosts
                        base_speed_bb = self._player_stage_modifier.speed_badge_boosts
                        base_special_bb = self._player_stage_modifier.special_badge_boosts
                    
                    # For Gen 1, combine badge boosts
                    combined_modifier = StageModifiers(
                        attack=max(min(self._player_stage_modifier.attack_stage + accumulated_modifiers.attack_stage, 6), -6),
                        defense=max(min(self._player_stage_modifier.defense_stage + accumulated_modifiers.defense_stage, 6), -6),
                        speed=max(min(self._player_stage_modifier.speed_stage + accumulated_modifiers.speed_stage, 6), -6),
                        special_attack=max(min(self._player_stage_modifier.special_attack_stage + accumulated_modifiers.special_attack_stage, 6), -6),
                        special_defense=max(min(self._player_stage_modifier.special_defense_stage + accumulated_modifiers.special_defense_stage, 6), -6),
                        accuracy=max(min(self._player_stage_modifier.accuracy_stage + accumulated_modifiers.accuracy_stage, 6), -6),
                        evasion=max(min(self._player_stage_modifier.evasion_stage + accumulated_modifiers.evasion_stage, 6), -6),
                        attack_bb=base_attack_bb + accumulated_modifiers.attack_badge_boosts,
                        defense_bb=base_defense_bb + accumulated_modifiers.defense_badge_boosts,
                        speed_bb=base_speed_bb + accumulated_modifiers.speed_badge_boosts,
                        special_bb=base_special_bb + accumulated_modifiers.special_badge_boosts,
                    )
                else:
                    # For non-Gen 1, combine normally
                    combined_modifier = self._player_stage_modifier
                    if accumulated_modifiers.attack_stage != 0:
                        combined_modifier = combined_modifier.apply_stat_mod([(const.ATK, accumulated_modifiers.attack_stage)])
                    if accumulated_modifiers.defense_stage != 0:
                        combined_modifier = combined_modifier.apply_stat_mod([(const.DEF, accumulated_modifiers.defense_stage)])
                    if accumulated_modifiers.speed_stage != 0:
                        combined_modifier = combined_modifier.apply_stat_mod([(const.SPE, accumulated_modifiers.speed_stage)])
                    if accumulated_modifiers.special_attack_stage != 0:
                        combined_modifier = combined_modifier.apply_stat_mod([(const.SPA, accumulated_modifiers.special_attack_stage)])
                    if accumulated_modifiers.special_defense_stage != 0:
                        combined_modifier = combined_modifier.apply_stat_mod([(const.SPD, accumulated_modifiers.special_defense_stage)])
                    if accumulated_modifiers.accuracy_stage != 0:
                        combined_modifier = combined_modifier.apply_stat_mod([(const.ACC, accumulated_modifiers.accuracy_stage)])
                    if accumulated_modifiers.evasion_stage != 0:
                        combined_modifier = combined_modifier.apply_stat_mod([(const.EV, accumulated_modifiers.evasion_stage)])
                player_stats = player_mon.get_battle_stats(combined_modifier)

            # Calculate accumulated setup modifiers for enemy Pokemon (all moves)
            enemy_mon = self._original_enemy_mon_list[mon_idx]
            accumulated_enemy_modifiers = self._calc_accumulated_setup_modifiers(mon_idx, 4, False)
            # Combine with base modifier for speed calculation
            if current_gen_info().get_generation() == 1:
                # For Gen 1, combine badge boosts
                combined_enemy_modifier = StageModifiers(
                    attack=max(min(self._enemy_stage_modifier.attack_stage + accumulated_enemy_modifiers.attack_stage, 6), -6),
                    defense=max(min(self._enemy_stage_modifier.defense_stage + accumulated_enemy_modifiers.defense_stage, 6), -6),
                    speed=max(min(self._enemy_stage_modifier.speed_stage + accumulated_enemy_modifiers.speed_stage, 6), -6),
                    special_attack=max(min(self._enemy_stage_modifier.special_attack_stage + accumulated_enemy_modifiers.special_attack_stage, 6), -6),
                    special_defense=max(min(self._enemy_stage_modifier.special_defense_stage + accumulated_enemy_modifiers.special_defense_stage, 6), -6),
                    accuracy=max(min(self._enemy_stage_modifier.accuracy_stage + accumulated_enemy_modifiers.accuracy_stage, 6), -6),
                    evasion=max(min(self._enemy_stage_modifier.evasion_stage + accumulated_enemy_modifiers.evasion_stage, 6), -6),
                    attack_bb=self._enemy_stage_modifier.attack_badge_boosts + accumulated_enemy_modifiers.attack_badge_boosts,
                    defense_bb=self._enemy_stage_modifier.defense_badge_boosts + accumulated_enemy_modifiers.defense_badge_boosts,
                    speed_bb=self._enemy_stage_modifier.speed_badge_boosts + accumulated_enemy_modifiers.speed_badge_boosts,
                    special_bb=self._enemy_stage_modifier.special_badge_boosts + accumulated_enemy_modifiers.special_badge_boosts,
                )
            else:
                # For non-Gen 1, combine normally
                combined_enemy_modifier = self._enemy_stage_modifier
                if accumulated_enemy_modifiers.attack_stage != 0:
                    combined_enemy_modifier = combined_enemy_modifier.apply_stat_mod([(const.ATK, accumulated_enemy_modifiers.attack_stage)])
                if accumulated_enemy_modifiers.defense_stage != 0:
                    combined_enemy_modifier = combined_enemy_modifier.apply_stat_mod([(const.DEF, accumulated_enemy_modifiers.defense_stage)])
                if accumulated_enemy_modifiers.speed_stage != 0:
                    combined_enemy_modifier = combined_enemy_modifier.apply_stat_mod([(const.SPE, accumulated_enemy_modifiers.speed_stage)])
                if accumulated_enemy_modifiers.special_attack_stage != 0:
                    combined_enemy_modifier = combined_enemy_modifier.apply_stat_mod([(const.SPA, accumulated_enemy_modifiers.special_attack_stage)])
                if accumulated_enemy_modifiers.special_defense_stage != 0:
                    combined_enemy_modifier = combined_enemy_modifier.apply_stat_mod([(const.SPD, accumulated_enemy_modifiers.special_defense_stage)])
                if accumulated_enemy_modifiers.accuracy_stage != 0:
                    combined_enemy_modifier = combined_enemy_modifier.apply_stat_mod([(const.ACC, accumulated_enemy_modifiers.accuracy_stage)])
                if accumulated_enemy_modifiers.evasion_stage != 0:
                    combined_enemy_modifier = combined_enemy_modifier.apply_stat_mod([(const.EV, accumulated_enemy_modifiers.evasion_stage)])
            enemy_stats = enemy_mon.get_battle_stats(combined_enemy_modifier)

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
                    
                    cur_player_move_data = self._recalculate_single_move(mon_idx, True, move_name, move_display_name=move_display_name, move_idx=move_idx)
                else:
                    cur_player_move_data = None

                # Now handle the enemy move calculation
                if move_idx < len(enemy_mon.move_list):
                    move_name = enemy_mon.move_list[move_idx]
                    if move_name and move_name not in self._mimic_options:
                        self._mimic_options.append(move_name)
                    cur_enemy_move_data = self._recalculate_single_move(mon_idx, False, move_name, move_idx=move_idx)
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
        move_idx:int=None,
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
                        attacking_mon_stats = attacking_mon.get_battle_stats(StageModifiers())
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
                        attacking_mon_stats = self._original_player_mon_list[mon_idx].get_battle_stats(self._player_stage_modifier)
                        crit_mon_stats = self._original_player_mon_list[mon_idx].get_battle_stats(self._player_stage_modifier, is_crit=True)
                else:
                    crit_mon = attacking_mon
                    crit_mon_stats = attacking_mon_stats
            else:
                attacking_mon = self._original_player_mon_list[mon_idx]
                attacking_mon_stats = None
                crit_mon = attacking_mon
                crit_mon_stats = attacking_mon_stats
            
            # Calculate accumulated setup modifiers for the attacking player Pokemon
            # Include: 1) player's own self-targeting modifiers (like Swords Dance), 2) enemy's enemy-targeting modifiers (like Charm)
            player_self_modifiers = self._calc_accumulated_setup_modifiers(mon_idx, move_idx if move_idx is not None else 0, True, target_self=True)
            enemy_enemy_modifiers = self._calc_accumulated_setup_modifiers(mon_idx, move_idx if move_idx is not None else 0, False, target_self=False)
            # Combine both sources of modifiers
            if current_gen_info().get_generation() == 1:
                accumulated_modifiers = StageModifiers(
                    attack=max(min(player_self_modifiers.attack_stage + enemy_enemy_modifiers.attack_stage, 6), -6),
                    defense=max(min(player_self_modifiers.defense_stage + enemy_enemy_modifiers.defense_stage, 6), -6),
                    speed=max(min(player_self_modifiers.speed_stage + enemy_enemy_modifiers.speed_stage, 6), -6),
                    special_attack=max(min(player_self_modifiers.special_attack_stage + enemy_enemy_modifiers.special_attack_stage, 6), -6),
                    special_defense=max(min(player_self_modifiers.special_defense_stage + enemy_enemy_modifiers.special_defense_stage, 6), -6),
                    accuracy=max(min(player_self_modifiers.accuracy_stage + enemy_enemy_modifiers.accuracy_stage, 6), -6),
                    evasion=max(min(player_self_modifiers.evasion_stage + enemy_enemy_modifiers.evasion_stage, 6), -6),
                    attack_bb=player_self_modifiers.attack_badge_boosts + enemy_enemy_modifiers.attack_badge_boosts,
                    defense_bb=player_self_modifiers.defense_badge_boosts + enemy_enemy_modifiers.defense_badge_boosts,
                    speed_bb=player_self_modifiers.speed_badge_boosts + enemy_enemy_modifiers.speed_badge_boosts,
                    special_bb=player_self_modifiers.special_badge_boosts + enemy_enemy_modifiers.special_badge_boosts,
                )
            else:
                accumulated_modifiers = player_self_modifiers
                if enemy_enemy_modifiers.attack_stage != 0:
                    accumulated_modifiers = accumulated_modifiers.apply_stat_mod([(const.ATK, enemy_enemy_modifiers.attack_stage)])
                if enemy_enemy_modifiers.defense_stage != 0:
                    accumulated_modifiers = accumulated_modifiers.apply_stat_mod([(const.DEF, enemy_enemy_modifiers.defense_stage)])
                if enemy_enemy_modifiers.speed_stage != 0:
                    accumulated_modifiers = accumulated_modifiers.apply_stat_mod([(const.SPE, enemy_enemy_modifiers.speed_stage)])
                if enemy_enemy_modifiers.special_attack_stage != 0:
                    accumulated_modifiers = accumulated_modifiers.apply_stat_mod([(const.SPA, enemy_enemy_modifiers.special_attack_stage)])
                if enemy_enemy_modifiers.special_defense_stage != 0:
                    accumulated_modifiers = accumulated_modifiers.apply_stat_mod([(const.SPD, enemy_enemy_modifiers.special_defense_stage)])
                if enemy_enemy_modifiers.accuracy_stage != 0:
                    accumulated_modifiers = accumulated_modifiers.apply_stat_mod([(const.ACC, enemy_enemy_modifiers.accuracy_stage)])
                if enemy_enemy_modifiers.evasion_stage != 0:
                    accumulated_modifiers = accumulated_modifiers.apply_stat_mod([(const.EV, enemy_enemy_modifiers.evasion_stage)])
            # Combine base stage modifier with accumulated modifiers
            base_modifier = self._player_stage_modifier
            # For Gen 1, we need to preserve badge boosts from accumulated modifiers
            # For other gens, just combine stage modifiers
            if current_gen_info().get_generation() == 1:
                # If global setup is active, always preserve badge boosts from base_modifier
                # (level-up clearing only applies when using per-move setup)
                if self._player_setup_move_list:
                    # Global setup is active - preserve all badge boosts
                    base_attack_bb = base_modifier.attack_badge_boosts
                    base_defense_bb = base_modifier.defense_badge_boosts
                    base_speed_bb = base_modifier.speed_badge_boosts
                    base_special_bb = base_modifier.special_badge_boosts
                else:
                    # Per-move setup is active - apply level-up clearing logic
                    # Check if the solo Pokemon leveled up before or during this Pokemon's battle
                    level_up_mon_idx = None
                    for idx in range(len(self._leveled_up_pokemon)):
                        if self._leveled_up_pokemon[idx]:
                            level_up_mon_idx = idx
                            break
                    
                    # If a level-up occurred, clear badge boosts from base_modifier for Pokemon AFTER the level-up
                    # (level-up removes all erroneously applied badge boosts)
                    # Note: Badge boosts still apply to the Pokemon where the level-up occurs
                    base_attack_bb = 0
                    base_defense_bb = 0
                    base_speed_bb = 0
                    base_special_bb = 0
                    if level_up_mon_idx is None or mon_idx <= level_up_mon_idx:
                        # No level-up yet, or calculating for Pokemon up to and including the level-up - preserve base badge boosts
                        base_attack_bb = base_modifier.attack_badge_boosts
                        base_defense_bb = base_modifier.defense_badge_boosts
                        base_speed_bb = base_modifier.speed_badge_boosts
                        base_special_bb = base_modifier.special_badge_boosts
                
                # Combine stage modifiers and badge boosts separately
                # Use the same logic for both global and per-move setup
                # When using global setup, base_modifier has the modifiers and badge boosts
                # When using per-move setup, base_modifier should be empty (all zeros) and accumulated_modifiers has everything
                # The level-up clearing logic (above) handles resetting base_modifier badge boosts for per-move setup
                # We always combine them the same way - when per-move setup is active, base_modifier is empty so it adds nothing
                attacking_stage_modifiers = StageModifiers(
                    attack=max(min(base_modifier.attack_stage + accumulated_modifiers.attack_stage, 6), -6),
                    defense=max(min(base_modifier.defense_stage + accumulated_modifiers.defense_stage, 6), -6),
                    speed=max(min(base_modifier.speed_stage + accumulated_modifiers.speed_stage, 6), -6),
                    special_attack=max(min(base_modifier.special_attack_stage + accumulated_modifiers.special_attack_stage, 6), -6),
                    special_defense=max(min(base_modifier.special_defense_stage + accumulated_modifiers.special_defense_stage, 6), -6),
                    accuracy=max(min(base_modifier.accuracy_stage + accumulated_modifiers.accuracy_stage, 6), -6),
                    evasion=max(min(base_modifier.evasion_stage + accumulated_modifiers.evasion_stage, 6), -6),
                    attack_bb=base_attack_bb + accumulated_modifiers.attack_badge_boosts,
                    defense_bb=base_defense_bb + accumulated_modifiers.defense_badge_boosts,
                    speed_bb=base_speed_bb + accumulated_modifiers.speed_badge_boosts,
                    special_bb=base_special_bb + accumulated_modifiers.special_badge_boosts,
                )
            else:
                # For non-Gen 1, combine stage modifiers normally
                attacking_stage_modifiers = base_modifier
                if accumulated_modifiers.attack_stage != 0:
                    attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.ATK, accumulated_modifiers.attack_stage)])
                if accumulated_modifiers.defense_stage != 0:
                    attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.DEF, accumulated_modifiers.defense_stage)])
                if accumulated_modifiers.speed_stage != 0:
                    attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.SPE, accumulated_modifiers.speed_stage)])
                if accumulated_modifiers.special_attack_stage != 0:
                    attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.SPA, accumulated_modifiers.special_attack_stage)])
                if accumulated_modifiers.special_defense_stage != 0:
                    attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.SPD, accumulated_modifiers.special_defense_stage)])
                if accumulated_modifiers.accuracy_stage != 0:
                    attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.ACC, accumulated_modifiers.accuracy_stage)])
                if accumulated_modifiers.evasion_stage != 0:
                    attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.EV, accumulated_modifiers.evasion_stage)])
            attacking_field_status = self._player_field_status
            defending_mon = self._original_enemy_mon_list[mon_idx]
            defending_mon_stats = None
            # Calculate accumulated setup modifiers for the defending enemy Pokemon
            # Note: defending_mon is the enemy Pokemon, so we use is_player_mon=False
            # Include: 1) enemy's own self-targeting modifiers, 2) player's enemy-targeting modifiers
            enemy_self_modifiers = self._calc_accumulated_setup_modifiers(mon_idx, move_idx if move_idx is not None else 0, False, target_self=True)
            player_enemy_modifiers = self._calc_accumulated_setup_modifiers(mon_idx, move_idx if move_idx is not None else 0, True, target_self=False, calculating_for_enemy=True)
            # Combine both sources of modifiers
            accumulated_defending_modifiers = StageModifiers()
            if current_gen_info().get_generation() == 1:
                accumulated_defending_modifiers = StageModifiers(
                    attack=max(min(enemy_self_modifiers.attack_stage + player_enemy_modifiers.attack_stage, 6), -6),
                    defense=max(min(enemy_self_modifiers.defense_stage + player_enemy_modifiers.defense_stage, 6), -6),
                    speed=max(min(enemy_self_modifiers.speed_stage + player_enemy_modifiers.speed_stage, 6), -6),
                    special_attack=max(min(enemy_self_modifiers.special_attack_stage + player_enemy_modifiers.special_attack_stage, 6), -6),
                    special_defense=max(min(enemy_self_modifiers.special_defense_stage + player_enemy_modifiers.special_defense_stage, 6), -6),
                    accuracy=max(min(enemy_self_modifiers.accuracy_stage + player_enemy_modifiers.accuracy_stage, 6), -6),
                    evasion=max(min(enemy_self_modifiers.evasion_stage + player_enemy_modifiers.evasion_stage, 6), -6),
                    attack_bb=enemy_self_modifiers.attack_badge_boosts + player_enemy_modifiers.attack_badge_boosts,
                    defense_bb=enemy_self_modifiers.defense_badge_boosts + player_enemy_modifiers.defense_badge_boosts,
                    speed_bb=enemy_self_modifiers.speed_badge_boosts + player_enemy_modifiers.speed_badge_boosts,
                    special_bb=enemy_self_modifiers.special_badge_boosts + player_enemy_modifiers.special_badge_boosts,
                )
            else:
                accumulated_defending_modifiers = enemy_self_modifiers
                if player_enemy_modifiers.attack_stage != 0:
                    accumulated_defending_modifiers = accumulated_defending_modifiers.apply_stat_mod([(const.ATK, player_enemy_modifiers.attack_stage)])
                if player_enemy_modifiers.defense_stage != 0:
                    accumulated_defending_modifiers = accumulated_defending_modifiers.apply_stat_mod([(const.DEF, player_enemy_modifiers.defense_stage)])
                if player_enemy_modifiers.speed_stage != 0:
                    accumulated_defending_modifiers = accumulated_defending_modifiers.apply_stat_mod([(const.SPE, player_enemy_modifiers.speed_stage)])
                if player_enemy_modifiers.special_attack_stage != 0:
                    accumulated_defending_modifiers = accumulated_defending_modifiers.apply_stat_mod([(const.SPA, player_enemy_modifiers.special_attack_stage)])
                if player_enemy_modifiers.special_defense_stage != 0:
                    accumulated_defending_modifiers = accumulated_defending_modifiers.apply_stat_mod([(const.SPD, player_enemy_modifiers.special_defense_stage)])
                if player_enemy_modifiers.accuracy_stage != 0:
                    accumulated_defending_modifiers = accumulated_defending_modifiers.apply_stat_mod([(const.ACC, player_enemy_modifiers.accuracy_stage)])
                if player_enemy_modifiers.evasion_stage != 0:
                    accumulated_defending_modifiers = accumulated_defending_modifiers.apply_stat_mod([(const.EV, player_enemy_modifiers.evasion_stage)])
            # Combine base enemy stage modifier with accumulated modifiers
            base_defending_modifier = self._enemy_stage_modifier
            # For Gen 1, we need to preserve badge boosts from accumulated modifiers
            # For other gens, just combine stage modifiers
            if current_gen_info().get_generation() == 1:
                # Combine stage modifiers and badge boosts separately
                defending_stage_modifiers = StageModifiers(
                    attack=max(min(base_defending_modifier.attack_stage + accumulated_defending_modifiers.attack_stage, 6), -6),
                    defense=max(min(base_defending_modifier.defense_stage + accumulated_defending_modifiers.defense_stage, 6), -6),
                    speed=max(min(base_defending_modifier.speed_stage + accumulated_defending_modifiers.speed_stage, 6), -6),
                    special_attack=max(min(base_defending_modifier.special_attack_stage + accumulated_defending_modifiers.special_attack_stage, 6), -6),
                    special_defense=max(min(base_defending_modifier.special_defense_stage + accumulated_defending_modifiers.special_defense_stage, 6), -6),
                    accuracy=max(min(base_defending_modifier.accuracy_stage + accumulated_defending_modifiers.accuracy_stage, 6), -6),
                    evasion=max(min(base_defending_modifier.evasion_stage + accumulated_defending_modifiers.evasion_stage, 6), -6),
                    attack_bb=base_defending_modifier.attack_badge_boosts + accumulated_defending_modifiers.attack_badge_boosts,
                    defense_bb=base_defending_modifier.defense_badge_boosts + accumulated_defending_modifiers.defense_badge_boosts,
                    speed_bb=base_defending_modifier.speed_badge_boosts + accumulated_defending_modifiers.speed_badge_boosts,
                    special_bb=base_defending_modifier.special_badge_boosts + accumulated_defending_modifiers.special_badge_boosts,
                )
            else:
                # For non-Gen 1, combine stage modifiers normally
                defending_stage_modifiers = base_defending_modifier
                if accumulated_defending_modifiers.attack_stage != 0:
                    defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.ATK, accumulated_defending_modifiers.attack_stage)])
                if accumulated_defending_modifiers.defense_stage != 0:
                    defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.DEF, accumulated_defending_modifiers.defense_stage)])
                if accumulated_defending_modifiers.speed_stage != 0:
                    defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.SPE, accumulated_defending_modifiers.speed_stage)])
                if accumulated_defending_modifiers.special_attack_stage != 0:
                    defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.SPA, accumulated_defending_modifiers.special_attack_stage)])
                if accumulated_defending_modifiers.special_defense_stage != 0:
                    defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.SPD, accumulated_defending_modifiers.special_defense_stage)])
                if accumulated_defending_modifiers.accuracy_stage != 0:
                    defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.ACC, accumulated_defending_modifiers.accuracy_stage)])
                if accumulated_defending_modifiers.evasion_stage != 0:
                    defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.EV, accumulated_defending_modifiers.evasion_stage)])
            defending_field_status = self._enemy_field_status
            custom_lookup_key = const.PLAYER_KEY
        else:
            attacking_mon = self._original_enemy_mon_list[mon_idx]
            attacking_mon_stats = None
            crit_mon = attacking_mon
            crit_mon_stats = attacking_mon_stats
            
            # Calculate accumulated setup modifiers for the attacking enemy Pokemon
            # Include: 1) enemy's own self-targeting modifiers (like Curse), 2) player's enemy-targeting modifiers (like Mud-Slap)
            enemy_self_modifiers = self._calc_accumulated_setup_modifiers(mon_idx, move_idx if move_idx is not None else 0, False, target_self=True, calculating_for_enemy=True)
            player_enemy_modifiers = self._calc_accumulated_setup_modifiers(mon_idx, move_idx if move_idx is not None else 0, True, target_self=False, calculating_for_enemy=True)
            # Combine both sources of modifiers
            if current_gen_info().get_generation() == 1:
                # In Gen 1, enemy enemy-targeting modifiers (like Psychic) should NOT affect the enemy's attacking stats
                # They only affect the defending player's stats. So we only use enemy_self_modifiers for special stats
                # (enemy's self-targeting modifiers like Amnesia), not enemy enemy-targeting modifiers.
                accumulated_modifiers = StageModifiers(
                    attack=max(min(enemy_self_modifiers.attack_stage + player_enemy_modifiers.attack_stage, 6), -6),
                    defense=max(min(enemy_self_modifiers.defense_stage + player_enemy_modifiers.defense_stage, 6), -6),
                    speed=max(min(enemy_self_modifiers.speed_stage + player_enemy_modifiers.speed_stage, 6), -6),
                    # For Gen 1 special stats: only use enemy_self_modifiers (self-targeting like Amnesia)
                    # Do NOT include player_enemy_modifiers for special attack, as enemy enemy-targeting moves
                    # (like Psychic) should only affect defending stats, not attacking stats
                    special_attack=max(min(enemy_self_modifiers.special_attack_stage, 6), -6),
                    special_defense=max(min(enemy_self_modifiers.special_defense_stage, 6), -6),
                    accuracy=max(min(enemy_self_modifiers.accuracy_stage + player_enemy_modifiers.accuracy_stage, 6), -6),
                    evasion=max(min(enemy_self_modifiers.evasion_stage + player_enemy_modifiers.evasion_stage, 6), -6),
                    attack_bb=enemy_self_modifiers.attack_badge_boosts + player_enemy_modifiers.attack_badge_boosts,
                    defense_bb=enemy_self_modifiers.defense_badge_boosts + player_enemy_modifiers.defense_badge_boosts,
                    speed_bb=enemy_self_modifiers.speed_badge_boosts + player_enemy_modifiers.speed_badge_boosts,
                    # For Gen 1 special badge boosts: only use enemy_self_modifiers
                    special_bb=enemy_self_modifiers.special_badge_boosts,
                )
            else:
                accumulated_modifiers = enemy_self_modifiers
                if player_enemy_modifiers.attack_stage != 0:
                    accumulated_modifiers = accumulated_modifiers.apply_stat_mod([(const.ATK, player_enemy_modifiers.attack_stage)])
                if player_enemy_modifiers.defense_stage != 0:
                    accumulated_modifiers = accumulated_modifiers.apply_stat_mod([(const.DEF, player_enemy_modifiers.defense_stage)])
                if player_enemy_modifiers.speed_stage != 0:
                    accumulated_modifiers = accumulated_modifiers.apply_stat_mod([(const.SPE, player_enemy_modifiers.speed_stage)])
                if player_enemy_modifiers.special_attack_stage != 0:
                    accumulated_modifiers = accumulated_modifiers.apply_stat_mod([(const.SPA, player_enemy_modifiers.special_attack_stage)])
                if player_enemy_modifiers.special_defense_stage != 0:
                    accumulated_modifiers = accumulated_modifiers.apply_stat_mod([(const.SPD, player_enemy_modifiers.special_defense_stage)])
                if player_enemy_modifiers.accuracy_stage != 0:
                    accumulated_modifiers = accumulated_modifiers.apply_stat_mod([(const.ACC, player_enemy_modifiers.accuracy_stage)])
                if player_enemy_modifiers.evasion_stage != 0:
                    accumulated_modifiers = accumulated_modifiers.apply_stat_mod([(const.EV, player_enemy_modifiers.evasion_stage)])
            # Combine base stage modifier with accumulated modifiers
            base_modifier = self._enemy_stage_modifier
            # For Gen 1, we need to preserve badge boosts from accumulated modifiers
            # For other gens, just combine stage modifiers
            if current_gen_info().get_generation() == 1:
                # Combine stage modifiers and badge boosts separately
                # Badge boosts from accumulated modifiers are the ones we care about (from per-move setup usage)
                # Base modifier badge boosts are from the old setup move system (top dropdown), which we preserve
                # IMPORTANT: In Gen 1, base_modifier may include enemy-targeting moves (like Psychic) from the top dropdown.
                # These should NOT affect the enemy's attacking special stats - they only affect defending player stats.
                # So for special stats, we only use accumulated_modifiers (which only includes self-targeting moves).
                attacking_stage_modifiers = StageModifiers(
                    attack=max(min(base_modifier.attack_stage + accumulated_modifiers.attack_stage, 6), -6),
                    defense=max(min(base_modifier.defense_stage + accumulated_modifiers.defense_stage, 6), -6),
                    speed=max(min(base_modifier.speed_stage + accumulated_modifiers.speed_stage, 6), -6),
                    # For Gen 1 special stats: only use accumulated_modifiers (self-targeting moves like Amnesia)
                    # Do NOT include base_modifier special stats, as they may include enemy-targeting moves (like Psychic)
                    # which should only affect defending stats, not attacking stats
                    special_attack=max(min(accumulated_modifiers.special_attack_stage, 6), -6),
                    special_defense=max(min(accumulated_modifiers.special_defense_stage, 6), -6),
                    accuracy=max(min(base_modifier.accuracy_stage + accumulated_modifiers.accuracy_stage, 6), -6),
                    evasion=max(min(base_modifier.evasion_stage + accumulated_modifiers.evasion_stage, 6), -6),
                    attack_bb=base_modifier.attack_badge_boosts + accumulated_modifiers.attack_badge_boosts,
                    defense_bb=base_modifier.defense_badge_boosts + accumulated_modifiers.defense_badge_boosts,
                    speed_bb=base_modifier.speed_badge_boosts + accumulated_modifiers.speed_badge_boosts,
                    # For Gen 1 special badge boosts: only use accumulated_modifiers
                    special_bb=accumulated_modifiers.special_badge_boosts,
                )
            else:
                # For non-Gen 1, combine stage modifiers normally
                attacking_stage_modifiers = base_modifier
                if accumulated_modifiers.attack_stage != 0:
                    attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.ATK, accumulated_modifiers.attack_stage)])
                if accumulated_modifiers.defense_stage != 0:
                    attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.DEF, accumulated_modifiers.defense_stage)])
                if accumulated_modifiers.speed_stage != 0:
                    attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.SPE, accumulated_modifiers.speed_stage)])
                if accumulated_modifiers.special_attack_stage != 0:
                    attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.SPA, accumulated_modifiers.special_attack_stage)])
                if accumulated_modifiers.special_defense_stage != 0:
                    attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.SPD, accumulated_modifiers.special_defense_stage)])
                if accumulated_modifiers.accuracy_stage != 0:
                    attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.ACC, accumulated_modifiers.accuracy_stage)])
                if accumulated_modifiers.evasion_stage != 0:
                    attacking_stage_modifiers = attacking_stage_modifiers.apply_stat_mod([(const.EV, accumulated_modifiers.evasion_stage)])
            attacking_field_status = self._enemy_field_status
            if self._is_player_transformed:
                defending_mon = self._transformed_mon_list[mon_idx]
                defending_mon_stats = defending_mon.cur_stats
            else:
                defending_mon = self._original_player_mon_list[mon_idx]
                defending_mon_stats = None
            # Calculate accumulated setup modifiers for the defending player Pokemon
            # Note: defending_mon is the player Pokemon, so we use is_player_mon=True
            # Include: 1) player's own self-targeting modifiers, 2) enemy's enemy-targeting modifiers
            player_self_modifiers = self._calc_accumulated_setup_modifiers(mon_idx, move_idx if move_idx is not None else 0, True, target_self=True)
            enemy_enemy_modifiers = self._calc_accumulated_setup_modifiers(mon_idx, move_idx if move_idx is not None else 0, False, target_self=False)
            # Combine both sources of modifiers
            accumulated_defending_modifiers = StageModifiers()
            if current_gen_info().get_generation() == 1:
                # Check if the solo Pokemon leveled up before or during this Pokemon's battle
                level_up_mon_idx = None
                for idx in range(len(self._leveled_up_pokemon)):
                    if self._leveled_up_pokemon[idx]:
                        level_up_mon_idx = idx
                        break
                
                # If a level-up occurred, clear badge boosts from player_self_modifiers for Pokemon AFTER the level-up
                player_bb_attack = player_self_modifiers.attack_badge_boosts
                player_bb_defense = player_self_modifiers.defense_badge_boosts
                player_bb_speed = player_self_modifiers.speed_badge_boosts
                player_bb_special = player_self_modifiers.special_badge_boosts
                if level_up_mon_idx is not None and mon_idx > level_up_mon_idx:
                    player_bb_attack = 0
                    player_bb_defense = 0
                    player_bb_speed = 0
                    player_bb_special = 0
                
                accumulated_defending_modifiers = StageModifiers(
                    attack=max(min(player_self_modifiers.attack_stage + enemy_enemy_modifiers.attack_stage, 6), -6),
                    defense=max(min(player_self_modifiers.defense_stage + enemy_enemy_modifiers.defense_stage, 6), -6),
                    speed=max(min(player_self_modifiers.speed_stage + enemy_enemy_modifiers.speed_stage, 6), -6),
                    special_attack=max(min(player_self_modifiers.special_attack_stage + enemy_enemy_modifiers.special_attack_stage, 6), -6),
                    special_defense=max(min(player_self_modifiers.special_defense_stage + enemy_enemy_modifiers.special_defense_stage, 6), -6),
                    accuracy=max(min(player_self_modifiers.accuracy_stage + enemy_enemy_modifiers.accuracy_stage, 6), -6),
                    evasion=max(min(player_self_modifiers.evasion_stage + enemy_enemy_modifiers.evasion_stage, 6), -6),
                    attack_bb=player_bb_attack + enemy_enemy_modifiers.attack_badge_boosts,
                    defense_bb=player_bb_defense + enemy_enemy_modifiers.defense_badge_boosts,
                    speed_bb=player_bb_speed + enemy_enemy_modifiers.speed_badge_boosts,
                    special_bb=player_bb_special + enemy_enemy_modifiers.special_badge_boosts,
                )
            else:
                accumulated_defending_modifiers = player_self_modifiers
                if enemy_enemy_modifiers.attack_stage != 0:
                    accumulated_defending_modifiers = accumulated_defending_modifiers.apply_stat_mod([(const.ATK, enemy_enemy_modifiers.attack_stage)])
                if enemy_enemy_modifiers.defense_stage != 0:
                    accumulated_defending_modifiers = accumulated_defending_modifiers.apply_stat_mod([(const.DEF, enemy_enemy_modifiers.defense_stage)])
                if enemy_enemy_modifiers.speed_stage != 0:
                    accumulated_defending_modifiers = accumulated_defending_modifiers.apply_stat_mod([(const.SPE, enemy_enemy_modifiers.speed_stage)])
                if enemy_enemy_modifiers.special_attack_stage != 0:
                    accumulated_defending_modifiers = accumulated_defending_modifiers.apply_stat_mod([(const.SPA, enemy_enemy_modifiers.special_attack_stage)])
                if enemy_enemy_modifiers.special_defense_stage != 0:
                    accumulated_defending_modifiers = accumulated_defending_modifiers.apply_stat_mod([(const.SPD, enemy_enemy_modifiers.special_defense_stage)])
                if enemy_enemy_modifiers.accuracy_stage != 0:
                    accumulated_defending_modifiers = accumulated_defending_modifiers.apply_stat_mod([(const.ACC, enemy_enemy_modifiers.accuracy_stage)])
                if enemy_enemy_modifiers.evasion_stage != 0:
                    accumulated_defending_modifiers = accumulated_defending_modifiers.apply_stat_mod([(const.EV, enemy_enemy_modifiers.evasion_stage)])
            # Combine base player stage modifier with accumulated modifiers
            base_defending_modifier = self._player_stage_modifier
            # For Gen 1, we need to preserve badge boosts from accumulated modifiers
            # For other gens, just combine stage modifiers
            if current_gen_info().get_generation() == 1:
                # Check if the solo Pokemon leveled up before or during this Pokemon's battle
                level_up_mon_idx = None
                for idx in range(len(self._leveled_up_pokemon)):
                    if self._leveled_up_pokemon[idx]:
                        level_up_mon_idx = idx
                        break
                
                # If a level-up occurred, clear badge boosts from base_defending_modifier for Pokemon AFTER the level-up
                # (level-up removes all erroneously applied badge boosts)
                # Note: Badge boosts still apply to the Pokemon where the level-up occurs
                base_attack_bb = 0
                base_defense_bb = 0
                base_speed_bb = 0
                base_special_bb = 0
                if level_up_mon_idx is None or mon_idx <= level_up_mon_idx:
                    # No level-up yet, or calculating for Pokemon up to and including the level-up - preserve base badge boosts
                    base_attack_bb = base_defending_modifier.attack_badge_boosts
                    base_defense_bb = base_defending_modifier.defense_badge_boosts
                    base_speed_bb = base_defending_modifier.speed_badge_boosts
                    base_special_bb = base_defending_modifier.special_badge_boosts
                
                # Combine stage modifiers and badge boosts separately
                defending_stage_modifiers = StageModifiers(
                    attack=max(min(base_defending_modifier.attack_stage + accumulated_defending_modifiers.attack_stage, 6), -6),
                    defense=max(min(base_defending_modifier.defense_stage + accumulated_defending_modifiers.defense_stage, 6), -6),
                    speed=max(min(base_defending_modifier.speed_stage + accumulated_defending_modifiers.speed_stage, 6), -6),
                    special_attack=max(min(base_defending_modifier.special_attack_stage + accumulated_defending_modifiers.special_attack_stage, 6), -6),
                    special_defense=max(min(base_defending_modifier.special_defense_stage + accumulated_defending_modifiers.special_defense_stage, 6), -6),
                    accuracy=max(min(base_defending_modifier.accuracy_stage + accumulated_defending_modifiers.accuracy_stage, 6), -6),
                    evasion=max(min(base_defending_modifier.evasion_stage + accumulated_defending_modifiers.evasion_stage, 6), -6),
                    attack_bb=base_attack_bb + accumulated_defending_modifiers.attack_badge_boosts,
                    defense_bb=base_defense_bb + accumulated_defending_modifiers.defense_badge_boosts,
                    speed_bb=base_speed_bb + accumulated_defending_modifiers.speed_badge_boosts,
                    special_bb=base_special_bb + accumulated_defending_modifiers.special_badge_boosts,
                )
            else:
                # For non-Gen 1, combine stage modifiers normally
                defending_stage_modifiers = base_defending_modifier
                if accumulated_defending_modifiers.attack_stage != 0:
                    defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.ATK, accumulated_defending_modifiers.attack_stage)])
                if accumulated_defending_modifiers.defense_stage != 0:
                    defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.DEF, accumulated_defending_modifiers.defense_stage)])
                if accumulated_defending_modifiers.speed_stage != 0:
                    defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.SPE, accumulated_defending_modifiers.speed_stage)])
                if accumulated_defending_modifiers.special_attack_stage != 0:
                    defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.SPA, accumulated_defending_modifiers.special_attack_stage)])
                if accumulated_defending_modifiers.special_defense_stage != 0:
                    defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.SPD, accumulated_defending_modifiers.special_defense_stage)])
                if accumulated_defending_modifiers.accuracy_stage != 0:
                    defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.ACC, accumulated_defending_modifiers.accuracy_stage)])
                if accumulated_defending_modifiers.evasion_stage != 0:
                    defending_stage_modifiers = defending_stage_modifiers.apply_stat_mod([(const.EV, accumulated_defending_modifiers.evasion_stage)])
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
            custom_data_selection
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

        # Load move setup usage (backward compatible - handle missing data)
        # move_setup_usage are stored in definition order (mon_order), need to convert to display order
        if trainer_def.move_setup_usage is not None and len(trainer_def.move_setup_usage) > 0:
            # _cached_definition_order maps: display_idx -> definition_idx
            # So for each display index, we need to get the definition index and use that to index into move_setup_usage
            self._move_setup_usage = []
            num_pokemon = len(event_group.event_definition.get_pokemon_list())
            for display_idx in range(num_pokemon):
                if display_idx < len(self._cached_definition_order):
                    def_idx = self._cached_definition_order[display_idx]
                    if def_idx < len(trainer_def.move_setup_usage):
                        # Convert string keys back to integers (JSON converts int keys to strings)
                        loaded_data = copy.deepcopy(trainer_def.move_setup_usage[def_idx])
                        converted_data = {}
                        for key in [const.PLAYER_KEY, const.ENEMY_KEY]:
                            if key in loaded_data:
                                converted_data[key] = {}
                                # Convert string move_idx keys back to integers
                                for move_idx_str, count in loaded_data[key].items():
                                    try:
                                        move_idx_int = int(move_idx_str)
                                        converted_data[key][move_idx_int] = count
                                    except (ValueError, TypeError):
                                        # Skip invalid keys
                                        continue
                            else:
                                converted_data[key] = {}
                        self._move_setup_usage.append(converted_data)
                    else:
                        self._move_setup_usage.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
                else:
                    self._move_setup_usage.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
        else:
            self._move_setup_usage = []
            for _ in range(len(event_group.event_definition.get_pokemon_list())):
                self._move_setup_usage.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})

        self._original_player_mon_list = []
        self._transformed_mon_list = []
        self._original_enemy_mon_list = []
        # Track which Pokemon have leveled up (for Gen 1 badge boost clearing)
        self._leveled_up_pokemon = []

        # NOTE: kind of weird, but basically we want to iterate over all the pokemon we want to fight, and then get the appropriate
        # event item for fighting that pokemon. This allows us to pull learned moves/levelups/etc automatically
        cur_item_idx = 0
        for cur_pkmn in event_group.event_definition.get_pokemon_list():
            initial_level = None
            final_level = None
            found_pokemon = False
            
            while cur_item_idx < len(event_group.event_items):
                cur_event_item = event_group.event_items[cur_item_idx]
                
                # Track level changes across all event items (including level-up events)
                if cur_event_item.init_state:
                    if initial_level is None:
                        initial_level = cur_event_item.init_state.solo_pkmn.cur_level
                if cur_event_item.final_state:
                    final_level = cur_event_item.final_state.solo_pkmn.cur_level
                
                cur_item_idx += 1
                
                # Check for level-up events mid-fight (these are skipped but we track levels)
                if cur_event_item.event_definition.trainer_def is None:
                    continue

                if cur_event_item.to_defeat_mon == cur_pkmn:
                    found_pokemon = True
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
            
            # Check if this Pokemon leveled up (compare initial to final level)
            if found_pokemon:
                leveled_up = (initial_level is not None and final_level is not None and final_level > initial_level)
                self._leveled_up_pokemon.append(leveled_up)
            else:
                # Shouldn't happen, but handle gracefully
                self._leveled_up_pokemon.append(False)

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
        self._move_setup_usage = []
        self._leveled_up_pokemon = []
        self._cached_definition_order = list(range(len(enemy_mons)))
        for _ in range(len(enemy_mons)):
            self._custom_move_data.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
            self._move_highlights.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
            self._move_setup_usage.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
            self._leveled_up_pokemon.append(False)

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
        self._move_setup_usage = []
        self._leveled_up_pokemon = []
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

        # Reorder move_setup_usage according to definition order (similar to move_highlights)
        # Check if move_setup_usage is present (any non-zero counts)
        is_move_setup_usage_present = False
        if self._move_setup_usage and len(self._move_setup_usage) > 0:
            for cur_test in self._move_setup_usage:
                # Check if cur_test has any keys (PLAYER_KEY or ENEMY_KEY)
                if const.PLAYER_KEY in cur_test or const.ENEMY_KEY in cur_test:
                    # Check if any count is non-zero
                    for key in [const.PLAYER_KEY, const.ENEMY_KEY]:
                        if key in cur_test and cur_test[key]:
                            for move_idx, count in cur_test[key].items():
                                if count > 0:
                                    is_move_setup_usage_present = True
                                    break
                            if is_move_setup_usage_present:
                                break
                    if is_move_setup_usage_present:
                        break
        
        final_move_setup_usage = None
        if is_move_setup_usage_present:
            # Convert from display order back to definition order
            # _cached_definition_order maps display_idx -> definition_idx
            # We need to create definition_idx -> display_idx mapping
            def_to_display = {}
            for display_idx, def_idx in enumerate(self._cached_definition_order):
                def_to_display[def_idx] = display_idx
            
            # Now reorder: for each definition index, get the corresponding display index data
            final_move_setup_usage = []
            for def_idx in sorted(def_to_display.keys()):
                display_idx = def_to_display[def_idx]
                if display_idx < len(self._move_setup_usage):
                    # Ensure the dict has both keys, even if empty
                    display_data = copy.deepcopy(self._move_setup_usage[display_idx])
                    if const.PLAYER_KEY not in display_data:
                        display_data[const.PLAYER_KEY] = {}
                    if const.ENEMY_KEY not in display_data:
                        display_data[const.ENEMY_KEY] = {}
                    final_move_setup_usage.append(display_data)
                else:
                    final_move_setup_usage.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
        
        # Only pass move_setup_usage if we have actual data, otherwise let it default to None
        # (which will become [] in TrainerEventDefinition, but serialize will handle it)
        trainer_kwargs = {
            'trainer_name': self._trainer_name,
            'second_trainer_name': self._second_trainer_name,
            'setup_moves': self._player_setup_move_list,
            'enemy_setup_moves': self._enemy_setup_move_list,
            'mimic_selection': self._mimic_selection,
            'custom_move_data': final_custom_move_data,
            'weather': self._weather,
            'transformed': self._is_player_transformed,
            'move_highlights': final_move_highlights,
        }
        if final_move_setup_usage is not None:
            trainer_kwargs['move_setup_usage'] = final_move_setup_usage
        
        return TrainerEventDefinition(**trainer_kwargs)

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

    def update_move_setup_usage(self, mon_idx:int, move_idx:int, is_player_mon:bool, count:int):
        """Update how many times a move has been used as a setup move"""
        if mon_idx < 0 or mon_idx >= len(self._move_setup_usage):
            return
        
        key = const.PLAYER_KEY if is_player_mon else const.ENEMY_KEY
        if key not in self._move_setup_usage[mon_idx]:
            self._move_setup_usage[mon_idx][key] = {}
        
        if count == 0:
            # Remove the entry if count is 0
            self._move_setup_usage[mon_idx][key].pop(move_idx, None)
        else:
            self._move_setup_usage[mon_idx][key][move_idx] = count
        
        # Trigger recalculation
        self._full_refresh()
        # Explicitly trigger save (similar to update_custom_move_data)
        self._on_nonload_change()

    def get_move_setup_usage(self, mon_idx:int, move_idx:int, is_player_mon:bool) -> int:
        """Get how many times a move has been used as a setup move"""
        if mon_idx < 0 or mon_idx >= len(self._move_setup_usage):
            return 0
        
        key = const.PLAYER_KEY if is_player_mon else const.ENEMY_KEY
        if key not in self._move_setup_usage[mon_idx]:
            return 0
        
        return self._move_setup_usage[mon_idx][key].get(move_idx, 0)

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

    def _calc_accumulated_setup_modifiers(self, mon_idx:int, move_idx:int, is_player_mon:bool, target_self:bool=True, calculating_for_enemy:bool=False) -> StageModifiers:
        """Calculate accumulated stage modifiers from all setup moves used up to this point in the battle
        
        Args:
            mon_idx: Index of the Pokemon
            move_idx: Index of the move being calculated
            is_player_mon: True if getting moves from player Pokemon, False for enemy
            target_self: If True, only include modifiers that target self. If False, only include modifiers that target enemy/foe.
            calculating_for_enemy: True if we're calculating modifiers for an enemy Pokemon (affects whether to check previous battles)
        """
        result = StageModifiers()
        
        if mon_idx < 0:
            return result
        
        key = const.PLAYER_KEY if is_player_mon else const.ENEMY_KEY
        
        # When calculating enemy-targeting modifiers (target_self=False), we need to be careful:
        # - If calculating for enemy mon (is_player_mon=False, target_self=False): only include player moves from THIS battle (mon_idx)
        #   because player moves that lower enemy stats only affect the specific enemy mon they were used against
        # - If calculating for player mon (is_player_mon=True, target_self=False): include enemy moves from ALL battles (0 to mon_idx)
        #   because enemy moves that lower player stats affect all player moves against all enemies
        # - For self-targeting modifiers (target_self=True): include all previous battles as normal
        
        # Determine if we should check previous battles
        # - Self-targeting modifiers (target_self=True): always check previous battles
        # - Enemy-targeting modifiers for player (is_player_mon=True, target_self=False, calculating_for_enemy=False): check previous battles (enemy moves affect all player moves)
        # - Enemy-targeting modifiers for enemy (is_player_mon=True, target_self=False, calculating_for_enemy=True): DON'T check previous battles (player moves only affect specific enemy)
        # - Enemy enemy-targeting modifiers (is_player_mon=False, target_self=False): In Gen 1, check previous battles if damage-dealing
        #   because enemy damage-dealing moves that lower player stats affect all subsequent player moves
        should_check_previous = target_self or (is_player_mon and not target_self and not calculating_for_enemy)
        
        # Special case for Gen 1: enemy enemy-targeting modifiers (enemy moves that affect player) should check previous battles
        # because they affect the player for the rest of the battle
        if current_gen_info().get_generation() == 1 and not is_player_mon and not target_self:
            should_check_previous = True
        
        # For Gen 1: Find the first Pokemon index where the solo Pokemon leveled up
        # This is needed to determine which moves should have badge boosts cleared
        level_up_mon_idx = None
        if current_gen_info().get_generation() == 1 and is_player_mon:
            for idx in range(len(self._leveled_up_pokemon)):
                if self._leveled_up_pokemon[idx]:
                    level_up_mon_idx = idx
                    break
        
        if should_check_previous:
            # Check all previous Pokemon (mon_idx 0 to mon_idx-1)
            # This applies to:
            # - Self-targeting modifiers (target_self=True) - always accumulate across battles
            # - Enemy-targeting modifiers for player (is_player_mon=True, target_self=False) - enemy moves affect all player moves
            for prev_mon_idx in range(mon_idx):
                if prev_mon_idx >= len(self._move_setup_usage):
                    continue
                
                if key not in self._move_setup_usage[prev_mon_idx]:
                    continue
                
                # Get the previous Pokemon's move list
                if is_player_mon:
                    if self._is_player_transformed:
                        prev_mon = self._transformed_mon_list[prev_mon_idx]
                    else:
                        prev_mon = self._original_player_mon_list[prev_mon_idx]
                else:
                    prev_mon = self._original_enemy_mon_list[prev_mon_idx]
                
                # Check all moves for this previous Pokemon
                setup_usage = self._move_setup_usage[prev_mon_idx][key]
                for prev_move_idx in range(4):  # Check all 4 move slots
                    if prev_move_idx < len(prev_mon.move_list) and prev_move_idx in setup_usage:
                        move_name = prev_mon.move_list[prev_move_idx]
                        if move_name:
                            # Handle Mimic special case
                            if move_name == const.MIMIC_MOVE_NAME:
                                if self._mimic_selection:
                                    move_name = self._mimic_selection
                                else:
                                    continue
                            
                            count = setup_usage[prev_move_idx]
                            if count > 0:
                                move_obj = current_gen_info().move_db().get_move(move_name)
                                is_damage_dealing = move_obj and move_obj.base_power and move_obj.base_power > 0
                                
                                # In Gen 1, damage-dealing moves (base_power > 0) with stat modifiers apply their effect to the TARGET, not the USER
                                # So when looking for self-targeting modifiers (target_self=True), skip damage-dealing moves
                                if current_gen_info().get_generation() == 1 and target_self and is_damage_dealing:
                                    # This is a damage-dealing move, its stat modifiers apply to the target, not the user
                                    continue
                                
                                # Get stat mods filtered by target
                                stat_mods = current_gen_info().move_db().get_stat_mod_for_target(move_name, target_self=target_self)
                                
                                # In Gen 1, when looking for enemy-targeting modifiers (target_self=False), also include damage-dealing moves
                                # because damage-dealing moves always apply their stat modifiers to the target, regardless of effect target field
                                if current_gen_info().get_generation() == 1 and not target_self and is_damage_dealing and not stat_mods:
                                    # Try getting all stat mods (without target filter) for damage-dealing moves
                                    stat_mods = current_gen_info().move_db().get_stat_mod(move_name)
                                
                                if stat_mods:
                                    # Apply the stat mods count times
                                    # For Gen 1, apply_stat_mod handles badge boosts correctly
                                    for _ in range(count):
                                        result = result.apply_stat_mod(stat_mods)
            
            # For Gen 1: If a level-up occurred and we're using per-move setup (not global setup),
            # clear badge boosts from moves used BEFORE the level-up, so moves AFTER can re-accumulate
            if current_gen_info().get_generation() == 1 and is_player_mon and not self._player_setup_move_list:
                level_up_mon_idx = None
                for idx in range(len(self._leveled_up_pokemon)):
                    if self._leveled_up_pokemon[idx]:
                        level_up_mon_idx = idx
                        break
                
                # Clear badge boosts from previous battles (before level-up), so current battle can re-accumulate
                if level_up_mon_idx is not None and mon_idx > level_up_mon_idx:
                    result = result.clear_badge_boosts()
            
            # Also check the current battle (mon_idx) when should_check_previous is True
            # This is needed because enemy moves that lower player stats affect all subsequent player moves
            if should_check_previous and mon_idx < len(self._move_setup_usage):
                if key not in self._move_setup_usage[mon_idx]:
                    pass  # No moves to check
                else:
                    # Get the current Pokemon's move list
                    if is_player_mon:
                        if self._is_player_transformed:
                            cur_mon = self._transformed_mon_list[mon_idx]
                        else:
                            cur_mon = self._original_player_mon_list[mon_idx]
                    else:
                        cur_mon = self._original_enemy_mon_list[mon_idx]
                    
                    # Check moves for the current Pokemon
                    # For per-move setup, only include moves before the current move_idx
                    # (moves accumulate across the battle, but we only want moves used before this one)
                    setup_usage = self._move_setup_usage[mon_idx][key]
                    for cur_move_idx in range(4):  # Check all 4 move slots
                        # Only process moves before the current move_idx (for per-move setup)
                        if cur_move_idx >= move_idx:
                            continue
                        if cur_move_idx < len(cur_mon.move_list) and cur_move_idx in setup_usage:
                            move_name = cur_mon.move_list[cur_move_idx]
                            if move_name:
                                # Handle Mimic special case
                                if move_name == const.MIMIC_MOVE_NAME:
                                    if self._mimic_selection:
                                        move_name = self._mimic_selection
                                    else:
                                        continue
                                
                                count = setup_usage[cur_move_idx]
                                if count > 0:
                                    move_obj = current_gen_info().move_db().get_move(move_name)
                                    is_damage_dealing = move_obj and move_obj.base_power and move_obj.base_power > 0
                                    
                                    # In Gen 1, damage-dealing moves (base_power > 0) with stat modifiers apply their effect to the TARGET, not the USER
                                    # So when looking for self-targeting modifiers (target_self=True), skip damage-dealing moves
                                    if current_gen_info().get_generation() == 1 and target_self and is_damage_dealing:
                                        # This is a damage-dealing move, its stat modifiers apply to the target, not the user
                                        continue
                                    
                                    # Get stat mods filtered by target
                                    stat_mods = current_gen_info().move_db().get_stat_mod_for_target(move_name, target_self=target_self)
                                    
                                    # In Gen 1, when looking for enemy-targeting modifiers (target_self=False), also include damage-dealing moves
                                    # because damage-dealing moves always apply their stat modifiers to the target, regardless of effect target field
                                    if current_gen_info().get_generation() == 1 and not target_self and is_damage_dealing and not stat_mods:
                                        # Try getting all stat mods (without target filter) for damage-dealing moves
                                        stat_mods = current_gen_info().move_db().get_stat_mod(move_name)
                                    
                                    if stat_mods:
                                        # Apply the stat mods count times
                                        # For Gen 1, apply_stat_mod handles badge boosts correctly
                                        for _ in range(count):
                                            result = result.apply_stat_mod(stat_mods)
        
        # else: target_self=False and is_player_mon=False
        # This means we're calculating enemy-targeting modifiers for an enemy Pokemon
        # Only include player moves from THIS battle (mon_idx), not previous battles
        # because player moves that lower enemy stats only affect the specific enemy mon they were used against
        
        # Only process current battle here if we didn't already process it above (when should_check_previous was True)
        if not should_check_previous and mon_idx < len(self._move_setup_usage):
            if key in self._move_setup_usage[mon_idx]:
                # Get the current Pokemon's move list
                if is_player_mon:
                    if self._is_player_transformed:
                        cur_mon = self._transformed_mon_list[mon_idx]
                    else:
                        cur_mon = self._original_player_mon_list[mon_idx]
                else:
                    cur_mon = self._original_enemy_mon_list[mon_idx]
                
                # Check all moves for the current Pokemon
                # For per-move setup, only include moves before the current move_idx
                setup_usage = self._move_setup_usage[mon_idx][key]
                for cur_move_idx in range(4):  # Check all 4 move slots
                    # Only process moves before the current move_idx (for per-move setup)
                    if cur_move_idx >= move_idx:
                        continue
                    if cur_move_idx < len(cur_mon.move_list) and cur_move_idx in setup_usage:
                        move_name = cur_mon.move_list[cur_move_idx]
                        if move_name:
                            # Handle Mimic special case
                            if move_name == const.MIMIC_MOVE_NAME:
                                if self._mimic_selection:
                                    move_name = self._mimic_selection
                                else:
                                    continue
                            
                            count = setup_usage[cur_move_idx]
                            if count > 0:
                                move_obj = current_gen_info().move_db().get_move(move_name)
                                is_damage_dealing = move_obj and move_obj.base_power and move_obj.base_power > 0
                                
                                # In Gen 1, damage-dealing moves (base_power > 0) with stat modifiers apply their effect to the TARGET, not the USER
                                # So when looking for self-targeting modifiers (target_self=True), skip damage-dealing moves
                                if current_gen_info().get_generation() == 1 and target_self and is_damage_dealing:
                                    # This is a damage-dealing move, its stat modifiers apply to the target, not the user
                                    continue
                                
                                # Get stat mods filtered by target
                                stat_mods = current_gen_info().move_db().get_stat_mod_for_target(move_name, target_self=target_self)
                                
                                # In Gen 1, when looking for enemy-targeting modifiers (target_self=False), also include damage-dealing moves
                                # because damage-dealing moves always apply their stat modifiers to the target, regardless of effect target field
                                if current_gen_info().get_generation() == 1 and not target_self and is_damage_dealing and not stat_mods:
                                    # Try getting all stat mods (without target filter) for damage-dealing moves
                                    stat_mods = current_gen_info().move_db().get_stat_mod(move_name)
                                
                                if stat_mods:
                                    # Apply the stat mods count times
                                    # For Gen 1, apply_stat_mod handles badge boosts correctly
                                    for _ in range(count):
                                        result = result.apply_stat_mod(stat_mods)
        
        # For Gen 1: If a level-up occurred and we're using per-move setup (not global setup),
        # clear badge boosts from moves used BEFORE the level-up, but allow compounding to continue after
        # Note: Only apply this if there's no global setup (top dropdown), otherwise global setup takes precedence
        if current_gen_info().get_generation() == 1 and is_player_mon and not self._player_setup_move_list:
            # Find the first Pokemon index where the solo Pokemon leveled up
            level_up_mon_idx = None
            for idx in range(len(self._leveled_up_pokemon)):
                if self._leveled_up_pokemon[idx]:
                    level_up_mon_idx = idx
                    break
            
        return result
    
    def has_global_setup(self) -> bool:
        """Check if there's any global setup (top dropdown) active"""
        return len(self._player_setup_move_list) > 0

    @staticmethod
    def _calc_field_status(move_list) -> FieldStatus:
        result = FieldStatus()

        for cur_move in move_list:
            if cur_move:  # Only process non-empty moves
                result = result.apply_move(cur_move)
        
        return result
    
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
        
