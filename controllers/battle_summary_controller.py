from __future__ import annotations
from dataclasses import dataclass, asdict
import copy
import logging
from typing import Dict, List, Tuple
from controllers.main_controller import MainController
from pkmn.damage_calc import DamageRange, find_kill
from pkmn.universal_data_objects import EnemyPkmn, FieldStatus, StageModifiers
from routing.full_route_state import RouteState
from utils.config_manager import config

from utils.constants import const
from routing.route_events import EventDefinition, EventFolder, EventGroup, HoldItemEventDefinition, InventoryEventDefinition, RareCandyEventDefinition, TrainerEventDefinition, VitaminEventDefinition
from pkmn.gen_factory import current_gen_info

logger = logging.getLogger(__name__)


@dataclass
class MoveRenderInfo:
    name:str
    attack_flavor:List[str]
    min_damage:int
    max_damage:int
    crit_min_damage:int
    crit_max_damage:int
    defending_mon_hp:int
    kill_ranges:List[Tuple[int, float]]
    mimic_data:str
    mimic_options:List[str]
    custom_data_options:List[str]
    custom_data_selection:str
    is_best_move:bool=False
    stat_stage_options:List[str]=None
    stat_stage_selection:str="0"
    stat_stage_info:dict=None

    def serialize(self):
        return asdict(self)

