"""
Regression tests for the gen 3 damage-calc fixes in docs/damage_calc_review/gen_3_findings.md.

Baseline mon used throughout (matching the findings doc): Emerald, trainer Machop L50
(DVs 8/9/8/8/8/8, 0 EVs, Hardy, no badges) unless noted -- HP 134 / Atk 89 / Def 59 /
SpA 44 / SpD 44 / Spe 44.
"""
import pytest
from utils.constants import const
from pkmn import universal_data_objects, gen_factory


def _setup():
    gen_factory.change_version(const.EMERALD_VERSION)
    gen = gen_factory.current_gen_info()
    attacker = gen.create_trainer_pkmn("Machop", 50)
    defender = gen.create_trainer_pkmn("Machop", 50)
    return gen, attacker, defender


class TestRolloutIceBall:
    def test_turn_1_is_not_doubled(self):
        """Bug #1: turn 1 must use the move's plain base power, not 2x it."""
        gen, attacker, defender = _setup()
        rollout = gen.move_db().get_move("Rollout")
        turn_1 = gen.calculate_damage(attacker, rollout, defender, custom_move_data="1")
        assert turn_1.min_damage == 8
        assert turn_1.max_damage == 10

    def test_power_doubles_each_turn(self):
        gen, attacker, defender = _setup()
        rollout = gen.move_db().get_move("Rollout")
        turn_1 = gen.calculate_damage(attacker, rollout, defender, custom_move_data="1")
        turn_2 = gen.calculate_damage(attacker, rollout, defender, custom_move_data="2")
        # power doubles turn-over-turn (allow +/-1 for independent rounding of each turn's formula)
        assert turn_2.max_damage in (turn_1.max_damage * 2 - 1, turn_1.max_damage * 2, turn_1.max_damage * 2 + 1)

    def test_defense_curl_bonus(self):
        gen, attacker, defender = _setup()
        rollout = gen.move_db().get_move("Rollout")
        turn_5 = gen.calculate_damage(attacker, rollout, defender, custom_move_data="5")
        turn_5_dc = gen.calculate_damage(attacker, rollout, defender, custom_move_data="5 + DefenseCurl")
        # BP 480 (30*2**4) vs BP 960 (double that, from Defense Curl); each independently
        # rounds through the formula so the max isn't an exact 2x of the other.
        assert turn_5.min_damage == 136
        assert turn_5.max_damage == 160
        assert turn_5_dc.min_damage == 271
        assert turn_5_dc.max_damage == 319


class TestFuryCutter:
    def test_caps_at_five_uses(self):
        """Bug #3: dropdown '6' must not exceed the power ceiling from '5' (x16 base-power)."""
        gen, attacker, defender = _setup()
        fury_cutter = gen.move_db().get_move("Fury Cutter")
        use_5 = gen.calculate_damage(attacker, fury_cutter, defender, custom_move_data="5")
        use_6 = gen.calculate_damage(attacker, fury_cutter, defender, custom_move_data="6")
        assert use_6.max_damage == use_5.max_damage

    def test_use_3_matches_hand_computed_value(self):
        gen, attacker, defender = _setup()
        fury_cutter = gen.move_db().get_move("Fury Cutter")
        result = gen.calculate_damage(attacker, fury_cutter, defender, custom_move_data="3")
        assert result.min_damage == 11
        assert result.max_damage == 14


class TestTripleKick:
    def test_three_kicks_sum_hand_computed_value(self):
        """Bug #4: hits have power 10/20/30 (not one hit x N) -- STAB applies per kick."""
        gen, attacker, defender = _setup()
        triple_kick = gen.move_db().get_move("Triple Kick")
        result = gen.calculate_damage(attacker, triple_kick, defender, custom_move_data="3")
        assert result.min_damage == 54
        assert result.max_damage == 65

    def test_kicks_increase_with_count(self):
        gen, attacker, defender = _setup()
        triple_kick = gen.move_db().get_move("Triple Kick")
        one = gen.calculate_damage(attacker, triple_kick, defender, custom_move_data="1")
        two = gen.calculate_damage(attacker, triple_kick, defender, custom_move_data="2")
        three = gen.calculate_damage(attacker, triple_kick, defender, custom_move_data="3")
        assert one.max_damage < two.max_damage < three.max_damage


