from __future__ import annotations
import logging

from route_recording.game_recorders.gen_four.platinum_fsm import Machine, State, StateType
from route_recording.gamehook_client import GameHookProperty
from routing.route_events import BlackoutEventDefinition, EventDefinition, HealEventDefinition, HoldItemEventDefinition, SaveEventDefinition, TrainerEventDefinition, WildPkmnEventDefinition
from route_recording.game_recorders.gen_four.platinum_gamehook_constants import gh_gen_four_const
from pkmn.gen_4.gen_four_constants import gen_four_const
from utils.constants import const

logger = logging.getLogger(__name__)


class DelayedUpdate:
    def __init__(self, machine: Machine, delay:int):
        self.machine = machine
        self.base_delay = delay
        self.cur_delay = 0
        self.is_active = False
    
    def reset(self):
        self.cur_delay = 0
        self.is_active = False
    
    def tick(self):
        if self.is_active:
            if self.cur_delay > 0:
                self.cur_delay -= 1
            else:
                self.trigger()
    
    def begin_waiting(self, force_reset=True):
        if self.is_active and not force_reset:
            return
        self.cur_delay = self.base_delay
        self.is_active = True
    
    def trigger(self, force=False):
        if self.is_active or force:
            self.cur_delay = 0
            self.is_active = False
            self._update_helper()

    def _update_helper(self):
        raise NotImplementedError()


class WatchForResetState(State):
    def watch_for_reset(self, new_prop:GameHookProperty, prev_prop:GameHookProperty) -> StateType:
        if self.machine._player_id is not None:
            if new_prop.path == gh_gen_four_const.KEY_PLAYER_PLAYERID and new_prop.value == 0:
                return StateType.RESETTING
            elif new_prop.value == 0 and self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_PLAYERID).value == 0:
                return StateType.RESETTING
        return None


def auto_reset(transition_fn):
    def wrapper(*args, **kwargs):
        obj:WatchForResetState = args[0]
        result = obj.watch_for_reset(*args[1:], **kwargs)
        if result is not None:
            return result
        
        return transition_fn(*args, **kwargs)
    return wrapper


class WatchState(State):
    def __init__(self, machine: Machine):
        super().__init__(StateType.UNINITIALIZED, machine)
        self._is_waiting = False
        self._seconds_delay = 2
    
    def _on_enter(self, prev_state: State):
        self._is_waiting = False
        self._seconds_delay = 2
    
    def _on_exit(self, next_state: State):
        pass

    def transition(self, new_prop:GameHookProperty, prev_prop:GameHookProperty) -> StateType:
        if new_prop.path != gh_gen_four_const.KEY_GAMETIME_SECONDS:
            frame_val = self.machine._gamehook_client.get(gh_gen_four_const.KEY_GAMETIME_FRAMES).value
            logger.info(f"On Frame {frame_val:02} Changing {new_prop.path} from {prev_prop.value} to {new_prop.value}({type(new_prop.value)})")

        return self.state_type

# No Pokemon state
class UninitializedState(WatchForResetState):
    def __init__(self, machine: Machine):
        super().__init__(StateType.UNINITIALIZED, machine)
        self._is_waiting = False
        self._seconds_delay = 2
    
    def _on_enter(self, prev_state: State):
        self._is_waiting = False
        self._seconds_delay = 2
    
    def _on_exit(self, next_state: State):
        self.machine._player_id = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_PLAYERID).value
        # Shouldn't really happen ever, but if the player connects to an active game, but then resets the emulator
        # it's possible that we exit (due to transitioning to a reset state) while the player id is 0
        # If this happens, just ignore the update, let the ResettingState handle setting the player id
        if self.machine._player_id == 0:
            self.machine._player_id = None
        
        self.machine.update_all_cached_info()
    
    @auto_reset
    def transition(self, new_prop:GameHookProperty, prev_prop:GameHookProperty) -> StateType:
        if new_prop.path == gh_gen_four_const.KEY_GAMETIME_SECONDS:
            if self._seconds_delay <= 0:
                if (
                    self.machine._gamehook_client.get(gh_gen_four_const.META_STATE).value == 'Battle'
                ):
                    return StateType.BATTLE
                else:
                    return StateType.OVERWORLD
            elif not self._is_waiting and self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_PLAYERID).value != 0:
                self._is_waiting = True

            if self._is_waiting:
                self._seconds_delay -= 1

        return self.state_type


class ResettingState(State):
    def __init__(self, machine: Machine):
        super().__init__(StateType.RESETTING, machine)
        self._is_waiting = False
        self._seconds_delay = None
    
    def _on_enter(self, prev_state: State):
        self._is_waiting = False
        self._seconds_delay = 2
        self.machine._queue_new_event(EventDefinition(notes=gh_gen_four_const.RESET_FLAG))
    
    def _on_exit(self, next_state: State):
        new_player_id = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_PLAYERID).value
        if self.machine._player_id is None:
            self.machine._player_id = new_player_id
        elif self.machine._player_id != new_player_id:
            self.machine._controller.route_restarted()

        self.machine.update_all_cached_info()

    def transition(self, new_prop:GameHookProperty, prev_prop:GameHookProperty) -> StateType:
        if new_prop.path == gh_gen_four_const.KEY_PLAYER_PLAYERID:
            if prev_prop.value == 0 and new_prop.value != 0:
                self._is_waiting = True
        elif new_prop.path == gh_gen_four_const.KEY_GAMETIME_SECONDS:
            if not self._is_waiting:
                self._is_waiting = True
            elif self._seconds_delay <= 0:
                return StateType.OVERWORLD
            else:
                self._seconds_delay -= 1

        return self.state_type


