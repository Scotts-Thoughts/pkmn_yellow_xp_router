from __future__ import annotations
import logging
from typing import List
from route_recording.game_recorders.gen_five.black_fsm import Machine

import route_recording.recorder
from route_recording.game_recorders.gen_five import black_states
from route_recording.game_recorders.gen_five.black_gamehook_constants import gh_gen_five_const, GameHookConstantConverter
from utils.constants import const

logger = logging.getLogger(__name__)


class BlackRecorder(route_recording.recorder.RecorderGameHookClient):
    def __init__(self, controller:route_recording.recorder.RecorderGameHookClient, expected_names:List[str], is_white=False):
        super().__init__(controller, expected_names)

        if is_white:
            gh_gen_five_const.configure_for_white()
        else:
            gh_gen_five_const.configure_for_black()

        self._machine = Machine(controller, self, GameHookConstantConverter(is_white=is_white), is_white=is_white)

        """
        self._machine.register(black_states.WatchState(self._machine))

        """
        self._machine.register(black_states.UninitializedState(self._machine))
        self._machine.register(black_states.ResettingState(self._machine))
        self._machine.register(black_states.BattleState(self._machine))
        self._machine.register(black_states.InventoryChangeState(self._machine))
        self._machine.register(black_states.UseRareCandyState(self._machine))
        self._machine.register(black_states.UseTMState(self._machine))
        self._machine.register(black_states.MoveDeleteState(self._machine))
        self._machine.register(black_states.UseVitaminState(self._machine))
        self._machine.register(black_states.OverworldState(self._machine))

    def on_mapper_loaded(self):
        result = super().on_mapper_loaded()

        if self._controller.is_ready():
            self.validate_constants(gh_gen_five_const)
            # The Black mapper is missing a number of paths the constants
            # file references (see gen_five/MAPPER_GAPS.md). validate_constants
            # logs but does not strip them, so guard the registration loop:
            # skip None entries and any path the mapper doesn't actually serve.
            for cur_key in gh_gen_five_const.ALL_KEYS_TO_REGISTER:
                if cur_key is None:
                    continue
                prop = self.get(cur_key)
                if prop is None:
                    logger.warning(f"Skipping registration of unmapped path: {cur_key}")
                    continue
                prop.change(self._machine.handle_event)

            if not self._machine._active:
                self._machine.startup()

        return result

    def disconnect(self):
        result = super().disconnect()
        self._machine.shutdown()
        return result