class TestFlashFire:
    def test_water_absorb_mon_takes_fire_damage(self):
        """Bug #5: the FLASH_FIRE_ABILITY constant used to be misspelled "Water Absorb"."""
        gen, attacker, defender = _setup()
        defender.ability = "Water Absorb"
        ember = gen.move_db().get_move("Ember")
        result = gen.calculate_damage(attacker, ember, defender)
        assert result is not None

    def test_real_flash_fire_mon_takes_no_fire_damage(self):
        gen, attacker, defender = _setup()
        defender.ability = "Flash Fire"
        ember = gen.move_db().get_move("Ember")
        assert gen.calculate_damage(attacker, ember, defender) is None


class TestLightningRod:
    def test_is_not_an_electric_immunity_in_gen_3(self):
        """Bug #6: Lightning Rod only redirects in gen 3 doubles; it's not an absorb."""
        gen, attacker, defender = _setup()
        defender.ability = "Lightning Rod"
        thunderbolt = gen.move_db().get_move("Thunderbolt")
        assert gen.calculate_damage(attacker, thunderbolt, defender) is not None


class TestSoundproof:
    def test_blocks_sound_moves(self):
        gen, attacker, defender = _setup()
        defender.ability = "Soundproof"
        hyper_voice = gen.move_db().get_move("Hyper Voice")
        assert gen.calculate_damage(attacker, hyper_voice, defender) is None


class TestWeatherBall:
    def test_sandstorm_matches_hand_computed_value(self):
        """Bug #8: the power boost is a final damage multiplier, not a base-power double."""
        gen, _, defender = _setup()
        castform = gen.create_trainer_pkmn("Castform", 50)
        weather_ball = gen.move_db().get_move(const.WEATHER_BALL_MOVE_NAME)
        result = gen.calculate_damage(castform, weather_ball, defender, weather=const.WEATHER_SANDSTORM)
        assert result.min_damage == 26
        assert result.max_damage == 31


class TestStatModifierOrder:
    def test_explosion_defense_halving_before_stage(self):
        """Bug #9: item/ability/Explosion stat modifiers apply before the stage multiplier."""
        gen, attacker, _ = _setup()
        geodude = gen.create_trainer_pkmn("Geodude", 28)
        explosion = gen.move_db().get_move("Explosion")
        defending_stages = universal_data_objects.StageModifiers(defense=-1)
        result = gen.calculate_damage(attacker, explosion, geodude, defending_stage_modifiers=defending_stages)
        assert result.min_damage == 208
        assert result.max_damage == 245

    def test_thick_fat_before_stage(self):
        gen, attacker, defender = _setup()
        attacker = gen.create_trainer_pkmn("Machop", 52)
        defender.ability = "Thick Fat"
        ember = gen.move_db().get_move("Ember")
        attacking_stages = universal_data_objects.StageModifiers(special_attack=-1)
        result = gen.calculate_damage(attacker, ember, defender, attacking_stage_modifiers=attacking_stages)
        assert result.min_damage == 5
        assert result.max_damage == 7

    def test_choice_band_before_stage(self):
        gen, attacker, defender = _setup()
        attacker = gen.create_trainer_pkmn("Machop", 52)
        attacker.held_item = "Choice Band"
        tackle = gen.move_db().get_move("Tackle")
        attacking_stages = universal_data_objects.StageModifiers(attack=-1)
        result = gen.calculate_damage(attacker, tackle, defender, attacking_stage_modifiers=attacking_stages)
        assert result.min_damage == 22
        assert result.max_damage == 26