@dataclass
class PkmnRenderInfo:
    attacking_mon_name:str
    attacking_mon_level:int
    attacking_mon_speed:int
    defending_mon_name:str
    defending_mon_level:int
    defending_mon_speed:int
    defending_mon_hp:int

    def serialize(self):
        return asdict(self)

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
        self._refresh_callbacks = []
        self._nonload_change_callbacks = []

        # trainer object data that we don't actually use, but need to hang on to to properly re-create events
        self._trainer_name = None
        self._second_trainer_name = None
        self._event_group_id = None

        # actual state used to calculate battle stats
        self._original_player_mon_list:List[EnemyPkmn] = []
        self._player_setup_move_list:List[str] = []
        self._player_field_move_list:List[str] = []
        self._is_player_transformed:bool = False
        self._transformed_mon_list:List[EnemyPkmn] = []
        self._original_enemy_mon_list:List[EnemyPkmn] = []
        self._enemy_setup_move_list:List[str] = []
        self._enemy_field_move_list:List[str] = []

        self._mimic_options:List[str] = []
        self._player_mimic_selection:str = ""
        self._custom_move_data:List[Dict[str, Dict[str, str]]] = []
        self._cached_definition_order = []
        self._weather = None
        # mon_idx where the current weather was activated (from a move toggle). None
        # means the weather was not scoped to a move (legacy / external source) and
        # applies to all matchups.
        self._weather_source_mon_idx:int = None
        # Per-side dicts: { const.SCREEN_REFLECT / SCREEN_LIGHT_SCREEN : mon_idx }.
        # A screen applies from its recorded mon_idx onward in the battle.
        self._player_screens:Dict[str, int] = {}
        self._enemy_screens:Dict[str, int] = {}
        self._double_battle_flag = False
        self._stat_stage_setup:List[Dict[str, Dict[str, str]]] = []
        # Per-battle set of mon indices (in display order) that the user has
        # collapsed in the battle summary view.
        self._collapsed_mons:set = set()
        self._is_wild_battle:bool = False
        self._wild_min_dv_mons:List[EnemyPkmn] = []
        self._wild_max_dv_mons:List[EnemyPkmn] = []

        # NOTE: all of the state above this comment is considered the "true" state
        # The below state is all calculated based on values from the above state
        self._using_global_setup:bool = False
        self._player_stage_modifier:StageModifiers = None
        self._enemy_stage_modifier:StageModifiers = None
        # Per-mon-idx field statuses. Screens scope to mon_idx where activated;
        # the stat/speed display pulls from index 0 when needed.
        self._player_field_statuses:List[FieldStatus] = []
        self._enemy_field_statuses:List[FieldStatus] = []
        self._per_matchup_player_modifiers:List[StageModifiers] = []
        self._per_matchup_enemy_modifiers:List[StageModifiers] = []

        # Shadow vitamin tracking: {stat: {'shadow_id': int, 'original_id': int|None}}
        self._vitamin_shadows = {}

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

    def register_nonload_change(self, callback):
        self._nonload_change_callbacks.append(callback)
        return lambda: self._nonload_change_callbacks.remove(callback)

    def register_refresh(self, callback):
        self._refresh_callbacks.append(callback)
        return lambda: self._refresh_callbacks.remove(callback)

    #####
    # Event callbacks
    #####

    def _on_refresh(self):
        for callback in self._refresh_callbacks:
            try:
                callback()
            except Exception as e:
                logger.info(f"Removing refresh callback due to error: {callback}")
                logger.exception(e)

    def _on_nonload_change(self):
        for callback in self._nonload_change_callbacks:
            try:
                callback()
            except Exception:
                logger.info(f"Removing nonload_change callback due to error: {callback}")

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

    def update_weather(self, new_weather, source_mon_idx:int=None):
        self._weather = new_weather
        self._weather_source_mon_idx = source_mon_idx
        self._full_refresh()

    def update_enemy_setup_moves(self, new_setup_moves):
        self._enemy_setup_move_list = new_setup_moves
        self._full_refresh()

    def update_player_setup_moves(self, new_setup_moves):
        self._player_setup_move_list = new_setup_moves
        self._full_refresh()

    def update_enemy_field_moves(self, new_field_moves):
        self._enemy_field_move_list = new_field_moves
        self._full_refresh()

    def update_player_field_moves(self, new_field_moves):
        self._player_field_move_list = new_field_moves
        self._full_refresh()

    def update_player_transform(self, is_transformed):
        self._is_player_transformed = is_transformed
        self._full_refresh()

    def update_prefight_candies(self, num_candies):
        if self._event_group_id is None:
            return

        # Save transient state that should survive the route change cascade.
        # The main controller calls below trigger _on_route_change -> load_from_event,
        # which reloads all state from the (not yet saved) event definition,
        # wiping out setup moves, stat stage selections, etc.
        saved_player_setup = self._player_setup_move_list.copy()
        saved_player_field = self._player_field_move_list.copy()
        saved_enemy_setup = self._enemy_setup_move_list.copy()
        saved_enemy_field = self._enemy_field_move_list.copy()
        saved_stat_stage = copy.deepcopy(self._stat_stage_setup)
        saved_custom_data = copy.deepcopy(self._custom_move_data)
        saved_weather = self._weather
        saved_weather_source_mon_idx = self._weather_source_mon_idx
        saved_player_screens = copy.deepcopy(self._player_screens)
        saved_enemy_screens = copy.deepcopy(self._enemy_screens)
        saved_mimic = self._mimic_selection
        saved_transformed = self._is_player_transformed
        saved_double = self._double_battle_flag

        # Suppress the _full_refresh that load_from_event triggers during the
        # route change cascade — we will run a single _full_refresh at the end
        # once transient state has been restored.
        self._suppress_refresh = True
        try:
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
                if num_candies <= 0:
                    self._main_controller.delete_events([prev_event.group_id])
                else:
                    self._main_controller.update_existing_event(
                        prev_event.group_id,
                        EventDefinition(rare_candy=RareCandyEventDefinition(amount=num_candies)),
                    )
        finally:
            self._suppress_refresh = False

        # Restore transient state (overwritten by the route change -> load_from_event cascade).
        # The player mon list was correctly rebuilt with the new level from rare candies;
        # we just need the user's setup moves and stat stage selections back.
        self._player_setup_move_list = saved_player_setup
        self._player_field_move_list = saved_player_field
        self._enemy_setup_move_list = saved_enemy_setup
        self._enemy_field_move_list = saved_enemy_field
        self._stat_stage_setup = saved_stat_stage
        self._custom_move_data = saved_custom_data
        self._weather = saved_weather
        self._weather_source_mon_idx = saved_weather_source_mon_idx
        self._player_screens = saved_player_screens
        self._enemy_screens = saved_enemy_screens
        self._mimic_selection = saved_mimic
        self._is_player_transformed = saved_transformed
        self._double_battle_flag = saved_double

        self._full_refresh()

    def _find_existing_prefight_hold_event(self):
        """Walk backwards from the current battle, skipping rare-candy events,
        looking for an existing prefight Hold event. Returns
        (hold_event, anchor_event) where hold_event is the existing Hold (or
        None) and anchor_event is the closest preceding candy event (used as
        the insertion target so a newly-inserted Hold lands before any candy
        events that already precede the battle, preserving the candy lookup)."""
        if self._event_group_id is None:
            return None, None

        cursor_id = self._event_group_id
        anchor_event = None
        while True:
            prev_event = self._main_controller.get_previous_event(cursor_id, enabled_only=True)
            if prev_event is None or prev_event.event_definition is None:
                return None, anchor_event
            if prev_event.event_definition.rare_candy is not None:
                anchor_event = prev_event
                cursor_id = prev_event.group_id
                continue
            if prev_event.event_definition.hold_item is not None:
                return prev_event, anchor_event
            return None, anchor_event

    def _maybe_insert_find_for_held_item(self, item_name, init_state, insert_before_id):
        """If *item_name* is not already in the bag at *init_state*, insert a
        free 'Find Item' event for one of it immediately before *insert_before_id*.
        No-ops on bad input or when the item is already present."""
        if not item_name:
            return
        if init_state is None or init_state.inventory is None:
            return
        try:
            already_in_bag = any(
                bag_item.base_item is not None and bag_item.base_item.name == item_name
                for bag_item in init_state.inventory.cur_items
            )
        except Exception:
            already_in_bag = False
        if already_in_bag:
            return
        try:
            self._main_controller.new_event(
                EventDefinition(
                    item_event_def=InventoryEventDefinition(
                        item_name=item_name,
                        item_amount=1,
                        is_acquire=True,
                        with_money=False,
                    )
                ),
                insert_before=insert_before_id,
                do_select=False,
            )
        except Exception as e:
            logger.error(f"Failed to insert auto Find event for held item {item_name!r}: {e}")

    def update_player_held_item(self, new_item):
        """Insert/update/delete a prefight Hold event for the currently-loaded
        battle. Mirrors the transient-state save/restore pattern from
        update_prefight_candies, since the route-change cascade calls
        load_from_event which would otherwise wipe transient battle state."""
        if self._event_group_id is None:
            return

        # Re-entry guard: a single QComboBox selection can fire both
        # `activated` and `editingFinished` signals, and the second may fire
        # synchronously while we're still mutating the route in this method.
        # Without this guard, the re-entrant call sees a partially-inserted
        # state (Find present, Hold not yet) and creates a duplicate Hold.
        if getattr(self, "_held_item_update_in_flight", False):
            return

        normalized = new_item.strip() if isinstance(new_item, str) else None
        if not normalized or normalized == const.NO_ITEM or normalized == "None":
            normalized = None

        # Early no-op: the controller's view of the player's current held item
        # already matches the request. Cheap and avoids any walking/inserting.
        current_held = ""
        if self._original_player_mon_list:
            cur = self._original_player_mon_list[0].held_item
            if cur and cur != "None" and cur != const.NO_ITEM:
                current_held = cur
        if current_held == (normalized or ""):
            return

        existing_hold, anchor_event = self._find_existing_prefight_hold_event()

        # No-op when nothing actually changes
        if existing_hold is not None:
            if existing_hold.event_definition.hold_item.item_name == normalized:
                return
        elif normalized is None:
            return

        self._held_item_update_in_flight = True
        try:
            # Save transient state that the route-change cascade would clobber.
            saved_player_setup = self._player_setup_move_list.copy()
            saved_player_field = self._player_field_move_list.copy()
            saved_enemy_setup = self._enemy_setup_move_list.copy()
            saved_enemy_field = self._enemy_field_move_list.copy()
            saved_stat_stage = copy.deepcopy(self._stat_stage_setup)
            saved_custom_data = copy.deepcopy(self._custom_move_data)
            saved_weather = self._weather
            saved_weather_source_mon_idx = self._weather_source_mon_idx
            saved_player_screens = copy.deepcopy(self._player_screens)
            saved_enemy_screens = copy.deepcopy(self._enemy_screens)
            saved_mimic = self._mimic_selection
            saved_transformed = self._is_player_transformed
            saved_double = self._double_battle_flag

            self._suppress_refresh = True
            try:
                if existing_hold is not None:
                    if normalized is None:
                        self._main_controller.delete_events([existing_hold.group_id])
                    else:
                        # Auto-create a Find event for the new item if it isn't
                        # already in the bag at the existing hold's position.
                        self._maybe_insert_find_for_held_item(
                            normalized, existing_hold.init_state, existing_hold.group_id
                        )
                        self._main_controller.update_existing_event(
                            existing_hold.group_id,
                            EventDefinition(hold_item=HoldItemEventDefinition(normalized)),
                        )
                else:
                    insert_target = anchor_event.group_id if anchor_event is not None else self._event_group_id
                    # The new hold will share insert_target's init_state, so we
                    # check that inventory and add a Find first if needed. Inserting
                    # the Find before the same target preserves [..., Find, Hold, target].
                    target_event = self._main_controller.get_event_by_id(insert_target)
                    target_init_state = target_event.init_state if target_event is not None else None
                    self._maybe_insert_find_for_held_item(
                        normalized, target_init_state, insert_target
                    )
                    self._main_controller.new_event(
                        EventDefinition(hold_item=HoldItemEventDefinition(normalized)),
                        insert_before=insert_target,
                        do_select=False,
                    )
            finally:
                self._suppress_refresh = False

            # Restore transient state.
            self._player_setup_move_list = saved_player_setup
            self._player_field_move_list = saved_player_field
            self._enemy_setup_move_list = saved_enemy_setup
            self._enemy_field_move_list = saved_enemy_field
            self._stat_stage_setup = saved_stat_stage
            self._custom_move_data = saved_custom_data
            self._weather = saved_weather
            self._weather_source_mon_idx = saved_weather_source_mon_idx
            self._player_screens = saved_player_screens
            self._enemy_screens = saved_enemy_screens
            self._mimic_selection = saved_mimic
            self._is_player_transformed = saved_transformed
            self._double_battle_flag = saved_double

            self._full_refresh()
        finally:
            self._held_item_update_in_flight = False

    def get_held_item_options(self):
        """Return the list of selectable items for the held item dropdown.
        First entry is empty (== "no held item")."""
        try:
            items = current_gen_info().item_db().get_filtered_names(item_type=const.ITEM_TYPE_ALL_ITEMS)
        except Exception:
            items = []
        return [""] + sorted(items)

    def get_player_battle_hp(self):
        if not self._original_player_mon_list:
            return 0
        return self._original_player_mon_list[0].cur_stats.hp

    def get_player_battle_speed(self):
        if not self._original_player_mon_list:
            return 0
        return self._original_player_mon_list[0].cur_stats.speed

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

        if best_move_idx is not None:
            logger.info(f"Best move for pkmn_idx={pkmn_idx}, is_player={is_player_mon}: idx={best_move_idx}, name={best_move.name}")
        else:
            logger.warning(f"No best move found for pkmn_idx={pkmn_idx}, is_player={is_player_mon}, strat={config.get_player_highlight_strategy() if is_player_mon else config.get_enemy_highlight_strategy()}")


    def _full_refresh(self, is_load=False):
        # Skip when an outer operation (e.g. candy update) is going to call
        # _full_refresh explicitly after restoring transient state.
        if getattr(self, '_suppress_refresh', False):
            return

        # Once the "true" state of the current battle has been updated, recalculate all the derived properties

        # Check if we're using global setup (which overrides per-move setup)
        # If ANY global setup is used on either side, disable per-move dropdowns for BOTH sides
        using_global_player_setup = len(self._player_setup_move_list) > 0 and any(m for m in self._player_setup_move_list)
        using_global_enemy_setup = len(self._enemy_setup_move_list) > 0 and any(m for m in self._enemy_setup_move_list)
        self._using_global_setup = using_global_player_setup or using_global_enemy_setup

        # Calculate global stage modifiers (used when global setup is enabled)
        self._player_stage_modifier = self._calc_stage_modifier(self._player_setup_move_list)
        self._enemy_stage_modifier = self._calc_stage_modifier(self._enemy_setup_move_list)
        # Per-mon-idx field statuses: weather/screen toggles scope to source mon_idx
        num_matchups = len(self._original_player_mon_list)
        self._player_field_statuses = [self._calc_field_status(True, mon_idx) for mon_idx in range(num_matchups)]
        self._enemy_field_statuses = [self._calc_field_status(False, mon_idx) for mon_idx in range(num_matchups)]

        # Calculate per-matchup stage modifiers based on stat_stage_setup
        # This is separate from global setup - if global setup is used, per-move setup is disabled
        self._per_matchup_player_modifiers = []
        self._per_matchup_enemy_modifiers = []

        if not self._using_global_setup:
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
                player_stats = player_mon.get_battle_stats(cur_player_stage_mod, mon_field=self._player_field_statuses[mon_idx])

            enemy_mon = self._original_enemy_mon_list[mon_idx]
            enemy_stats = enemy_mon.get_battle_stats(cur_enemy_stage_mod, mon_field=self._enemy_field_statuses[mon_idx])

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
            # Add test moves (player moves 5-8) - always calculate if any test moves are defined
            # so data is ready when display is enabled
            #####
            test_moves = self._main_controller.get_raw_route().test_moves
            has_test_moves = any(test_moves[i] and test_moves[i].strip() for i in range(min(4, len(test_moves))))
            if has_test_moves or config.get_test_moves_enabled():
                for test_idx in range(4):
                    if test_idx < len(test_moves) and test_moves[test_idx] and test_moves[test_idx].strip():
                        test_move_name = test_moves[test_idx].strip()
                        test_move_data = self._recalculate_single_move(mon_idx, True, test_move_name)
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
        current_weather = self._get_weather_for_mon_idx(mon_idx)
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
            attacking_field_status = self._player_field_statuses[mon_idx] if mon_idx < len(self._player_field_statuses) else FieldStatus()
            defending_mon = self._original_enemy_mon_list[mon_idx]
            defending_mon_stats = None
            if mon_idx < len(self._per_matchup_enemy_modifiers):
                defending_stage_modifiers = self._per_matchup_enemy_modifiers[mon_idx]
            else:
                defending_stage_modifiers = self._enemy_stage_modifier
            defending_field_status = self._enemy_field_statuses[mon_idx] if mon_idx < len(self._enemy_field_statuses) else FieldStatus()
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
            attacking_field_status = self._enemy_field_statuses[mon_idx] if mon_idx < len(self._enemy_field_statuses) else FieldStatus()
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
            defending_field_status = self._player_field_statuses[mon_idx] if mon_idx < len(self._player_field_statuses) else FieldStatus()
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
        elif move.name == const.NATURAL_GIFT_MOVE_NAME:
            natural_gift_data = current_gen_info().get_natural_gift(attacking_mon.held_item)
            if natural_gift_data is not None:
                natural_gift_type, natural_gift_base_power = natural_gift_data
                move_display_name = f"{move.name} ({natural_gift_type}: {natural_gift_base_power})"

        if move_display_name is None:
            move_display_name = move.name

        custom_data_selection = self._custom_move_data[mon_idx][custom_lookup_key].get(move_name)
        custom_data_options = current_gen_info().get_move_custom_data(move.name)
        if custom_data_options is None and const.FLAVOR_MULTI_HIT in move.attack_flavor:
            custom_data_options = const.MULTI_HIT_CUSTOM_DATA

        if custom_data_options is None:
            custom_data_selection = None
        elif custom_data_selection not in custom_data_options:
            custom_data_selection = custom_data_options[0]

        # For wild pokemon battles, calculate against both 0-DV and 15-DV
        # variants to get the true min/max damage range across all possible DVs.
        if self._is_wild_battle and mon_idx < len(self._wild_min_dv_mons):
            wild_min = self._wild_min_dv_mons[mon_idx]  # 0 DVs
            wild_max = self._wild_max_dv_mons[mon_idx]  # 15 DVs

            if is_player_mon:
                # Player attacking: min = worst roll vs tankiest (15 DV), max = best roll vs squishiest (0 DV)
                ranges_tanky = current_gen_info().calculate_damage(
                    attacking_mon, move, wild_max,
                    attacking_stage_modifiers=attacking_stage_modifiers,
                    defending_stage_modifiers=defending_stage_modifiers,
                    attacking_field=attacking_field_status, defending_field=defending_field_status,
                    custom_move_data=custom_data_selection, weather=current_weather,
                    is_double_battle=self._double_battle_flag,
                    attacking_battle_stats=attacking_mon_stats,
                )
                ranges_squishy = current_gen_info().calculate_damage(
                    attacking_mon, move, wild_min,
                    attacking_stage_modifiers=attacking_stage_modifiers,
                    defending_stage_modifiers=defending_stage_modifiers,
                    attacking_field=attacking_field_status, defending_field=defending_field_status,
                    custom_move_data=custom_data_selection, weather=current_weather,
                    is_double_battle=self._double_battle_flag,
                    attacking_battle_stats=attacking_mon_stats,
                )
                crit_tanky = current_gen_info().calculate_damage(
                    crit_mon, move, wild_max,
                    attacking_stage_modifiers=attacking_stage_modifiers,
                    defending_stage_modifiers=defending_stage_modifiers,
                    attacking_field=attacking_field_status, defending_field=defending_field_status,
                    custom_move_data=custom_data_selection, is_crit=True, weather=current_weather,
                    is_double_battle=self._double_battle_flag,
                    attacking_battle_stats=crit_mon_stats,
                )
                crit_squishy = current_gen_info().calculate_damage(
                    crit_mon, move, wild_min,
                    attacking_stage_modifiers=attacking_stage_modifiers,
                    defending_stage_modifiers=defending_stage_modifiers,
                    attacking_field=attacking_field_status, defending_field=defending_field_status,
                    custom_move_data=custom_data_selection, is_crit=True, weather=current_weather,
                    is_double_battle=self._double_battle_flag,
                    attacking_battle_stats=crit_mon_stats,
                )
                normal_ranges = DamageRange.merge_Pokemon_min_max(ranges_tanky, ranges_squishy)
                crit_ranges = DamageRange.merge_Pokemon_min_max(crit_tanky, crit_squishy)
                # Use 0-DV mon HP for kill range (best case for player)
                defending_mon = wild_min
            else:
                # Enemy attacking: min = weakest (0 DV) worst roll, max = strongest (15 DV) best roll
                ranges_weak = current_gen_info().calculate_damage(
                    wild_min, move, defending_mon,
                    attacking_stage_modifiers=attacking_stage_modifiers,
                    defending_stage_modifiers=defending_stage_modifiers,
                    attacking_field=attacking_field_status, defending_field=defending_field_status,
                    custom_move_data=custom_data_selection, weather=current_weather,
                    is_double_battle=self._double_battle_flag,
                    defending_battle_stats=defending_mon_stats,
                )
                ranges_strong = current_gen_info().calculate_damage(
                    wild_max, move, defending_mon,
                    attacking_stage_modifiers=attacking_stage_modifiers,
                    defending_stage_modifiers=defending_stage_modifiers,
                    attacking_field=attacking_field_status, defending_field=defending_field_status,
                    custom_move_data=custom_data_selection, weather=current_weather,
                    is_double_battle=self._double_battle_flag,
                    defending_battle_stats=defending_mon_stats,
                )
                crit_weak = current_gen_info().calculate_damage(
                    wild_min, move, defending_mon,
                    attacking_stage_modifiers=attacking_stage_modifiers,
                    defending_stage_modifiers=defending_stage_modifiers,
                    attacking_field=attacking_field_status, defending_field=defending_field_status,
                    custom_move_data=custom_data_selection, is_crit=True, weather=current_weather,
                    is_double_battle=self._double_battle_flag,
                    defending_battle_stats=defending_mon_stats,
                )
                crit_strong = current_gen_info().calculate_damage(
                    wild_max, move, defending_mon,
                    attacking_stage_modifiers=attacking_stage_modifiers,
                    defending_stage_modifiers=defending_stage_modifiers,
                    attacking_field=attacking_field_status, defending_field=defending_field_status,
                    custom_move_data=custom_data_selection, is_crit=True, weather=current_weather,
                    is_double_battle=self._double_battle_flag,
                    defending_battle_stats=defending_mon_stats,
                )
                normal_ranges = DamageRange.merge_Pokemon_min_max(ranges_weak, ranges_strong)
                crit_ranges = DamageRange.merge_Pokemon_min_max(crit_weak, crit_strong)
                attacking_mon = wild_max  # for crit rate calculation
        else:
            normal_ranges = current_gen_info().calculate_damage(
                attacking_mon,
                move,
                defending_mon,
                attacking_stage_modifiers=attacking_stage_modifiers,
                defending_stage_modifiers=defending_stage_modifiers,
                attacking_field=attacking_field_status,
                defending_field=defending_field_status,
                custom_move_data=custom_data_selection,
                weather=current_weather,
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
                custom_move_data=custom_data_selection,
                is_crit=True,
                weather=current_weather,
                is_double_battle=self._double_battle_flag,
                attacking_battle_stats=crit_mon_stats,
                defending_battle_stats=defending_mon_stats,
            )
        if normal_ranges is not None and crit_ranges is not None:
            if config.do_ignore_accuracy():
                accuracy = 100
            else:
                accuracy = current_gen_info().get_move_accuracy(attacking_mon, move, custom_data_selection, defending_mon, current_weather)
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

        # Stat stage dropdown options
        # For Mimic, when there's no real selection (or the selection isn't on
        # the enemy yet), the caller substitutes a placeholder move name (e.g.
        # "Leer") just so damage calc has something to chew on. In that case
        # we must NOT surface a stat-stage dropdown for the placeholder — the
        # second dropdown should only appear once the user actually picks a
        # setup move via the mimic dropdown.
        is_mimic_placeholder = (
            move_display_name == const.MIMIC_MOVE_NAME
            and move.name != self._mimic_selection
        )
        if self._using_global_setup or is_mimic_placeholder:
            stat_stage_options = None
            stat_stage_info = {'has_stat_effect': False}
            stat_stage_selection = "0"
        else:
            stat_stage_options = current_gen_info().move_db().get_stat_stage_dropdown_options(move.name)
            stat_stage_info = current_gen_info().move_db().get_stat_stage_info(move.name)
            stat_stage_selection = "0"
            if stat_stage_options is not None:
                stat_stage_selection = self._get_stat_stage_selection(mon_idx, is_player_mon, move.name)

        return MoveRenderInfo(
            move_display_name,
            move.attack_flavor,
            -1 if normal_ranges is None else normal_ranges.min_damage,
            -1 if normal_ranges is None else normal_ranges.max_damage,
            -1 if crit_ranges is None else crit_ranges.min_damage,
            -1 if crit_ranges is None else crit_ranges.max_damage,
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

        # Only clear shadow tracking when actually switching to a different
        # event group. Reloading the same event mid-update (e.g. from the
        # route-change cascade triggered by adjust_vitamin_for_stat itself)
        # must preserve shadows so subsequent +/- clicks find them.
        if event_group.group_id != self._event_group_id:
            self._vitamin_shadows = {}
        self._event_group_id = event_group.group_id
        trainer_def = event_group.event_definition.trainer_def
        trainer_obj = event_group.event_definition.get_first_trainer_obj()
        second_trainer_obj = event_group.event_definition.get_second_trainer_obj()

        self._trainer_name = trainer_def.trainer_name
        self._second_trainer_name = trainer_def.second_trainer_name
        self._weather = trainer_def.weather
        self._weather_source_mon_idx = trainer_def.weather_source_mon_idx
        self._player_screens = copy.deepcopy(trainer_def.player_screens) if trainer_def.player_screens else {}
        self._enemy_screens = copy.deepcopy(trainer_def.enemy_screens) if trainer_def.enemy_screens else {}
        self._double_battle_flag = trainer_obj.double_battle or second_trainer_obj is not None
        self._mimic_selection = trainer_def.mimic_selection
        self._is_player_transformed = trainer_def.transformed
        self._is_wild_battle = False
        self._wild_min_dv_mons = []
        self._wild_max_dv_mons = []
        self._player_setup_move_list = trainer_def.setup_moves.copy()
        self._player_field_move_list = trainer_def.player_field_moves.copy()
        self._player_stage_modifier = self._calc_stage_modifier(self._player_setup_move_list)
        self._enemy_setup_move_list = trainer_def.enemy_setup_moves.copy()
        self._enemy_field_move_list = trainer_def.enemy_field_moves.copy()
        self._enemy_stage_modifier = self._calc_stage_modifier(self._enemy_setup_move_list)
        # Per-mon-idx field statuses get rebuilt in _full_refresh below.
        self._cached_definition_order = [x.mon_order - 1 for x in event_group.event_definition.get_pokemon_list(definition_order=True)]
        if not trainer_def.custom_move_data:
            self._custom_move_data = []
            for _ in range(len(event_group.event_definition.get_pokemon_list())):
                self._custom_move_data.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
        else:
            self._custom_move_data = [copy.deepcopy(x.custom_move_data) for x in event_group.event_definition.get_pokemon_list()]
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

        # Convert collapsed_mons from definition order (storage) to display order (controller state)
        self._collapsed_mons = set()
        if trainer_def.collapsed_mons:
            stored = set(trainer_def.collapsed_mons)
            for display_idx, def_idx in enumerate(self._cached_definition_order):
                if def_idx in stored:
                    self._collapsed_mons.add(display_idx)

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

    def load_from_state(self, init_state:RouteState, enemy_mons:List[EnemyPkmn], trainer_name:str=None, is_wild:bool=False):
        if init_state is None or not enemy_mons:
            self.load_empty()
            return

        self._event_group_id = None
        self._trainer_name = trainer_name
        self._second_trainer_name = ""
        self._weather = const.WEATHER_NONE
        self._weather_source_mon_idx = None
        self._player_screens = {}
        self._enemy_screens = {}
        self._mimic_selection = ""
        self._is_player_transformed = False
        self._player_setup_move_list = []
        self._player_field_move_list = []
        self._enemy_setup_move_list = []
        self._enemy_field_move_list = []
        self._custom_move_data = []
        self._stat_stage_setup = []
        self._collapsed_mons = set()
        self._is_wild_battle = is_wild
        self._wild_min_dv_mons = []
        self._wild_max_dv_mons = []
        self._cached_definition_order = list(range(len(enemy_mons)))
        for _ in range(len(enemy_mons)):
            self._custom_move_data.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})
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

            if is_wild:
                self._wild_min_dv_mons.append(
                    current_gen_info().create_wild_pkmn(cur_enemy.name, cur_enemy.level, dv=0)
                )
                self._wild_max_dv_mons.append(
                    current_gen_info().create_wild_pkmn(cur_enemy.name, cur_enemy.level, dv=15)
                )

            cur_state = cur_state.defeat_pkmn(cur_enemy)[0]

        trainer_obj = current_gen_info().trainer_db().get_trainer(trainer_name)
        if trainer_obj is None:
            self._double_battle_flag = False
        else:
            self._double_battle_flag = trainer_obj.double_battle

        self._full_refresh(is_load=True)

    def load_empty(self):
        self._vitamin_shadows = {}
        self._event_group_id = None
        self._trainer_name = ""
        self._second_trainer_name = ""
        self._weather = const.WEATHER_NONE
        self._weather_source_mon_idx = None
        self._player_screens = {}
        self._enemy_screens = {}
        self._double_battle_flag = False
        self._mimic_selection = ""
        self._is_player_transformed = False
        self._is_wild_battle = False
        self._wild_min_dv_mons = []
        self._wild_max_dv_mons = []
        self._player_setup_move_list = []
        self._player_field_move_list = []
        self._enemy_setup_move_list = []
        self._enemy_field_move_list = []
        self._custom_move_data = []
        self._stat_stage_setup = []
        self._collapsed_mons = set()
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

        # Convert collapsed_mons from display order (controller state) back to definition order (storage)
        final_collapsed_mons = sorted({
            self._cached_definition_order[display_idx]
            for display_idx in self._collapsed_mons
            if 0 <= display_idx < len(self._cached_definition_order)
        })

        return TrainerEventDefinition(
            self._trainer_name,
            second_trainer_name=self._second_trainer_name,
            setup_moves=self._player_setup_move_list,
            player_field_moves=self._player_field_move_list,
            enemy_setup_moves=self._enemy_setup_move_list,
            enemy_field_moves=self._enemy_field_move_list,
            mimic_selection=self._mimic_selection,
            custom_move_data=final_custom_move_data,
            weather=self._weather,
            weather_source_mon_idx=self._weather_source_mon_idx,
            player_screens=copy.deepcopy(self._player_screens),
            enemy_screens=copy.deepcopy(self._enemy_screens),
            transformed=self._is_player_transformed,
            stat_stage_setup=final_stat_stage_setup,
            collapsed_mons=final_collapsed_mons,
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

        # For player moves, allow indices 0-7 when test moves exist or display is enabled (0-3 regular, 4-7 test)
        # For enemy moves, only allow 0-3
        if is_player_mon:
            test_moves = self._main_controller.get_raw_route().test_moves
            has_test_moves = any(test_moves[i] and test_moves[i].strip() for i in range(min(4, len(test_moves))))
            max_idx = 7 if (has_test_moves or config.get_test_moves_enabled()) else 3
        else:
            max_idx = 3

        if move_idx > max_idx:
            return None

        if move_idx >= len(cur_move_data[pkmn_idx]):
            return None

        return cur_move_data[pkmn_idx][move_idx]

    def get_weather(self) -> str:
        return self._weather

    def get_weather_for_move(self, move_name: str) -> str:
        """If *move_name* is a weather-inducing move whose weather is supported by
        the current generation, return that weather constant. Otherwise return None."""
        if not move_name:
            return None
        weather = const.WEATHER_MOVE_MAP.get(move_name)
        if weather is None:
            return None
        try:
            valid_weather = current_gen_info().get_valid_weather()
        except Exception:
            return None
        if weather not in valid_weather:
            return None
        return weather

    def toggle_weather_from_move(self, move_name: str, enabled: bool, mon_idx:int=None):
        """Set the active weather to *move_name*'s weather (when *enabled*),
        or clear it back to WEATHER_NONE (when not). No-op for non-weather moves
        or weather not supported by the current generation.

        When *mon_idx* is provided, the weather applies from that matchup onward
        (earlier matchups see WEATHER_NONE). When None, weather applies to all
        matchups (legacy behavior)."""
        weather = self.get_weather_for_move(move_name)
        if weather is None:
            return
        if enabled:
            self.update_weather(weather, source_mon_idx=mon_idx)
        else:
            # Only clear if the currently-active weather is the one this move
            # would have set; otherwise leave the existing weather alone.
            if self._weather == weather:
                self.update_weather(const.WEATHER_NONE, source_mon_idx=None)

    def get_weather_source_mon_idx(self) -> int:
        return self._weather_source_mon_idx

    def get_screen_for_move(self, move_name:str) -> str:
        """Returns the screen id (const.SCREEN_REFLECT / SCREEN_LIGHT_SCREEN) for a
        Reflect / Light Screen move, or None otherwise."""
        if not move_name:
            return None
        return const.SCREEN_MOVE_MAP.get(move_name)

    def get_screen_source_mon_idx(self, is_player:bool, screen_id:str) -> int:
        """Returns the mon_idx at which *screen_id* was activated for the given
        side, or None if inactive."""
        screens = self._player_screens if is_player else self._enemy_screens
        return screens.get(screen_id)

    def toggle_screen_from_move(self, move_name:str, enabled:bool, mon_idx:int, is_player:bool):
        """Activate or deactivate Reflect / Light Screen for the given side, scoped
        to *mon_idx* and later matchups. No-op for non-screen moves."""
        screen_id = self.get_screen_for_move(move_name)
        if screen_id is None:
            return
        screens = self._player_screens if is_player else self._enemy_screens
        if enabled:
            screens[screen_id] = mon_idx
        else:
            # Only clear when the stored source matches the mon_idx we're toggling
            # off — keeps a distant toggle from clobbering an earlier activation.
            if screens.get(screen_id) == mon_idx:
                screens.pop(screen_id, None)
        self._full_refresh()

    def get_player_setup_moves(self) -> List[str]:
        return self._player_setup_move_list

    def get_enemy_setup_moves(self) -> List[str]:
        return self._enemy_setup_move_list

    def get_player_field_moves(self) -> List[str]:
        return self._player_field_move_list

    def get_enemy_field_moves(self) -> List[str]:
        return self._enemy_field_move_list

    def is_double_battle(self) -> bool:
        return self._double_battle_flag

    def is_player_transformed(self) -> bool:
        return self._is_player_transformed

    def is_mon_collapsed(self, mon_idx:int) -> bool:
        return mon_idx in self._collapsed_mons

    def update_mon_collapsed(self, mon_idx:int, collapsed:bool):
        already = mon_idx in self._collapsed_mons
        if collapsed == already:
            return
        if collapsed:
            self._collapsed_mons.add(mon_idx)
        else:
            self._collapsed_mons.discard(mon_idx)
        # Persist via the standard "non-load change" save path. Wild battles
        # have no event_group_id so this is a no-op write for them, but the
        # in-memory per-battle state still tracks correctly.
        self._on_nonload_change()

    def get_player_held_item(self) -> str:
        if self._original_player_mon_list:
            return self._original_player_mon_list[0].held_item or ""
        return ""

    @staticmethod
    def _is_move_better(new_move:MoveRenderInfo, prev_move:MoveRenderInfo, strat:str, other_mon:PkmnRenderInfo) -> bool:
        if (
            strat is None or
            not isinstance(strat, str) or
            strat == const.HIGHLIGHT_NONE or
            strat not in const.ALL_HIGHLIGHT_STRATS
        ):
            return False

        if new_move is None or new_move.min_damage == -1:
            return False

        if prev_move is None or prev_move.min_damage == -1:
            return True

        # special case for recharge moves (e.g. Hyper Beam)
        # If a one-hit is not possible without a crit, always prefer other damage dealing moves
        if const.FLAVOR_RECHARGE in new_move.attack_flavor and new_move.max_damage < other_mon.defending_mon_hp:
            return False
        elif const.FLAVOR_RECHARGE in prev_move.attack_flavor and prev_move.max_damage < other_mon.defending_mon_hp:
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
        return new_move.max_damage > prev_move.max_damage

    @staticmethod
    def _calc_stage_modifier(move_list) -> StageModifiers:
        result = StageModifiers()

        for cur_move in move_list:
            result = result.apply_stat_mod(current_gen_info().move_db().get_stat_mod(cur_move))

        return result

    def _calc_field_status(self, is_player:bool, mon_idx:int=0) -> FieldStatus:
        result = FieldStatus()

        if is_player:
            move_list = self._player_field_move_list + self._player_setup_move_list
            screens = self._player_screens
        else:
            move_list = self._enemy_field_move_list + self._enemy_setup_move_list
            screens = self._enemy_screens

        # Legacy trainer-level field/setup moves still apply globally (preserves
        # existing route behavior). The new per-mon-idx screen toggles below add
        # on top for matchup-scoped activation.
        for cur_move_name in move_list:
            result = result.apply_move(cur_move_name)

        # Apply per-mon-idx screen toggles (active when mon_idx >= source_idx).
        reflect_idx = screens.get(const.SCREEN_REFLECT)
        if reflect_idx is not None and mon_idx >= reflect_idx:
            result.reflect = True
        light_screen_idx = screens.get(const.SCREEN_LIGHT_SCREEN)
        if light_screen_idx is not None and mon_idx >= light_screen_idx:
            result.light_screen = True

        return result

    def _get_weather_for_mon_idx(self, mon_idx:int) -> str:
        """Returns the weather in effect for the given matchup. When the weather
        has a source mon_idx (toggled from a move), earlier matchups see
        WEATHER_NONE; otherwise the stored weather applies to all matchups."""
        if self._weather is None:
            return const.WEATHER_NONE
        if self._weather_source_mon_idx is None:
            return self._weather
        if mon_idx < self._weather_source_mon_idx:
            return const.WEATHER_NONE
        return self._weather

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
        """
        num_matchups = len(self._original_player_mon_list)
        player_modifiers = []
        enemy_modifiers = []

        # Track persistent modifiers that carry across matchups
        persistent_player_modifier = StageModifiers()

        move_db = current_gen_info().move_db()
        gen = current_gen_info().get_generation()
        prev_player_level = None

        for mon_idx in range(num_matchups):
            # Gen 1 specific: Check if player leveled up - if so, reset badge boosts
            if gen == 1 and mon_idx < len(self._original_player_mon_list):
                cur_player_level = self._original_player_mon_list[mon_idx].level
                if prev_player_level is not None and cur_player_level > prev_player_level:
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

                    if not is_guaranteed and not is_damaging:
                        continue

                    if targets_self and not is_damaging:
                        # Self-targeting status move - apply to player, persists
                        if stat_info.get('is_belly_drum', False):
                            # Belly Drum: dropdown value is the desired ATK stage directly
                            persistent_player_modifier = persistent_player_modifier.set_attack_stage(count)
                            cur_player_modifier = cur_player_modifier.set_attack_stage(count)
                        else:
                            stat_mods = move_db.get_stat_mod_for_target(move_name, target_self=True)
                            for _ in range(count):
                                persistent_player_modifier = persistent_player_modifier.apply_stat_mod(stat_mods)
                                cur_player_modifier = cur_player_modifier.apply_stat_mod(stat_mods)
                    elif not targets_self and not is_damaging:
                        # Opponent-targeting status move - apply to enemy, current matchup only
                        stat_mods = move_db.get_stat_mod_for_target(move_name, target_self=False)
                        for _ in range(count):
                            cur_enemy_modifier = cur_enemy_modifier.apply_stat_mod(stat_mods)
                    elif is_damaging:
                        # Damage-dealing move with stat effect - apply to enemy, current matchup
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

                    if not is_guaranteed and not is_damaging:
                        continue

                    if targets_self and not is_damaging:
                        # Enemy self-targeting: only affects current matchup
                        if stat_info.get('is_belly_drum', False):
                            cur_enemy_modifier = cur_enemy_modifier.set_attack_stage(count)
                        else:
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
        if prev_event is None or prev_event.event_definition.rare_candy is None:
            return 0

        return prev_event.event_definition.rare_candy.amount

    def get_vitamins_used_per_stat(self):
        """Return a dict {stat: count} of vitamins used (by enabled events)
        strictly before the currently-loaded battle. Returns zeros when no
        battle is loaded or the route isn't available."""
        counts = {
            const.HP: 0,
            const.ATK: 0,
            const.DEF: 0,
            const.SPA: 0,
            const.SPD: 0,
            const.SPE: 0,
        }
        if self._event_group_id is None:
            return counts
        try:
            raw_route = self._main_controller.get_raw_route()
        except Exception:
            return counts
        if raw_route is None or raw_route.root_folder is None:
            return counts

        gen_info = current_gen_info()
        found = [False]

        def _walk(folder):
            for child in folder.children:
                if found[0]:
                    return
                if isinstance(child, EventFolder):
                    _walk(child)
                elif isinstance(child, EventGroup):
                    if child.group_id == self._event_group_id:
                        found[0] = True
                        return
                    if not child.is_enabled():
                        continue
                    vit_def = child.event_definition.vitamin
                    if vit_def is None:
                        continue
                    try:
                        boosted = gen_info.get_stats_boosted_by_vitamin(vit_def.vitamin)
                    except Exception:
                        continue
                    amount = getattr(vit_def, "amount", 1) or 1
                    for stat in boosted:
                        if stat in counts:
                            counts[stat] += amount

        _walk(raw_route.root_folder)
        return counts

    # ------------------------------------------------------------------
    # Vitamin adjustment from the controls bar
    # ------------------------------------------------------------------

    def _stat_to_vitamin_name(self, stat):
        """Map a stat constant (e.g. const.ATK) to the vitamin that boosts it."""
        gen_info = current_gen_info()
        for vit_name in gen_info.get_valid_vitamins():
            try:
                if stat in gen_info.get_stats_boosted_by_vitamin(vit_name):
                    return vit_name
            except Exception:
                continue
        return None

    def _find_last_vitamins(self, target_stat):
        """Walk the route before the current battle and return
        (last_enabled_vitamin_for_stat, last_vitamin_any_type).
        Both are EventGroup or None."""
        if self._event_group_id is None:
            return None, None
        try:
            root = self._main_controller.get_raw_route().root_folder
        except Exception:
            return None, None
        gen_info = current_gen_info()
        last_for_stat = None
        last_any = None
        found = [False]

        def _walk(folder):
            nonlocal last_for_stat, last_any
            for child in folder.children:
                if found[0]:
                    return
                if isinstance(child, EventFolder):
                    _walk(child)
                elif isinstance(child, EventGroup):
                    if child.group_id == self._event_group_id:
                        found[0] = True
                        return
                    vit_def = child.event_definition.vitamin
                    if vit_def is not None:
                        last_any = child
                        if child.is_enabled():
                            try:
                                if target_stat in gen_info.get_stats_boosted_by_vitamin(vit_def.vitamin):
                                    last_for_stat = child
                            except Exception:
                                pass

        _walk(root)
        return last_for_stat, last_any

    def _save_transient_state(self):
        return dict(
            player_setup=self._player_setup_move_list.copy(),
            player_field=self._player_field_move_list.copy(),
            enemy_setup=self._enemy_setup_move_list.copy(),
            enemy_field=self._enemy_field_move_list.copy(),
            stat_stage=copy.deepcopy(self._stat_stage_setup),
            custom_data=copy.deepcopy(self._custom_move_data),
            weather=self._weather,
            weather_source_mon_idx=self._weather_source_mon_idx,
            player_screens=copy.deepcopy(self._player_screens),
            enemy_screens=copy.deepcopy(self._enemy_screens),
            mimic=self._mimic_selection,
            transformed=self._is_player_transformed,
            double=self._double_battle_flag,
        )

    def _restore_transient_state(self, saved):
        self._player_setup_move_list = saved['player_setup']
        self._player_field_move_list = saved['player_field']
        self._enemy_setup_move_list = saved['enemy_setup']
        self._enemy_field_move_list = saved['enemy_field']
        self._stat_stage_setup = saved['stat_stage']
        self._custom_move_data = saved['custom_data']
        self._weather = saved['weather']
        self._weather_source_mon_idx = saved.get('weather_source_mon_idx')
        self._player_screens = saved.get('player_screens', {}) or {}
        self._enemy_screens = saved.get('enemy_screens', {}) or {}
        self._mimic_selection = saved['mimic']
        self._is_player_transformed = saved['transformed']
        self._double_battle_flag = saved['double']

    def adjust_vitamin_for_stat(self, stat, delta, _skip_refresh=False):
        """Increment or decrement the vitamin count for *stat* by *delta*.

        On first interaction, the original vitamin event is disabled and a
        shadow replacement is created right after it.  Subsequent calls adjust
        the shadow.  When the shadow reaches 0 the original is re-enabled.

        Pass ``_skip_refresh=True`` when batching multiple adjustments — the
        caller is responsible for calling ``_full_refresh()`` afterwards.
        """
        if self._event_group_id is None:
            return

        vit_name = self._stat_to_vitamin_name(stat)
        if vit_name is None:
            return

        saved = self._save_transient_state()

        # Suppress the _full_refresh that the route change cascade would
        # otherwise trigger via load_from_event — we will run a single
        # _full_refresh at the end once transient state has been restored.
        # Without this, every +/- click pays the cost of a full battle
        # recalculation during the cascade, plus another one at the end.
        self._suppress_refresh = True
        try:
            shadow_info = self._vitamin_shadows.get(stat)

            if shadow_info and shadow_info.get('shadow_id'):
                # ---- existing shadow: just adjust it ----
                shadow_event = self._main_controller.get_event_by_id(shadow_info['shadow_id'])
                if shadow_event and shadow_event.event_definition.vitamin:
                    new_amount = shadow_event.event_definition.vitamin.amount + delta
                    if new_amount > 0:
                        self._main_controller.update_existing_event(
                            shadow_info['shadow_id'],
                            EventDefinition(vitamin=VitaminEventDefinition(vit_name, new_amount)),
                        )
                    else:
                        # Shadow reaches 0 — delete it and re-enable original
                        self._main_controller.delete_events([shadow_info['shadow_id']])
                        orig_id = shadow_info.get('original_id')
                        if orig_id:
                            orig = self._main_controller.get_event_by_id(orig_id)
                            if orig:
                                orig.set_enabled_status(True)
                                self._main_controller.update_existing_event(orig.group_id, orig.event_definition)
                        del self._vitamin_shadows[stat]
                else:
                    del self._vitamin_shadows[stat]
            else:
                # ---- no shadow yet ----
                last_for_stat, last_any = self._find_last_vitamins(stat)

                if last_for_stat:
                    orig_amount = last_for_stat.event_definition.vitamin.amount
                    new_amount = orig_amount + delta
                    if new_amount > 0:
                        # Disable original, create shadow
                        last_for_stat.set_enabled_status(False)
                        self._main_controller.update_existing_event(
                            last_for_stat.group_id, last_for_stat.event_definition
                        )
                        shadow_id = self._main_controller.new_event(
                            EventDefinition(vitamin=VitaminEventDefinition(vit_name, new_amount)),
                            insert_after=last_for_stat.group_id,
                            do_select=False,
                        )
                        self._vitamin_shadows[stat] = {
                            'shadow_id': shadow_id,
                            'original_id': last_for_stat.group_id,
                        }
                    else:
                        # Disable the original entirely (user wants 0 of this vitamin)
                        last_for_stat.set_enabled_status(False)
                        self._main_controller.update_existing_event(
                            last_for_stat.group_id, last_for_stat.event_definition
                        )
                        self._vitamin_shadows[stat] = {
                            'shadow_id': None,
                            'original_id': last_for_stat.group_id,
                        }
                elif delta > 0:
                    # No existing vitamin for this stat — create one fresh
                    if last_any:
                        insert_kw = dict(insert_after=last_any.group_id)
                    else:
                        insert_kw = dict(insert_before=self._event_group_id)
                    shadow_id = self._main_controller.new_event(
                        EventDefinition(vitamin=VitaminEventDefinition(vit_name, delta)),
                        do_select=False,
                        **insert_kw,
                    )
                    self._vitamin_shadows[stat] = {
                        'shadow_id': shadow_id,
                        'original_id': None,
                    }
                # else: delta <= 0 and no vitamin exists — nothing to do
        finally:
            self._suppress_refresh = False

        self._restore_transient_state(saved)
        if not _skip_refresh:
            self._full_refresh()

    def take_screenshot(self, bbox):
        if not self._trainer_name:
            self._main_controller.send_message(f"No active battle to screenshot")
            return

        self._main_controller.take_screenshot(
            self._trainer_name.replace(" ", "_"),
            bbox
        )

    def get_test_moves_enabled(self):
        return config.get_test_moves_enabled()

    def get_test_moves(self):
        test_moves = self._main_controller.get_raw_route().test_moves.copy()
        while len(test_moves) < 4:
            test_moves.append("")
        return test_moves[:4]

    def update_test_move(self, slot_idx, move_name):
        if slot_idx < 0 or slot_idx >= 4:
            return
        test_moves = self._main_controller.get_raw_route().test_moves
        while len(test_moves) < 4:
            test_moves.append("")
        test_moves[slot_idx] = move_name
        self._main_controller.get_raw_route().test_moves = test_moves
        self._full_refresh()
        self._on_nonload_change()

    def get_show_move_highlights(self):
        return config.get_show_move_highlights() if hasattr(config, 'get_show_move_highlights') else False

    def get_move_highlight_state(self, mon_idx, move_idx, is_player_mon):
        highlights = getattr(self, '_move_highlights', {})
        battle_highlights = highlights.get(self._event_group_id, {})
        return battle_highlights.get((mon_idx, move_idx, is_player_mon), 0)

    def set_move_highlight_state(self, mon_idx, move_idx, is_player_mon, state):
        if not hasattr(self, '_move_highlights'):
            self._move_highlights = {}
        battle_highlights = self._move_highlights.setdefault(self._event_group_id, {})
        battle_highlights[(mon_idx, move_idx, is_player_mon)] = state
        self._on_refresh()
        self._on_nonload_change()

    def update_move_highlight(self, pkmn_idx, move_idx, is_player_mon, reset=False):
        """Cycle through highlight states: 0 -> 1 -> 2 -> 3 -> 0. If reset=True, set to 0."""
        current_state = self.get_move_highlight_state(pkmn_idx, move_idx, is_player_mon)
        if reset:
            new_state = 0
        else:
            new_state = (current_state + 1) % 4
        self.set_move_highlight_state(pkmn_idx, move_idx, is_player_mon, new_state)

    def _get_stat_stage_selection(self, pkmn_idx: int, is_player_mon: bool, move_name: str) -> str:
        if pkmn_idx < 0 or pkmn_idx >= len(self._stat_stage_setup):
            return "0"
        lookup_key = const.PLAYER_KEY if is_player_mon else const.ENEMY_KEY
        return self._stat_stage_setup[pkmn_idx].get(lookup_key, {}).get(move_name, "0")

    def update_stat_stage_setup(self, pkmn_idx, move_idx, is_player_mon, new_value):
        """Update stat stage setup for a specific move. Triggers recalculation."""
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

            while len(self._stat_stage_setup) <= pkmn_idx:
                self._stat_stage_setup.append({const.PLAYER_KEY: {}, const.ENEMY_KEY: {}})

            if lookup_key not in self._stat_stage_setup[pkmn_idx]:
                self._stat_stage_setup[pkmn_idx][lookup_key] = {}

            self._stat_stage_setup[pkmn_idx][lookup_key][move_name] = new_value
            self._full_refresh()
        except Exception as e:
            logger.error(f"encountered error updating stat stage setup: {pkmn_idx, move_idx, is_player_mon, new_value}")
            logger.exception(e)
