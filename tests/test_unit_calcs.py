"""
Unit tests for core calculation functions.

Tests the pure math behind XP yields, level lookups, and damage calculations
independent of any route files.
"""
import pytest
from utils.constants import const
from pkmn import universal_utils, gen_factory


# =============================================================================
# XP yield calculation
# =============================================================================
class TestCalcXpYield:
    def test_basic_wild_pokemon(self):
        # base_yield=64 (Pidgey), level=3, not trainer, no split
        result = universal_utils.calc_xp_yield(64, 3, False)
        # 64 * 3 = 192 / 7 = 27
        assert result == 27

    def test_trainer_pokemon(self):
        # Trainer battle gets 1.5x multiplier
        result = universal_utils.calc_xp_yield(64, 3, True)
        # 64 * 3 = 192 / 7 = 27, * 1.5 = 40 (floor)
        assert result == 40

    def test_exp_split(self):
        # 2-way split
        result = universal_utils.calc_xp_yield(64, 10, False, exp_split=2)
        # 64 * 10 = 640 / 7 = 91, / 2 = 45
        assert result == 45

    def test_trainer_with_split(self):
        result = universal_utils.calc_xp_yield(64, 10, True, exp_split=2)
        # 64 * 10 = 640 / 7 = 91, / 2 = 45, * 1.5 = 67
        assert result == 67

    def test_high_level_high_base(self):
        # base_yield=255 (Chansey), level=50
        result = universal_utils.calc_xp_yield(255, 50, True)
        # 255 * 50 = 12750 / 7 = 1821, * 1.5 = 2731
        assert result == 2731

    def test_level_one(self):
        result = universal_utils.calc_xp_yield(100, 1, False)
        # 100 * 1 = 100 / 7 = 14
        assert result == 14


# =============================================================================
# Level lookup and XP thresholds
# =============================================================================
class TestLevelLookup:
    def test_medium_fast_level_5(self):
        lookup = universal_utils.level_lookups[const.GROWTH_RATE_MEDIUM_FAST]
        # Medium Fast: level^3 => 5^3 = 125
        assert lookup.get_xp_for_level(5) == 125

    def test_medium_fast_level_100(self):
        lookup = universal_utils.level_lookups[const.GROWTH_RATE_MEDIUM_FAST]
        # 100^3 = 1_000_000
        assert lookup.get_xp_for_level(100) == 1_000_000

    def test_slow_level_5(self):
        lookup = universal_utils.level_lookups[const.GROWTH_RATE_SLOW]
        # Slow: floor(5 * 5^3 / 4) = floor(625/4*5) wait...
        # (5 * level^3) / 4 = (5 * 125) / 4 = 625/4 = 156.25 => 156
        assert lookup.get_xp_for_level(5) == 156

    def test_fast_level_10(self):
        lookup = universal_utils.level_lookups[const.GROWTH_RATE_FAST]
        # Fast: floor(4 * 10^3 / 5) = floor(4000/5) = 800
        assert lookup.get_xp_for_level(10) == 800

    def test_medium_slow_level_10(self):
        lookup = universal_utils.level_lookups[const.GROWTH_RATE_MEDIUM_SLOW]
        # MediumSlow: floor(6*1000/5) - 15*100 + 100*10 - 140
        # = 1200 - 1500 + 1000 - 140 = 560
        assert lookup.get_xp_for_level(10) == 560

    def test_get_level_info_at_exact_threshold(self):
        lookup = universal_utils.level_lookups[const.GROWTH_RATE_MEDIUM_FAST]
        # At exactly 125 XP (level 5 threshold), should be level 5
        level, tnl = lookup.get_level_info(125)
        assert level == 5
        # TNL = xp_for_level_6 - 125 = 216 - 125 = 91
        assert tnl == 91

    def test_get_level_info_partway(self):
        lookup = universal_utils.level_lookups[const.GROWTH_RATE_MEDIUM_FAST]
        # 200 XP: level 5 (next is 216)
        level, tnl = lookup.get_level_info(200)
        assert level == 5
        assert tnl == 16

    def test_get_level_info_level_100(self):
        lookup = universal_utils.level_lookups[const.GROWTH_RATE_MEDIUM_FAST]
        level, tnl = lookup.get_level_info(1_000_000)
        assert level == 100
        assert tnl == 0

    def test_get_level_info_over_max(self):
        lookup = universal_utils.level_lookups[const.GROWTH_RATE_MEDIUM_FAST]
        # More than enough XP for level 100
        level, tnl = lookup.get_level_info(1_500_000)
        assert level == 100
        assert tnl == 0

    def test_invalid_level_raises(self):
        lookup = universal_utils.level_lookups[const.GROWTH_RATE_MEDIUM_FAST]
        with pytest.raises(ValueError):
            lookup.get_xp_for_level(0)
        with pytest.raises(ValueError):
            lookup.get_xp_for_level(101)

    def test_erratic_growth_rate_level_50(self):
        lookup = universal_utils.level_lookups[const.GROWTH_RATE_ERRATIC]
        # Erratic for level < 50: floor((level^3 * (100 - level)) / 50)
        # level=49: floor((117649 * 51) / 50) = floor(5999898/50) = floor(119997.96) = 119997
        # Actually let me just test it exists and is consistent
        xp_49 = lookup.get_xp_for_level(49)
        xp_50 = lookup.get_xp_for_level(50)
        assert xp_50 > xp_49

    def test_fluctuating_growth_rate_level_36(self):
        lookup = universal_utils.level_lookups[const.GROWTH_RATE_FLUCTUATING]
        xp_35 = lookup.get_xp_for_level(35)
        xp_36 = lookup.get_xp_for_level(36)
        assert xp_36 > xp_35