class TestRage:
    def test_rage_deals_plain_vanilla_damage_with_no_hits_yet(self):
        """Bug #10: gen 3 Rage has no damage multiplier at all -- with no custom_move_data
        (i.e. no hits landed on the Rage user yet) it's a plain 20 BP hit."""
        gen, attacker, defender = _setup()
        rage = gen.move_db().get_move("Rage")
        result = gen.calculate_damage(attacker, rage, defender)
        assert result.min_damage == 12
        assert result.max_damage == 15

    def test_rage_hits_raise_attack_stage(self):
        gen, attacker, defender = _setup()
        rage = gen.move_db().get_move("Rage")
        one_hit = gen.calculate_damage(attacker, rage, defender, custom_move_data="1")
        three_hits = gen.calculate_damage(attacker, rage, defender, custom_move_data="3")
        assert three_hits.max_damage > one_hit.max_damage


class TestSoulDewAndDeepSea:
    def test_soul_dew_boosts_latios_special_attack(self):
        """Bug #11: Soul Dew (not DeepSeaScale) boosts Lati@s -- and it boosts the
        DEFENDING Lati@s's Special Defense too, not just the attacker's."""
        gen, _, defender = _setup()
        latios = gen.create_trainer_pkmn("Latios", 50)
        latios.held_item = "Soul Dew"
        psychic = gen.move_db().get_move("Psychic")
        result = gen.calculate_damage(latios, psychic, defender)
        assert result.min_damage == 481
        assert result.max_damage == 566

    def test_deepseascale_boosts_defending_clamperl_special_defense(self):
        gen, attacker, _ = _setup()
        clamperl = gen.create_trainer_pkmn("Clamperl", 30)
        clamperl.held_item = "DeepSeaScale"
        confusion = gen.move_db().get_move("Confusion")
        result = gen.calculate_damage(attacker, confusion, clamperl)
        assert result.min_damage == 11
        assert result.max_damage == 14


class TestHeldItemBoostTable:
    def test_dragon_fang_boosts_dragon_moves(self):
        """Bug #12: the table used to list Dragon Scale (no boost) instead of Dragon Fang."""
        gen, attacker, defender = _setup()
        attacker.held_item = "Dragon Fang"
        dragon_breath = gen.move_db().get_move("DragonBreath")
        result = gen.calculate_damage(attacker, dragon_breath, defender)
        assert result.max_damage == 30

    def test_sea_incense_uses_1_05_param(self):
        gen, attacker, defender = _setup()
        attacker.held_item = "Sea Incense"
        water_gun = gen.move_db().get_move("Water Gun")
        result = gen.calculate_damage(attacker, water_gun, defender)
        assert result.max_damage == 20

    def test_silverpowder_key_matches_item_db(self):
        gen, attacker, defender = _setup()
        attacker.held_item = "Silverpowder"
        assert gen.item_db().get_item(attacker.held_item) is not None


class TestThickClub:
    def test_boosts_cubone_too(self):
        """Bug #13: the game boosts Cubone OR Marowak, not just Marowak."""
        gen, _, defender = _setup()
        cubone = gen.create_trainer_pkmn("Cubone", 30)
        cubone.held_item = "Thick Club"
        bone_club = gen.move_db().get_move("Bone Club")
        result = gen.calculate_damage(cubone, bone_club, defender)
        assert result.min_damage == 30
        assert result.max_damage == 36


class TestScreensInDoubles:
    def test_reflect_uses_two_thirds_not_one_half(self):
        """Bug #16: with 2 alive defenders, Reflect/Light Screen is 2*(d/3), not d/2."""
        gen, attacker, defender = _setup()
        karate_chop = gen.move_db().get_move("Karate Chop")

        # calculate_damage on GenThree doesn't expose defender_has_reflect directly;
        # go through the lower-level calc function used by every gen object instead.
        from pkmn.gen_3 import pkmn_damage_calc
        result = pkmn_damage_calc.calculate_gen_three_damage(
            attacker, gen.pkmn_db().get_pkmn(attacker.name), karate_chop,
            defender, gen.pkmn_db().get_pkmn(defender.name),
            gen._special_types, gen._type_chart, gen._held_item_boosts,
            defender_has_reflect=True, is_double_battle=True,
        )
        assert result.min_damage == 30
        assert result.max_damage == 36


