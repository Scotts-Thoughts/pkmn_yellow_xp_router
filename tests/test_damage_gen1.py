"""
Regression tests for the Gen 1 (Red/Blue/Yellow) damage-calculation fixes documented in
docs/damage_calc_review/gen_1_findings.md. Each test case's expected numbers are the
hand-derived "Game:" values from that file's numbered bugs/sections, verified against the
app after the fix.
"""
import pytest
from utils.constants import const
from pkmn import gen_factory, universal_data_objects
from pkmn.gen_1 import pkmn_damage_calc


VERSION = const.YELLOW_VERSION


def _setup():
    gen_factory.change_version(VERSION)
    return gen_factory.current_gen_info()


# =============================================================================
# Bug 1 -- missing >255 stat scaling / low-byte truncation, and Explosion ordering
# =============================================================================
class TestStatScalingAndTruncation:
    def test_special_stats_over_255_are_scaled_and_truncated(self):
        """Alakazam L61 +2 SpAtk (358) Psychic vs Dewgong L54 (SpDef 116): both stats get
        divided by 4 because the attacking stat is > 255, giving scaled A=89, D=29."""
        gen = _setup()
        alakazam = gen.create_trainer_pkmn("Alakazam", 61)
        dewgong = gen.create_trainer_pkmn("Dewgong", 54)
        psychic = gen.move_db().get_move("Psychic")

        result = gen.calculate_damage(
            alakazam, psychic, dewgong,
            attacking_stage_modifiers=universal_data_objects.StageModifiers(special_attack=2),
        )
        assert (result.min_damage, result.max_damage) == (184, 217)

    def test_reflect_doubled_defense_over_255_scales_both_stats_and_wraps_low_byte(self):
        """Nidoking L50 Horn Attack (Atk 106) vs Onix L50 +6 Def with Reflect (Def doubles
        to 1384): since the defence alone exceeds 255, BOTH stats are divided by 4 (even
        though attack alone is only 106), then only the low byte survives (346 -> 90)."""
        gen = _setup()
        nidoking = gen.create_trainer_pkmn("Nidoking", 50)
        onix = gen.create_trainer_pkmn("Onix", 50)
        horn_attack = gen.move_db().get_move("Horn Attack")

        result = gen.calculate_damage(
            nidoking, horn_attack, onix,
            defending_stage_modifiers=universal_data_objects.StageModifiers(defense=6),
            defending_field=universal_data_objects.FieldStatus(reflect=True),
        )
        assert (result.min_damage, result.max_damage) == (4, 5)

    def test_attacking_stat_over_255_is_scaled(self):
        """Nidoking L50 +6 Atk (424) Earthquake (STAB) vs Rhydon L50: attacking stat alone
        exceeds 255 and gets divided by 4, along with the defending stat."""
        gen = _setup()
        nidoking = gen.create_trainer_pkmn("Nidoking", 50)
        rhydon = gen.create_trainer_pkmn("Rhydon", 50)
        earthquake = gen.move_db().get_move("Earthquake")

        result = gen.calculate_damage(
            nidoking, earthquake, rhydon,
            attacking_stage_modifiers=universal_data_objects.StageModifiers(attack=6),
        )
        assert (result.min_damage, result.max_damage) == (364, 428)

    def test_stats_at_or_below_255_are_not_scaled(self):
        """Sanity check: nothing above should fire for an ordinary matchup with no stat
        anywhere near 256, so the scaling/truncation code must be a no-op here."""
        gen = _setup()
        pikachu = gen.create_trainer_pkmn("Pikachu", 20)
        rattata = gen.create_trainer_pkmn("Rattata", 20)
        thundershock = gen.move_db().get_move("Thundershock")

        result = gen.calculate_damage(pikachu, thundershock, rattata)
        assert result is not None
        assert result.max_damage > 0


