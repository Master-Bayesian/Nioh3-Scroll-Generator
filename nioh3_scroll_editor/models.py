from __future__ import annotations

import struct
from dataclasses import dataclass
from enum import Enum
from typing import TYPE_CHECKING

from emaki_exchange import EFFECT_COUNT, EFFECT_START, EFFECT_STRIDE, SCROLL_RECORD_SIZE

from .catalog import R4_FINAL_GRACE_IDS, contextual_effect_name, effect_name

if TYPE_CHECKING:
    from .auxiliary_generation import CompleteAuxiliaryResult
    from .effect_sequence import EffectSequenceResult


EMPTY_EFFECT_ID = 0xFFFFFFFF


class CandidateRecordStage(str, Enum):
    FINAL_RECORD = "final_record"
    NATIVE_STAGE_ONE = "native_stage_one"
    EFFECT_SEQUENCE_ONLY = "effect_sequence_only"


@dataclass(frozen=True, slots=True)
class ScrollEffect:
    slot: int
    effect_id: int
    value: int
    metadata: int
    prefix: int
    tail_0: int
    tail_1: int
    # Exact native percentile roll recovered by the game-closed effect path.
    # It is present only for effect-sequence previews; it is not a serialized
    # replacement for the final record's value/metadata fields.
    roll_percent: int | None = None

    @property
    def name(self) -> str:
        return effect_name(self.effect_id)

    @property
    def is_empty(self) -> bool:
        return self.effect_id == EMPTY_EFFECT_ID


@dataclass(frozen=True, slots=True)
class ScrollCandidate:
    seed: int
    record: bytes
    effects: tuple[ScrollEffect, ...]
    rarity: int
    playthrough: int | None = None
    # Experimental research-only raw result-code prediction for rarity-3
    # records containing slot-5 0x0001.  This value is NOT stored in the R3
    # record.  It comes from a same-seed native R4 shadow record's slot-5 raw
    # output.  Despite the legacy field name, it is not assumed to be a Grace.
    predicted_growth_grace_id: int | None = None
    # Exact pivot-family cursor used by the Grace + primary joint solver.
    # It is search metadata only and is never serialized into the 0xE8 record.
    joint_search_trial: int | None = None
    record_stage: CandidateRecordStage = CandidateRecordStage.FINAL_RECORD
    # Offline-reconstructed enemies, terrain, and special rules. These fields
    # are deterministic from the displayed Seed and playthrough but are not
    # part of the compact online Emaki exchange tuple.
    auxiliary: CompleteAuxiliaryResult | None = None
    auxiliary_error: str | None = None

    @property
    def primary(self) -> ScrollEffect:
        return self.effects[0]

    @property
    def grace_slot_index(self) -> int | None:
        """Return the zero-based canonical Grace slot, when one survives."""

        if self.rarity == 5 and len(self.effects) >= 6:
            return 5
        if (
            self.rarity == 4
            and len(self.effects) >= 5
            and self.effects[4].effect_id in R4_FINAL_GRACE_IDS
            and ((self.effects[4].metadata >> 16) & 0x02)
        ):
            return 4
        return None

    @property
    def grace(self) -> ScrollEffect | None:
        index = self.grace_slot_index
        return self.effects[index] if index is not None else None

    @property
    def secondaries(self) -> tuple[ScrollEffect, ...]:
        """Return ordinary secondary effects only.

        Rarity-5 slot 6 is always Grace.  Rarity-4 final slot 5 is Grace when
        its stage-one Grace survives finalization, but is an ordinary secondary
        when the finalizer replaces that slot.  Stage-one special slots never
        satisfy an ordinary secondary requirement.
        """
        if (
            self.record_stage is CandidateRecordStage.NATIVE_STAGE_ONE
            and self.rarity in (3, 4)
        ):
            stop = 4  # slots 2..4
        elif self.grace_slot_index == 4:
            stop = 4  # slots 2..4; retained final Grace is physical slot 5
        elif self.rarity == 5:
            stop = 5  # slots 2..5; slot 6 is Grace
        else:
            stop = min(max(self.rarity + 1, 1), 5)
        return tuple(effect for effect in self.effects[1:stop] if not effect.is_empty)

    def display_name(self, effect: ScrollEffect) -> str:
        return contextual_effect_name(
            effect.effect_id,
            rarity=self.rarity,
            slot=effect.slot,
            native_stage_one=self.record_stage is CandidateRecordStage.NATIVE_STAGE_ONE,
        )

    @property
    def unresolved_effect_slots(self) -> tuple[int, ...]:
        if self.record_stage is not CandidateRecordStage.NATIVE_STAGE_ONE:
            return ()
        # Fail closed for every rarity-4 native result. The observed list is
        # incomplete by definition, and an unknown token is not evidence that
        # finalization is unnecessary.
        if self.rarity == 4:
            return (5,)
        return ()

    @property
    def can_materialize_for_install(self) -> bool:
        """Return whether a preview can be bound to a live save at install time."""

        return (
            self.record_stage is CandidateRecordStage.EFFECT_SEQUENCE_ONLY
            and self.playthrough == 3
            and self.rarity in (3, 4, 5)
        )

    @property
    def install_blocker(self) -> str | None:
        if self.record_stage is CandidateRecordStage.EFFECT_SEQUENCE_ONLY:
            if self.can_materialize_for_install:
                return None
            return (
                "当前候选只包含离线词条序列，而且该周目/稀有度尚未通过完整记录原生一致性门禁，"
                "暂不允许写入。"
            )
        if self.unresolved_effect_slots:
            return (
                "当前候选仍是原生中间态，包含尚未完成最终解析的结果码，拒绝写入。"
            )
        if (
            self.record_stage is CandidateRecordStage.NATIVE_STAGE_ONE
            and self.rarity < 4
        ):
            return "当前低稀有度原生候选尚未通过最终记录一致性验证，暂不允许写入。"
        return None

    @classmethod
    def from_record(
        cls,
        record: bytes,
        *,
        playthrough: int | None = None,
        predicted_growth_grace_id: int | None = None,
        record_stage: CandidateRecordStage = CandidateRecordStage.FINAL_RECORD,
    ) -> "ScrollCandidate":
        if len(record) != SCROLL_RECORD_SIZE:
            raise ValueError("record must be exactly 0xE8 bytes")
        if playthrough is not None and not 1 <= playthrough <= 5:
            raise ValueError("playthrough must be between 1 and 5, or None")
        effects: list[ScrollEffect] = []
        for index in range(EFFECT_COUNT):
            offset = EFFECT_START + index * EFFECT_STRIDE
            prefix, effect_id, value, metadata, tail_0, tail_1 = struct.unpack_from(
                "<6I", record, offset
            )
            effects.append(
                ScrollEffect(
                    slot=index + 1,
                    effect_id=effect_id,
                    value=value,
                    metadata=metadata,
                    prefix=prefix,
                    tail_0=tail_0,
                    tail_1=tail_1,
                )
            )
        return cls(
            seed=struct.unpack_from("<I", record, 0x20)[0],
            record=record,
            effects=tuple(effects),
            rarity=record[0x30],
            playthrough=playthrough,
            predicted_growth_grace_id=predicted_growth_grace_id,
            record_stage=record_stage,
        )

    @classmethod
    def from_effect_sequence(
        cls,
        result: "EffectSequenceResult",
        *,
        auxiliary: "CompleteAuxiliaryResult | None" = None,
        joint_search_trial: int | None = None,
    ) -> "ScrollCandidate":
        """Build a non-installable preview from exact offline effect replay."""

        effects = tuple(
            ScrollEffect(
                slot=effect.slot,
                effect_id=effect.effect_id,
                value=effect.resolved_value,
                metadata=(
                    effect.roll_percent
                    | (effect.category_and_flags << 8)
                    | (effect.effect_flags << 16)
                ),
                prefix=effect.prefix_word,
                tail_0=0,
                tail_1=0,
                roll_percent=effect.roll_percent,
            )
            for effect in result.effects
        )
        return cls(
            seed=result.seed,
            record=b"",
            effects=effects,
            rarity=result.rarity,
            playthrough=result.playthrough,
            joint_search_trial=joint_search_trial,
            record_stage=CandidateRecordStage.EFFECT_SEQUENCE_ONLY,
            auxiliary=auxiliary,
        )