class TestPhysicalMinimumBeforePlusTwo:
    def test_low_level_attacker_deals_at_least_1_before_plus_2(self):
        """Bug #18: a physical hit rounding to 0 must be floored to 1 before the final +2."""
        gen, _, defender = _setup()
        rattata = gen.create_trainer_pkmn("Rattata", 2)
        tackle = gen.move_db().get_move("Tackle")
        result = gen.calculate_damage(rattata, tackle, defender)
        assert result.damage_vals == {3: 15, 4: 1}


class TestPsywaveDistribution:
    def test_eleven_equiprobable_values(self):
        """Bug #22: 11 equiprobable values level*(50+10k)/100, not a uniform 1..1.5L-1 range."""
        gen, attacker, defender = _setup()
        psywave = gen.move_db().get_move("Psywave")
        result = gen.calculate_damage(attacker, psywave, defender)
        expected = {25, 30, 35, 40, 45, 50, 55, 60, 65, 70, 75}
        assert set(result.damage_vals.keys()) == expected
        assert all(count == 1 for count in result.damage_vals.values())


class TestOHKOMoves:
    def test_fissure_deals_target_full_hp_at_equal_level(self):
        """Bug #23: OHKO moves must deal the target's HP, not run through the vanilla formula."""
        gen, attacker, defender = _setup()
        fissure = gen.move_db().get_move("Fissure")
        result = gen.calculate_damage(attacker, fissure, defender)
        assert result.min_damage == defender.cur_stats.hp
        assert result.max_damage == defender.cur_stats.hp

    def test_fails_against_higher_level_target(self):
        gen, attacker, _ = _setup()
        strong_defender = gen.create_trainer_pkmn("Machop", 60)
        fissure = gen.move_db().get_move("Fissure")
        assert gen.calculate_damage(attacker, fissure, strong_defender) is None

    def test_blocked_by_sturdy(self):
        gen, attacker, defender = _setup()
        defender.ability = "Sturdy"
        fissure = gen.move_db().get_move("Fissure")
        assert gen.calculate_damage(attacker, fissure, defender) is None

    def test_accuracy_29_percent_at_equal_level(self):
        gen, attacker, defender = _setup()
        fissure = gen.move_db().get_move("Fissure")
        assert gen.get_move_accuracy(attacker, fissure, "", defender, const.WEATHER_NONE) == 29


class TestFutureSightAndDoomDesire:
    def test_ignores_type_effectiveness_and_immunity(self):
        """Bug #24: no STAB, no type effectiveness, no immunity at all -- hits Dark types."""
        gen, attacker, defender = _setup()
        future_sight = gen.move_db().get_move("Future Sight")
        result = gen.calculate_damage(attacker, future_sight, defender)
        assert result.min_damage == 31
        assert result.max_damage == 37

        poochyena = gen.create_trainer_pkmn("Poochyena", 50)
        assert gen.calculate_damage(attacker, future_sight, poochyena) is not None


class TestStruggleVsWonderGuard:
    def test_struggle_hits_wonder_guard(self):
        """Misc bug #26: Struggle's typecalc is skipped entirely, so it should hit Shedinja."""
        gen, attacker, _ = _setup()
        shedinja = gen.create_trainer_pkmn("Shedinja", 50)
        shedinja.ability = "Wonder Guard"
        struggle = gen.move_db().get_move("Struggle")
        assert gen.calculate_damage(attacker, struggle, shedinja) is not None


class TestWonderGuardVsFixedDamage:
    def test_dragon_rage_blocked_by_wonder_guard(self):
        """Misc bug #26: Wonder Guard's typecalc still runs for fixed-damage moves."""
        gen, attacker, _ = _setup()
        shedinja = gen.create_trainer_pkmn("Shedinja", 50)
        shedinja.ability = "Wonder Guard"
        dragon_rage = gen.move_db().get_move("Dragon Rage")
        assert gen.calculate_damage(attacker, dragon_rage, shedinja) is None