# =============================================================================
# Bug 2 -- type effectiveness must apply in ROM TypeEffects table order
# =============================================================================
class TestTypeEffectivenessOrder:
    def test_super_effective_row_before_not_very_effective_row(self):
        """Tangela L5 Vine Whip (Grass, STAB) vs Nidoking L5 (Poison/Ground): the ROM applies
        the Grass>Ground SE row before the Grass>Poison NVE row, so the halving happens on
        the already-doubled value (14 -> 7), not the reverse (which the old type_1-then-
        type_2 order produced: 3 -> 6)."""
        gen = _setup()
        tangela = gen.create_trainer_pkmn("Tangela", 5)
        nidoking = gen.create_trainer_pkmn("Nidoking", 5)
        vine_whip = gen.move_db().get_move("Vine Whip")

        result = gen.calculate_damage(tangela, vine_whip, nidoking)
        assert (result.min_damage, result.max_damage) == (5, 7)

    def test_not_very_effective_row_before_super_effective_row(self):
        """Nidoran M L13 Poison Sting (Poison, STAB) vs Weedle L13 (Bug/Poison): the ROM
        applies the Poison>Poison NVE row before the Poison>Bug SE row (3 -> 6)."""
        gen = _setup()
        nidoran_m = gen.create_trainer_pkmn("Nidoran M", 13)
        weedle = gen.create_trainer_pkmn("Weedle", 13)
        poison_sting = gen.move_db().get_move("Poison Sting")

        result = gen.calculate_damage(nidoran_m, poison_sting, weedle)
        assert (result.min_damage, result.max_damage) == (5, 6)

    def test_fire_vs_ice_water_applies_super_effective_first(self):
        """Charizard L13 Fire Blast (STAB) vs Lapras L13 (Water/Ice): Fire>Ice (SE) comes
        before Fire>Water (NVE) in the ROM table."""
        gen = _setup()
        charizard = gen.create_trainer_pkmn("Charizard", 13)
        lapras = gen.create_trainer_pkmn("Lapras", 13)
        fire_blast = gen.move_db().get_move("Fire Blast")

        result = gen.calculate_damage(charizard, fire_blast, lapras)
        assert (result.min_damage, result.max_damage) == (21, 25)

    def test_immunity_still_short_circuits_regardless_of_order(self):
        gen = _setup()
        golem = gen.create_trainer_pkmn("Golem", 40)
        pikachu = gen.create_trainer_pkmn("Pikachu", 40)
        thunderbolt = gen.move_db().get_move("Thunderbolt")

        assert gen.calculate_damage(pikachu, thunderbolt, golem) is None