class BattleState(WatchForResetState):
    BASE_DELAY = 3

    class DelayedBattleMovesUpdate(DelayedUpdate):
        def _update_helper(self):
            if self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_PLAYER_MON_PARTY_POS).value == 0:
                self.machine.update_team_cache()
                self.machine._move_cache_update(levelup_source=True)

    class DelayedBattleItemsUpdate(DelayedUpdate):
        def _update_helper(self):
            self.machine._item_cache_update()

    class DelayedHeldItemUpdate(DelayedUpdate):
        def configure_held_item(self, held_item:str):
            self._init_held_item = held_item

        def _update_helper(self):
            if self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_PLAYER_MON_PARTY_POS).value == 0:
                if self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_HELD_ITEM).value is None:
                    self.machine._queue_new_event(EventDefinition(hold_item=HoldItemEventDefinition(None, True)))
                elif self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_HELD_ITEM).value != self._init_held_item:
                    do_consume = self._init_held_item is not None
                    self._init_held_item = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_HELD_ITEM).value
                    self.machine._queue_new_event(
                        EventDefinition(
                            hold_item=HoldItemEventDefinition(
                                self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_HELD_ITEM).value,
                                do_consume
                            )
                        )
                    )

    class DelayedLevelUpdate(DelayedUpdate):
        def configure_level(self, original_level:int):
            self.original_level = original_level

        def _update_helper(self):
            if self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_PLAYER_MON_PARTY_POS).value == 0:
                new_level = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_LEVEL).value
                if new_level > self.original_level:
                    self.machine._solo_mon_levelup(new_level)
                    self.original_level = new_level

    class DelayedInitialization(DelayedUpdate):
        def __init__(self, machine, delay, init_fn):
            super().__init__(machine, delay)
            self.init_fn = init_fn

        def _update_helper(self):
            self.init_fn()

    def __init__(self, machine: Machine):
        super().__init__(StateType.BATTLE, machine)
        self.is_trainer_battle = None
        self._trainer_name = ""
        self._second_trainer_name = ""
        self._enemy_pos_lookup = {}
        self._trainer_event_created = False
        self._defeated_trainer_mons = []
        self._delayed_move_updater = self.DelayedBattleMovesUpdate(self.machine, self.BASE_DELAY)
        self._delayed_item_updater = self.DelayedBattleItemsUpdate(self.machine, self.BASE_DELAY)
        self._delayed_held_item_updater = self.DelayedHeldItemUpdate(self.machine, self.BASE_DELAY)
        self._delayed_levelup = self.DelayedLevelUpdate(self.machine, self.BASE_DELAY)
        self._delayed_initialization = self.DelayedInitialization(self.machine, self.BASE_DELAY, self._battle_ready)
        self._loss_detected = False
        self._cached_first_mon_species = ""
        self._cached_first_mon_level = 0
        self._cached_second_mon_species = ""
        self._cached_second_mon_level = 0
        self._exp_split = []
        self._enemy_mon_order = []
        self._friendship_data = []
        self._battle_started = False
        self._battle_finished = False
        self._is_double_battle = False
        # self._is_tutorial_battle = False
        self._initial_money = 0
        self._init_held_item = None
        self._solo_hp_zero = False
        self._team_hp_zero = False
        self._watching_for_map_change = False
        self._initial_map = None
    
    def _on_enter(self, prev_state: State):
        self._defeated_trainer_mons = []
        self._delayed_move_updater.reset()
        self._delayed_item_updater.reset()
        self._delayed_held_item_updater.reset()
        self._delayed_levelup.reset()
        self._delayed_initialization.reset()
        self.is_trainer_battle = None
        self._trainer_event_created = False
        self._loss_detected = False
        self._cached_first_mon_species = ""
        self._cached_first_mon_level = 0
        self._cached_second_mon_species = ""
        self._cached_second_mon_level = 0
        self._trainer_name = ""
        self._second_trainer_name = ""
        self._enemy_pos_lookup = {}
        self._exp_split = []
        self._enemy_mon_order = []
        self._friendship_data = []
        self._battle_started = False
        self._battle_finished = False
        self._is_double_battle = False
        self._multi_battle = False
        # self._is_tutorial_battle = False
        self._ally_id = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_ALLY_NUMBER).value
        logger.info(f"ally id: {self._ally_id}")
        self._initial_money = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MONEY).value
        self._init_held_item = None
        self._solo_hp_zero = False
        self._team_hp_zero = False
        self._watching_for_map_change = False
        self._initial_map = self.machine._gamehook_client.get(gh_gen_four_const.KEY_OVERWORLD_MAP).value
        
        # PID-based exp split tracking for double battles
        self._solo_mon_pid = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_PID).value
        self._current_ally_pid = None
        self._ally_hp_zero = False
        
        # NEW: Track by enemy PID instead of position
        self._enemy_pids_in_battle = []  # List of enemy PIDs (in order they appear in enemy team)
        self._enemy_pid_to_participating_player_pids = {}  # Map: enemy_pid -> set of player PIDs that participated
        self._enemy_pid_to_exp_split = {}  # Map: enemy_pid -> split count (set when exp is awarded)
        
        self._delayed_initialization.begin_waiting()
    
    def _get_num_enemy_trainer_pokemon(self):
        # Ideally, this should be pulled from a single property. However that mapped property doesn't work currently
        # so, for now, just iterate over enemy pokemon team species, and figure out how many non-empty team members are loaded
        result = 0
        for cur_key in gh_gen_four_const.ALL_KEYS_ENEMY_TEAM_SPECIES:
            if self.machine._gamehook_client.get(cur_key).value:
                result += 1
        if self.trainer_2 > 0:
            for cur_key in gh_gen_four_const.ALL_KEYS_ENEMY_2_TEAM_SPECIES:
                if self.machine._gamehook_client.get(cur_key).value:
                    result += 1
        return result

    def _get_enemy_pos_lookup(self):
        # because we have a weird default mon order for multi battles, need to create this to make sure we handle things appropriately
        if not self._second_trainer_name:
            # single trainer battles (single or double) are trivial. just create all 6 lookups
            return {x: x for x in range(6)}
        
        # just statically define the "real" order. Alternating between 2 trainers, taking their mons in order
        # however, there's no guarantee we need all of them, so calculate the ones we actually need
        real_order = [0, 3, 1, 4, 2, 5]
        result = {}
        next_pos_idx = 0
        for cur_key_idx in real_order:
            if self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_ENEMY_TEAM_SPECIES[cur_key_idx]).value:
                result[cur_key_idx] = next_pos_idx
                next_pos_idx += 1

        return result

    def _battle_ready(self):
        # to be called after battle is actually initialized
        battle_mode = self.machine._gamehook_client.get(gh_gen_four_const.KEY_TRAINER_BATTLE_FLAG).value
        if battle_mode is None or battle_mode == 'null':
            self._delayed_initialization.reset()
            return

        self.trainer_1 = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_TRAINER_A_NUMBER).value
        self.trainer_2 = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_TRAINER_B_NUMBER).value

        self._battle_started = True
        self._init_held_item = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_HELD_ITEM).value
        if not self.trainer_2:
            self._is_double_battle = False
        else:
            self._is_double_battle = True
            if self._ally_id != 0:
                self._multi_battle = True
        # self._is_tutorial_battle = self.machine._gamehook_client.get(gh_gen_four_const.KEY_TUTORIAL_BATTLE_FLAG).value
        self.is_trainer_battle = battle_mode
        self._delayed_levelup.configure_level(self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_LEVEL).value)

        # if self._is_tutorial_battle:
        #     logger.info(f"tutorial fight found")
        if self.is_trainer_battle == 'Trainer':
            logger.info(f"trainer battle found")
            self._trainer_name = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_TRAINER_A_NUMBER).value
            if self.machine.is_hgss:
                self._second_trainer_name = ""
            else:
                self._second_trainer_name = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_TRAINER_B_NUMBER).value
                if self._second_trainer_name is None:
                    self._second_trainer_name = ""

            num_enemy_pokemon = self._get_num_enemy_trainer_pokemon()
            self._enemy_pos_lookup = self._get_enemy_pos_lookup()
            
            # NEW: Initialize enemy PID tracking - read all enemy team PIDs at battle start
            self._enemy_pids_in_battle = []
            for cur_key in gh_gen_four_const.ALL_KEYS_BATTLE_ENEMY_1_PID:
                enemy_pid = self.machine._gamehook_client.get(cur_key).value
                if enemy_pid and enemy_pid != 0:
                    self._enemy_pids_in_battle.append(enemy_pid)
            if self.trainer_2 > 0:
                for cur_key in gh_gen_four_const.ALL_KEYS_BATTLE_ENEMY_2_PID:
                    enemy_pid = self.machine._gamehook_client.get(cur_key).value
                    if enemy_pid and enemy_pid != 0:
                        self._enemy_pids_in_battle.append(enemy_pid)
            
            logger.info(f"[EXP_SPLIT] ===== BATTLE INITIALIZATION =====")
            logger.info(f"[EXP_SPLIT] Enemy PIDs in battle: {self._enemy_pids_in_battle}")
            logger.info(f"[EXP_SPLIT] Num enemies: {num_enemy_pokemon}")
            logger.info(f"[EXP_SPLIT] Solo PID: {self._solo_mon_pid}")
            logger.info(f"[EXP_SPLIT] Is double battle: {self._is_double_battle}")
            logger.info(f"[EXP_SPLIT] Is multi-battle: {self._multi_battle}")
            
            # Initialize participation tracking for each enemy PID
            for enemy_pid in self._enemy_pids_in_battle:
                if self._is_double_battle and not self._multi_battle:
                    # Double battle: start with solo + ally
                    self._current_ally_pid = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_ALLY_MON_PID).value
                    self._enemy_pid_to_participating_player_pids[enemy_pid] = set([self._solo_mon_pid, self._current_ally_pid])
                    logger.info(f"[EXP_SPLIT] Enemy PID {enemy_pid} starts with participants: {self._enemy_pid_to_participating_player_pids[enemy_pid]}")
                else:
                    # Single or multi-battle: only solo mon
                    self._enemy_pid_to_participating_player_pids[enemy_pid] = set([self._solo_mon_pid])
                    logger.info(f"[EXP_SPLIT] Enemy PID {enemy_pid} starts with participants: {self._enemy_pid_to_participating_player_pids[enemy_pid]}")
            
            if self._is_double_battle and not self._multi_battle:
                logger.info(f"[EXP_SPLIT] Initial Ally PID: {self._current_ally_pid}")
                self._enemy_mon_order = [0, 1]
            else:
                self._enemy_mon_order = [0]
            
            logger.info(f"[EXP_SPLIT] ===== END BATTLE INITIALIZATION =====")
            
            # For backward compatibility, also track legacy exp_split
            if self._is_double_battle and not self._multi_battle:
                ally_mon_pos = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_ALLY_MON_PARTY_POS).value
                self._exp_split = [set([0, ally_mon_pos]) for _ in range(num_enemy_pokemon)]
            else:
                self._exp_split = [set([0]) for _ in range(num_enemy_pokemon)]

            return_custom_move_data = None
            if gen_four_const.RETURN_MOVE_NAME in self.machine._cached_moves:
                return_custom_move_data = []
                cur_friendship = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_FRIENDSHIP).value
                for _ in range(6):
                    return_custom_move_data.append({
                        const.PLAYER_KEY: {gen_four_const.RETURN_MOVE_NAME: str(int(cur_friendship / 2.5))},
                        const.ENEMY_KEY: {}
                    })

            logger.info(f"exp split: {[len(x) for x in self._exp_split]}")
            self.machine._queue_new_event(
                EventDefinition(
                    trainer_def=TrainerEventDefinition(
                        self._trainer_name,
                        second_trainer_name=self._second_trainer_name,
                        custom_move_data=return_custom_move_data,
                        exp_split=[len(x) for x in self._exp_split]
                    )
                )
            )
        else:
            logger.info(f"wild battle found")
    
    def _on_exit(self, next_state: State):
        if next_state.state_type != StateType.RESETTING:
            # If solo HP hit 0 and we're transitioning to OVERWORLD, pass blackout data to machine
            # OverworldState will check team HP when map changes to confirm blackout
            if next_state.state_type == StateType.OVERWORLD and self._solo_hp_zero:
                logger.info(f"[BLACKOUT DEBUG] Passing blackout detection to OVERWORLD state")
                logger.info(f"[BLACKOUT DEBUG] Solo HP hit 0: {self._solo_hp_zero}, Team HP hit 0: {self._team_hp_zero}, Watching for map change: {self._watching_for_map_change}")
                logger.info(f"[BLACKOUT DEBUG] Trainer name: {self._trainer_name}")
                logger.info(f"[BLACKOUT DEBUG] Initial map: {self._initial_map}")
                logger.info(f"[BLACKOUT DEBUG] Cached first mon: {self._cached_first_mon_species} level {self._cached_first_mon_level}")
                logger.info(f"[BLACKOUT DEBUG] Cached second mon: {self._cached_second_mon_species} level {self._cached_second_mon_level}")
                logger.info(f"[BLACKOUT DEBUG] Defeated trainer mons count: {len(self._defeated_trainer_mons)}")
                self.machine._potential_blackout_flag = True
                self.machine._blackout_cached_first_mon_species = self._cached_first_mon_species
                self.machine._blackout_cached_first_mon_level = self._cached_first_mon_level
                self.machine._blackout_cached_second_mon_species = self._cached_second_mon_species
                self.machine._blackout_cached_second_mon_level = self._cached_second_mon_level
                self.machine._blackout_defeated_trainer_mons = self._defeated_trainer_mons.copy()
                self.machine._blackout_trainer_name = self._trainer_name
                self.machine._blackout_initial_map = self._initial_map
            
            if self.is_trainer_battle == 'Trainer':
                logger.info(f"[EXP_SPLIT] ===== BATTLE EXIT =====")
                logger.info(f"[EXP_SPLIT] Enemy PIDs in team order: {self._enemy_pids_in_battle}")
                logger.info(f"[EXP_SPLIT] Exp split mapping (by PID): {self._enemy_pid_to_exp_split}")
                logger.info(f"[EXP_SPLIT] Raw _exp_split (legacy): {self._exp_split}")
                
                # Remap exp splits from PID mapping to team order
                final_exp_split = []
                for enemy_pid in self._enemy_pids_in_battle:
                    if enemy_pid in self._enemy_pid_to_exp_split:
                        split_count = self._enemy_pid_to_exp_split[enemy_pid]
                        final_exp_split.append(split_count)
                        logger.info(f"[EXP_SPLIT] Enemy PID {enemy_pid} → split count {split_count}")
                    else:
                        # Pokemon was not defeated, default to 1 (shouldn't happen in normal battles)
                        final_exp_split.append(1)
                        logger.info(f"[EXP_SPLIT] Enemy PID {enemy_pid} → no split recorded, defaulting to 1")
                
                logger.info(f"[EXP_SPLIT] Final exp_split in team order: {final_exp_split}")
                
                # If no splits > 1, set to None
                if not any([x > 1 for x in final_exp_split]):
                    logger.info(f"[EXP_SPLIT] No splits > 1, setting to None")
                    final_exp_split = None
                else:
                    logger.info(f"[EXP_SPLIT] Final exp_split with splits: {final_exp_split}")
                logger.info(f"[EXP_SPLIT] ===== END BATTLE EXIT =====")
                
                # In battles with two trainers, skip setting mon_order
                # The Pokemon from both trainers are interleaved in get_pokemon_list(),
                # and applying a custom mon_order would disrupt this interleaving,
                # causing some Pokemon to be skipped and incorrect experience to accumulate
                if self._second_trainer_name:
                    final_mon_order = None
                else:
                    final_mon_order = [self._enemy_mon_order.index(x) + 1 for x in sorted(self._enemy_mon_order)]

                return_custom_move_data = None
                if gen_four_const.RETURN_MOVE_NAME in self.machine._cached_moves:
                    return_custom_move_data = []
                    for cur_friendship in self._friendship_data:
                        return_custom_move_data.append({
                            const.PLAYER_KEY: {gen_four_const.RETURN_MOVE_NAME: str(int(cur_friendship / 2.5))},
                            const.ENEMY_KEY: {}
                        })

                self.machine._queue_new_event(
                    EventDefinition(
                        trainer_def=TrainerEventDefinition(
                            self._trainer_name,
                            second_trainer_name=self._second_trainer_name,
                            exp_split=final_exp_split,
                            mon_order=final_mon_order,
                            custom_move_data=return_custom_move_data,
                            pay_day_amount=self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MONEY).value - self._initial_money,
                        ),
                        notes=gh_gen_four_const.ROAR_FLAG
                    )
                )
            # Blackout is now handled in OVERWORLD state when map change is detected

            self._delayed_move_updater.trigger()
            self._delayed_item_updater.trigger()
            self._delayed_held_item_updater.trigger()
            self._delayed_levelup.trigger()
    
    def _get_first_enemy_mon_pos(self, value=None):
        if value is None:
            value = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_PARTY_POS).value
        return self._enemy_pos_lookup[value]

    def _get_second_enemy_mon_pos(self, value=None):
        if value is None:
            value = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_PARTY_POS).value
        return self._enemy_pos_lookup[value]

    def _check_all_team_hp_zero(self):
        """Check if all team HP values are 0 or below"""
        for hp_key in gh_gen_four_const.ALL_KEYS_BATTLE_TEAM_HP:
            hp_value = self.machine._gamehook_client.get(hp_key).value
            if hp_value is not None and hp_value > 0:
                return False
        return True
    
    def _handle_blackout(self):
        """Handle blackout logic: remove trainer event, add defeated Pokemon, add blackout event"""
        logger.info("Handling blackout")
        if self.is_trainer_battle == 'Trainer':
            self.machine._queue_new_event(
                EventDefinition(trainer_def=TrainerEventDefinition(self._trainer_name), notes=gh_gen_four_const.TRAINER_LOSS_FLAG)
            )
            # Add any cached Pokemon that haven't been processed yet (in case blackout happened before EXP was processed)
            # Check both cache slots - multiple Pokemon can be cached sequentially
            if self._cached_first_mon_species or self._cached_first_mon_level:
                self._defeated_trainer_mons.append(EventDefinition(wild_pkmn_info=WildPkmnEventDefinition(
                    self._cached_first_mon_species,
                    self._cached_first_mon_level,
                    trainer_pkmn=True
                )))
                logger.info(f"Adding cached Pokemon to defeated list during blackout: {self._cached_first_mon_species} level {self._cached_first_mon_level}")
            if self._cached_second_mon_species or self._cached_second_mon_level:
                self._defeated_trainer_mons.append(EventDefinition(wild_pkmn_info=WildPkmnEventDefinition(
                    self._cached_second_mon_species,
                    self._cached_second_mon_level,
                    trainer_pkmn=True
                )))
                logger.info(f"Adding cached Pokemon to defeated list during blackout: {self._cached_second_mon_species} level {self._cached_second_mon_level}")
            
            # Add all defeated trainer Pokemon
            for trainer_mon_event in self._defeated_trainer_mons:
                self.machine._queue_new_event(trainer_mon_event)
        self.machine._queue_new_event(EventDefinition(blackout=BlackoutEventDefinition()))

    @auto_reset
    def transition(self, new_prop: GameHookProperty, prev_prop: GameHookProperty) -> StateType:
        # don't actually track anything during the tutorial battle
        # if self._is_tutorial_battle:
        #     return self.state_type

        # Track solo HP hitting 0 - start blackout detection
        if new_prop.path == gh_gen_four_const.ALL_KEYS_BATTLE_SOLO_HP:
            if new_prop.value is not None and new_prop.value <= 0 and not self._solo_hp_zero:
                logger.info(f"Solo HP hit 0, starting blackout detection")
                self._solo_hp_zero = True
                # Check if team HP is already 0 (in case it happened before solo HP hit 0)
                if not self._team_hp_zero and self._check_all_team_hp_zero():
                    logger.info(f"All team HP already 0, watching for map change")
                    self._team_hp_zero = True
                    self._watching_for_map_change = True
        
        # Track team HP hitting 0 - start watching for map change
        # Check whenever any team HP property changes
        if new_prop.path in gh_gen_four_const.ALL_KEYS_BATTLE_TEAM_HP:
            if self._solo_hp_zero and not self._team_hp_zero:
                if self._check_all_team_hp_zero():
                    logger.info(f"All team HP hit 0, watching for map change")
                    self._team_hp_zero = True
                    self._watching_for_map_change = True
        
        # Map change handling moved to OverworldState - transition to OVERWORLD when watching for blackout
        # This allows OverworldState to handle map changes and check team HP

        if new_prop.path == gh_gen_four_const.KEY_PLAYER_MON_EXPPOINTS or new_prop.path == gh_gen_four_const.KEY_BATTLE_PLAYER_MON_EXP:
            logger.info(f"[EXP_SPLIT] ===== EXP CHANGE DETECTED =====")
            logger.info(f"[EXP_SPLIT] Path: {new_prop.path}, Old: {prev_prop.value}, New: {new_prop.value}, Diff: {new_prop.value - prev_prop.value if prev_prop.value and new_prop.value else 'N/A'}")
            logger.info(f"[EXP_SPLIT] Cached first: '{self._cached_first_mon_species}' level {self._cached_first_mon_level}")
            logger.info(f"[EXP_SPLIT] Cached second: '{self._cached_second_mon_species}' level {self._cached_second_mon_level}")
            logger.info(f"[EXP_SPLIT] Is trainer battle: {self.is_trainer_battle}")
            logger.info(f"[EXP_SPLIT] Ally HP zero flag: {self._ally_hp_zero}")
            logger.info(f"[EXP_SPLIT] Current ally PID: {self._current_ally_pid}")
            
            # NEW APPROACH: Get the enemy PID that just fainted (from active enemy slots)
            # When an enemy faints, their HP goes to 0 but they remain in the active slot briefly
            # We cached their species/level when HP hit 0, now we need to match that to their PID
            defeated_enemy_pid = None
            
            # Check first enemy slot
            if self._cached_first_mon_species or self._cached_first_mon_level:
                first_enemy_pid = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_PID).value
                logger.info(f"[EXP_SPLIT] First enemy cached, checking PID: {first_enemy_pid}")
                if first_enemy_pid and first_enemy_pid in self._enemy_pid_to_participating_player_pids:
                    defeated_enemy_pid = first_enemy_pid
                    logger.info(f"[EXP_SPLIT] Matched defeated enemy to first slot PID: {defeated_enemy_pid}")
                elif first_enemy_pid:
                    logger.info(f"[EXP_SPLIT] WARNING: First enemy PID {first_enemy_pid} not in tracking dictionary!")
            
            # Check second enemy slot
            if not defeated_enemy_pid and (self._cached_second_mon_species or self._cached_second_mon_level):
                second_enemy_pid = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_PID).value
                logger.info(f"[EXP_SPLIT] Second enemy cached, checking PID: {second_enemy_pid}")
                if second_enemy_pid and second_enemy_pid in self._enemy_pid_to_participating_player_pids:
                    defeated_enemy_pid = second_enemy_pid
                    logger.info(f"[EXP_SPLIT] Matched defeated enemy to second slot PID: {defeated_enemy_pid}")
                elif second_enemy_pid:
                    logger.info(f"[EXP_SPLIT] WARNING: Second enemy PID {second_enemy_pid} not in tracking dictionary!")
            
            # Calculate and record the exp split for this enemy PID
            if defeated_enemy_pid and defeated_enemy_pid in self._enemy_pid_to_participating_player_pids:
                participating_player_pids = self._enemy_pid_to_participating_player_pids[defeated_enemy_pid].copy()
                split_count = len(participating_player_pids)
                
                logger.info(f"[EXP_SPLIT] FINAL exp split for enemy PID {defeated_enemy_pid}: {split_count} participants")
                logger.info(f"[EXP_SPLIT] Participating player PIDs: {participating_player_pids}")
                
                # Store the split count mapped by enemy PID
                self._enemy_pid_to_exp_split[defeated_enemy_pid] = split_count
                logger.info(f"[EXP_SPLIT] Stored split count {split_count} for enemy PID {defeated_enemy_pid}")
                logger.info(f"[EXP_SPLIT] Current exp split mapping: {self._enemy_pid_to_exp_split}")
            else:
                logger.info(f"[EXP_SPLIT] WARNING: Could not determine defeated enemy PID for exp split calculation")
            
            # Reset ally faint flag after processing exp
            self._ally_hp_zero = False
            logger.info(f"[EXP_SPLIT] Reset ally_hp_zero flag")
            logger.info(f"[EXP_SPLIT] ===== END EXP CHANGE =====")
            
            if self._cached_first_mon_species or self._cached_first_mon_level:
                if self.is_trainer_battle == 'Trainer':
                    self._defeated_trainer_mons.append(EventDefinition(wild_pkmn_info=WildPkmnEventDefinition(
                        self._cached_first_mon_species,
                        self._cached_first_mon_level,
                        trainer_pkmn=True
                    )))
                else:
                    logger.info(f"Queueing wild Pokemon event: {self._cached_first_mon_species} level {self._cached_first_mon_level}")
                    self.machine._queue_new_event(EventDefinition(wild_pkmn_info=WildPkmnEventDefinition(
                        self._cached_first_mon_species,
                        self._cached_first_mon_level,
                    )))
                self._cached_first_mon_species = ""
                self._cached_first_mon_level = 0
            elif self._cached_second_mon_species or self._cached_second_mon_level:
                if self.is_trainer_battle == 'Trainer':
                    self._defeated_trainer_mons.append(EventDefinition(wild_pkmn_info=WildPkmnEventDefinition(
                        self._cached_second_mon_species,
                        self._cached_second_mon_level,
                        trainer_pkmn=True
                    )))
                else:
                    self.machine._queue_new_event(EventDefinition(wild_pkmn_info=WildPkmnEventDefinition(
                        self._cached_second_mon_species,
                        self._cached_second_mon_level,
                    )))
                self._cached_second_mon_species = ""
                self._cached_second_mon_level = 0
            else:
                logger.error(f"Solo mon gained experience, but we didn't properly cache which enemy mon was defeated... This is normal if the a different pokemon has been pulled into player's slot 1")
            
            # Check if battle ended AFTER processing EXP changes
            # If META_STATE is not 'Battle', we've left battle
            # BUT: Don't transition if we're tracking a blackout
            meta_state = self.machine._gamehook_client.get(gh_gen_four_const.META_STATE).value
            if meta_state != 'Battle':
                # If we're tracking a blackout, stay in battle state
                if self._solo_hp_zero:
                    return self.state_type
                return StateType.OVERWORLD

        # elif new_prop.path == gh_gen_four_const.KEY_BATTLE_FLAG:
        #     if new_prop.value and not self._battle_started:
        #         self._delayed_initialization.trigger()
        elif new_prop.path == gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_HP:
            if new_prop.value == 0 and prev_prop.value > 0:
                logger.info(f"[EXP_SPLIT] ===== FIRST ENEMY FAINTED =====")
                logger.info(f"[EXP_SPLIT] First enemy HP: {prev_prop.value} -> {new_prop.value}")
                
                # Get the enemy PID
                enemy_pid = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_PID).value
                logger.info(f"[EXP_SPLIT] First enemy PID: {enemy_pid}")
                
                # Always cache when HP hits 0 (like Emerald) - allows caching multiple Pokemon
                # If there's already a cached Pokemon, add it to defeated list before overwriting (for trainer battles)
                if (self._cached_first_mon_species or self._cached_first_mon_level) and self.is_trainer_battle == 'Trainer':
                    self._defeated_trainer_mons.append(EventDefinition(wild_pkmn_info=WildPkmnEventDefinition(
                        self._cached_first_mon_species,
                        self._cached_first_mon_level,
                        trainer_pkmn=True
                    )))
                    logger.info(f"[EXP_SPLIT] Adding previously cached Pokemon to defeated list before overwriting: {self._cached_first_mon_species} level {self._cached_first_mon_level}")
                
                species_raw = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_SPECIES).value
                level_raw = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_LEVEL).value
                self._cached_first_mon_species = self.machine.gh_converter.pkmn_name_convert(species_raw)
                self._cached_first_mon_level = level_raw
                logger.info(f"[EXP_SPLIT] Cached first enemy: {self._cached_first_mon_species} level {self._cached_first_mon_level} (raw: {species_raw}, {level_raw})")
                self._friendship_data.append(self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_FRIENDSHIP).value)
                
                # Log participation info for this enemy PID
                if enemy_pid in self._enemy_pid_to_participating_player_pids:
                    logger.info(f"[EXP_SPLIT] Current participating player PIDs for enemy PID {enemy_pid}: {self._enemy_pid_to_participating_player_pids[enemy_pid]}")
                else:
                    logger.info(f"[EXP_SPLIT] WARNING: Enemy PID {enemy_pid} not found in tracking dictionary!")
                logger.info(f"[EXP_SPLIT] Ally HP zero flag: {self._ally_hp_zero}")
                logger.info(f"[EXP_SPLIT] ===== END FIRST ENEMY FAINTED =====")
        elif new_prop.path == gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_HP:
            if new_prop.value == 0 and prev_prop.value > 0:
                logger.info(f"[EXP_SPLIT] ===== SECOND ENEMY FAINTED =====")
                logger.info(f"[EXP_SPLIT] Second enemy HP: {prev_prop.value} -> {new_prop.value}")
                
                # Get the enemy PID
                enemy_pid = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_PID).value
                logger.info(f"[EXP_SPLIT] Second enemy PID: {enemy_pid}")
                
                # Always cache when HP hits 0 (like Emerald) - allows caching multiple Pokemon
                # If there's already a cached Pokemon, add it to defeated list before overwriting (for trainer battles)
                if (self._cached_second_mon_species or self._cached_second_mon_level) and self.is_trainer_battle == 'Trainer':
                    self._defeated_trainer_mons.append(EventDefinition(wild_pkmn_info=WildPkmnEventDefinition(
                        self._cached_second_mon_species,
                        self._cached_second_mon_level,
                        trainer_pkmn=True
                    )))
                    logger.info(f"[EXP_SPLIT] Adding previously cached Pokemon to defeated list before overwriting: {self._cached_second_mon_species} level {self._cached_second_mon_level}")
                
                self._cached_second_mon_species = self.machine.gh_converter.pkmn_name_convert(self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_SPECIES).value)
                self._cached_second_mon_level = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_LEVEL).value
                logger.info(f"[EXP_SPLIT] Cached second enemy: {self._cached_second_mon_species} level {self._cached_second_mon_level}")
                self._friendship_data.append(self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_FRIENDSHIP).value)
                
                # Log participation info for this enemy PID
                if enemy_pid in self._enemy_pid_to_participating_player_pids:
                    logger.info(f"[EXP_SPLIT] Current participating player PIDs for enemy PID {enemy_pid}: {self._enemy_pid_to_participating_player_pids[enemy_pid]}")
                else:
                    logger.info(f"[EXP_SPLIT] WARNING: Enemy PID {enemy_pid} not found in tracking dictionary!")
                logger.info(f"[EXP_SPLIT] Ally HP zero flag: {self._ally_hp_zero}")
                logger.info(f"[EXP_SPLIT] ===== END SECOND ENEMY FAINTED =====")
        elif new_prop.path == gh_gen_four_const.KEY_BATTLE_ALLY_MON_PID and not self._multi_battle:
            # Track PID changes for ally Pokemon in double battles
            if self._is_double_battle and new_prop.value is not None and new_prop.value != 0:
                new_pid = new_prop.value
                if new_pid != self._current_ally_pid:
                    logger.info(f"[EXP_SPLIT] ===== ALLY PID CHANGE =====")
                    logger.info(f"[EXP_SPLIT] Old ally PID: {self._current_ally_pid}")
                    logger.info(f"[EXP_SPLIT] New ally PID: {new_pid}")
                    self._current_ally_pid = new_pid
                    self._ally_hp_zero = False  # Reset ally faint flag
                    
                    # Get currently active enemy PIDs
                    first_enemy_pid = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_PID).value
                    second_enemy_pid = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_PID).value
                    first_enemy_hp = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_HP).value
                    second_enemy_hp = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_HP).value
                    
                    logger.info(f"[EXP_SPLIT] First enemy PID: {first_enemy_pid}, HP: {first_enemy_hp}")
                    logger.info(f"[EXP_SPLIT] Second enemy PID: {second_enemy_pid}, HP: {second_enemy_hp}")
                    
                    # Add new PID to all living enemies' participation sets (by enemy PID)
                    if first_enemy_pid and first_enemy_hp and first_enemy_hp > 0:
                        if first_enemy_pid in self._enemy_pid_to_participating_player_pids:
                            self._enemy_pid_to_participating_player_pids[first_enemy_pid].add(new_pid)
                            logger.info(f"[EXP_SPLIT] Added player PID {new_pid} to enemy PID {first_enemy_pid}. Player PIDs now: {self._enemy_pid_to_participating_player_pids[first_enemy_pid]}")
                        else:
                            logger.info(f"[EXP_SPLIT] WARNING: First enemy PID {first_enemy_pid} not in tracking dictionary!")
                    
                    if self._is_double_battle and second_enemy_pid and second_enemy_hp and second_enemy_hp > 0:
                        if second_enemy_pid in self._enemy_pid_to_participating_player_pids:
                            self._enemy_pid_to_participating_player_pids[second_enemy_pid].add(new_pid)
                            logger.info(f"[EXP_SPLIT] Added player PID {new_pid} to enemy PID {second_enemy_pid}. Player PIDs now: {self._enemy_pid_to_participating_player_pids[second_enemy_pid]}")
                        else:
                            logger.info(f"[EXP_SPLIT] WARNING: Second enemy PID {second_enemy_pid} not in tracking dictionary!")
                    
                    logger.info(f"[EXP_SPLIT] ===== END ALLY PID CHANGE =====")
                else:
                    logger.info(f"[EXP_SPLIT] Ally PID unchanged: {new_pid}")
            elif new_prop.value is None or new_prop.value == 0:
                logger.info(f"[EXP_SPLIT] Ally PID set to None/0 (value: {new_prop.value})")
        elif new_prop.path == gh_gen_four_const.KEY_BATTLE_PLAYER_MON_HP:
            player_mon_pos = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_PLAYER_MON_PARTY_POS).value
            if player_mon_pos == 0 and new_prop.value <= 0:
                if self._battle_started:
                    logger.info(f"Player mon HP dropped to 0 or below")
                    self._loss_detected = True
                    logger.info(f"Loss detected: {self._loss_detected}")
            elif new_prop.value <= 0:
                enemy_mon_pos = self._get_first_enemy_mon_pos()
                if player_mon_pos in self._exp_split[enemy_mon_pos]:
                    self._exp_split[enemy_mon_pos].remove(player_mon_pos)
                if self._is_double_battle:
                    second_enemy_mon_pos = self._get_second_enemy_mon_pos()
                    if player_mon_pos in self._exp_split[second_enemy_mon_pos]:
                        self._exp_split[second_enemy_mon_pos].remove(player_mon_pos)
        elif new_prop.path == gh_gen_four_const.KEY_BATTLE_ALLY_MON_HP and not self._multi_battle:
            if self._is_double_battle and new_prop.value <= 0 and prev_prop.value > 0:
                # Mark ally as fainted for PID-based tracking
                logger.info(f"[EXP_SPLIT] ===== ALLY FAINTED =====")
                logger.info(f"[EXP_SPLIT] Ally HP: {prev_prop.value} -> {new_prop.value}")
                logger.info(f"[EXP_SPLIT] Current ally PID: {self._current_ally_pid}")
                logger.info(f"[EXP_SPLIT] Setting ally_hp_zero flag to True")
                self._ally_hp_zero = True
                
                # CRITICAL: Remove ally PID from ALL enemy PIDs in the dictionary
                if self._current_ally_pid:
                    for enemy_pid in self._enemy_pid_to_participating_player_pids:
                        if self._current_ally_pid in self._enemy_pid_to_participating_player_pids[enemy_pid]:
                            self._enemy_pid_to_participating_player_pids[enemy_pid].discard(self._current_ally_pid)
                            logger.info(f"[EXP_SPLIT] Removed ally PID {self._current_ally_pid} from enemy PID {enemy_pid}. Player PIDs now: {self._enemy_pid_to_participating_player_pids[enemy_pid]}")
                
                # Log enemy status for context
                first_enemy_pid = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_PID).value
                second_enemy_pid = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_PID).value
                first_hp = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_HP).value
                second_hp = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_HP).value
                logger.info(f"[EXP_SPLIT] First enemy PID: {first_enemy_pid}, HP: {first_hp}")
                logger.info(f"[EXP_SPLIT] Second enemy PID: {second_enemy_pid}, HP: {second_hp}")
                logger.info(f"[EXP_SPLIT] ===== END ALLY FAINTED =====")
                
                # Legacy exp_split tracking (for backwards compatibility)
                # for each of these we want to remove the ally from exp split if the enemy mon is still alive
                # additionally, we also want to remove from the exp split if the enemy is cached, as this means they died on the same turn (e.g. earthquake)
                # if the enemy has no HP *AND* is not cached for exp distribution, then they have died and enemy trainer has no further mons
                #   In that case, leave the ally in the exp split, as they did participate already
                ally_mon_pos = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_ALLY_MON_PARTY_POS).value
                enemy_mon_pos = self._get_first_enemy_mon_pos()
                if (
                    ally_mon_pos in self._exp_split[enemy_mon_pos] and (
                        self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_HP).value > 0 or
                        self._cached_first_mon_species
                    )
                ):
                    self._exp_split[enemy_mon_pos].remove(ally_mon_pos)

                # do the same checks for second enemy mon
                second_enemy_mon_pos = self._get_second_enemy_mon_pos()
                if (
                    ally_mon_pos in self._exp_split[second_enemy_mon_pos] and (
                        self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_HP).value > 0 or
                        self._cached_second_mon_species
                    )
                ):
                    self._exp_split[second_enemy_mon_pos].remove(ally_mon_pos)

        elif new_prop.path in gh_gen_four_const.ALL_KEYS_PLAYER_MOVES:
            self._delayed_move_updater.begin_waiting()
        elif new_prop.path in gh_gen_four_const.ALL_KEYS_ALL_ITEM_FIELDS:
            self._delayed_item_updater.begin_waiting()
        elif new_prop.path == gh_gen_four_const.KEY_PLAYER_MON_HELD_ITEM:
            self._delayed_held_item_updater.begin_waiting()
        elif new_prop.path == gh_gen_four_const.KEY_PLAYER_MON_LEVEL:
            self._delayed_levelup.begin_waiting()
        elif new_prop.path == gh_gen_four_const.KEY_GAMETIME_SECONDS:
            self._delayed_move_updater.tick()
            self._delayed_item_updater.tick()
            self._delayed_held_item_updater.tick()
            self._delayed_levelup.tick()
            self._delayed_initialization.tick()
        elif new_prop.path == gh_gen_four_const.KEY_BATTLE_PLAYER_MON_PARTY_POS:
            if new_prop.value >= 0 and new_prop.value < 6:
                if self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_HP).value > 0:
                    self._exp_split[self._get_first_enemy_mon_pos()].add(new_prop.value)
                if self._is_double_battle:
                    if self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_HP).value > 0:
                        self._exp_split[self._get_second_enemy_mon_pos()].add(new_prop.value)
        elif new_prop.path == gh_gen_four_const.KEY_BATTLE_ALLY_MON_PARTY_POS:
            if self._is_double_battle and not self._multi_battle and new_prop.value >= 0 and new_prop.value < 6:
                if self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_HP).value > 0:
                    self._exp_split[self._get_first_enemy_mon_pos()].add(new_prop.value)
                if self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_HP).value > 0:
                    self._exp_split[self._get_second_enemy_mon_pos()].add(new_prop.value)
        elif new_prop.path == gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_PARTY_POS:
            real_new_value = self._get_first_enemy_mon_pos(value=new_prop.value)
            logger.info(f"[EXP_SPLIT] ===== FIRST ENEMY SWITCHED =====")
            logger.info(f"[EXP_SPLIT] Enemy party pos changed: {prev_prop.value} -> {new_prop.value}")
            logger.info(f"[EXP_SPLIT] Real enemy position: {real_new_value}")
            
            if real_new_value >= 0 and real_new_value < len(self._exp_split):
                # NEW: With enemy PID tracking, we don't need to reset anything!
                # The enemy PID remains associated with the Pokemon regardless of position.
                # Just log the new enemy PID for debugging
                new_enemy_pid = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_PID).value
                logger.info(f"[EXP_SPLIT] New enemy PID in first slot: {new_enemy_pid}")
                if new_enemy_pid in self._enemy_pid_to_participating_player_pids:
                    logger.info(f"[EXP_SPLIT] Participating player PIDs for this enemy: {self._enemy_pid_to_participating_player_pids[new_enemy_pid]}")
                
                # Legacy exp_split tracking (for backwards compatibility)
                if self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_PLAYER_MON_HP).value > 0:
                    self._exp_split[real_new_value] = set([self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_PLAYER_MON_PARTY_POS).value])
                if self._is_double_battle and not self._multi_battle and self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_ALLY_MON_HP).value > 0:
                    self._exp_split[real_new_value].add(self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_ALLY_MON_PARTY_POS).value)

                # NOTE: this logic won't perfectly reflect things if the player uses roar/whirlwind
                # or if the enemy trainer switches pokemon (and doesn't only send out new mons on previous death)
                if real_new_value not in self._enemy_mon_order:
                    self._enemy_mon_order.append(real_new_value)
                
                # When party position changes, check if HP is already 0 and cache it
                # This handles cases where a new Pokemon is sent out and immediately faints
                # If there's already a cached Pokemon, add it to defeated list before overwriting (for trainer battles)
                current_hp = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_HP).value
                if current_hp is not None and current_hp == 0:
                    if (self._cached_first_mon_species or self._cached_first_mon_level) and self.is_trainer_battle == 'Trainer':
                        self._defeated_trainer_mons.append(EventDefinition(wild_pkmn_info=WildPkmnEventDefinition(
                            self._cached_first_mon_species,
                            self._cached_first_mon_level,
                            trainer_pkmn=True
                        )))
                        logger.info(f"Adding previously cached Pokemon to defeated list before overwriting (party pos change): {self._cached_first_mon_species} level {self._cached_first_mon_level}")
                    
                    species_raw = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_SPECIES).value
                    level_raw = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_FIRST_ENEMY_LEVEL).value
                    if species_raw:
                        self._cached_first_mon_species = self.machine.gh_converter.pkmn_name_convert(species_raw)
                        self._cached_first_mon_level = level_raw
                        logger.info(f"Cached wild Pokemon (from party pos change): {self._cached_first_mon_species} level {self._cached_first_mon_level}")
                        self._friendship_data.append(self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_FRIENDSHIP).value)
            logger.info(f"[EXP_SPLIT] ===== END FIRST ENEMY SWITCHED =====")
        elif new_prop.path == gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_PARTY_POS:
            real_new_value = self._get_second_enemy_mon_pos(value=new_prop.value)
            logger.info(f"[EXP_SPLIT] ===== SECOND ENEMY SWITCHED =====")
            logger.info(f"[EXP_SPLIT] Enemy party pos changed: {prev_prop.value} -> {new_prop.value}")
            logger.info(f"[EXP_SPLIT] Real enemy position: {real_new_value}")
            
            if self._is_double_battle and real_new_value >= 0 and real_new_value < len(self._exp_split):
                # NEW: With enemy PID tracking, we don't need to reset anything!
                # The enemy PID remains associated with the Pokemon regardless of position.
                # Just log the new enemy PID for debugging
                new_enemy_pid = self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_SECOND_ENEMY_PID).value
                logger.info(f"[EXP_SPLIT] New enemy PID in second slot: {new_enemy_pid}")
                if new_enemy_pid in self._enemy_pid_to_participating_player_pids:
                    logger.info(f"[EXP_SPLIT] Participating player PIDs for this enemy: {self._enemy_pid_to_participating_player_pids[new_enemy_pid]}")
                
                # Legacy exp_split tracking (for backwards compatibility)
                if self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_PLAYER_MON_HP).value > 0:
                    self._exp_split[real_new_value] = set([self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_PLAYER_MON_PARTY_POS).value])
                if not self._multi_battle and self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_ALLY_MON_HP).value > 0:
                    self._exp_split[real_new_value].add(self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_ALLY_MON_PARTY_POS).value)

                # NOTE: this logic won't perfectly reflect things if the player uses roar/whirlwind
                # or if the enemy trainer switches pokemon (and doesn't only send out new mons on previous death)
                if real_new_value not in self._enemy_mon_order:
                    self._enemy_mon_order.append(real_new_value)
            logger.info(f"[EXP_SPLIT] ===== END SECOND ENEMY SWITCHED =====")
        elif new_prop.path == gh_gen_four_const.KEY_PLAYER_MON_SPECIES:
            # Species changed - could be an evolution! Update team cache to detect it
            # This handles the case where evolution occurs during battle end transition
            self.machine.update_team_cache()
            # Only transition if META_STATE indicates we've left battle
            # Don't transition immediately on species change - wait for META_STATE to change
            # If we're watching for blackout, transition to OVERWORLD so it can handle map change detection
            meta_state = self.machine._gamehook_client.get(gh_gen_four_const.META_STATE).value
            if meta_state != 'Battle':
                # If we're watching for blackout, transition to OVERWORLD (it will handle map change)
                if self._watching_for_map_change:
                    return StateType.OVERWORLD
                # If we have a cached Pokemon, don't transition yet - wait for EXP change
                if not self._cached_first_mon_species and not self._cached_second_mon_species:
                    return StateType.OVERWORLD
        elif new_prop.path == gh_gen_four_const.META_STATE:
            # META_STATE changed - check if battle ended
            # If META_STATE is not 'Battle', we've left battle (could be 'From Battle' or anything else)
            # If we're watching for blackout, transition to OVERWORLD so it can handle map change detection
            if new_prop.value != 'Battle':
                # If we're watching for blackout, transition to OVERWORLD (it will handle map change)
                if self._watching_for_map_change:
                    logger.info(f"META_STATE changed to '{new_prop.value}', transitioning to OVERWORLD for blackout detection")
                    return StateType.OVERWORLD
                # If we have a cached Pokemon, don't transition yet - wait for EXP change
                if not self._cached_first_mon_species and not self._cached_second_mon_species:
                    return StateType.OVERWORLD
        
        # Check if battle ended (only if not already checked after EXP change)
        # BUT: Don't transition if we have a cached Pokemon waiting for EXP change
        # If we're watching for blackout, transition to OVERWORLD so it can handle map change detection
        if new_prop.path != gh_gen_four_const.KEY_PLAYER_MON_EXPPOINTS:
            meta_state = self.machine._gamehook_client.get(gh_gen_four_const.META_STATE).value
            # If META_STATE is not 'Battle', we've left battle (could be 'From Battle', 'Overworld', or anything else)
            if meta_state != 'Battle':
                # If we're watching for blackout, transition to OVERWORLD (it will handle map change)
                if self._watching_for_map_change:
                    return StateType.OVERWORLD
                # If we have a cached Pokemon, don't transition yet - wait for EXP change
                if not self._cached_first_mon_species and not self._cached_second_mon_species:
                    return StateType.OVERWORLD

        return self.state_type


