"""
Integration tests for route processing.

Each test loads a real route file, processes it through the Router,
and verifies that key calculated values (level, XP, stats, stat XP, etc.)
match expected results at checkpoints throughout the route.

These tests serve as regression tests: if core calculation logic changes,
these will catch unintended differences.
"""
import pytest


# =============================================================================
# Yellow - Pinsir route (25 events, exercises Brock badge + various wilds)
# =============================================================================
class TestYellowPinsirRoute:
    ROUTE_FILE = "yellow-pinsir-lv10brock.json"

    def test_loads_without_errors(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        assert router.pkmn_version == "Yellow"
        assert len(events) == 25

    def test_initial_state(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        init = router.init_route_state.solo_pkmn
        assert init.name == "Pinsir"
        assert init.cur_level == 5

    def test_final_state(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        final = router.get_final_state().solo_pkmn
        assert final.name == "Pinsir"
        assert final.cur_level == 11
        assert final.cur_xp == 1947
        assert final.xp_to_next_level == 213

    def test_final_stats(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        stats = router.get_final_state().solo_pkmn.cur_stats
        assert stats.hp == 39
        assert stats.attack == 40
        assert stats.defense == 31
        assert stats.special_attack == 20
        assert stats.special_defense == 20
        assert stats.speed == 27

    def test_final_stat_xp(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        stat_xp = router.get_final_state().solo_pkmn.realized_stat_xp
        assert stat_xp.hp == 800
        assert stat_xp.attack == 660
        assert stat_xp.defense == 835
        assert stat_xp.special_attack == 485
        assert stat_xp.speed == 791

    def test_final_inventory(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        final = router.get_final_state()
        assert final.inventory.cur_money == 1753

    def test_checkpoint_event_6(self, load_route):
        """After BugCatcher 2 in Viridian Forest."""
        router, events = load_route(self.ROUTE_FILE)
        mon = events[6].final_state.solo_pkmn
        assert mon.cur_level == 8
        assert mon.cur_xp == 660
        assert mon.cur_stats.hp == 31
        assert mon.cur_stats.attack == 27
        assert mon.cur_stats.defense == 23
        assert mon.cur_stats.speed == 21

    def test_checkpoint_event_12(self, load_route):
        """After wild Caterpie battle."""
        router, events = load_route(self.ROUTE_FILE)
        mon = events[12].final_state.solo_pkmn
        assert mon.cur_level == 9
        assert mon.cur_xp == 988
        assert mon.cur_stats.hp == 33
        assert mon.cur_stats.attack == 30

    def test_brock_fight(self, load_route):
        """Brock is the last event - verify level up from 10 to 11."""
        router, events = load_route(self.ROUTE_FILE)
        brock = events[24]
        assert brock.event_definition.trainer_def is not None
        assert "Brock" in brock.event_definition.trainer_def.trainer_name

        before = brock.init_state.solo_pkmn
        assert before.cur_level == 10
        assert before.cur_stats.hp == 36
        assert before.cur_stats.attack == 33

        after = brock.final_state.solo_pkmn
        assert after.cur_level == 11
        assert after.cur_stats.hp == 39
        assert after.cur_stats.attack == 40

    def test_no_event_errors(self, load_route):
        """No events should have error messages."""
        router, events = load_route(self.ROUTE_FILE)
        for eg in events:
            for item in eg.event_items:
                assert item.error_message == "", (
                    f"Unexpected error at event '{eg.event_definition}': {item.error_message}"
                )


# =============================================================================
# Crystal - Porygon/Charizard route (19 events, gen 2 mechanics)
# =============================================================================
class TestCrystalPorygonRoute:
    ROUTE_FILE = "c-porygon-1-.json"

    def test_loads_without_errors(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        assert router.pkmn_version == "Crystal"
        assert len(events) == 19

    def test_initial_state(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        init = router.init_route_state.solo_pkmn
        assert init.name == "Charizard"
        assert init.cur_level == 5

    def test_final_state(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        final = router.get_final_state().solo_pkmn
        assert final.name == "Charizard"
        assert final.cur_level == 10
        assert final.cur_xp == 613
        assert final.xp_to_next_level == 129

    def test_final_stats(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        stats = router.get_final_state().solo_pkmn.cur_stats
        assert stats.hp == 39
        assert stats.attack == 25
        assert stats.defense == 23
        assert stats.special_attack == 30
        assert stats.special_defense == 25
        assert stats.speed == 28

    def test_final_stat_xp(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        stat_xp = router.get_final_state().solo_pkmn.realized_stat_xp
        assert stat_xp.hp == 325
        assert stat_xp.attack == 360
        assert stat_xp.defense == 320
        assert stat_xp.special_attack == 250
        assert stat_xp.special_defense == 276
        assert stat_xp.speed == 445

    def test_final_moves(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        moves = router.get_final_state().solo_pkmn.move_list
        assert moves == ["Scratch", "Growl", "Ember", "Smokescreen"]

    def test_final_money(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        assert router.get_final_state().inventory.cur_money == 3796

    def test_checkpoint_event_4(self, load_route):
        """After finding Antidote."""
        router, events = load_route(self.ROUTE_FILE)
        mon = events[4].final_state.solo_pkmn
        assert mon.cur_level == 5
        assert mon.cur_xp == 135
        assert mon.cur_stats.hp == 24

    def test_checkpoint_event_9(self, load_route):
        """After Youngster Mikey on Route 30."""
        router, events = load_route(self.ROUTE_FILE)
        mon = events[9].final_state.solo_pkmn
        assert mon.cur_level == 7
        assert mon.cur_xp == 272
        assert mon.cur_stats.hp == 30
        assert mon.cur_stats.attack == 19
        assert mon.cur_stats.speed == 21

    def test_no_event_errors(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        for eg in events:
            for item in eg.event_items:
                assert item.error_message == "", (
                    f"Unexpected error at event '{eg.event_definition}': {item.error_message}"
                )


# =============================================================================
# FireRed - Ditto route (36 events, gen 3 mechanics, high level)
# =============================================================================
class TestFireRedDittoRoute:
    ROUTE_FILE = "f-ditto-tests.json"

    def test_loads_without_errors(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        assert router.pkmn_version == "FireRed"
        assert len(events) == 36

    def test_initial_state(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        init = router.init_route_state.solo_pkmn
        assert init.name == "Ditto"
        assert init.cur_level == 5

    def test_final_state(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        final = router.get_final_state().solo_pkmn
        assert final.name == "Ditto"
        assert final.cur_level == 99
        assert final.cur_xp == 984653
        assert final.xp_to_next_level == 15347

    def test_final_stats(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        stats = router.get_final_state().solo_pkmn.cur_stats
        assert stats.hp == 234
        assert stats.attack == 159
        assert stats.defense == 131
        assert stats.special_attack == 118
        assert stats.special_defense == 131
        assert stats.speed == 131

    def test_final_stat_xp(self, load_route):
        """FireRed uses EVs (0-255 range), not stat XP."""
        router, events = load_route(self.ROUTE_FILE)
        evs = router.get_final_state().solo_pkmn.realized_stat_xp
        assert evs.hp == 2
        assert evs.attack == 60
        assert evs.defense == 6
        assert evs.special_attack == 10
        assert evs.special_defense == 4
        assert evs.speed == 7

    def test_final_money(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        assert router.get_final_state().inventory.cur_money == 32100

    def test_checkpoint_event_9(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        mon = events[9].final_state.solo_pkmn
        assert mon.cur_level == 97
        assert mon.cur_xp == 928753

    def test_xp_monotonically_increases(self, load_route):
        """XP should never decrease through the route."""
        router, events = load_route(self.ROUTE_FILE)
        prev_xp = 0
        for eg in events:
            cur_xp = eg.final_state.solo_pkmn.cur_xp
            assert cur_xp >= prev_xp, (
                f"XP decreased from {prev_xp} to {cur_xp} at event '{eg.event_definition}'"
            )
            prev_xp = cur_xp

    def test_level_monotonically_increases(self, load_route):
        """Level should never decrease through the route."""
        router, events = load_route(self.ROUTE_FILE)
        prev_level = 0
        for eg in events:
            cur_level = eg.final_state.solo_pkmn.cur_level
            assert cur_level >= prev_level, (
                f"Level decreased from {prev_level} to {cur_level}"
            )
            prev_level = cur_level


# =============================================================================
# Platinum - Chimchar route (15 events, gen 4 mechanics)
# =============================================================================
class TestPlatinumChimcharRoute:
    ROUTE_FILE = "platinum_chimchar.json"

    def test_loads_without_errors(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        assert router.pkmn_version == "Platinum"
        assert len(events) == 15

    def test_initial_state(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        init = router.init_route_state.solo_pkmn
        assert init.name == "Chimchar"
        assert init.cur_level == 5

    def test_final_state(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        final = router.get_final_state().solo_pkmn
        assert final.name == "Chimchar"
        assert final.cur_level == 8
        assert final.cur_xp == 328
        assert final.xp_to_next_level == 91

    def test_final_stats(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        stats = router.get_final_state().solo_pkmn.cur_stats
        assert stats.hp == 27
        assert stats.attack == 16
        assert stats.defense == 14
        assert stats.special_attack == 16
        assert stats.special_defense == 14
        assert stats.speed == 17

    def test_final_stat_xp(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        evs = router.get_final_state().solo_pkmn.realized_stat_xp
        assert evs.hp == 3
        assert evs.attack == 0
        assert evs.defense == 0
        assert evs.special_attack == 0
        assert evs.special_defense == 0
        assert evs.speed == 1

    def test_final_moves(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        moves = router.get_final_state().solo_pkmn.move_list
        assert moves == ["Scratch", "Leer", "Ember", None]

    def test_final_money(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        assert router.get_final_state().inventory.cur_money == 3060

    def test_checkpoint_event_3(self, load_route):
        """After finding Antidote."""
        router, events = load_route(self.ROUTE_FILE)
        mon = events[3].final_state.solo_pkmn
        assert mon.cur_level == 5
        assert mon.cur_xp == 159

    def test_checkpoint_event_7(self, load_route):
        """After School Kid Christine."""
        router, events = load_route(self.ROUTE_FILE)
        mon = events[7].final_state.solo_pkmn
        assert mon.cur_level == 8
        assert mon.cur_xp == 328
        assert mon.cur_stats.hp == 27
        assert mon.cur_stats.speed == 17

    def test_no_event_errors(self, load_route):
        router, events = load_route(self.ROUTE_FILE)
        for eg in events:
            for item in eg.event_items:
                assert item.error_message == "", (
                    f"Unexpected error at event '{eg.event_definition}': {item.error_message}"
                )