# =============================================================================
# Bug 3 / 4.4 -- Super Fang
# =============================================================================
class TestSuperFang:
    def test_full_hp_default(self):
        gen = _setup()
        raticate = gen.create_trainer_pkmn("Raticate", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        super_fang = gen.move_db().get_move("Super Fang")

        result = gen.calculate_damage(raticate, super_fang, rattata)
        expected = max(rattata.cur_stats.hp // 2, 1)
        assert result.min_damage == result.max_damage == expected

    def test_hp_percentage_dropdown(self):
        gen = _setup()
        raticate = gen.create_trainer_pkmn("Raticate", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        super_fang = gen.move_db().get_move("Super Fang")

        half_hp_result = gen.calculate_damage(raticate, super_fang, rattata, custom_move_data="50% HP")
        expected = max((rattata.cur_stats.hp // 2) // 2, 1)
        assert half_hp_result.min_damage == half_hp_result.max_damage == expected

    def test_min_damage_is_one(self):
        gen = _setup()
        raticate = gen.create_trainer_pkmn("Raticate", 2)
        rattata = gen.create_trainer_pkmn("Rattata", 2)
        super_fang = gen.move_db().get_move("Super Fang")

        result = gen.calculate_damage(raticate, super_fang, rattata, custom_move_data="10% HP")
        assert result.min_damage >= 1


# =============================================================================
# Bug 4 -- missing 997 cap before the +2
# =============================================================================
class TestDamageCap:
    def test_extreme_matchup_is_capped_before_stab(self):
        """Snorlax L100 crit Explosion (STAB) vs Chansey L5: the raw quotient is far above
        997, so it must be clamped to 997 before the +2 and STAB are applied (999 -> 1498),
        not left to explode into the tens of thousands."""
        gen = _setup()
        snorlax = gen.create_trainer_pkmn("Snorlax", 100)
        chansey = gen.create_trainer_pkmn("Chansey", 5)
        explosion = gen.move_db().get_move("Explosion")

        result = gen.calculate_damage(snorlax, explosion, chansey, is_crit=True)
        assert (result.min_damage, result.max_damage) == (1274, 1498)
        assert result.max_damage <= 1498


# =============================================================================
# Bug 5 -- enemy Psywave can roll 0
# =============================================================================
class TestPsywaveRange:
    def test_player_psywave_excludes_zero(self):
        gen = _setup()
        gengar = gen.create_trainer_pkmn("Gengar", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        psywave = gen.move_db().get_move("Psywave")

        result = gen.calculate_damage(gengar, psywave, rattata, attacker_is_enemy=False)
        assert result.min_damage == 1
        assert result.max_damage == 44
        assert result.size == 44
        assert 0 not in result.damage_vals

    def test_enemy_psywave_includes_zero(self):
        gen = _setup()
        gengar = gen.create_trainer_pkmn("Gengar", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        psywave = gen.move_db().get_move("Psywave")

        result = gen.calculate_damage(gengar, psywave, rattata, attacker_is_enemy=True)
        assert result.min_damage == 0
        assert result.max_damage == 44
        assert result.size == 45
        assert 0 in result.damage_vals


# =============================================================================
# Bug 6 -- stat-exp sqrt term must cap ceil(sqrt(...)) at 255
# =============================================================================
class TestStatExpSqrtCap:
    def test_stat_exp_above_65025_is_capped(self):
        from pkmn.gen_1 import pkmn_utils
        # Both values round-trip through ceil(sqrt(...)) capped at 255 -> the same final stat.
        assert pkmn_utils.calc_stat(100, 100, 15, 65535) == 298
        assert pkmn_utils.calc_stat(100, 100, 15, 65025) == 298


# =============================================================================
# Bug 7 -- moves.json accuracy data
# =============================================================================
class TestAccuracyData:
    def test_bind_accuracy_is_75(self):
        gen = _setup()
        assert gen.move_db().get_move("Bind").accuracy == 75

    def test_psywave_accuracy_is_80(self):
        gen = _setup()
        assert gen.move_db().get_move("Psywave").accuracy == 80

    def test_poison_gas_accuracy_is_55(self):
        gen = _setup()
        assert gen.move_db().get_move("Poison Gas").accuracy == 55


# =============================================================================
# 4.1 -- OHKO moves (Guillotine, Horn Drill, Fissure)
# =============================================================================
class TestOneHitKO:
    def test_fails_when_user_is_slower(self):
        gen = _setup()
        golem = gen.create_trainer_pkmn("Golem", 40)  # base speed 45
        jolteon = gen.create_trainer_pkmn("Jolteon", 40)  # base speed 130
        guillotine = gen.move_db().get_move("Guillotine")

        assert gen.calculate_damage(golem, guillotine, jolteon) is None

    def test_deals_full_remaining_hp_when_at_least_as_fast(self):
        gen = _setup()
        jolteon = gen.create_trainer_pkmn("Jolteon", 40)
        golem = gen.create_trainer_pkmn("Golem", 40)
        guillotine = gen.move_db().get_move("Guillotine")

        result = gen.calculate_damage(jolteon, guillotine, golem)
        assert result.min_damage == result.max_damage == golem.cur_stats.hp

    def test_fissure_fails_against_flying_immunity(self):
        gen = _setup()
        golem = gen.create_trainer_pkmn("Golem", 40)
        pidgeotto = gen.create_trainer_pkmn("Pidgeotto", 40)
        fissure = gen.move_db().get_move("Fissure")

        assert gen.calculate_damage(golem, fissure, pidgeotto) is None

    def test_guillotine_fails_against_ghost_immunity(self):
        gen = _setup()
        golem = gen.create_trainer_pkmn("Golem", 40)
        gengar = gen.create_trainer_pkmn("Gengar", 40)
        guillotine = gen.move_db().get_move("Guillotine")

        assert gen.calculate_damage(golem, guillotine, gengar) is None


# =============================================================================
# 4.2 / 4.3 -- Counter and Bide (require a "damage taken" numeric input)
# =============================================================================
class TestCounterAndBide:
    def test_counter_doubles_supplied_damage(self):
        gen = _setup()
        raticate = gen.create_trainer_pkmn("Raticate", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        counter = gen.move_db().get_move("Counter")

        result = gen.calculate_damage(raticate, counter, rattata, custom_move_data="100")
        assert result.min_damage == result.max_damage == 200

    def test_counter_without_data_is_none(self):
        gen = _setup()
        raticate = gen.create_trainer_pkmn("Raticate", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        counter = gen.move_db().get_move("Counter")

        assert gen.calculate_damage(raticate, counter, rattata) is None

    def test_bide_doubles_accumulated_damage(self):
        gen = _setup()
        raticate = gen.create_trainer_pkmn("Raticate", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        bide = gen.move_db().get_move("Bide")

        result = gen.calculate_damage(raticate, bide, rattata, custom_move_data="50")
        assert result.min_damage == result.max_damage == 100

    def test_counter_ignores_type_immunity(self):
        """Counter hits Ghost-types (no type check at all)."""
        gen = _setup()
        raticate = gen.create_trainer_pkmn("Raticate", 30)
        gengar = gen.create_trainer_pkmn("Gengar", 30)
        counter = gen.move_db().get_move("Counter")

        result = gen.calculate_damage(raticate, counter, gengar, custom_move_data="60")
        assert result.min_damage == result.max_damage == 120


# =============================================================================
# 4.6 -- partial trapping moves (Bind/Wrap/Fire Spin/Clamp) as multi-turn sequences
# =============================================================================
class TestPartialTrapping:
    @pytest.mark.parametrize("move_name", ["Bind", "Wrap", "Fire Spin", "Clamp"])
    def test_turn_count_multiplies_the_single_rolled_damage(self, move_name):
        gen = _setup()
        raticate = gen.create_trainer_pkmn("Raticate", 30)
        rattata = gen.create_trainer_pkmn("Rattata", 30)
        move = gen.move_db().get_move(move_name)

        one_turn = gen.calculate_damage(raticate, move, rattata)
        three_turns = gen.calculate_damage(raticate, move, rattata, custom_move_data="3 Turns")

        assert three_turns.min_damage == one_turn.min_damage * 3
        assert three_turns.max_damage == one_turn.max_damage * 3

    def test_custom_move_data_options_are_wired_up(self):
        gen = _setup()
        assert gen.get_move_custom_data("Bind") == ["2 Turns", "3 Turns", "4 Turns", "5 Turns"]


# =============================================================================
# 4.7 -- Focus Energy's effect on crit rate
# =============================================================================
class TestFocusEnergyCritRate:
    def test_normal_move_crit_rate_quartered(self):
        gen = _setup()
        raticate = gen.create_trainer_pkmn("Raticate", 30)  # base speed 97
        tackle = gen.move_db().get_move("Tackle")

        base_rate = pkmn_damage_calc.get_crit_rate(raticate, tackle, "")
        focus_energy_rate = pkmn_damage_calc.get_crit_rate(raticate, tackle, "focus_energy")

        assert base_rate == pytest.approx(48 / 256)
        assert focus_energy_rate == pytest.approx(12 / 256)

    def test_high_crit_move_rate_halved(self):
        gen = _setup()
        raticate = gen.create_trainer_pkmn("Raticate", 30)
        karate_chop = gen.move_db().get_move("Karate Chop")

        base_rate = pkmn_damage_calc.get_crit_rate(raticate, karate_chop, "")
        focus_energy_rate = pkmn_damage_calc.get_crit_rate(raticate, karate_chop, "focus_energy")

        assert base_rate == pytest.approx(255 / 256)
        assert focus_energy_rate == pytest.approx(96 / 256)