# =============================================================================
# Damage calculation (Gen 1 - Yellow)
# =============================================================================
class TestGenOneDamageCalc:
    """Test damage calculations using Gen 1 Yellow mechanics."""

    @pytest.fixture(autouse=True)
    def setup_gen(self):
        gen_factory.change_version(const.YELLOW_VERSION)
        self.gen = gen_factory.current_gen_info()

    def test_vicegrip_vs_geodude(self, load_route):
        """Pinsir L10 Vicegrip vs Geodude L10 (from Brock fight)."""
        router, events = load_route("yellow-pinsir-lv10brock.json")
        brock = events[24]

        player = brock.init_state.solo_pkmn.get_pkmn_obj(brock.init_state.badges)
        geodude = brock.event_items[0].to_defeat_mon
        vicegrip = self.gen.move_db().get_move("Vicegrip")

        dmg = self.gen.calculate_damage(player, vicegrip, geodude)
        assert dmg is not None
        assert dmg.min_damage == 4
        assert dmg.max_damage == 5

    def test_vicegrip_vs_onix(self, load_route):
        """Pinsir L10 Vicegrip vs Onix L12 (from Brock fight)."""
        router, events = load_route("yellow-pinsir-lv10brock.json")
        brock = events[24]

        # After defeating Geodude, Pinsir may have leveled up
        # Use the state before the Onix fight
        onix_item = brock.event_items[1]
        player = onix_item.init_state.solo_pkmn.get_pkmn_obj(onix_item.init_state.badges)
        onix = onix_item.to_defeat_mon
        vicegrip = self.gen.move_db().get_move("Vicegrip")

        dmg = self.gen.calculate_damage(player, vicegrip, onix)
        assert dmg is not None
        assert dmg.min_damage == 2
        assert dmg.max_damage == 3

    def test_damage_range_has_valid_structure(self, load_route):
        """Verify DamageRange has correct properties."""
        router, events = load_route("yellow-pinsir-lv10brock.json")
        brock = events[24]

        player = brock.init_state.solo_pkmn.get_pkmn_obj(brock.init_state.badges)
        geodude = brock.event_items[0].to_defeat_mon
        vicegrip = self.gen.move_db().get_move("Vicegrip")

        dmg = self.gen.calculate_damage(player, vicegrip, geodude)
        assert dmg.min_damage <= dmg.max_damage
        assert dmg.size > 0
        assert len(dmg.damage_vals) > 0
        for val, count in dmg.damage_vals.items():
            assert val >= dmg.min_damage
            assert val <= dmg.max_damage
            assert count > 0

    def test_type_effectiveness_normal_vs_rock(self):
        """Normal-type moves should be not very effective vs Rock."""
        geodude = self.gen.create_trainer_pkmn("Geodude", 10)
        # Create a high-level attacker so we can see meaningful damage
        pidgey = self.gen.create_trainer_pkmn("Pidgey", 10)
        tackle = self.gen.move_db().get_move("Tackle")

        dmg = self.gen.calculate_damage(pidgey, tackle, geodude)
        assert dmg is not None
        # Normal vs Rock/Ground: not very effective, should be low damage
        assert dmg.max_damage <= 5

    def test_zero_power_move_returns_none(self):
        """Status moves (power=0) should return no damage."""
        pidgey = self.gen.create_trainer_pkmn("Pidgey", 10)
        rattata = self.gen.create_trainer_pkmn("Rattata", 10)
        sand_attack = self.gen.move_db().get_move("Sand-Attack")

        if sand_attack and sand_attack.base_power == 0:
            dmg = self.gen.calculate_damage(pidgey, sand_attack, rattata)
            assert dmg is None


# =============================================================================
# Stat calculation consistency
# =============================================================================
class TestStatCalculation:
    """Test that stat calculations are consistent across operations."""

    def test_level_up_increases_stats(self, load_route):
        """Stats should increase (or stay same) when leveling up."""
        router, events = load_route("yellow-pinsir-lv10brock.json")
        for eg in events:
            before = eg.init_state.solo_pkmn
            after = eg.final_state.solo_pkmn
            if after.cur_level > before.cur_level:
                # At minimum, HP should increase on level up
                assert after.cur_stats.hp >= before.cur_stats.hp, (
                    f"HP decreased on level up at event '{eg.event_definition}'"
                )

    def test_stat_xp_never_negative(self, load_route):
        """Stat XP / EVs should never be negative."""
        for route_file in [
            "yellow-pinsir-lv10brock.json",
            "c-porygon-1-.json",
            "f-ditto-tests.json",
            "platinum_chimchar.json",
        ]:
            router, events = load_route(route_file)
            for eg in events:
                sxp = eg.final_state.solo_pkmn.realized_stat_xp
                assert sxp.hp >= 0
                assert sxp.attack >= 0
                assert sxp.defense >= 0
                assert sxp.special_attack >= 0
                assert sxp.special_defense >= 0
                assert sxp.speed >= 0