class InventoryChangeState(WatchForResetState):
    BASE_DELAY = 2
    def __init__(self, machine: Machine):
        super().__init__(StateType.INVENTORY_CHANGE, machine)
        self._seconds_delay = self.BASE_DELAY
        self._money_gained = False
        self._money_lost = False
        self._money_change_amount = None
        self._held_item_changed = False
        self.external_held_item_flag = False
    
    def _on_enter(self, prev_state: State):
        self._seconds_delay = self.BASE_DELAY
        money_change = self.machine._money_cache_update()
        if money_change is not None:
            self._money_change_amount = money_change
            self._money_gained = money_change > 0
            self._money_lost = money_change < 0
        else:
            self._money_change_amount = None
            self._money_gained = False
            self._money_lost = False

        # Set it to True if we are getting flagged for it externally. Otherwise set it to False
        self._held_item_changed = self.external_held_item_flag
        self.external_held_item_flag = False
        
        # Note: EVs are cached in OverworldState (machine._cached_evs) BEFORE we enter this state
        # This is crucial because EV changes happen BEFORE inventory changes are detected
    
    def _on_exit(self, next_state: State):
        if next_state.state_type != StateType.RESETTING:
            # If transitioning to RARE_CANDY state, skip item cache update here
            # UseRareCandyState._on_exit will handle it with candy_flag=True
            if next_state.state_type == StateType.RARE_CANDY:
                return
            
            # If transitioning to VITAMIN state, skip item cache update here
            # UseVitaminState._on_exit will handle it with vitamin_flag=True
            if next_state.state_type == StateType.VITAMIN:
                return
            
            # Check if EVs changed by comparing machine's cached EVs (from overworld) with current EVs
            # Machine's cached_evs were captured in OverworldState BEFORE the EV change happened
            current_evs = {
                'hp': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_HP[0]).value,
                'attack': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_ATTACK[0]).value,
                'defense': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_DEFENSE[0]).value,
                'speed': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_SPEED[0]).value,
                'special_attack': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_SPECIAL_ATTACK[0]).value,
                'special_defense': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_SPECIAL_DEFENSE[0]).value,
            }
            
            evs_changed = any(
                current_evs[stat] != self.machine._cached_evs.get(stat) 
                for stat in current_evs.keys() 
                if self.machine._cached_evs.get(stat) is not None and current_evs[stat] is not None
            )
            
            logger.info(f"InventoryChangeState._on_exit: old_cache = {self.machine._cached_items}")
            logger.info(f"InventoryChangeState._on_exit: cached EVs = {self.machine._cached_evs}, current EVs = {current_evs}, changed = {evs_changed}")
            
            # Use vitamin_flag if EVs changed
            self.machine._item_cache_update(
                sale_expected=self._money_gained,
                purchase_expected=self._money_lost,
                money_change_amount=self._money_change_amount,
                held_item_changed=self._held_item_changed,
                vitamin_flag=evs_changed
            )
    
    @auto_reset
    def transition(self, new_prop:GameHookProperty, prev_prop:GameHookProperty) -> StateType:
        if new_prop.path == gh_gen_four_const.KEY_PLAYER_MONEY:
            money_change = new_prop.value - prev_prop.value
            if money_change != 0:
                self._money_change_amount = money_change
                self._money_gained = money_change > 0
                self._money_lost = money_change < 0
        elif new_prop.path in gh_gen_four_const.ALL_KEYS_ALL_ITEM_FIELDS:
            self._seconds_delay = self.BASE_DELAY
        elif new_prop.path == gh_gen_four_const.KEY_PLAYER_MON_HELD_ITEM:
            self._held_item_changed = True
        elif new_prop.path == gh_gen_four_const.KEY_PLAYER_MON_LEVEL:
            # Check if level increased - this indicates rare candy usage
            if prev_prop.value is not None and new_prop.value is not None and new_prop.value > prev_prop.value:
                return StateType.RARE_CANDY
        elif new_prop.path in gh_gen_four_const.ALL_KEYS_STAT_EXP:
            # Check if any EV changed - this indicates vitamin usage
            # We only care about the solo mon (slot 0)
            if (
                new_prop.path == gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_HP[0] or
                new_prop.path == gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_ATTACK[0] or
                new_prop.path == gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_DEFENSE[0] or
                new_prop.path == gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_SPEED[0] or
                new_prop.path == gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_SPECIAL_ATTACK[0] or
                new_prop.path == gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_SPECIAL_DEFENSE[0]
            ):
                if prev_prop.value is not None and new_prop.value is not None and new_prop.value != prev_prop.value:
                    return StateType.VITAMIN
        elif new_prop.path == gh_gen_four_const.KEY_GAMETIME_SECONDS:
            if self._seconds_delay <= 0:
                return StateType.OVERWORLD
            else:
                self._seconds_delay -= 1

        return self.state_type


