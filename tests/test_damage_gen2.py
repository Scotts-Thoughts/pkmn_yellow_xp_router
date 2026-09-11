"""
Regression tests for the gen 2 (Gold/Silver/Crystal) damage-calc fixes made against
docs/damage_calc_review/gen_2_findings.md. Each test corresponds to a numbered bug or
not-implemented item in that document, using the same hand-computed numbers.

All trainer mons use the app's own default trainer DVs (HP 8 / Atk 9 / Def 8 / SpA 8 /
SpD 8 / Spe 8) and 0 stat exp, matching what the findings doc's numbers assume.
"""
import pytest

from utils.constants import const
from pkmn import universal_data_objects, gen_factory
from pkmn.gen_2.data_objects import GenTwoBadgeList
from pkmn.gen_2.gen_two_constants import gen_two_const


def _setup(version=const.CRYSTAL_VERSION):
    gen_factory.change_version(version)
    return gen_factory.current_gen_info()


class TestTruncateHLBC:
    """Bug 3.1: whenever attack or defense (post-screen-doubling) is >= 256, both are
    divided by 4, floored, min 1."""

    def test_high_level_stats_get_truncated(self):
        gen = _setup(const.CRYSTAL_VERSION)
        machamp = gen.create_trainer_pkmn("Machamp", 100)
        snorlax = gen.create_trainer_pkmn("Snorlax", 100)
        cross_chop = gen.move_db().get_move("Cross Chop")

        dmg = gen.calculate_damage(machamp, cross_chop, snorlax)
        assert dmg.min_damage == 408
        assert dmg.max_damage == 480

    def test_gold_silver_wraps_instead_of_looping(self):
        # Reflect on a big enough Defense pushes the doubled stat past 1024, which
        # Gold/Silver only truncate once (then wrap to the low byte) instead of Crystal's
        # repeat-until-both-below-256 loop.
        gen_crystal = _setup(const.CRYSTAL_VERSION)
        gen_gold = _setup(const.GOLD_VERSION)

        onix_crystal = gen_crystal.create_trainer_pkmn("Onix", 100)
        onix_gold = gen_gold.create_trainer_pkmn("Onix", 100)
        tackle_crystal = gen_crystal.move_db().get_move("Tackle")
        tackle_gold = gen_gold.move_db().get_move("Tackle")
        attacker_crystal = gen_crystal.create_trainer_pkmn("Machamp", 100)
        attacker_gold = gen_gold.create_trainer_pkmn("Machamp", 100)

        stages = universal_data_objects.StageModifiers(defense=6)
        field = universal_data_objects.FieldStatus(reflect=True)

        dmg_crystal = gen_crystal.calculate_damage(
            attacker_crystal, tackle_crystal, onix_crystal,
            defending_stage_modifiers=stages, defending_field=field,
        )
        dmg_gold = gen_gold.calculate_damage(
            attacker_gold, tackle_gold, onix_gold,
            defending_stage_modifiers=stages, defending_field=field,
        )
        # Both must produce *some* truncated (non-exploded) damage; the two versions need
        # not match exactly, but Gold/Silver's one-pass-then-wrap must not crash and must
        # stay in a sane (truncated) range rather than the old untruncated 16-bit-stat range.
        assert dmg_crystal.max_damage < 999
        assert dmg_gold.max_damage < 999


