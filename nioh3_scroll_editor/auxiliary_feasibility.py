"""Conservative structural feasibility checks for scroll enemy constraints.

The checks in this module never claim that a Seed exists. They only reject a
request when the recovered native branch/role layout makes it impossible before
any RNG draw is considered. A request reported as possible still requires exact
generation replay.
"""

from __future__ import annotations

from dataclasses import dataclass
import struct
from typing import Iterable

from .auxiliary_generation import (
    AuxiliaryGenerationTables,
    _rule_rows_conflict,
    _rule_weight,
    f32,
    f32_sub,
    load_default_auxiliary_generation_tables,
)


@dataclass(frozen=True, slots=True)
class EnemyKeyRequirement:
    label: str
    lookup_keys: frozenset[int]

    def __post_init__(self) -> None:
        if not self.lookup_keys:
            raise ValueError(f"enemy requirement {self.label!r} has no lookup keys")
        if any(not 0 <= key <= 0xFFFFFFFF for key in self.lookup_keys):
            raise ValueError("enemy lookup keys must fit in uint32")


@dataclass(frozen=True, slots=True)
class EnemyRequirementRoles:
    label: str
    lookup_keys: frozenset[int]
    candidate_keys: frozenset[int]
    roles: frozenset[int]


@dataclass(frozen=True, slots=True)
class EnemyFeasibilityReport:
    playthrough: int
    possible: bool
    viable_branch_classes: tuple[int, ...]
    requirements: tuple[EnemyRequirementRoles, ...]
    reasons: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class SpecialRuleKeyRequirement:
    label: str
    keys: frozenset[int]

    def __post_init__(self) -> None:
        if not self.keys:
            raise ValueError(f"special-rule requirement {self.label!r} has no keys")
        if any(not 0 <= key <= 0xFFFF for key in self.keys):
            raise ValueError("special-rule keys must fit in uint16")


@dataclass(frozen=True, slots=True)
class SpecialRuleFeasibilityReport:
    playthrough: int
    possible: bool
    requirements: tuple[SpecialRuleKeyRequirement, ...]
    unavailable_labels: tuple[str, ...]
    universally_conflicting_pairs: tuple[tuple[str, str], ...]
    witness_budget: int | None
    witness_keys: tuple[int, ...]
    failure_code: str | None


def _class1_roles_can_fit(role_sets: tuple[frozenset[int], ...]) -> bool:
    """Class 1 permits general roles plus at most one role-5 requirement."""

    states = {0}
    for roles in role_sets:
        choices = set()
        if any(role not in (4, 5) for role in roles):
            choices.add(0)
        if 5 in roles:
            choices.add(1)
        if not choices:
            return False
        states = {
            used_role_five + choice
            for used_role_five in states
            for choice in choices
            if used_role_five + choice <= 1
        }
        if not states:
            return False
    return bool(states)


def viable_enemy_branch_classes(
    role_sets: Iterable[Iterable[int]],
) -> tuple[int, ...]:
    """Return native branch classes that can fit all requested role choices.

    Each input item represents one player-visible enemy requirement and may
    contain multiple native roles when the localized display name resolves to
    more than one candidate row.  This is a structural preflight only.  A
    surviving class still requires exact generation replay for playthrough,
    terrain, parameter-gate, budget, linked-group, and RNG constraints.
    """

    normalized = tuple(frozenset(int(role) for role in roles) for roles in role_sets)
    if any(not roles for roles in normalized):
        return ()
    if any(any(role < 0 or role > 5 for role in roles) for roles in normalized):
        raise ValueError("enemy roles must be in 0..5")

    viable: list[int] = []
    dedicated_only = all(roles.intersection((4, 5)) for roles in normalized)
    role_four_only_count = sum(5 not in roles for roles in normalized)
    if (
        dedicated_only
        and len(normalized) <= 3
        and role_four_only_count <= 2
    ):
        viable.append(0)
    if _class1_roles_can_fit(normalized):
        viable.append(1)
    if all(roles.difference((4, 5)) for roles in normalized):
        viable.append(2)
    return tuple(viable)