def effective_required_secondary_ids(
    *,
    primary_id: int,
    primary_effect_ids: frozenset[int],
    required_secondary_ids: frozenset[int],
) -> frozenset[int]:
    """Return the secondary requirements after resolving cross-list pairing.

    The UI deliberately allows the same effect to be selected in both the
    primary candidate pool and the required-secondary pool.  When that exact
    effect becomes the candidate's actual primary, the primary slot satisfies
    that one duplicated requirement; the remaining selected effects must still
    appear in ordinary secondary slots.

    When the primary pool is unconstrained, the user's ordinary-effect
    selections mean "must appear in any ordinary slot".  In that mode the
    actual primary is allowed to satisfy one selected requirement as well.
    """
    if (
        primary_id in required_secondary_ids
        and (not primary_effect_ids or primary_id in primary_effect_ids)
    ):
        return required_secondary_ids - frozenset((primary_id,))
    return required_secondary_ids


def candidate_matches(
    candidate: ScrollCandidate,
    *,
    primary_effect_ids: frozenset[int],
    required_secondary_ids: frozenset[int],
    required_secondary_id_groups: tuple[frozenset[int], ...] = (),
) -> bool:
    primary_id = candidate.primary.effect_id
    if primary_effect_ids and primary_id not in primary_effect_ids:
        return False
    actual_secondaries = {effect.effect_id for effect in candidate.secondaries}
    effective_required = effective_required_secondary_ids(
        primary_id=primary_id,
        primary_effect_ids=primary_effect_ids,
        required_secondary_ids=required_secondary_ids,
    )
    if effective_required and not effective_required.issubset(actual_secondaries):
        return False
    group_slots = set(actual_secondaries)
    if not primary_effect_ids:
        group_slots.add(primary_id)
    return all(
        group.intersection(group_slots) for group in required_secondary_id_groups
    )


def candidate_has_expected_effect_count(candidate: ScrollCandidate, rarity: int) -> bool:
    """Return whether a native result has the expected resolved effect count."""
    expected = min(max(rarity + 1, 1), 6)
    return all(not effect.is_empty for effect in candidate.effects[:expected])
