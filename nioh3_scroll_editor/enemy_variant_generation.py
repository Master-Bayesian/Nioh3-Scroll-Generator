"""Additive PC v2.01 solo/expedition roster generation reference.

Port of the 0x1029E80 roster stage and 0x1027A10/0x1027FE0 helpers. This is
NOT an actor, persistent-task, or Curse oracle. Position data and source-flag
assignment are handled separately so a missing table cannot corrupt presence.
The pre-existing solo APIs and their native accelerator semantics are unchanged.
"""
from __future__ import annotations
from dataclasses import dataclass
from typing import Literal
import struct
import math
from . import auxiliary_generation as ag
from .enemy_state_rng import LcgStream, MT19937, f32, native_shuffle, lottery_10000

MissionVariant = Literal['solo', 'expedition']


@dataclass(frozen=True, slots=True)
class VariantOccurrence:
    wave_index: int
    position: int
    native_spawn_key: int
    lookup_key: int
    role: int
    source_row_index: int
    selector_class: int
    availability: str
    scratch_rule_key: int


@dataclass(frozen=True, slots=True)
class EnemyVariantResult:
    seed: int
    playthrough: int
    variant: MissionVariant
    auxiliary_mode: int
    terrain: int
    branch_class: int
    waves: tuple[tuple[VariantOccurrence, ...], ...]
    state_after_roster: int
    parent_draws: int
    trace: tuple[dict, ...]
    evidence_grade: str = 'static_replay'

    @property
    def occurrences(self) -> tuple[VariantOccurrence, ...]:
        return tuple(x for w in self.waves for x in w)


def _pool(rows, terrain, flags, selector, playthrough, gate):
    """0x102A160..0x102A3E0; selector-match jumps over the type gates."""
    out = []
    for i, row in enumerate(rows):
        if not row[0x16] & (1 << (playthrough-1)):
            continue
        matched_selector_branch = selector and row[0x1A] not in (4, 5)
        if matched_selector_branch:
            if row[0x19] != selector:
                continue
        else:
            if flags[0] and row[0x1B] and row[0x1B] != terrain[0x31]:
                continue
            if not ag._enemy_parameter_gate_accepts(row, descriptor_flag_22=flags[1], enemy_param_type_by_key=gate):
                continue
        if ag._enemy_terrain_gate_accepts(row, terrain):
            out.append(i)
    return out


def append_group_extras(general: list[int], selected: list[int], anchor: int,
                        count: int, rows: list[bytes], local: LcgStream) -> list[int]:
    """0x1027FE0. Row identity, not enemy name, controls exclusion."""
    if count <= 0:
        return []
    if count > 255 or not 0 <= anchor < len(rows):
        raise ValueError('invalid extra count/anchor')
    group = rows[anchor][0x18]
    if group == 0:
        choices = [anchor]  # no shuffle, no additional draw
    else:
        all_group = [i for i in general if rows[i][0x18] == group]
        if not all_group:
            return []
        choices = [i for i in all_group if i not in selected] or all_group
        seed = local.u16('extra-group-MT-seed', anchor=anchor, group=group)
        mt = MT19937(seed)
        native_shuffle(choices, mt)
        local.events.append(dict(stream=local.name+'.mt', reason='extra-group-shuffle',
                                 seed=seed, draws=mt.draws, rejected=mt.rejections,
                                 choice_rows=choices.copy()))
    return [choices[j % len(choices)] for j in range(count)]


def budget_and_extras(pool, general, selected, remaining, rows, parent,
                      wave, count, *, first_group=False, class0=False):
    """One parent draw seeds ALL local budget/anchor/shuffle work.

    Important: native advances local before discovering there is no affordable
    row. Omitting that failed local step changes the extra anchor choice.
    """
    local = LcgStream(parent.u16('budget-local-seed', wave=wave), f'budget[{wave}]', parent.events)
    prefer_max = first_group or class0
    while remaining > 0 and pool:
        ticket = local.u16('budget-attempt')
        eligible = [i for i in pool if ag._enemy_cost(rows[i]) <= remaining]
        if not eligible:
            local.events.append(dict(stream=local.name, reason='budget-no-affordable-row'))
            break
        if prefer_max:
            cost = max(ag._enemy_cost(rows[i]) for i in eligible)
            eligible = [i for i in eligible if ag._enemy_cost(rows[i]) == cost]
        index = eligible[ticket % len(eligible)]
        selected.append(index)
        local.events.append(dict(stream=local.name, reason='budget-selected', row=index))
        if first_group:
            break
        remaining = f32(remaining - ag._enemy_cost(rows[index]))
        prefer_max = False
    base_count = len(selected)
    anchors = [i for i in selected if rows[i][0x1A] not in (4, 5)]
    if count and anchors:
        anchor = anchors[local.u16('extra-anchor') % len(anchors)]
        extras = append_group_extras(general, selected, anchor, count, rows, local)
        # Native general contains the anchor's group. Empty would represent an
        # incoherent table/context; never label base entries as class 1 to hide it.
        if len(extras) != count:
            raise ag.AuxiliaryGenerationError('extra helper could not fulfil native anchor group')
        selected.extend(extras)
    return base_count