class TestNaturePower:
    def test_deals_damage_instead_of_none(self):
        """Not implemented -> now delegates to the mapped move per terrain."""
        gen, attacker, defender = _setup()
        nature_power = gen.move_db().get_move("Nature Power")
        result = gen.calculate_damage(attacker, nature_power, defender, custom_move_data="Sand")
        assert result is not None
        assert result.min_damage == 57
        assert result.max_damage == 68

    def test_tall_grass_is_a_status_move_and_deals_no_damage(self):
        gen, attacker, defender = _setup()
        nature_power = gen.move_db().get_move("Nature Power")
        assert gen.calculate_damage(attacker, nature_power, defender, custom_move_data="Tall Grass") is None


class TestFrustration:
    def test_uses_the_dropdown_power_directly(self):
        gen, attacker, defender = _setup()
        frustration = gen.move_db().get_move("Frustration")
        result = gen.calculate_damage(attacker, frustration, defender, custom_move_data="102")
        return_move = gen.move_db().get_move("Return")
        return_result = gen.calculate_damage(attacker, return_move, defender, custom_move_data="102")
        assert result.damage_vals == return_result.damage_vals


class TestSuperFang:
    def test_deals_half_of_target_current_hp(self):
        gen, attacker, defender = _setup()
        super_fang = gen.move_db().get_move("Super Fang")
        result = gen.calculate_damage(attacker, super_fang, defender)
        assert result.min_damage == defender.cur_stats.hp // 2
        assert result.max_damage == defender.cur_stats.hp // 2


class TestPresent:
    def test_dropdown_selects_base_power(self):
        gen, attacker, defender = _setup()
        present = gen.move_db().get_move("Present")
        low = gen.calculate_damage(attacker, present, defender, custom_move_data="40 BP")
        high = gen.calculate_damage(attacker, present, defender, custom_move_data="120 BP")
        assert low.max_damage < high.max_damage


class TestEndeavor:
    def test_damage_is_target_hp_minus_user_hp(self):
        gen, attacker, defender = _setup()
        endeavor = gen.move_db().get_move("Endeavor")
        result = gen.calculate_damage(attacker, endeavor, defender, custom_move_data="50")
        expected = defender.cur_stats.hp - (attacker.cur_stats.hp * 50 // 100)
        assert result.min_damage == expected
        assert result.max_damage == expected

    def test_fails_when_user_hp_not_lower(self):
        gen, attacker, defender = _setup()
        endeavor = gen.move_db().get_move("Endeavor")
        assert gen.calculate_damage(attacker, endeavor, defender, custom_move_data="100") is None


class TestCounterMirrorCoatBide:
    def test_counter_doubles_supplied_damage_taken(self):
        gen, attacker, defender = _setup()
        counter = gen.move_db().get_move("Counter")
        result = gen.calculate_damage(attacker, counter, defender, custom_move_data="30")
        assert result.min_damage == 60
        assert result.max_damage == 60

    def test_returns_none_without_input(self):
        gen, attacker, defender = _setup()
        bide = gen.move_db().get_move("Bide")
        assert gen.calculate_damage(attacker, bide, defender) is None


class TestBeatUp:
    def test_one_hit_matches_wild_mon_formula(self):
        """Beat Up's per-hit formula uses base Attack/level vs the target's base Defense,
        with no STAB or type effectiveness at all (it's typeless in the app's model)."""
        gen, attacker, defender = _setup()
        beat_up = gen.move_db().get_move("Beat Up")
        result = gen.calculate_damage(attacker, beat_up, defender, custom_move_data="1")
        assert result is not None
        assert result.min_damage > 0

    def test_more_hits_deal_more_total_damage(self):
        gen, attacker, defender = _setup()
        beat_up = gen.move_db().get_move("Beat Up")
        one = gen.calculate_damage(attacker, beat_up, defender, custom_move_data="1")
        three = gen.calculate_damage(attacker, beat_up, defender, custom_move_data="3")
        assert three.min_damage == one.min_damage * 3

    def test_ignores_type_effectiveness(self):
        """Ghost is immune to nothing here -- Beat Up should still hit a Ghost-type target."""
        gen, attacker, _ = _setup()
        gengar = gen.create_trainer_pkmn("Gengar", 50)
        beat_up = gen.move_db().get_move("Beat Up")
        assert gen.calculate_damage(attacker, beat_up, gengar, custom_move_data="1") is not None
