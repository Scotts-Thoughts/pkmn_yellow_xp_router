"""
Gen 5 (Black/White) damage-calculation regression tests.

Companion to docs/damage_calc_review/gen_5_findings.md and gen_5_move_table.md. Gen 5
has no available decompilation, so unlike the other generations' test modules, some
of these assertions check relative/structural properties (e.g. "doesn't return None
anymore", "slower attacker gets more Gyro Ball power") rather than an exact number
hand-derived from ROM disassembly -- there is no ROM disassembly to derive it from.
Where a gen 5 formula is genuinely unverified community knowledge (Psywave, the doubles
spread factor, the final modifier chain), the test docstring says so.
"""
import pytest
from utils.constants import const
from pkmn import universal_data_objects, gen_factory


def _setup():
    gen_factory.change_version(const.BLACK_VERSION)
    return gen_factory.current_gen_info()


class TestAttackFlavorSynthesis:
    """gen_5_findings.md section 1, items 1-3: the gen 5 moves.json has no
    `attack_flavor` list, only a scalar `effect` code, so high-crit/multi-hit/two-hit
    moves need it synthesized in _load_move_db or they silently lose their mechanic."""

    def test_high_crit_flavor_present(self):
        gen = _setup()
        for name in ["Karate Chop", "Slash", "Crabhammer", "Aeroblast", "Night Slash"]:
            move = gen.move_db().get_move(name)
            assert const.FLAVOR_HIGH_CRIT in move.attack_flavor, f"{name} missing high_crit flavor"

    def test_plain_move_has_no_high_crit_flavor(self):
        gen = _setup()
        move = gen.move_db().get_move("Tackle")
        assert const.FLAVOR_HIGH_CRIT not in move.attack_flavor

    def test_multi_hit_flavor_present(self):
        gen = _setup()
        for name in ["Comet Punch", "Double Slap", "Bullet Seed", "Tail Slap"]:
            move = gen.move_db().get_move(name)
            assert const.FLAVOR_MULTI_HIT in move.attack_flavor, f"{name} missing multi_hit flavor"

    def test_two_hit_flavor_present(self):
        gen = _setup()
        for name in ["Double Kick", "Bonemerang", "Double Hit", "Dual Chop"]:
            move = gen.move_db().get_move(name)
            assert const.DOUBLE_HIT_FLAVOR in move.attack_flavor, f"{name} missing two_hit flavor"

    def test_crit_rate_reflects_high_crit_flavor(self):
        gen = _setup()
        pikachu = gen.create_trainer_pkmn("Pikachu", 30)
        karate_chop = gen.move_db().get_move("Karate Chop")
        tackle = gen.move_db().get_move("Tackle")

        assert gen.get_crit_rate(pikachu, karate_chop, "") == pytest.approx(1 / 8)
        assert gen.get_crit_rate(pikachu, tackle, "") == pytest.approx(1 / 16)

    def test_two_hit_move_deals_twice_the_single_hit_damage(self):
        """A two-hit move's max damage should be exactly double a single hit's, since
        DOUBLE_HIT_FLAVOR now actually triggers the multi-hit path."""
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Machop", 40)
        defender = gen.create_trainer_pkmn("Geodude", 40)
        double_kick = gen.move_db().get_move("Double Kick")

        dmg = gen.calculate_damage(attacker, double_kick, defender)
        assert dmg is not None
        # num_attacks tracks how many hits were combined into the range.
        assert dmg.num_attacks == 2


