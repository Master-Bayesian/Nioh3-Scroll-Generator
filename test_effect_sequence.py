from __future__ import annotations

import json
from pathlib import Path
import struct
import random
import unittest

from emaki_exchange import EFFECT_START, EFFECT_STRIDE
from nioh3_scroll_editor.effect_sequence import (
    generate_ng3_rarity3_effect_sequence,
    generate_ng3_rarity34_primary_effect_ids,
    generate_ng3_rarity4_final_effect_sequence,
    generate_ng3_certified_effect_sequence,
    generate_challenge_attempt_count,
    generate_ng3_rarity5_primary_effect,
    generate_ng3_rarity5_primary_effect_id,
    generate_ng3_rarity5_primary_effect_ids,
    generate_ng3_rarity5_effect_sequence,
    generate_rarity5_any_grace_primary_effect_ids,
    generate_rarity5_grace_effect_sequence,
    generate_rarity5_grace_primary_effect_id,
    generate_rarity5_grace_primary_effect_ids,
    materialize_ng3_rarity3_record,
    materialize_ng3_certified_record,
    materialize_ng3_rarity5_record,
    serialize_ng3_rarity3_effect_slots,
    serialize_rarity5_grace_effect_slots,
    serialize_ng3_rarity5_effect_slots,
)
from nioh3_scroll_editor.grace_map import (
    GraceOutputMap,
    GraceRange,
    grace_id_for_first_u16,
    load_grace_output_map,
)
from nioh3_seed_math import state_after_draw_from_seed


ROOT = Path(__file__).resolve().parent
VECTORS = json.loads(
    (ROOT / "test_fixtures" / "effect_sequence_vectors.json").read_text(
        encoding="utf-8"
    )
)