class UseRareCandyState(WatchForResetState):
    BASE_DELAY = 2
    def __init__(self, machine: Machine):
        super().__init__(StateType.RARE_CANDY, machine)
        self._move_learned = False
        self._item_removal_detected = False
        self._cur_delay = self.BASE_DELAY

    def _on_enter(self, prev_state: State):
        self._move_learned = False
        self._item_removal_detected = False
        self._cur_delay = self.BASE_DELAY
    
    def _on_exit(self, next_state: State):
        if next_state.state_type != StateType.RESETTING:
            if self.machine._item_cache_update(candy_flag=True):
                self.machine._solo_mon_levelup(self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_LEVEL).value)
            if self._move_learned:
                self.machine.update_team_cache()
                self.machine._move_cache_update(levelup_source=True)
    
    @auto_reset
    def transition(self, new_prop:GameHookProperty, prev_prop:GameHookProperty) -> StateType:
        if new_prop.path in gh_gen_four_const.ALL_KEYS_PLAYER_MOVES:
            self._move_learned = True
        elif new_prop.path in gh_gen_four_const.ALL_KEYS_ITEM_QUANTITY:
            self._cur_delay = self.BASE_DELAY
        elif new_prop.path in gh_gen_four_const.ALL_KEYS_ITEM_TYPE:
            self._cur_delay = self.BASE_DELAY
        elif new_prop.path in gh_gen_four_const.KEY_GAMETIME_SECONDS:
            if self._cur_delay <= 0:
                return StateType.OVERWORLD
            else:
                self._cur_delay -= 1

        return self.state_type


