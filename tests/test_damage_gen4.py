"""
Regression tests for the gen 4 (Diamond/Pearl/Platinum/HeartGold/SoulSilver) damage
calculation fixes documented in docs/damage_calc_review/gen_4_findings.md.

Fixtures mirror the ones used throughout that audit (trainer-mon IVs 8/9/8/8/8/8, 0 EVs,
Hardy nature): Chimchar L20 (Atk 30/Def 24/SpA 29/SpD 24/Spe 31, Fire), Starly L20
(Atk 28/Def 18/SpA 18/SpD 18/Spe 30, Normal/Flying, HP 47), Gastly L20 (Ghost/Poison),
Geodude L20 (Rock/Ground), Magikarp L20 (Water, 10.0 kg), Scizor L30 (Bug/Steel),
Cherrim L20 (Grass), Pikachu L20 (Electric), Latios L50 (Dragon/Psychic).

Expected numbers were cross-checked against the audit's reference implementation of
Platinum's BattleSystem_CalcMoveDamage; where noted, they come directly from the bug
write-up in the findings doc.
"""
import pytest
from utils.constants import const
from pkmn import gen_factory, universal_data_objects


def _setup():
    gen_factory.change_version(const.PLATINUM_VERSION)
    gen = gen_factory.current_gen_info()
    return gen