class Ng3Rarity5EffectSequenceTests(unittest.TestCase):
    def test_any_grace_primary_batch_matches_full_sequence(self) -> None:
        seeds = (0, 1, 2, 241719428, 0xFFFFFFFF)
        self.assertEqual(
            generate_rarity5_any_grace_primary_effect_ids(
                seeds,
                playthrough=3,
            ),
            tuple(
                generate_ng3_rarity5_effect_sequence(seed).primary.effect_id
                for seed in seeds
            ),
        )

    def test_challenge_attempt_count_matches_native_vectors(self) -> None:
        self.assertEqual(generate_challenge_attempt_count(1), 4)
        self.assertEqual(generate_challenge_attempt_count(0x0FFFFFFF), 7)
        self.assertEqual(generate_challenge_attempt_count(82212268), 5)
        self.assertEqual(generate_challenge_attempt_count(183696634), 6)

    def test_native_seed_one_vector(self) -> None:
        result = generate_ng3_rarity5_effect_sequence(1)
        self.assertEqual(
            [effect.effect_id for effect in result.effects],
            [0xA051, 0xD40A, 0x34F3, 0x3E7A, 0xAE5A, 0x6553],
        )
        self.assertEqual(
            [effect.roll_percent for effect in result.effects],
            [94, 91, 94, 96, 91, 0],
        )
        self.assertEqual(
            [effect.resolved_value for effect in result.effects],
            [9, 47, 14, 78, 150, 0],
        )
        self.assertEqual(
            [effect.category_and_flags for effect in result.effects],
            [0x43, 0x05, 0x06, 0x17, 0x0D, 0x0C],
        )
        self.assertEqual(
            [effect.prefix_word for effect in result.effects],
            [0x4C80, 0x30A6, 0x242A, 0xD405, 0x89DE, 0xB991],
        )
        self.assertEqual(
            [effect.effect_flags for effect in result.effects],
            [0, 0, 0, 0, 4, 2],
        )
        self.assertEqual(result.promoted_source_indexes, (5,))
        self.assertEqual(result.random_draws, 24)
        self.assertEqual(result.final_rng_state, 0x2FAC1E69)

    def test_native_seed_241719428_vector(self) -> None:
        result = generate_ng3_rarity5_effect_sequence(241719428)
        self.assertEqual(
            [effect.effect_id for effect in result.effects],
            [0x2B06, 0x92E0, 0x3F41, 0x4647, 0x3E7A, 0xBABD],
        )
        self.assertEqual(
            [effect.roll_percent for effect in result.effects],
            [94, 93, 93, 92, 94, 0],
        )
        self.assertEqual(
            [effect.resolved_value for effect in result.effects],
            [150, 96, 11, 16, 77, 0],
        )
        self.assertEqual(
            [effect.category_and_flags for effect in result.effects],
            [0x4D, 0x09, 0x06, 0x03, 0x17, 0x0C],
        )
        self.assertEqual(
            [effect.effect_flags for effect in result.effects],
            [4, 0, 0, 0, 0, 2],
        )
        self.assertEqual(result.promoted_source_indexes, (1,))
        self.assertEqual(result.random_draws, 24)
        self.assertEqual(result.final_rng_state, 0xCAD9B20C)

    def test_native_seed_82212268_vector(self) -> None:
        result = generate_ng3_rarity5_effect_sequence(82212268)
        self.assertEqual(
            [effect.effect_id for effect in result.effects],
            [0xBC51, 0x8184, 0x5CAC, 0x28C4, 0x512D, 0x6553],
        )
        self.assertEqual(
            [effect.roll_percent for effect in result.effects],
            [94, 92, 95, 92, 94, 0],
        )
        self.assertEqual(
            [effect.resolved_value for effect in result.effects],
            [97, 66, 68, 5, 0, 0],
        )

    def test_serialized_slots_match_native_seed_one_capture(self) -> None:
        record = bytes.fromhex(VECTORS["ng3_seed_1_record_hex"])
        result = generate_ng3_rarity5_effect_sequence(1)
        self.assertEqual(
            serialize_ng3_rarity5_effect_slots(result),
            record[0x34:0xDC],
        )

    def test_cross_seed_materialization_matches_complete_native_record(self) -> None:
        template = bytes.fromhex(VECTORS["ng3_seed_1_record_hex"])
        expected = bytes.fromhex(VECTORS["ng3_seed_241719428_record_hex"])
        record, result = materialize_ng3_rarity5_record(
            template,
            seed=241719428,
            level=180,
            recommended_level=183,
            transfer_count=0,
            generation_serial=struct.unpack_from("<I", expected, 0x28)[0],
        )

        self.assertEqual(result.seed, 241719428)
        self.assertEqual(record, expected)

    def test_seed_validation(self) -> None:
        with self.assertRaises(ValueError):
            generate_ng3_rarity5_effect_sequence(-1)
        with self.assertRaises(ValueError):
            generate_ng3_rarity5_effect_sequence(0x1_0000_0000)

    def test_primary_fast_path_matches_complete_sequence(self) -> None:
        for seed in (0, 1, 241719428, 255766105, 264410626, 0xFFFFFFFF):
            self.assertEqual(
                generate_ng3_rarity5_primary_effect(seed),
                generate_ng3_rarity5_effect_sequence(seed).primary,
            )

    def test_cached_primary_id_path_matches_complete_sequence(self) -> None:
        rng = random.Random(0x4E494F4833)
        seeds = [0, 1, 241719428, 255766105, 264410626, 0xFFFFFFFF]
        seeds.extend(rng.randrange(0x10000000) for _ in range(10_000))
        for seed in seeds:
            self.assertEqual(
                generate_ng3_rarity5_primary_effect_id(seed),
                generate_ng3_rarity5_effect_sequence(seed).primary.effect_id,
            )

    def test_native_primary_batch_matches_complete_sequence(self) -> None:
        rng = random.Random(0x43554441)
        seeds = tuple(rng.randrange(0x10000000) for _ in range(10_000))
        mapping = load_grace_output_map(rarity=5)
        groups: dict[int, list[int]] = {}
        for seed in seeds:
            grace_id = grace_id_for_first_u16(
                state_after_draw_from_seed(seed, 1) >> 16,
                mapping,
            )
            groups.setdefault(grace_id, []).append(seed)
        checked = 0
        for grace_id, group in groups.items():
            actual = generate_ng3_rarity5_primary_effect_ids(
                tuple(group),
                grace_id=grace_id,
                grace_mapping=mapping,
            )
            expected = tuple(
                generate_ng3_rarity5_effect_sequence(seed).primary.effect_id
                for seed in group
            )
            self.assertEqual(actual, expected)
            checked += len(group)
        self.assertEqual(checked, len(seeds))

    def test_primary_is_not_invariant_inside_one_draw2_high16_bucket(self) -> None:
        seeds = (255766105, 264410626)
        mapping = load_grace_output_map(rarity=5)
        outputs = [generate_ng3_rarity5_effect_sequence(seed) for seed in seeds]
        self.assertEqual(
            {state_after_draw_from_seed(seed, 2) >> 16 for seed in seeds},
            {0},
        )
        self.assertEqual(
            {
                grace_id_for_first_u16(
                    state_after_draw_from_seed(seed, 1) >> 16,
                    mapping,
                )
                for seed in seeds
            },
            {0x6553},
        )
        self.assertEqual(
            [output.primary.effect_id for output in outputs],
            [0x512D, 0x23E8],
        )

    def test_ng4_ng5_seed_one_matches_historical_native_effect_bytes(self) -> None:
        for playthrough, record_type in ((4, 0xDD82), (5, 0xD523)):
            with self.subTest(playthrough=playthrough):
                native = bytes.fromhex(VECTORS[f"ng{playthrough}_seed_1_record_hex"])
                mapping = GraceOutputMap(
                    record_type=record_type,
                    rarity=5,
                    playthrough=f"category-{playthrough}-live-native",
                    effect_slot=6,
                    ranges=(GraceRange(0, 0xFFFF, 0x6553),),
                )
                result = generate_rarity5_grace_effect_sequence(
                    1,
                    playthrough=playthrough,
                    level=180,
                    grace_mapping=mapping,
                )
                self.assertEqual(result.record_type, record_type)
                self.assertEqual(
                    serialize_rarity5_grace_effect_slots(result),
                    native[EFFECT_START : EFFECT_START + 7 * EFFECT_STRIDE],
                )
                self.assertEqual(
                    generate_rarity5_grace_primary_effect_id(
                        1,
                        playthrough=playthrough,
                        grace_mapping=mapping,
                    ),
                    result.primary.effect_id,
                )

                seeds = (1, 2, 0x12345678, 0xFFFFFFFF)
                expected_primary_ids = tuple(
                    generate_rarity5_grace_effect_sequence(
                        seed,
                        playthrough=playthrough,
                        level=180,
                        grace_mapping=mapping,
                    ).primary.effect_id
                    for seed in seeds
                )
                self.assertEqual(
                    generate_rarity5_grace_primary_effect_ids(
                        seeds,
                        playthrough=playthrough,
                        grace_id=0x6553,
                        grace_mapping=mapping,
                    ),
                    expected_primary_ids,
                )


