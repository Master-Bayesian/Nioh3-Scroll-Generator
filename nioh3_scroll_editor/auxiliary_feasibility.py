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
    viable: list[int] = []
    if all(any(role in (4, 5) for role in roles) for roles in role_sets):
        viable.append(0)
    if _class1_roles_can_fit(role_sets):
        viable.append(1)
    if all(any(role not in (4, 5) for role in roles) for roles in role_sets):
        viable.append(2)

    if not viable:
        role_summary = ", ".join(
            f"{item.label}=roles[{','.join(str(role) for role in sorted(item.roles))}]"
            for item in resolved
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


__all__ = [
    "EnemyFeasibilityReport",
    "EnemyKeyRequirement",
    "EnemyRequirementRoles",
    "analyze_enemy_feasibility",
]