def analyze_enemy_feasibility(
    requirements: Iterable[EnemyKeyRequirement],
    *,
    playthrough: int,
    tables: AuxiliaryGenerationTables | None = None,
) -> EnemyFeasibilityReport:
    """Reject enemy sets that no normal native branch can structurally emit."""

    if not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be in 1..5")
    requested = tuple(requirements)
    if tables is None:
        tables = load_default_auxiliary_generation_tables()
    if tables.enemy_candidates is None:
        raise ValueError("enemy candidate table is unavailable")

    candidate_roles_by_key: dict[int, set[int]] = {}
    playthrough_bit = 1 << (playthrough - 1)
    for row in tables.enemy_candidates.rows():
        if not (row[0x16] & playthrough_bit):
            continue
        key = struct.unpack_from("<I", row, 0x04)[0]
        candidate_roles_by_key.setdefault(key, set()).add(row[0x1A])

    resolved: list[EnemyRequirementRoles] = []
    reasons: list[str] = []
    for requirement in requested:
        candidate_keys = frozenset(
            key for key in requirement.lookup_keys if key in candidate_roles_by_key
        )
        roles = frozenset(
            role
            for key in candidate_keys
            for role in candidate_roles_by_key[key]
        )
        resolved.append(
            EnemyRequirementRoles(
                label=requirement.label,
                lookup_keys=requirement.lookup_keys,
                candidate_keys=candidate_keys,
                roles=roles,
            )
        )
        if not candidate_keys:
            reasons.append(
                f"{requirement.label}: no candidate row is enabled for playthrough {playthrough}"
            )

    if reasons:
        return EnemyFeasibilityReport(
            playthrough=playthrough,
            possible=False,
            viable_branch_classes=(),
            requirements=tuple(resolved),
            reasons=tuple(reasons),
        )

    role_sets = tuple(item.roles for item in resolved)
    viable = list(viable_enemy_branch_classes(role_sets))

    if not viable:
        role_summary = ", ".join(
            f"{item.label}=roles[{','.join(str(role) for role in sorted(item.roles))}]"
            for item in resolved
        )
        if all(item.roles.intersection((4, 5)) for item in resolved):
            if len(resolved) > 3:
                reasons.append(
                    "class 0 has at most three dedicated role-4/role-5 enemy "
                    "groups"
                )
            elif len(resolved) == 3 and all(5 not in item.roles for item in resolved):
                reasons.append(
                    "class 0 reserves its highest group for role 5, so three "
                    "role-4-only requirements cannot all fit"
                )
        reasons.append(
            "no normal enemy branch can emit all required role families together: "
            + role_summary
        )
    return EnemyFeasibilityReport(
        playthrough=playthrough,
        possible=bool(viable),
        viable_branch_classes=tuple(viable),
        requirements=tuple(resolved),
        reasons=tuple(reasons),
    )