class TestCritStageHandling:
    """Bug #1: a crit must only ignore the NEGATIVE stage of the attacking stat this
    move's category actually uses, and the POSITIVE stage of the defending stat it
    uses -- not wipe every stage on both mons, and not check the other category's stats."""

    def test_positive_attacker_stage_is_kept_on_a_physical_crit(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        scratch = gen.move_db().get_move("Scratch")

        plus2_atk = universal_data_objects.StageModifiers(attack=2)
        boosted_crit = gen.calculate_damage(chimchar, scratch, starly, attacking_stage_modifiers=plus2_atk, is_crit=True)
        plain_crit = gen.calculate_damage(chimchar, scratch, starly, is_crit=True)

        assert boosted_crit.damage_vals == {47: 1, 48: 2, 49: 2, 50: 2, 51: 1, 52: 2, 53: 2, 54: 2, 55: 1, 56: 1}
        assert plain_crit.max_damage < boosted_crit.max_damage

    def test_positive_defender_stage_is_ignored_on_a_physical_crit(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        scratch = gen.move_db().get_move("Scratch")

        plus2_def = universal_data_objects.StageModifiers(defense=2)
        crit_vs_boosted_def = gen.calculate_damage(chimchar, scratch, starly, defending_stage_modifiers=plus2_def, is_crit=True)
        plain_crit = gen.calculate_damage(chimchar, scratch, starly, is_crit=True)

        # the +2 Def stage must be discarded entirely on the crit, not just capped
        assert crit_vs_boosted_def.damage_vals == plain_crit.damage_vals
        assert plain_crit.damage_vals == {25: 2, 26: 3, 27: 4, 28: 3, 29: 3, 30: 1}


class TestRolloutIceBall:
    """Bug #2: power is 30*2^(n-1), not 2^n; Defense Curl doubles that (not another
    full power level)."""

    def test_turn_one_power(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        rollout = gen.move_db().get_move("Rollout")

        result = gen.calculate_damage(chimchar, rollout, starly, custom_move_data="1")
        assert result.damage_vals == {20: 3, 21: 4, 22: 4, 23: 4, 24: 1}

    def test_fifth_turn_with_defense_curl(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        rollout = gen.move_db().get_move("Rollout")

        result = gen.calculate_damage(chimchar, rollout, starly, custom_move_data="5 + DefenseCurl")
        assert result.min_damage == 547
        assert result.max_damage == 644


class TestFuryCutter:
    """Bug #3: the counter caps at 5 (power 160), the dropdown no longer offers "6"."""

    def test_capped_power(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        fury_cutter = gen.move_db().get_move("Fury Cutter")

        result = gen.calculate_damage(chimchar, fury_cutter, starly, custom_move_data="5")
        assert result.damage_vals == {22: 1, 23: 3, 24: 4, 25: 4, 26: 3, 27: 1}

    def test_dropdown_has_no_sixth_option(self):
        gen = _setup()
        options = gen.get_move_custom_data("Fury Cutter")
        assert options == ["1", "2", "3", "4", "5"]


class TestPunishment:
    """Bug #4d: power = 60 + 20*(sum of the target's positive stat stages), cap 200 --
    not `move.base_power (1) + 20*sum`."""

    def test_power_scales_with_defender_buffs(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        punishment = gen.move_db().get_move("Punishment")

        plus2_atk = universal_data_objects.StageModifiers(attack=2)
        result = gen.calculate_damage(chimchar, punishment, starly, defending_stage_modifiers=plus2_atk)
        assert result.damage_vals == {29: 1, 30: 3, 31: 3, 32: 3, 33: 3, 34: 2, 35: 1}


class TestTrumpCard:
    """Bug #4e: "4+" must resolve to power 40, and "0" (200 power) must exist."""

    def test_four_plus_resolves_to_forty(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        trump_card = gen.move_db().get_move("Trump Card")

        result = gen.calculate_damage(chimchar, trump_card, starly, custom_move_data="4+")
        assert result.damage_vals == {11: 1, 12: 7, 13: 7, 14: 1}

    def test_zero_pp_is_two_hundred_power(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        trump_card = gen.move_db().get_move("Trump Card")

        four_plus = gen.calculate_damage(chimchar, trump_card, starly, custom_move_data="4+")
        zero = gen.calculate_damage(chimchar, trump_card, starly, custom_move_data="0")
        assert zero.min_damage > four_plus.max_damage
        assert gen.get_move_custom_data("Trump Card") == ["4+", "3", "2", "1", "0"]


class TestSpitUp:
    """Bug #5: power is 100*stockpiles (not 1*n), and it CAN crit."""

    def test_power_is_100_times_stockpiles(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        spit_up = gen.move_db().get_move("Spit Up")

        result = gen.calculate_damage(chimchar, spit_up, starly, custom_move_data="3")
        assert result.damage_vals == {98: 1}

    def test_can_crit(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        spit_up = gen.move_db().get_move("Spit Up")

        crit_result = gen.calculate_damage(chimchar, spit_up, starly, custom_move_data="3", is_crit=True)
        assert crit_result.damage_vals == {196: 1}


class TestRage:
    """Bug #6: gen 4 Rage has no damage multiplier at all -- it's a plain move whose
    effect is raising the user's Attack stage when hit."""

    def test_no_custom_dropdown(self):
        gen = _setup()
        assert gen.get_move_custom_data("Rage") is None

    def test_deals_plain_damage(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        rage = gen.move_db().get_move("Rage")
        tackle = gen.move_db().get_move("Tackle")

        rage_dmg = gen.calculate_damage(chimchar, rage, starly)
        tackle_dmg = gen.calculate_damage(chimchar, tackle, starly)
        # Rage (20 power) and Tackle (40 power) should differ roughly by power ratio,
        # not by some leftover "hit count" multiplier inflating Rage far past Tackle.
        assert rage_dmg.max_damage < tackle_dmg.max_damage


class TestHighCritData:
    """Bug #8: 8 moves were missing the high_crit flavor (crit stage +1)."""

    @pytest.mark.parametrize("move_name", [
        "Night Slash", "Shadow Claw", "Psycho Cut", "Stone Edge",
        "Cross Poison", "Attack Order", "Spacial Rend", "Razor Wind",
    ])
    def test_move_is_high_crit(self, move_name):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        move = gen.move_db().get_move(move_name)
        assert gen.get_crit_rate(chimchar, move, "") == pytest.approx(1 / 8)


class TestDoubleHit:
    """Bug #9: Double Hit must be tagged two_hit so it strikes twice."""

    def test_hits_twice(self):
        gen = _setup()
        move = gen.move_db().get_move("Double Hit")
        assert const.DOUBLE_HIT_FLAVOR in move.attack_flavor

    def test_joint_distribution_uses_multiplied_counts(self):
        """Also exercises the shared DamageRange.add fix (bug #22): the joint
        distribution of two independent {11:8,12:7,13:1} hits is {22:64,23:112,24:65,25:14,26:1}."""
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        double_hit = gen.move_db().get_move("Double Hit")

        result = gen.calculate_damage(chimchar, double_hit, starly)
        assert result.damage_vals == {22: 64, 23: 112, 24: 65, 25: 14, 26: 1}


class TestLowKickGrassKnot:
    """Bug #10: weight brackets use `<=` on hectograms, not `<` on kg -- a species at
    exactly 10.0 kg must land in the lightest (20 power) bracket."""

    def test_ten_kilogram_species_gets_lightest_bracket(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        magikarp = gen.create_trainer_pkmn("Magikarp", 20)
        low_kick = gen.move_db().get_move("Low Kick")

        result = gen.calculate_damage(chimchar, low_kick, magikarp)
        assert result.damage_vals == {5: 15, 6: 1}


class TestFlowerGift:
    """Bug #11: Flower Gift boosts the HOLDER's own Attack (attacker side) and the
    OTHER side's Special Defense -- not the holder's own SpA/SpD."""

    def test_defender_side_spd_is_boosted_in_sun(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        cherrim = gen.create_trainer_pkmn("Cherrim", 20)
        cherrim.ability = "Flower Gift"
        ember = gen.move_db().get_move("Ember")

        result = gen.calculate_damage(chimchar, ember, cherrim, weather=const.WEATHER_SUN)
        assert result.damage_vals == {20: 3, 21: 4, 22: 4, 23: 4, 24: 1}


class TestLightBall:
    """Bug #12: Light Ball doubles Pikachu's move POWER (both categories), not its
    Special Attack stat -- so it boosts physical moves too."""

    def test_boosts_physical_move(self):
        gen = _setup()
        pikachu = gen.create_trainer_pkmn("Pikachu", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        quick_attack = gen.move_db().get_move("Quick Attack")

        pikachu.held_item = "Light Ball"
        boosted = gen.calculate_damage(pikachu, quick_attack, starly)
        assert boosted.damage_vals == {22: 4, 23: 4, 24: 4, 25: 3, 26: 1}

        pikachu.held_item = None
        unboosted = gen.calculate_damage(pikachu, quick_attack, starly)
        assert unboosted.max_damage < boosted.max_damage


class TestSoulDew:
    """Bug #13: Soul Dew (not DeepSeaScale) boosts Latios/Latias's SpA and SpD 1.5x."""

    def test_soul_dew_boosts_latios_special_attack(self):
        gen = _setup()
        latios = gen.create_trainer_pkmn("Latios", 50)
        starly = gen.create_trainer_pkmn("Starly", 20)
        dragon_pulse = gen.move_db().get_move("Dragon Pulse")

        latios.held_item = "Soul Dew"
        boosted = gen.calculate_damage(latios, dragon_pulse, starly)
        assert boosted.min_damage == 584
        assert boosted.max_damage == 688


class TestDoublesSpreadAndScreens:
    """Bug #16: doubles spread reduction is *3/4 for "All Foes"/"Others" targeting
    (moves.json's real vocabulary), not /2 keyed on a string that never matched."""

    def test_earthquake_spread_reduction_in_doubles(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        geodude = gen.create_trainer_pkmn("Geodude", 20)
        earthquake = gen.move_db().get_move("Earthquake")
        assert earthquake.targeting == "Others"

        doubles = gen.calculate_damage(chimchar, earthquake, geodude, is_double_battle=True)
        assert doubles.damage_vals == {18: 2, 19: 4, 20: 5, 21: 4, 22: 1}


class TestStruggleIsTypeless:
    """Bug #17: Struggle has no type at all -- no STAB, no type chart, and it must
    hit Ghost-types (the app used to treat it as Normal-typed and return None vs Ghost)."""

    def test_struggle_hits_ghost_types(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        gastly = gen.create_trainer_pkmn("Gastly", 20)
        struggle = gen.move_db().get_move("Struggle")

        result = gen.calculate_damage(chimchar, struggle, gastly)
        assert result is not None
        assert result.damage_vals == {15: 4, 16: 6, 17: 5, 18: 1}


class TestNaturalGiftBerryPower:
    """Bug #18: 17 gen-3-style berries (Occa, Passho, ...) were 80 power; gen 4 is 60."""

    def test_occa_berry_is_sixty_power(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        natural_gift = gen.move_db().get_move("Natural Gift")

        chimchar.held_item = "Occa Berry"
        result = gen.calculate_damage(chimchar, natural_gift, starly)
        assert result.damage_vals == {28: 3, 29: 3, 30: 3, 31: 3, 32: 3, 33: 1}


class TestPsywaveDistribution:
    """Bug #21: damage = level*(r+5)//10 for r uniform 0..10 (11 equiprobable values),
    not a uniform 1..floor(1.5L)-1 range."""

    def test_eleven_equiprobable_values(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        psywave = gen.move_db().get_move("Psywave")

        result = gen.calculate_damage(chimchar, psywave, starly)
        assert result.damage_vals == {v: 1 for v in range(10, 31, 2)}


class TestNaturePower:
    """Bug #23: the base_power-null early-out used to fire before Nature Power's
    override ever ran, so every terrain returned None; the terrain table is also
    gen 4's own (not gen 3's)."""

    def test_plain_sand_is_earthquake_and_respects_flying_immunity(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        nature_power = gen.move_db().get_move("Nature Power")

        # Earthquake (Ground) vs Starly (Normal/Flying) is a hard immunity
        assert gen.calculate_damage(chimchar, nature_power, starly, custom_move_data="Plain/Sand") is None

    def test_mountain_cave_is_rock_slide(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        nature_power = gen.move_db().get_move("Nature Power")

        result = gen.calculate_damage(chimchar, nature_power, starly, custom_move_data="Mountain/Cave")
        assert result.min_damage == 45
        assert result.max_damage == 54


class TestOHKOMoves:
    """Not-implemented item #6: OHKO damage = target's current HP; the move fails
    outright if the user isn't at least as fast as the target."""

    def test_succeeds_when_faster(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        guillotine = gen.move_db().get_move("Guillotine")

        result = gen.calculate_damage(chimchar, guillotine, starly)
        assert result.damage_vals == {starly.cur_stats.hp: 1}

    def test_fails_when_slower(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        scizor = gen.create_trainer_pkmn("Scizor", 30)
        guillotine = gen.move_db().get_move("Guillotine")

        assert gen.calculate_damage(chimchar, guillotine, scizor) is None


class TestSuperFangAndEndeavor:
    """Not-implemented items #5: Super Fang = target HP // 2 (min 1); Endeavor =
    defender HP - attacker HP (fails if not positive)."""

    def test_super_fang_halves_current_hp(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        super_fang = gen.move_db().get_move("Super Fang")

        result = gen.calculate_damage(chimchar, super_fang, starly)
        assert result.damage_vals == {starly.cur_stats.hp // 2: 1}

    def test_endeavor_fails_when_user_has_more_hp(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        endeavor = gen.move_db().get_move("Endeavor")

        assert chimchar.cur_stats.hp > starly.cur_stats.hp
        assert gen.calculate_damage(chimchar, endeavor, starly) is None


class TestPresent:
    """Not-implemented item #1: Present's power comes from a dropdown; "Heal" deals
    no damage (it heals the target instead)."""

    def test_power_dropdown(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        present = gen.move_db().get_move("Present")

        result = gen.calculate_damage(chimchar, present, starly, custom_move_data="40")
        assert result.damage_vals == {12: 2, 13: 7, 14: 6, 15: 1}

    def test_heal_option_deals_no_damage(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        present = gen.move_db().get_move("Present")

        assert gen.calculate_damage(chimchar, present, starly, custom_move_data="Heal") is None


class TestFling:
    """Not-implemented item #3: Fling's power comes from the held item's fling-power table."""

    def test_iron_ball_is_130_power(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        fling = gen.move_db().get_move("Fling")

        chimchar.held_item = "Iron Ball"
        result = gen.calculate_damage(chimchar, fling, starly)
        assert result is not None
        assert result.min_damage > 0

    def test_no_item_fails(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        fling = gen.move_db().get_move("Fling")

        chimchar.held_item = None
        assert gen.calculate_damage(chimchar, fling, starly) is None


class TestTripleKick:
    """Not-implemented item #8: Triple Kick is 3 independent hits (power 10/20/30),
    each with its own crit/roll -- not the n-th kick's damage, and not a flat
    multiplier applied to a single roll."""

    def test_single_kick_is_ten_power(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        triple_kick = gen.move_db().get_move("Triple Kick")

        result = gen.calculate_damage(chimchar, triple_kick, starly, custom_move_data="1")
        assert result.damage_vals == {4: 15, 5: 1}

    def test_three_kicks_is_a_joint_distribution_not_a_flat_multiplier(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        triple_kick = gen.move_db().get_move("Triple Kick")

        one_kick = gen.calculate_damage(chimchar, triple_kick, starly, custom_move_data="1")
        three_kicks = gen.calculate_damage(chimchar, triple_kick, starly, custom_move_data="3")

        # kicks are power 10/20/30, so three kicks is far more than 3x a single
        # (power-10) kick's damage, and the roll distribution has many more
        # possible totals than the flat "one_kick.max * 3" model would produce
        assert three_kicks.min_damage > one_kick.max_damage * 3
        assert len(three_kicks.damage_vals) > 4


class TestSoundproofImmunity:
    """Not-implemented ability: Soundproof blocks sound-based moves entirely."""

    def test_soundproof_blocks_uproar(self):
        gen = _setup()
        chimchar = gen.create_trainer_pkmn("Chimchar", 20)
        starly = gen.create_trainer_pkmn("Starly", 20)
        uproar = gen.move_db().get_move("Uproar")

        starly.ability = "Soundproof"
        assert gen.calculate_damage(chimchar, uproar, starly) is None