class UseTMState(WatchForResetState):
    BASE_DELAY = 2
    def __init__(self, machine: Machine):
        super().__init__(StateType.TM, machine)

    def _on_enter(self, prev_state: State):
        self._seconds_delay = self.BASE_DELAY
    
    def _on_exit(self, next_state: State):
        if next_state.state_type != StateType.RESETTING:
            if not self.machine._item_cache_update(tm_flag=True):
                self.machine._move_cache_update(levelup_source=True)
    
    @auto_reset
    def transition(self, new_prop:GameHookProperty, prev_prop:GameHookProperty) -> StateType:
        if new_prop.path in gh_gen_four_const.ALL_KEYS_ALL_ITEM_FIELDS:
            self._seconds_delay = self.BASE_DELAY
        elif new_prop.path == gh_gen_four_const.KEY_GAMETIME_SECONDS:
            if self._seconds_delay <= 0:
                return StateType.OVERWORLD
            else:
                self._seconds_delay -= 1

        return self.state_type


class MoveDeleteState(WatchForResetState):
    BASE_DELAY = 2
    def __init__(self, machine: Machine):
        super().__init__(StateType.MOVE_DELETE, machine)
        self._cur_delay = self.BASE_DELAY

    def _on_enter(self, prev_state: State):
        self._cur_delay = self.BASE_DELAY
    
    def _on_exit(self, next_state: State):
        if next_state.state_type != StateType.RESETTING:
            self.machine._move_cache_update(tutor_expected=True)
    
    @auto_reset
    def transition(self, new_prop:GameHookProperty, prev_prop:GameHookProperty) -> StateType:
        if new_prop.path == gh_gen_four_const.KEY_GAMETIME_SECONDS:
            if self._cur_delay <= 0:
                return StateType.OVERWORLD
            else:
                self._cur_delay -= 1
        elif new_prop.path in gh_gen_four_const.ALL_KEYS_PLAYER_MOVES:
            self._cur_delay = self.BASE_DELAY

        return self.state_type