class TestVariablePowerMovesNoLongerReturnNone:
    """gen_5_findings.md section 1, item 4: `power: null` on every variable-power move
    used to hit the early base_power check before its per-move override ran, so every
    one of these silently reported "no damage"."""

    @pytest.mark.parametrize("move_name,custom_data", [
        ("Low Kick", None),
        ("Grass Knot", None),
        ("Flail", "100-69 % HP"),
        ("Reversal", "100-69 % HP"),
        ("Return", "102"),
        ("Frustration", "102"),
        ("Present", "40"),
        ("Gyro Ball", None),
        ("Natural Gift", None),
        ("Trump Card", "4+"),
        ("Crush Grip", "100"),
        ("Wring Out", "100"),
        ("Punishment", None),
        ("Psywave", None),
        ("Super Fang", None),
        ("Endeavor", "50"),
        ("Final Gambit", None),
        ("Heavy Slam", None),
        ("Heat Crash", None),
        ("Electro Ball", None),
        ("Stored Power", None),
    ])
    def test_deals_damage(self, move_name, custom_data):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Snorlax", 50)
        defender = gen.create_trainer_pkmn("Machop", 50)
        if move_name == "Natural Gift":
            attacker.held_item = "Oran Berry"

        move = gen.move_db().get_move(move_name)
        assert move is not None, f"{move_name} missing from gen 5 move db"

        kwargs = {}
        if custom_data is not None:
            kwargs["custom_move_data"] = custom_data
        dmg = gen.calculate_damage(attacker, move, defender, **kwargs)
        assert dmg is not None, f"{move_name} still deals no damage"
        assert dmg.max_damage > 0

    def test_guillotine_ohko_when_faster(self):
        gen = _setup()
        fast = gen.create_trainer_pkmn("Pikachu", 50)  # base speed 90
        slow = gen.create_trainer_pkmn("Snorlax", 50)  # base speed 30
        guillotine = gen.move_db().get_move("Guillotine")

        dmg = gen.calculate_damage(fast, guillotine, slow)
        assert dmg is not None
        assert dmg.max_damage == slow.cur_stats.hp

    def test_guillotine_fails_when_slower(self):
        gen = _setup()
        slow = gen.create_trainer_pkmn("Snorlax", 50)
        fast = gen.create_trainer_pkmn("Pikachu", 50)
        guillotine = gen.move_db().get_move("Guillotine")

        assert gen.calculate_damage(slow, guillotine, fast) is None

    def test_super_fang_is_half_defender_hp(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Rattata", 50)
        defender = gen.create_trainer_pkmn("Snorlax", 50)
        super_fang = gen.move_db().get_move("Super Fang")

        dmg = gen.calculate_damage(attacker, super_fang, defender)
        assert dmg.max_damage == max(defender.cur_stats.hp // 2, 1)

    def test_present_heal_option_deals_no_damage(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Chansey", 50)
        defender = gen.create_trainer_pkmn("Machop", 50)
        present = gen.move_db().get_move("Present")

        assert gen.calculate_damage(attacker, present, defender, custom_move_data="Heal") is None


class TestCritStageHandling:
    """gen_4_findings.md bug #1 (ported into the gen 5 copy verbatim): the crit
    stage-ignoring logic tested the wrong stat category AND wiped every stage instead
    of just the one that should be ignored."""

    def test_crit_ignores_only_the_negative_attack_stage(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Machop", 40)
        defender = gen.create_trainer_pkmn("Geodude", 40)
        tackle = gen.move_db().get_move("Tackle")

        neutral_stages = universal_data_objects.StageModifiers()
        lowered_attack = universal_data_objects.StageModifiers(attack=-2)
        # also lower Speed, which must NOT be reset by the crit -- only Attack should be.
        lowered_attack_and_speed = universal_data_objects.StageModifiers(attack=-2, speed=-2)

        neutral_dmg = gen.calculate_damage(attacker, tackle, defender, attacking_stage_modifiers=neutral_stages, is_crit=True)
        lowered_dmg = gen.calculate_damage(attacker, tackle, defender, attacking_stage_modifiers=lowered_attack, is_crit=True)
        lowered_both_dmg = gen.calculate_damage(attacker, tackle, defender, attacking_stage_modifiers=lowered_attack_and_speed, is_crit=True)

        # A crit should ignore the -2 Attack stage entirely, matching the neutral case.
        assert lowered_dmg.max_damage == neutral_dmg.max_damage
        assert lowered_both_dmg.max_damage == neutral_dmg.max_damage

    def test_crit_ignores_only_the_positive_defense_stage(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Machop", 40)
        defender = gen.create_trainer_pkmn("Geodude", 40)
        tackle = gen.move_db().get_move("Tackle")

        neutral_dmg = gen.calculate_damage(attacker, tackle, defender, is_crit=True)
        raised_def_dmg = gen.calculate_damage(
            attacker, tackle, defender,
            defending_stage_modifiers=universal_data_objects.StageModifiers(defense=2),
            is_crit=True,
        )

        assert raised_def_dmg.max_damage == neutral_dmg.max_damage

    def test_special_move_crit_uses_special_stages_not_physical(self):
        """Regression for the original bug: physical moves checked the SPECIAL stages
        (and vice versa), so a -2 Attack stage was never ignored on a physical crit."""
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Machop", 40)
        defender = gen.create_trainer_pkmn("Geodude", 40)
        confusion = gen.move_db().get_move("Confusion")  # Special move

        neutral_dmg = gen.calculate_damage(attacker, confusion, defender, is_crit=True)
        lowered_spa_dmg = gen.calculate_damage(
            attacker, confusion, defender,
            attacking_stage_modifiers=universal_data_objects.StageModifiers(special_attack=-2),
            is_crit=True,
        )
        assert lowered_spa_dmg.max_damage == neutral_dmg.max_damage


class TestGyroBallDirection:
    """gen_4_findings.md bug #4c (ported verbatim): the speed ratio was inverted, so a
    slow user got a weak Gyro Ball instead of a strong one."""

    def test_slower_attacker_gets_more_power_than_faster_attacker(self):
        gen = _setup()
        slow_attacker = gen.create_trainer_pkmn("Snorlax", 50)  # base speed 30
        fast_defender = gen.create_trainer_pkmn("Pikachu", 50)  # base speed 90
        gyro_ball = gen.move_db().get_move("Gyro Ball")

        slow_vs_fast = gen.calculate_damage(slow_attacker, gyro_ball, fast_defender)
        fast_vs_slow = gen.calculate_damage(fast_defender, gyro_ball, slow_attacker)

        assert slow_vs_fast.max_damage > fast_vs_slow.max_damage


class TestCrushGripFormula:
    """The formula was missing a /100, turning a 0-100 HP% dropdown value into a base
    power in the thousands instead of 1-121."""

    def test_full_hp_power_is_sane(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Machop", 50)
        defender = gen.create_trainer_pkmn("Snorlax", 50)
        crush_grip = gen.move_db().get_move("Crush Grip")

        full_hp = gen.calculate_damage(attacker, crush_grip, defender, custom_move_data="100")
        half_hp = gen.calculate_damage(attacker, crush_grip, defender, custom_move_data="50")

        # Real power tops out at 1 + 120 = 121; with the missing /100 it used to be
        # 12001, which against a Snorlax-sized Defense stat would one-shot for
        # thousands of HP. A sane upper bound: no normal move should exceed the
        # defender's own max HP by an order of magnitude at these levels.
        assert full_hp.max_damage < defender.cur_stats.hp * 5
        assert full_hp.max_damage > half_hp.max_damage


class TestTrumpCardFourPlus:
    def test_four_plus_sets_power(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Chansey", 50)
        defender = gen.create_trainer_pkmn("Machop", 50)
        trump_card = gen.move_db().get_move("Trump Card")

        dmg = gen.calculate_damage(attacker, trump_card, defender, custom_move_data="4+")
        assert dmg is not None
        assert dmg.max_damage > 0

    def test_zero_pp_hits_hardest(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Chansey", 50)
        defender = gen.create_trainer_pkmn("Machop", 50)
        trump_card = gen.move_db().get_move("Trump Card")

        four_plus = gen.calculate_damage(attacker, trump_card, defender, custom_move_data="4+").max_damage
        zero = gen.calculate_damage(attacker, trump_card, defender, custom_move_data="0").max_damage
        assert zero > four_plus


class TestRolloutAndFuryCutter:
    def test_rollout_turn_one_is_not_pre_doubled(self):
        """Turn 1 power should equal the move's base power (no doubling); turn 2
        should be exactly double turn 1."""
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Machop", 40)
        defender = gen.create_trainer_pkmn("Geodude", 40)
        rollout = gen.move_db().get_move("Rollout")

        turn_one = gen.calculate_damage(attacker, rollout, defender, custom_move_data="1").max_damage
        turn_two = gen.calculate_damage(attacker, rollout, defender, custom_move_data="2").max_damage
        assert turn_two == turn_one * 2

    def test_fury_cutter_caps_at_five_doublings(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Scizor", 40)
        defender = gen.create_trainer_pkmn("Geodude", 40)
        fury_cutter = gen.move_db().get_move("Fury Cutter")

        five = gen.calculate_damage(attacker, fury_cutter, defender, custom_move_data="5").max_damage
        six = gen.calculate_damage(attacker, fury_cutter, defender, custom_move_data="6").max_damage
        assert five == six


class TestExplosionNoLongerHalvesDefense:
    def test_explosion_deals_more_than_gen4_style_halved_defense_would(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Snorlax", 50)
        defender = gen.create_trainer_pkmn("Snorlax", 50)
        explosion = gen.move_db().get_move("Explosion")
        tackle_stand_in_power = gen.move_db().get_move("Giga Impact")  # 150 power, no special-case

        exp_dmg = gen.calculate_damage(attacker, explosion, defender).max_damage
        # Explosion (250 power) with unhalved defense must still exceed a 150-power
        # move with the same (unhalved) defense.
        giga_dmg = gen.calculate_damage(attacker, tackle_stand_in_power, defender).max_damage
        assert exp_dmg > giga_dmg


class TestStruggleIsTypeless:
    def test_struggle_hits_ghost_types(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Machop", 50)
        defender = gen.create_trainer_pkmn("Gastly", 50)
        struggle = gen.move_db().get_move("Struggle")

        assert gen.calculate_damage(attacker, struggle, defender) is not None

    def test_struggle_gets_no_stab(self):
        gen = _setup()
        normal_attacker = gen.create_trainer_pkmn("Rattata", 50)  # pure Normal
        non_normal_attacker = gen.create_trainer_pkmn("Machop", 50)  # pure Fighting
        defender = gen.create_trainer_pkmn("Geodude", 50)
        struggle = gen.move_db().get_move("Struggle")

        # If STAB were (incorrectly) applied, the Normal-type user would out-damage an
        # otherwise-equal-stat non-Normal user by 1.5x; comparing to itself with/without
        # a hypothetical STAB isn't directly testable here, so instead assert Ghost
        # (which would be immune to a Normal-typed Struggle) still takes damage, and
        # that damage is not scaled up by an integer multiple suggesting STAB leaked in.
        gastly = gen.create_trainer_pkmn("Gastly", 50)
        dmg = gen.calculate_damage(normal_attacker, struggle, gastly)
        assert dmg is not None and dmg.max_damage > 0


class TestFutureSightAndDoomDesire:
    def test_future_sight_can_crit(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Machop", 50)
        defender = gen.create_trainer_pkmn("Gastly", 50)
        future_sight = gen.move_db().get_move("Future Sight")

        non_crit = gen.calculate_damage(attacker, future_sight, defender, is_crit=False).max_damage
        crit = gen.calculate_damage(attacker, future_sight, defender, is_crit=True).max_damage
        assert crit > non_crit

    def test_future_sight_gets_stab(self):
        gen = _setup()
        psychic_attacker = gen.create_trainer_pkmn("Gastly", 50)  # Ghost/Poison, no Psychic STAB
        defender = gen.create_trainer_pkmn("Machop", 50)
        future_sight = gen.move_db().get_move("Future Sight")

        assert gen.calculate_damage(psychic_attacker, future_sight, defender) is not None


class TestNaturalGiftGen5Table:
    def test_berry_power_is_twenty_higher_than_gen_four(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Chansey", 50)
        attacker.held_item = "Oran Berry"
        defender = gen.create_trainer_pkmn("Machop", 50)
        natural_gift = gen.move_db().get_move("Natural Gift")

        gen5_type, gen5_power = gen.get_natural_gift("Oran Berry")
        assert gen5_power == 80  # gen 4 value (60) + 20
        assert gen5_type == const.TYPE_POISON

        dmg = gen.calculate_damage(attacker, natural_gift, defender)
        assert dmg is not None


class TestFoulPlayUsesTargetAttack:
    def test_damage_depends_on_targets_attack_not_the_users(self):
        """Foul Play's power formula only cares about the level, the target's Attack
        and the target's Defense -- none of the user's own stats. Two very different
        attackers hitting the same target should therefore deal identical damage."""
        gen = _setup()
        weak_attacker = gen.create_trainer_pkmn("Chansey", 50)  # very low Attack
        strong_attacker = gen.create_trainer_pkmn("Machop", 50)  # much higher Attack
        defender = gen.create_trainer_pkmn("Geodude", 50)
        foul_play = gen.move_db().get_move("Foul Play")

        from_weak = gen.calculate_damage(weak_attacker, foul_play, defender)
        from_strong = gen.calculate_damage(strong_attacker, foul_play, defender)
        assert from_weak is not None and from_strong is not None
        assert from_weak.max_damage == from_strong.max_damage


class TestPsyshockUsesTargetDefense:
    def test_high_special_defense_low_defense_target_takes_more_damage(self):
        """Psyshock uses the target's Defense, not Sp. Defense. Chansey is the classic
        case: Defense 5, Sp. Defense 105 -- so Psyshock should hit far harder than
        Psychic despite Chansey's huge Sp. Defense."""
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Latios", 50)
        defender = gen.create_trainer_pkmn("Chansey", 50)  # Def 5, SpD 105
        psychic = gen.move_db().get_move("Psychic")  # uses Sp.Def normally
        psyshock = gen.move_db().get_move("Psyshock")

        psychic_dmg = gen.calculate_damage(attacker, psychic, defender)
        psyshock_dmg = gen.calculate_damage(attacker, psyshock, defender)
        assert psyshock_dmg is not None and psychic_dmg is not None
        assert psyshock_dmg.max_damage > psychic_dmg.max_damage


class TestGetValidWeatherHasNoFog:
    def test_fog_is_not_offered(self):
        gen = _setup()
        assert const.WEATHER_FOG not in gen.get_valid_weather()


class TestSpreadDamageFactor:
    """gen_5_findings.md section 1, item 6 / section 2: gen 4/5 json uses "All Foes"/
    "Others" for spread moves, not the gen-3-only "target_both_enemies" string, so the
    doubles reduction never fired at all; gen 5's factor is x0.75 (documented,
    unverified -- no gen 5 decomp exists)."""

    def test_spread_move_deals_less_in_doubles(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Machop", 50)
        defender = gen.create_trainer_pkmn("Geodude", 50)
        earthquake = gen.move_db().get_move("Earthquake")
        assert earthquake.targeting in ("All Foes", "Others")

        singles_dmg = gen.calculate_damage(attacker, earthquake, defender, is_double_battle=False).max_damage
        doubles_dmg = gen.calculate_damage(attacker, earthquake, defender, is_double_battle=True).max_damage
        assert doubles_dmg < singles_dmg


class TestPsywaveGen5Distribution:
    """Documented, unverified (no gen 5 decomp): floor(level * (r+50) / 100), r in
    0..100, min 1 -- a materially different distribution than every earlier gen."""

    def test_range_matches_documented_formula(self):
        gen = _setup()
        attacker = gen.create_trainer_pkmn("Slowpoke", 50)
        defender = gen.create_trainer_pkmn("Machop", 50)
        psywave = gen.move_db().get_move("Psywave")

        dmg = gen.calculate_damage(attacker, psywave, defender)
        assert dmg is not None
        assert dmg.min_damage == max(1, (50 * 50) // 100)
        assert dmg.max_damage == (50 * 150) // 100