def generate_roster(seed: int, playthrough: int, auxiliary_mode: int,
                    terrain_row_index: int, *, variant: MissionVariant,
                    descriptor_selector: int = 0, descriptor_flags=(False, False, False),
                    tables=None, resource=None) -> EnemyVariantResult:
    """Native roster for an explicit, already-resolved generator context.

    No captured RNG state is accepted. Branch classes 0/1/2 are distinct from
    occurrence selector classes 0/1. Coordinates are deliberately not invented.
    """
    if variant not in ('solo', 'expedition'):
        raise ValueError('variant must be solo or expedition')
    if type(seed) is not int or not 0 <= seed <= 0xFFFFFFFF:
        raise ValueError('seed outside uint32')
    if not 1 <= playthrough <= 5 or len(descriptor_flags) != 3 or not 0 <= descriptor_selector <= 255:
        raise ValueError('unsupported progression or descriptor input')
    tables = tables or ag.load_default_auxiliary_generation_tables()
    if tables.enemy_candidates is None or tables.special_context is None or not tables.enemy_param_type_by_key:
        raise ag.AuxiliaryGenerationError('incomplete native roster tables')
    matches = [row for row in tables.special_context.rows() if row[0x28] == auxiliary_mode]
    if len(matches) != 1:
        raise ag.AuxiliaryGenerationError('context row must resolve uniquely')
    context = matches[0]
    branch = context[0x29]
    if branch not in (0, 1, 2):
        raise ag.AuxiliaryGenerationError('unknown enemy branch class')
    terrain_row = tables.terrain.row(terrain_row_index)
    terrain = tables.terrain_keys_by_row[terrain_row_index] & 255
    rows = list(tables.enemy_candidates.rows())
    if any(not math.isfinite(ag._enemy_cost(r)) or ag._enemy_cost(r)<=0 for r in rows):
        raise ag.AuxiliaryGenerationError("enemy costs must be positive finite native values")
    pool = _pool(rows, terrain_row, descriptor_flags, descriptor_selector,
                 playthrough, tables.enemy_param_type_by_key)
    general = [i for i in pool if rows[i][0x1A] not in (4, 5)]
    four = [i for i in pool if rows[i][0x1A] == 4]
    five = [i for i in pool if rows[i][0x1A] == 5]
    common = [i for i in general if rows[i][0x1A] in (0, 2)]
    special = [i for i in general if rows[i][0x1A] not in (0, 2)]
    budgets = list(struct.unpack_from('<5f', context, 4))
    n = next((j for j, b in enumerate(budgets) if b <= 0), 5)
    order = list(range(n)) if branch == 2 else list(range(n-1, -1, -1))
    parent = LcgStream(seed & 0x0FFFFFFF)
    result = [None] * n
    spawn = 0xF3C
    for w in order:
        remaining = f32(budgets[w])
        work = general.copy()
        selected: list[int] = []
        count = context[0x2A+w] if variant == 'expedition' else 0
        if branch in (1, 2):
            highest = branch == 1 and w == n-1
            if not highest and not descriptor_flags[2]:
                first = ag._select_enemy_by_u16_ticket(special, rows, remaining,
                            parent.u16('special-preselection', wave=w))
                if first is not None:
                    selected.append(first)
                    remaining = f32(remaining-ag._enemy_cost(rows[first]))
                second = ag._select_enemy_by_u16_ticket(common, rows, remaining,
                            parent.u16('common-pool-shaping', wave=w))
                if second is not None:
                    g = rows[second][0x18]
                    replacements = [second] + ([i for i in general if i != second and rows[i][0x18] == g] if g else [])
                    j = 0
                    for k, i in enumerate(work):
                        if rows[i][0x1A] in (0, 2):
                            work[k] = replacements[j % len(replacements)]; j += 1
            base_n = budget_and_extras(five if highest else work, general, selected,
                                      remaining, rows, parent, w, count, first_group=highest)
        else:
            if resource is None:
                resource = ag.load_default_r4_finalizer_resource()
            role5 = bool(five)
            if five and w != n-1:
                threshold = ag._optional_multiplier_threshold(resource.table('optional_multiplier'), 0xCEFC)
                role5 = lottery_10000(parent.u16('role4-role5', wave=w)) >= threshold
            work = five if role5 else four
            base_n = budget_and_extras(work, general, selected, remaining, rows,
                                      parent, w, count, class0=True)
            for i in selected:
                if i in work:
                    work.remove(i)
                group = rows[i][0x18]
                if group:
                    four[:] = [j for j in four if rows[j][0x18] != group]
                    five[:] = [j for j in five if rows[j][0x18] != group]
        occurrences = []
        for k, i in enumerate(selected):
            if spawn > 0xF96:
                raise ag.AuxiliaryGenerationError('native spawn cap reached; truncated profile not supported')
            occurrences.append(VariantOccurrence(w, k, spawn, ag._enemy_lookup_key(rows[i]),
                     rows[i][0x1A], i, int(k >= base_n), 'expedition_only' if k >= base_n else 'base',
                     struct.unpack_from('<H', rows[i], 0x12)[0]))
            spawn += 1
        result[w] = tuple(occurrences)
    return EnemyVariantResult(seed, playthrough, variant, auxiliary_mode, terrain, branch,
                              tuple(result), parent.state, parent.draws, tuple(parent.events))


def generate_enemy_variant(seed: int, playthrough: int, *, variant: MissionVariant,
                           tables=None, resource=None) -> EnemyVariantResult:
    tables = tables or ag.load_default_auxiliary_generation_tables()
    resource = resource or ag.load_default_r4_finalizer_resource()
    mode = ag.generate_auxiliary_mode(seed, resource=resource)
    terrain = ag.generate_terrain(seed, mode.value, tables=tables, resource=resource)
    desc = ag.generate_auxiliary_descriptor_flags(seed, mode.value, tables=tables, resource=resource)
    if desc.selector != 0:
        raise ag.AuxiliaryGenerationError("selector-nonzero branch needs independent parity; use explicit generate_roster for research")
    return generate_roster(seed, playthrough, mode.value, terrain.selected_row_index,
                           variant=variant, descriptor_selector=desc.selector, descriptor_flags=desc.flags,
                           tables=tables, resource=resource)