class UseVitaminState(WatchForResetState):
    BASE_DELAY = 2
    ERROR_DELAY = 5
    def __init__(self, machine: Machine):
        super().__init__(StateType.VITAMIN, machine)
        self._item_removal_detected = False
        self._cur_delay = self.BASE_DELAY
        self._error_delay = self.ERROR_DELAY

    def _on_enter(self, prev_state: State):
        self._item_removal_detected = False
        self._cur_delay = self.BASE_DELAY
        self._error_delay = self.ERROR_DELAY
        # Note: Don't cache EVs here - they've already changed by the time we enter this state
        # Use machine's cached EVs from OverworldState instead (cached BEFORE the change)
        logger.info(f"Vitamin state entered. Machine's cached EVs (from before change): {self.machine._cached_evs}")
    
    def _on_exit(self, next_state: State):
        if next_state.state_type != StateType.RESETTING:
            if self._error_delay <= 0:
                logger.error(f"Vitamin state hit error timeout. Will attempt to see if any vitamins were used anyways")
            
            # Check if EVs changed by comparing machine's cached EVs (from overworld) with current EVs
            # Machine's cached_evs were captured in OverworldState BEFORE the EV change happened
            current_evs = {
                'hp': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_HP[0]).value,
                'attack': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_ATTACK[0]).value,
                'defense': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_DEFENSE[0]).value,
                'speed': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_SPEED[0]).value,
                'special_attack': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_SPECIAL_ATTACK[0]).value,
                'special_defense': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_SPECIAL_DEFENSE[0]).value,
            }
            logger.info(f"Vitamin state exiting. Cached EVs (from overworld): {self.machine._cached_evs}, Current EVs: {current_evs}")
            
            # Check if any EV changed by comparing with machine's cached EVs
            evs_changed = any(
                current_evs[stat] != self.machine._cached_evs.get(stat) 
                for stat in current_evs.keys() 
                if self.machine._cached_evs.get(stat) is not None and current_evs[stat] is not None
            )
            
            if evs_changed:
                logger.info(f"EVs changed, vitamin was applied")
                self.machine._item_cache_update(vitamin_flag=True)
            else:
                logger.info(f"EVs did not change, vitamin was dropped/sold")
                self.machine._item_cache_update(vitamin_flag=False)
    
    @auto_reset
    def transition(self, new_prop:GameHookProperty, prev_prop:GameHookProperty) -> StateType:
        if new_prop.path in gh_gen_four_const.ALL_KEYS_ITEM_TYPE:
            self._item_removal_detected = True
        elif new_prop.path == gh_gen_four_const.KEY_GAMETIME_SECONDS:
            if self._item_removal_detected:
                if self._cur_delay <= 0:
                    return StateType.OVERWORLD
                else:
                    self._cur_delay -= 1
            
            if self._error_delay > 0:
                self._error_delay -= 1
            else:
                return StateType.OVERWORLD

        return self.state_type