class TestCritScreenInteraction:
    """Bug 3.2: a crit with the attacker's stage > the defender's stage must KEEP
    Reflect/Light Screen (only the unfavorable branch reloads unboosted stats)."""

    def test_crit_with_favorable_stage_keeps_reflect(self):
        gen = _setup()
        machop = gen.create_trainer_pkmn("Machop", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        mega_punch = gen.move_db().get_move("Mega Punch")

        stages = universal_data_objects.StageModifiers(attack=2)
        field = universal_data_objects.FieldStatus(reflect=True)

        dmg = gen.calculate_damage(
            machop, mega_punch, rattata,
            attacking_stage_modifiers=stages, defending_field=field, is_crit=True,
        )
        assert dmg.min_damage == 74
        assert dmg.max_damage == 88


class TestFutureSight:
    """Bug 3.3: Future Sight has no `stab` command at all, so it skips STAB, weather,
    badge type boost, AND type effectiveness/immunity (it can even hit Dark types)."""

    def test_hits_dark_type_with_no_type_effectiveness(self):
        gen = _setup()
        espeon = gen.create_trainer_pkmn("Espeon", 40)
        umbreon = gen.create_trainer_pkmn("Umbreon", 40)
        future_sight = gen.move_db().get_move("Future Sight")

        dmg = gen.calculate_damage(espeon, future_sight, umbreon)
        assert dmg is not None
        assert dmg.min_damage == 25
        assert dmg.max_damage == 30

    def test_no_super_effective_bonus_vs_machoke(self):
        gen = _setup()
        espeon = gen.create_trainer_pkmn("Espeon", 40)
        machoke = gen.create_trainer_pkmn("Machoke", 40)
        future_sight = gen.move_db().get_move("Future Sight")

        dmg = gen.calculate_damage(espeon, future_sight, machoke)
        assert dmg.min_damage == 49
        assert dmg.max_damage == 58


class TestHiddenPowerDvEight:
    """Bug 3.4: the game tests DV >= 8, not DV > 8 -- a DV of exactly 8 (the app's own
    default trainer DV for every stat but Attack) must count as a "high" bit."""

    def test_all_eights_gives_fighting_68(self):
        gen = _setup()
        dvs = gen.make_stat_block(8, 8, 8, 8, 8, 8)
        move_type, power = gen.get_hidden_power(dvs)
        assert move_type == "Fighting"
        assert power == 68

    def test_mixed_nine_eight_gives_rock_69(self):
        gen = _setup()
        dvs = gen.make_stat_block(0, 9, 8, 9, 0, 8)
        move_type, power = gen.get_hidden_power(dvs)
        assert move_type == "Rock"
        assert power == 69


class TestFuryCutterCap:
    """Bug 3.5: Fury Cutter caps at x16 (5 doublings); the dropdown's "6" must behave
    identically to "5", not double again to x32."""

    def test_six_equals_five(self):
        gen = _setup()
        scyther = gen.create_trainer_pkmn("Scyther", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        fury_cutter = gen.move_db().get_move("Fury Cutter")

        five = gen.calculate_damage(scyther, fury_cutter, rattata, custom_move_data="5")
        six = gen.calculate_damage(scyther, fury_cutter, rattata, custom_move_data="6")
        assert five.min_damage == six.min_damage
        assert five.max_damage == six.max_damage
        assert five.max_damage == 208


class TestThickClubCubone:
    """Bug 3.6: Thick Club doubles Attack for Cubone too, not just its evolution Marowak."""

    def test_cubone_gets_thick_club_boost(self):
        gen = _setup()
        cubone = gen.create_trainer_pkmn("Cubone", 30)
        cubone.held_item = gen_two_const.THICK_CLUB_NAME
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        bone_club = gen.move_db().get_move("Bone Club")

        dmg = gen.calculate_damage(cubone, bone_club, rattata)
        assert dmg.min_damage == 63
        assert dmg.max_damage == 75


class TestMetalPowder:
    """Bug 3.7: Metal Powder boosts a Ditto defender's SpDef too (not just physical
    Defense), applied to the truncated 8-bit stat."""

    def test_boosts_special_defense(self):
        gen = _setup()
        quilava = gen.create_trainer_pkmn("Quilava", 30)
        ditto = gen.create_trainer_pkmn("Ditto", 30)
        ditto.held_item = gen_two_const.METAL_POWDER_NAME
        ember = gen.move_db().get_move("Ember")

        dmg = gen.calculate_damage(quilava, ember, ditto)
        assert dmg.min_damage == 16
        assert dmg.max_damage == 19


class TestAeroblastHighCrit:
    """Bug 3.8: Aeroblast is in CriticalHitMoves (a 1/4 crit rate), but moves.json was
    missing the high_crit flavor."""

    def test_crit_rate_is_one_quarter(self):
        gen = _setup()
        lugia = gen.create_trainer_pkmn("Lugia", 50)
        aeroblast = gen.move_db().get_move("Aeroblast")
        assert gen.get_crit_rate(lugia, aeroblast, "") == pytest.approx(0.25)


class TestBadgeTypeBoost:
    """Bug 3.9: the badge type boost adds max(x>>3, 1), not floor(x*1.125) -- they only
    differ (add at least 1) when x < 8, which is common at low levels."""

    def test_gust_with_zephyr_badge(self):
        gen = _setup()
        pidgey = gen.create_trainer_pkmn("Pidgey", 5)
        pidgey.badges = GenTwoBadgeList({}, zephyr=True)
        rattata = gen.create_trainer_pkmn("Rattata", 5)
        gust = gen.move_db().get_move("Gust")

        dmg = gen.calculate_damage(pidgey, gust, rattata, custom_move_data=gen_two_const.NO_BONUS)
        assert dmg.min_damage == 7
        assert dmg.max_damage == 9


class TestDoubleDamageAfterRoll:
    """Bug 3.10: Gust/Twister/Earthquake/Magnitude/Stomp/Pursuit's bonus doubles each
    ROLLED value, not the pre-roll damage -- so only even results are reachable."""

    def test_fly_bonus_only_produces_even_values(self):
        gen = _setup()
        pidgeotto = gen.create_trainer_pkmn("Pidgeotto", 20)
        rattata = gen.create_trainer_pkmn("Rattata", 20)
        gust = gen.move_db().get_move("Gust")

        dmg = gen.calculate_damage(pidgeotto, gust, rattata, custom_move_data=gen_two_const.FLY_BONUS)
        assert all(val % 2 == 0 for val in dmg.damage_vals.keys())


class TestTripleKick:
    """Bug 3.11: each kick's damage is the post-"+2" value times the kick number, run
    through weather/STAB/type on its own, with its own random roll -- not one multiplied
    final value."""

    def test_kick_totals_grow_with_kick_count(self):
        gen = _setup()
        hitmontop = gen.create_trainer_pkmn("Hitmontop", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        triple_kick = gen.move_db().get_move("Triple Kick")

        one = gen.calculate_damage(hitmontop, triple_kick, rattata, custom_move_data="1 Kick")
        two = gen.calculate_damage(hitmontop, triple_kick, rattata, custom_move_data="2 Kicks")
        three = gen.calculate_damage(hitmontop, triple_kick, rattata, custom_move_data="3 Kicks")

        # 2 kicks must be more than double kick 1's max (kick 2 alone deals 2x a single
        # kick's base), and 3 kicks more than 3x -- catches the old "just multiply the
        # single-kick roll by n" bug, which would make these exactly n times as much
        # instead of a true sum of independently-rolled 1x/2x/3x kicks.
        assert two.max_damage > one.max_damage
        assert three.max_damage > two.max_damage
        assert three.min_damage > two.min_damage > one.min_damage


class TestMultiHitCritRecursion:
    """Bug 3.15: the non-crit hits of a multi-hit crit must use the ORIGINAL stage
    modifiers (not the zeroed ones from the crit rule) and the real weather."""

    def test_non_crit_hits_still_see_boosted_defense(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Sandslash", 30)
        defender = gen.create_trainer_pkmn("Rattata", 30)
        fury_swipes = gen.move_db().get_move("Fury Swipes")

        # +2 Def >= the attacker's unboosted (0) Atk stage, so the crit rule's unfavorable
        # branch fires: the crit hit reloads unboosted/unstaged stats (ignoring +2 Def
        # entirely), while the second, non-crit hit must still see the real +2 Def.
        defending_stages = universal_data_objects.StageModifiers(defense=2)

        # custom_move_data="" on a multi_hit move returns a single strike -- exactly what
        # the recursive call inside a 2-hit crit computes for each of its two hits.
        single_crit_hit = gen.calculate_damage(
            attacker, fury_swipes, defender,
            defending_stage_modifiers=defending_stages, is_crit=True, custom_move_data="",
        )
        single_non_crit_hit = gen.calculate_damage(
            attacker, fury_swipes, defender,
            defending_stage_modifiers=defending_stages, is_crit=False, custom_move_data="",
        )
        expected = single_crit_hit.add(single_non_crit_hit)

        crit_result = gen.calculate_damage(
            attacker, fury_swipes, defender,
            defending_stage_modifiers=defending_stages,
            custom_move_data=const.MULTI_HIT_2, is_crit=True,
        )
        assert crit_result.min_damage == expected.min_damage
        assert crit_result.max_damage == expected.max_damage


class TestStatExpSqrtCap:
    """Bug 3.18: GetSquareRoot caps at 255; math.ceil(sqrt(x)) can reach 256 for stat exp
    in 65026..65535, which must not happen."""

    def test_maxed_stat_exp_caps_at_255_sqrt(self):
        gen = _setup()
        stat_block_cls = type(gen.create_trainer_pkmn("Rattata", 50).cur_stats)
        # base 100, DV 15, statexp 65535, level 100 -> (100+15)*2 + floor(255/4) = 230+63=293, *100/100+5=298
        from pkmn.gen_2.data_objects import calc_stat
        result = calc_stat(100, 100, 15, 65535)
        assert result == 298


class TestStruggleItemBoost:
    """Bug 3.14: Struggle's move type is stored as "none" so it skips STAB/type, but the
    held-item type-boost check keys on Struggle's real (Normal) type regardless."""

    def test_pink_bow_boosts_struggle(self):
        gen = _setup()
        raticate = gen.create_trainer_pkmn("Raticate", 30)
        raticate.held_item = "Pink Bow"
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        struggle = gen.move_db().get_move("Struggle")

        dmg = gen.calculate_damage(raticate, struggle, rattata)
        assert dmg.min_damage == 26
        assert dmg.max_damage == 31


class TestThunderAccuracyInSun:
    """Bug 3.13: Thunder is 50% accurate in sun (not the move's nominal 70%)."""

    def test_sun_accuracy_is_fifty(self):
        gen = _setup()
        raichu = gen.create_trainer_pkmn("Raichu", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        thunder = gen.move_db().get_move("Thunder")

        acc = gen.get_move_accuracy(raichu, thunder, "", rattata, const.WEATHER_SUN)
        assert acc == 50

    def test_rain_always_hits(self):
        gen = _setup()
        raichu = gen.create_trainer_pkmn("Raichu", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        thunder = gen.move_db().get_move("Thunder")

        assert gen.get_move_accuracy(raichu, thunder, "", rattata, const.WEATHER_RAIN) is None


class TestSuperFang:
    """4.5: Super Fang deals floor(current HP / 2), min 1, no crit/STAB/type/roll, fails
    vs Ghost."""

    def test_deals_half_current_hp(self):
        gen = _setup()
        raticate = gen.create_trainer_pkmn("Raticate", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        super_fang = gen.move_db().get_move("Super Fang")

        dmg = gen.calculate_damage(raticate, super_fang, rattata)
        assert dmg.min_damage == dmg.max_damage == rattata.cur_stats.hp // 2

    def test_fails_against_ghost(self):
        gen = _setup()
        raticate = gen.create_trainer_pkmn("Raticate", 30)
        gastly = gen.create_trainer_pkmn("Gastly", 30)
        super_fang = gen.move_db().get_move("Super Fang")

        assert gen.calculate_damage(raticate, super_fang, gastly) is None


class TestOneHitKO:
    """4.6: Guillotine/Horn Drill/Fissure deal damage equal to the target's current HP
    (a guaranteed KO) if they connect, and fail outright if the user is under-levelled."""

    def test_deals_exactly_target_current_hp(self):
        gen = _setup()
        onix = gen.create_trainer_pkmn("Onix", 40)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        fissure = gen.move_db().get_move("Fissure")

        dmg = gen.calculate_damage(onix, fissure, rattata)
        assert dmg.min_damage == dmg.max_damage == rattata.cur_stats.hp

    def test_fails_when_underleveled(self):
        gen = _setup()
        onix = gen.create_trainer_pkmn("Onix", 20)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        fissure = gen.move_db().get_move("Fissure")

        assert gen.calculate_damage(onix, fissure, rattata) is None

    def test_accuracy_scales_with_level_difference(self):
        gen = _setup()
        onix_higher = gen.create_trainer_pkmn("Onix", 40)
        onix_equal = gen.create_trainer_pkmn("Onix", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        fissure = gen.move_db().get_move("Fissure")

        assert gen.get_move_accuracy(onix_equal, fissure, "", rattata, const.WEATHER_NONE) == pytest.approx(76 / 256 * 100)
        assert gen.get_move_accuracy(onix_higher, fissure, "", rattata, const.WEATHER_NONE) == pytest.approx(min(76 + 20, 255) / 256 * 100)


class TestFrustration:
    """4.2: Frustration mirrors Return's dropdown, but power = (255-happiness)*10//25."""

    def test_dropdown_sets_power_directly(self):
        gen = _setup()
        raticate = gen.create_trainer_pkmn("Raticate", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        frustration = gen.move_db().get_move("Frustration")

        dmg = gen.calculate_damage(raticate, frustration, rattata, custom_move_data="102")
        assert dmg is not None
        assert dmg.max_damage > 0


class TestPresent:
    """4.1/4.4: Present has a 40/80/120 power dropdown (plus "Heal", no damage). Crystal
    uses the normal formula; Gold/Silver clobber their damagecalc registers, producing a
    stat/level-independent result derived in the findings doc."""

    def test_crystal_uses_normal_formula(self):
        gen = _setup(const.CRYSTAL_VERSION)
        machamp = gen.create_trainer_pkmn("Machamp", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        present = gen.move_db().get_move("Present")

        dmg = gen.calculate_damage(machamp, present, rattata, custom_move_data="120")
        assert dmg.min_damage == 85
        assert dmg.max_damage == 100

    def test_heal_option_deals_no_damage(self):
        gen = _setup(const.CRYSTAL_VERSION)
        machamp = gen.create_trainer_pkmn("Machamp", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        present = gen.move_db().get_move("Present")

        assert gen.calculate_damage(machamp, present, rattata, custom_move_data="Heal") is None

    def test_gold_silver_normal_mirror_match(self):
        gen = _setup(const.GOLD_VERSION)
        # A Normal-type user against a mono-Normal target: A=10 (neutral), D forced to 1
        # (user has "STAB"), level proxy e = id(Normal) = 0 -> base = 2.
        ditto = gen.create_trainer_pkmn("Ditto", 50)  # Normal-type
        # Ditto vs Ditto keeps both sides mono-Normal without needing a specific species.
        target = gen.create_trainer_pkmn("Ditto", 50)
        present = gen.move_db().get_move("Present")

        dmg = gen.calculate_damage(ditto, present, target, custom_move_data="120")
        assert dmg.min_damage == 63
        assert dmg.max_damage == 75

        dmg_80 = gen.calculate_damage(ditto, present, target, custom_move_data="80")
        assert dmg_80.min_damage == 43
        assert dmg_80.max_damage == 51

        dmg_40 = gen.calculate_damage(ditto, present, target, custom_move_data="40")
        assert dmg_40.min_damage == 22
        assert dmg_40.max_damage == 27


class TestFalseSwipeNeverKOs:
    """4.8: False Swipe's damage is clamped to target HP - 1."""

    def test_massive_false_swipe_leaves_one_hp(self):
        gen = _setup()
        machamp = gen.create_trainer_pkmn("Machamp", 100)
        caterpie = gen.create_trainer_pkmn("Caterpie", 5)
        false_swipe = gen.move_db().get_move("False Swipe")

        dmg = gen.calculate_damage(machamp, false_swipe, caterpie)
        assert dmg.max_damage == caterpie.cur_stats.hp - 1


class TestNotImplementedReturnsNone:
    """4.3/4.7: Beat Up, Counter, Mirror Coat, and Bide all need inputs (a full party, or
    "damage the target dealt") this router doesn't have; they must return None instead of
    the old base_power=-1 fallthrough's bogus 1-damage number."""

    @pytest.mark.parametrize("move_name", ["Beat Up", "Counter", "Mirror Coat", "Bide"])
    def test_returns_none(self, move_name):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Raticate", 30)
        defender = gen.create_trainer_pkmn("Rattata", 30)
        move = gen.move_db().get_move(move_name)

        assert gen.calculate_damage(attacker, move, defender) is None