def analyze_special_rule_feasibility(
    requirements: Iterable[SpecialRuleKeyRequirement],
    *,
    playthrough: int,
    tables: AuxiliaryGenerationTables | None = None,
) -> SpecialRuleFeasibilityReport:
    """Reject rule sets excluded by native slots, signs, budget, or conflicts.

    This explores every selectable ordering that could satisfy the requested
    key groups for any native starting budget from one through five. RNG ticket
    correlation is deliberately left to exact Seed replay; therefore a PASS is
    a structural possibility, while a failure is a deterministic impossibility.
    Enemy scratch-rule blocking is not assumed here, which keeps the rejection
    conservative when the requested enemy name has multiple native rows.
    """

    if not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be in 1..5")
    requested = tuple(requirements)
    if tables is None:
        tables = load_default_auxiliary_generation_tables()
    if tables.special_rules is None or tables.rule_conflicts is None:
        raise ValueError("special-rule tables are unavailable")
    if len(tables.special_rule_keys_by_row) != tables.special_rules.row_count:
        raise ValueError("special-rule key index does not match row count")
    if len(tables.rule_conflict_keys_by_row) != tables.rule_conflicts.row_count:
        raise ValueError("rule-conflict key index does not match row count")
    if not requested:
        return SpecialRuleFeasibilityReport(
            playthrough=playthrough,
            possible=True,
            requirements=(),
            unavailable_labels=(),
            universally_conflicting_pairs=(),
            witness_budget=1,
            witness_keys=(),
            failure_code=None,
        )

    indexed_rows = tuple(
        (index, key, row)
        for index, (key, row) in enumerate(
            zip(
                tables.special_rule_keys_by_row,
                tables.special_rules.rows(),
                strict=True,
            )
        )
        if row[0x36] & 0x01
    )
    selectable_rows = tuple(
        item for item in indexed_rows if _rule_weight(item[2], playthrough) > 0
    )
    selectable_keys = frozenset(key for _index, key, _row in selectable_rows)
    unavailable_labels = tuple(
        requirement.label
        for requirement in requested
        if not requirement.keys.intersection(selectable_keys)
    )
    if unavailable_labels:
        return SpecialRuleFeasibilityReport(
            playthrough=playthrough,
            possible=False,
            requirements=requested,
            unavailable_labels=unavailable_labels,
            universally_conflicting_pairs=(),
            witness_budget=None,
            witness_keys=(),
            failure_code="unavailable",
        )

    conflict_rows_by_key = dict(
        zip(
            tables.rule_conflict_keys_by_row,
            tables.rule_conflicts.rows(),
            strict=True,
        )
    )

    def rows_conflict(left: bytes, right: bytes) -> bool:
        return _rule_rows_conflict(
            left,
            right,
            conflict_rows_by_key=conflict_rows_by_key,
        ) or _rule_rows_conflict(
            right,
            left,
            conflict_rows_by_key=conflict_rows_by_key,
        )

    rows_by_key: dict[int, tuple[bytes, ...]] = {}
    for _index, key, row in selectable_rows:
        rows_by_key[key] = (*rows_by_key.get(key, ()), row)
    universally_conflicting_pairs: list[tuple[str, str]] = []
    for left_index, left in enumerate(requested):
        for right in requested[left_index + 1 :]:
            pairs = tuple(
                (left_row, right_row)
                for left_key in left.keys.intersection(selectable_keys)
                for right_key in right.keys.intersection(selectable_keys)
                for left_row in rows_by_key[left_key]
                for right_row in rows_by_key[right_key]
            )
            if pairs and all(rows_conflict(left_row, right_row) for left_row, right_row in pairs):
                universally_conflicting_pairs.append((left.label, right.label))
    if universally_conflicting_pairs:
        return SpecialRuleFeasibilityReport(
            playthrough=playthrough,
            possible=False,
            requirements=requested,
            unavailable_labels=(),
            universally_conflicting_pairs=tuple(universally_conflicting_pairs),
            witness_budget=None,
            witness_keys=(),
            failure_code="conflict",
        )

    def requirements_satisfied(selected_keys: tuple[int, ...]) -> bool:
        actual = frozenset(key for key in selected_keys if key)
        return all(requirement.keys.intersection(actual) for requirement in requested)

    def selectable_candidates(
        selected: tuple[tuple[int, int, bytes], ...],
        remaining: float,
        original_budget: float,
    ) -> tuple[tuple[int, int, bytes], ...]:
        accepted_count = len(selected)
        selected_keys = tuple(key for _index, key, _row in selected)
        zero_selected = 0 in selected_keys
        candidates: list[tuple[int, int, bytes]] = []
        for candidate in indexed_rows:
            _index, key, row = candidate
            if accepted_count:
                if key == 0:
                    if zero_selected:
                        continue
                else:
                    if key in selected_keys:
                        continue
                    if any(rows_conflict(row, previous[2]) for previous in selected):
                        continue
            cost = struct.unpack_from("<f", row, 0x14)[0]
            if accepted_count == 1:
                accumulated_delta = f32_sub(remaining, original_budget)
                if accumulated_delta < 0.0 and cost >= 0.0:
                    continue
                if accumulated_delta > 0.0 and cost <= 0.0:
                    continue
            elif accepted_count == 2:
                if remaining < 0.0 and cost > 0.0:
                    continue
                if remaining > 0.0 and cost < 0.0:
                    continue
                if f32(abs(cost)) > f32(abs(remaining)):
                    continue
            candidates.append(candidate)
        if accepted_count == 2 and candidates:
            best_abs = max(
                f32(abs(struct.unpack_from("<f", row, 0x14)[0]))
                for _index, _key, row in candidates
            )
            candidates = [
                candidate
                for candidate in candidates
                if f32(abs(struct.unpack_from("<f", candidate[2], 0x14)[0]))
                == best_abs
            ]
        return tuple(
            candidate
            for candidate in candidates
            if _rule_weight(candidate[2], playthrough) > 0
        )

    def search(
        selected: tuple[tuple[int, int, bytes], ...],
        remaining: float,
        original_budget: float,
    ) -> tuple[int, ...] | None:
        selected_keys = tuple(key for _index, key, _row in selected)
        if requirements_satisfied(selected_keys):
            return tuple(key for key in selected_keys if key)
        if len(selected) >= 3 or remaining == 0.0:
            return None
        candidates = selectable_candidates(selected, remaining, original_budget)
        if not candidates:
            return None
        actual = frozenset(key for key in selected_keys if key)
        unsatisfied = tuple(
            requirement
            for requirement in requested
            if not requirement.keys.intersection(actual)
        )
        target_keys = frozenset().union(*(requirement.keys for requirement in unsatisfied))
        slots_left = 3 - len(selected)
        choices = (
            tuple(candidate for candidate in candidates if candidate[1] in target_keys)
            if len(unsatisfied) >= slots_left
            else candidates
        )
        for candidate in choices:
            cost = struct.unpack_from("<f", candidate[2], 0x14)[0]
            witness = search(
                (*selected, candidate),
                f32_sub(remaining, cost),
                original_budget,
            )
            if witness is not None:
                return witness
        return None

    for budget in range(1, 6):
        original_budget = f32(float(budget))
        witness = search((), original_budget, original_budget)
        if witness is not None:
            return SpecialRuleFeasibilityReport(
                playthrough=playthrough,
                possible=True,
                requirements=requested,
                unavailable_labels=(),
                universally_conflicting_pairs=(),
                witness_budget=budget,
                witness_keys=witness,
                failure_code=None,
            )
    return SpecialRuleFeasibilityReport(
        playthrough=playthrough,
        possible=False,
        requirements=requested,
        unavailable_labels=(),
        universally_conflicting_pairs=(),
        witness_budget=None,
        witness_keys=(),
        failure_code="budget_or_order",
    )


__all__ = [
    "EnemyFeasibilityReport",
    "EnemyKeyRequirement",
    "EnemyRequirementRoles",
    "SpecialRuleFeasibilityReport",
    "SpecialRuleKeyRequirement",
    "analyze_enemy_feasibility",
    "analyze_special_rule_feasibility",
    "viable_enemy_branch_classes",
]