# Overworld state
class OverworldState(WatchForResetState):
    BASE_DELAY = 2
    SAVE_DELAY = 2
    HEAL_DELAY = 3
    def __init__(self, machine: Machine):
        super().__init__(StateType.OVERWORLD, machine)
        self._waiting_for_registration = False
        self._register_delay = self.BASE_DELAY
        self._propagate_held_item_flag = False
        self._validation_delay = 5
        self._save_detected = False
        self._save_delay = self.SAVE_DELAY
        self._heal_detected = False
        self._heal_delay = self.HEAL_DELAY
        self._previous_save_count = None
    
    def _on_enter(self, prev_state: State):
        self.machine._money_cache_update()
        self.machine.update_team_cache()
        # Check for moves that were learned during evolution
        # This must happen after update_team_cache so move data is current
        if self.machine._pending_evolution_level_check is not None:
            # Update move cache to get accurate post-evolution moves
            self.machine._move_cache_update(generate_events=False)
            # Now check level-up moves with accurate move data
            pre_moves = self.machine._pre_evolution_moves if self.machine._pre_evolution_moves is not None else set()
            self.machine._solo_mon_levelup(self.machine._pending_evolution_level_check, pre_evolution_moves=pre_moves)
            self.machine._pending_evolution_level_check = None
            self.machine._pre_evolution_moves = None
            self.machine._pre_evolution_move_list = None
        self._waiting_for_registration = False
        self._register_delay = self.BASE_DELAY
        self._waiting_for_new_file = False
        self._new_file_delay = self.BASE_DELAY

        self._wrong_mon_delay = self.BASE_DELAY
        if self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_SPECIES).value == None:
            self._waiting_for_solo_mon_in_slot_1 = True
            self._wrong_mon_in_slot_1 = False
        elif self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_SPECIES).value != self.machine._solo_mon_key.species:
            self._waiting_for_solo_mon_in_slot_1 = False
            self._wrong_mon_in_slot_1 = True
        else:
            self._waiting_for_solo_mon_in_slot_1 = False
            self._wrong_mon_in_slot_1 = False
        
        self._validation_delay = 5
        self._save_detected = False
        self._save_delay = self.SAVE_DELAY
        self._heal_detected = False
        self._heal_delay = self.HEAL_DELAY
        # Initialize previous save count from current value if available
        save_count_prop = self.machine._gamehook_client.get(gh_gen_four_const.KEY_SAVE_COUNT)
        if save_count_prop is not None and save_count_prop.value is not None:
            self._previous_save_count = save_count_prop.value
        else:
            self._previous_save_count = None
        
        # Log blackout flag status when entering OVERWORLD
        if self.machine._potential_blackout_flag:
            current_map = self.machine._gamehook_client.get(gh_gen_four_const.KEY_OVERWORLD_MAP).value
            logger.info(f"[BLACKOUT DEBUG] Entered OVERWORLD with blackout flag set. Current map: {current_map}, Initial map: {self.machine._blackout_initial_map}")
        
        # Check for potential blackout if flag is set (in case map changed before we transitioned)
        if self.machine._potential_blackout_flag and self.machine._blackout_initial_map is not None:
            current_map = self.machine._gamehook_client.get(gh_gen_four_const.KEY_OVERWORLD_MAP).value
            if current_map != self.machine._blackout_initial_map:
                # Map has already changed, check team HP
                all_team_hp_zero = True
                for hp_key in gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_HP:
                    hp_value = self.machine._gamehook_client.get(hp_key).value
                    if hp_value is not None and hp_value > 0:
                        all_team_hp_zero = False
                        break
                
                if all_team_hp_zero:
                    logger.info(f"[BLACKOUT DEBUG] Entered OVERWORLD with blackout flag set, map already changed and team HP is 0 - blackout confirmed")
                    logger.info(f"[BLACKOUT DEBUG] Current map: {current_map}, Initial map: {self.machine._blackout_initial_map}")
                    # Handle blackout
                    if self.machine._blackout_trainer_name:
                        logger.info(f"[BLACKOUT DEBUG] Queueing TRAINER_LOSS_FLAG event for trainer: {self.machine._blackout_trainer_name}")
                        self.machine._queue_new_event(
                            EventDefinition(trainer_def=TrainerEventDefinition(self.machine._blackout_trainer_name), notes=gh_gen_four_const.TRAINER_LOSS_FLAG)
                        )
                    
                    # Add any cached Pokemon that haven't been processed yet
                    if self.machine._blackout_cached_first_mon_species or self.machine._blackout_cached_first_mon_level:
                        self.machine._blackout_defeated_trainer_mons.append(EventDefinition(wild_pkmn_info=WildPkmnEventDefinition(
                            self.machine._blackout_cached_first_mon_species,
                            self.machine._blackout_cached_first_mon_level,
                            trainer_pkmn=True
                        )))
                    if self.machine._blackout_cached_second_mon_species or self.machine._blackout_cached_second_mon_level:
                        self.machine._blackout_defeated_trainer_mons.append(EventDefinition(wild_pkmn_info=WildPkmnEventDefinition(
                            self.machine._blackout_cached_second_mon_species,
                            self.machine._blackout_cached_second_mon_level,
                            trainer_pkmn=True
                        )))
                    
                    # Add all defeated trainer Pokemon
                    for trainer_mon_event in self.machine._blackout_defeated_trainer_mons:
                        self.machine._queue_new_event(trainer_mon_event)
                    
                    # Add blackout event
                    self.machine._queue_new_event(EventDefinition(blackout=BlackoutEventDefinition()))
                    
                    # Clear blackout flags
                    self.machine._potential_blackout_flag = False
                    self.machine._blackout_cached_first_mon_species = ""
                    self.machine._blackout_cached_first_mon_level = 0
                    self.machine._blackout_cached_second_mon_species = ""
                    self.machine._blackout_cached_second_mon_level = 0
                    self.machine._blackout_defeated_trainer_mons = []
                    self.machine._blackout_trainer_name = ""
                    self.machine._blackout_initial_map = None
                else:
                    logger.info(f"Entered OVERWORLD with blackout flag set, but team HP is not 0 - not a blackout, clearing flag")
                    # Clear blackout flags since it's not actually a blackout
                    self.machine._potential_blackout_flag = False
                    self.machine._blackout_cached_first_mon_species = ""
                    self.machine._blackout_cached_first_mon_level = 0
                    self.machine._blackout_cached_second_mon_species = ""
                    self.machine._blackout_cached_second_mon_level = 0
                    self.machine._blackout_defeated_trainer_mons = []
                    self.machine._blackout_trainer_name = ""
                    self.machine._blackout_initial_map = None
        
        # Cache current EVs for vitamin detection (cache early in overworld, before inventory changes)
        self._update_ev_cache()
    
    def _update_ev_cache(self):
        """Update the machine's EV cache for vitamin detection"""
        self.machine._cached_evs = {
            'hp': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_HP[0]).value,
            'attack': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_ATTACK[0]).value,
            'defense': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_DEFENSE[0]).value,
            'speed': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_SPEED[0]).value,
            'special_attack': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_SPECIAL_ATTACK[0]).value,
            'special_defense': self.machine._gamehook_client.get(gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_EV_SPECIAL_DEFENSE[0]).value,
        }
    
    def _on_exit(self, next_state: State):
        if isinstance(next_state, InventoryChangeState):
            next_state.external_held_item_flag = self._propagate_held_item_flag
        
        self._propagate_held_item_flag = False
    
    def _validate(self):
        logger.info("*" * 50)
        logger.info("Attempting validation!")
        valid = True
        cur_state = self.machine._controller._controller.get_final_state()
        live_xp = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_EXPPOINTS).value
        if live_xp != cur_state.solo_pkmn.cur_xp:
            logger.error(f"VALIDATION FAILED for xp: {cur_state.solo_pkmn.cur_xp} vs {live_xp}")
            valid = False
        cur_total_evs = cur_state.solo_pkmn.unrealized_stat_xp.add(cur_state.solo_pkmn.realized_stat_xp)
        hp_ev = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_STAT_EXP_HP).value
        if hp_ev != cur_total_evs.hp:
            logger.error(f"VALIDATION FAILED for HP EV: {cur_total_evs.hp} vs {hp_ev}")
            valid = False
        attack_ev = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_STAT_EXP_ATTACK).value
        if attack_ev != cur_total_evs.attack:
            logger.error(f"VALIDATION FAILED for attack EV: {cur_total_evs.attack} vs {attack_ev}")
            valid = False
        defense_ev = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_STAT_EXP_DEFENSE).value
        if defense_ev != cur_total_evs.defense:
            logger.error(f"VALIDATION FAILED for defense EV: {cur_total_evs.defense} vs {defense_ev}")
            valid = False
        special_attack_ev = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_STAT_EXP_SPECIAL_ATTACK).value
        if special_attack_ev != cur_total_evs.special_attack:
            logger.error(f"VALIDATION FAILED for special attack EV: {cur_total_evs.special_attack} vs {special_attack_ev}")
            valid = False
        special_defense_ev = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_STAT_EXP_SPECIAL_DEFENSE).value
        if special_defense_ev != cur_total_evs.special_defense:
            logger.error(f"VALIDATION FAILED for special defense EV: {cur_total_evs.special_attack} vs {special_defense_ev}")
            valid = False
        speed_ev = self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_STAT_EXP_SPEED).value
        if speed_ev != cur_total_evs.speed:
            logger.error(f"VALIDATION FAILED for speed EV: {cur_total_evs.speed} vs {speed_ev}")
            valid = False
        
        if valid:
            logger.info("VALIDATION SUCCESS!!!")
        logger.info("*" * 50)
    
    @auto_reset
    def transition(self, new_prop:GameHookProperty, prev_prop:GameHookProperty) -> StateType:
        # intentionally ignore all updates while waiting for a new file
        if self._waiting_for_new_file or self._waiting_for_solo_mon_in_slot_1:
            check_for_battle = False
            if new_prop.path == gh_gen_four_const.KEY_GAMETIME_SECONDS:
                if self._wrong_mon_delay <= 0:
                    self._waiting_for_solo_mon_in_slot_1 = False
                    self._wrong_mon_in_slot_1 = False
                    check_for_battle = True
                if self._new_file_delay <= 0:
                    self._waiting_for_new_file = False
                    self.machine._controller.route_restarted()
                    self.machine.update_all_cached_info()
                    check_for_battle = True
                self._new_file_delay -= 1
                self._wrong_mon_delay -= 1
            
            if (
                check_for_battle and
                self.machine._gamehook_client.get(gh_gen_four_const.META_STATE).value == 'Battle' and
                self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_OUTCOME).value is None
            ):
                logger.info(f"tranitioning into battle pre-emptively")
                return StateType.BATTLE
            return self.state_type

        elif new_prop.path == gh_gen_four_const.KEY_OVERWORLD_MAP:
            self.machine._controller.entered_new_area(
                f"{self.machine._gamehook_client.get(gh_gen_four_const.KEY_OVERWORLD_MAP).value}"
            )
            
            # Check for potential blackout if flag is set
            if self.machine._potential_blackout_flag:
                logger.info(f"[BLACKOUT DEBUG] Map changed, checking for blackout. Old map: {prev_prop.value}, New map: {new_prop.value}")
                logger.info(f"[BLACKOUT DEBUG] Initial map was: {self.machine._blackout_initial_map}")
                logger.info(f"[BLACKOUT DEBUG] Trainer name: {self.machine._blackout_trainer_name}")
                
                # Verify that team HP is actually 0 before confirming blackout
                all_team_hp_zero = True
                team_hp_values = []
                for hp_key in gh_gen_four_const.ALL_KEYS_PLAYER_TEAM_HP:
                    hp_value = self.machine._gamehook_client.get(hp_key).value
                    team_hp_values.append(hp_value)
                    if hp_value is not None and hp_value > 0:
                        all_team_hp_zero = False
                
                logger.info(f"[BLACKOUT DEBUG] Team HP values: {team_hp_values}, all_zero: {all_team_hp_zero}")
                
                if all_team_hp_zero:
                    logger.info(f"[BLACKOUT DEBUG] Map changed and team HP is 0 - blackout confirmed")
                    # Handle blackout: remove trainer event, add defeated Pokemon, add blackout event
                    if self.machine._blackout_trainer_name:
                        logger.info(f"[BLACKOUT DEBUG] Queueing TRAINER_LOSS_FLAG event for trainer: {self.machine._blackout_trainer_name}")
                        self.machine._queue_new_event(
                            EventDefinition(trainer_def=TrainerEventDefinition(self.machine._blackout_trainer_name), notes=gh_gen_four_const.TRAINER_LOSS_FLAG)
                        )
                    
                    # Add any cached Pokemon that haven't been processed yet
                    if self.machine._blackout_cached_first_mon_species or self.machine._blackout_cached_first_mon_level:
                        self.machine._blackout_defeated_trainer_mons.append(EventDefinition(wild_pkmn_info=WildPkmnEventDefinition(
                            self.machine._blackout_cached_first_mon_species,
                            self.machine._blackout_cached_first_mon_level,
                            trainer_pkmn=True
                        )))
                    if self.machine._blackout_cached_second_mon_species or self.machine._blackout_cached_second_mon_level:
                        self.machine._blackout_defeated_trainer_mons.append(EventDefinition(wild_pkmn_info=WildPkmnEventDefinition(
                            self.machine._blackout_cached_second_mon_species,
                            self.machine._blackout_cached_second_mon_level,
                            trainer_pkmn=True
                        )))
                    
                    # Add all defeated trainer Pokemon
                    for trainer_mon_event in self.machine._blackout_defeated_trainer_mons:
                        self.machine._queue_new_event(trainer_mon_event)
                    
                    # Add blackout event
                    self.machine._queue_new_event(EventDefinition(blackout=BlackoutEventDefinition()))
                    
                    # Clear blackout flags
                    self.machine._potential_blackout_flag = False
                    self.machine._blackout_cached_first_mon_species = ""
                    self.machine._blackout_cached_first_mon_level = 0
                    self.machine._blackout_cached_second_mon_species = ""
                    self.machine._blackout_cached_second_mon_level = 0
                    self.machine._blackout_defeated_trainer_mons = []
                    self.machine._blackout_trainer_name = ""
                    self.machine._blackout_initial_map = None
                else:
                    logger.info(f"Map changed but team HP is not 0 - not a blackout, clearing flag")
                    # Clear blackout flags since it's not actually a blackout
                    self.machine._potential_blackout_flag = False
                    self.machine._blackout_cached_first_mon_species = ""
                    self.machine._blackout_cached_first_mon_level = 0
                    self.machine._blackout_cached_second_mon_species = ""
                    self.machine._blackout_cached_second_mon_level = 0
                    self.machine._blackout_defeated_trainer_mons = []
                    self.machine._blackout_trainer_name = ""
                    self.machine._blackout_initial_map = None
        elif new_prop.path == gh_gen_four_const.KEY_PLAYER_PLAYERID:
            if prev_prop.value and self.machine._player_id != self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_PLAYERID).value:
                self._waiting_for_new_file = True
        elif new_prop.path == gh_gen_four_const.KEY_PLAYER_MON_HELD_ITEM:
            self._propagate_held_item_flag = True
            if False:
                self.machine._queue_new_event(
                    EventDefinition(
                        hold_item=HoldItemEventDefinition(self.machine.gh_converter.item_name_convert(new_prop.value)),
                        notes=gh_gen_four_const.HELD_CHECK_FLAG
                    )
                )
        elif new_prop.path in gh_gen_four_const.ALL_KEYS_ALL_ITEM_FIELDS:
            return StateType.INVENTORY_CHANGE
        elif new_prop.path == gh_gen_four_const.KEY_PLAYER_MON_SPECIES:
            if not prev_prop.value:
                self._waiting_for_registration = True
            elif self.machine._solo_mon_key.species == self.machine.gh_converter.pkmn_name_convert(prev_prop.value):
                self._wrong_mon_in_slot_1 = True
            elif self.machine._solo_mon_key.species == self.machine.gh_converter.pkmn_name_convert(new_prop.value):
                self._wrong_mon_delay = self.BASE_DELAY
                self._waiting_for_solo_mon_in_slot_1 = True
        elif new_prop.path == gh_gen_four_const.KEY_PLAYER_MON_LEVEL:
            if not self._waiting_for_registration and not self._wrong_mon_in_slot_1:
                return StateType.RARE_CANDY
        elif new_prop.path in gh_gen_four_const.ALL_KEYS_PLAYER_MOVES:
            if not self._waiting_for_registration and not self._wrong_mon_in_slot_1:
                all_cur_moves = []
                for move_path in gh_gen_four_const.ALL_KEYS_PLAYER_MOVES:
                    if move_path == prev_prop.path:
                        all_cur_moves.append(prev_prop.value)
                    else:
                        all_cur_moves.append(self.machine._gamehook_client.get(move_path).value)
                if new_prop.value is None or new_prop.value in all_cur_moves:
                    return StateType.MOVE_DELETE
                else:
                    return StateType.TM
        # Note: EV changes are now handled in InventoryChangeState (for vitamins)
        # This is similar to how rare candy is handled - item change triggers INVENTORY_CHANGE,
        # then level/EV change triggers RARE_CANDY/VITAMIN state
        # elif new_prop.path in gh_gen_four_const.ALL_KEYS_STAT_EXP:
        #     if not self._waiting_for_registration and not self._wrong_mon_in_slot_1:
        #         return StateType.VITAMIN
        elif new_prop.path == gh_gen_four_const.KEY_SAVE_COUNT:
            # Track save count increments. Only add save event if:
            # 1. The value increments by exactly 1 (new_value == prev_value + 1)
            # 2. If prev_value is 0, only allow if new_value is 1 (first save)
            #    If prev_value > 0, allow any increment of 1 (prevents jumps from 0 to >1 on reset)
            # 3. We're in overworld (already handled by being in OverworldState)
            if (
                prev_prop.value is not None and
                new_prop.value is not None and
                new_prop.value == prev_prop.value + 1 and
                (prev_prop.value != 0 or new_prop.value == 1)
            ):
                self._save_detected = True
                self._save_delay = self.SAVE_DELAY
            # Update previous save count for next check
            self._previous_save_count = new_prop.value
        elif new_prop.path == gh_gen_four_const.KEY_AUDIO_SOUND_EFFECT_1:
            # NOTE: Legacy save detection via sound effect - kept for reference but no longer used
            # This is replaced by KEY_SAVE_COUNT tracking above
            pass
        elif new_prop.path == gh_gen_four_const.KEY_AUDIO_SOUND_EFFECT_2:
            # NOTE: same limitations as above
            # Set flag when heal sound effect is detected, delay will be handled on gametime ticks
            if new_prop.value == gh_gen_four_const.HEAL_SOUND_EFFECT_VALUE and prev_prop.value != gh_gen_four_const.HEAL_SOUND_EFFECT_VALUE:
                self._heal_detected = True
                self._heal_delay = self.HEAL_DELAY
        elif new_prop.path == gh_gen_four_const.KEY_GAMETIME_SECONDS:
            if self._waiting_for_registration:
                if self._register_delay <= 0:
                    self._waiting_for_registration = False
                    self.machine.update_team_cache(regenerate_move_cache=True)
                self._register_delay -= 1
            elif self._wrong_mon_in_slot_1:
                if self._wrong_mon_delay <= 0:
                    self.machine.update_team_cache(regenerate_move_cache=True)
                    self._wrong_mon_in_slot_1 = (
                        self.machine._solo_mon_key.species != 
                        self.machine.gh_converter.pkmn_name_convert(self.machine._gamehook_client.get(gh_gen_four_const.KEY_PLAYER_MON_SPECIES).value)
                    )
                self._wrong_mon_delay -= 1
            
            # Handle save delay using gametime ticks (independent check, can run with other conditions)
            if self._save_detected:
                if self._save_delay <= 0:
                    self.machine._queue_new_event(EventDefinition(save=SaveEventDefinition(location=self.machine._gamehook_client.get(gh_gen_four_const.KEY_OVERWORLD_MAP).value)))
                    self._save_detected = False
                    self._save_delay = self.SAVE_DELAY
                else:
                    self._save_delay -= 1
            
            # Handle heal delay using gametime ticks (independent check, can run with other conditions)
            if self._heal_detected:
                if self._heal_delay <= 0:
                    self.machine._queue_new_event(EventDefinition(heal=HealEventDefinition(location=self.machine._gamehook_client.get(gh_gen_four_const.KEY_OVERWORLD_MAP).value)))
                    self._heal_detected = False
                    self._heal_delay = self.HEAL_DELAY
                else:
                    self._heal_delay -= 1

            
            if self._validation_delay > 0:
                self._validation_delay -= 1
            elif self._validation_delay == 0:
                #self._validate()
                self._validation_delay -= 1
            
            # Update EV cache every game second for vitamin detection
            self._update_ev_cache()

            if (
                self.machine._gamehook_client.get(gh_gen_four_const.META_STATE).value == 'Battle' and
                self.machine._gamehook_client.get(gh_gen_four_const.KEY_BATTLE_OUTCOME).value is None
            ):
                return StateType.BATTLE


        return self.state_type