class Ng3Rarity3EffectSequenceTests(unittest.TestCase):
    VECTOR = bytes.fromhex(
        VECTORS["ng3_rarity3_seed_6096970_record_hex"]
    )

    def test_seed_6096970_matches_revealed_saved_record(self) -> None:
        expected = self.VECTOR
        seed = struct.unpack_from("<I", expected, 0x20)[0]
        level = struct.unpack_from("<H", expected, 0x06)[0]
        result = generate_ng3_rarity3_effect_sequence(seed, level=level)
        self.assertEqual(
            serialize_ng3_rarity3_effect_slots(result),
            expected[EFFECT_START : EFFECT_START + 7 * EFFECT_STRIDE],
        )
        rebuilt, _ = materialize_ng3_rarity3_record(
            expected,
            seed=seed,
            level=level,
            recommended_level=struct.unpack_from("<H", expected, 0x10)[0],
            transfer_count=struct.unpack_from("<I", expected, 0xDC)[0],
            generation_serial=struct.unpack_from("<I", expected, 0x28)[0],
        )
        self.assertEqual(rebuilt, expected)

    def test_promotion_shuffle_skips_fixed_growing_token(self) -> None:
        result = generate_ng3_rarity3_effect_sequence(74063692, level=180)
        self.assertEqual(result.promoted_source_indexes, (3,))
        self.assertEqual(
            tuple(effect.effect_id for effect in result.effects),
            (0xB383, 0xD495, 0x5CAC, 0x23E8, 0x0001),
        )
        self.assertEqual(result.effects[3].effect_flags, 0x04)

    def test_rarity34_primary_batch_matches_exact_sequences(self) -> None:
        seeds = (0, 1, 2, 74063692, 183696634, 0xFFFFFFFF)
        for rarity in (3, 4):
            expected = tuple(
                generate_ng3_certified_effect_sequence(
                    seed,
                    rarity=rarity,
                    level=180,
                ).primary.effect_id
                for seed in seeds
            )
            self.assertEqual(
                generate_ng3_rarity34_primary_effect_ids(
                    seeds,
                    rarity=rarity,
                ),
                expected,
            )

    def test_rarity3_terminal_token_is_not_a_secondary(self) -> None:
        result = generate_ng3_rarity3_effect_sequence(74063692, level=180)
        self.assertTrue(result.terminal_is_special)
        self.assertNotIn(result.effects[-1], result.secondaries)

    def test_rarity4_final_slots_are_all_normal_effects(self) -> None:
        result = generate_ng3_rarity4_final_effect_sequence(183696634, level=180)
        self.assertFalse(result.terminal_is_special)
        self.assertEqual(len(result.secondaries), len(result.effects) - 1)

    def test_certified_materializer_preserves_preview_for_rarity345(self) -> None:
        template = self.VECTOR
        for rarity in (3, 4, 5):
            rebuilt, preview = materialize_ng3_certified_record(
                template,
                seed=183696634,
                rarity=rarity,
                level=180,
                recommended_level=183,
                transfer_count=0,
                generation_serial=1,
            )
            self.assertEqual(rebuilt[0x30], rarity)
            self.assertEqual(rebuilt[0x31], rarity)
            self.assertEqual(
                tuple(effect.effect_id for effect in preview.effects),
                tuple(
                    int.from_bytes(
                        rebuilt[
                            EFFECT_START + index * EFFECT_STRIDE + 4 :
                            EFFECT_START + index * EFFECT_STRIDE + 8
                        ],
                        "little",
                    )
                    for index in range(len(preview.effects))
                ),
            )


if __name__ == "__main__":
    unittest.main()
