"""Conditional Curse (+E9) selection, not descriptor Possessed (+8F).

A caller must supply late-selection inputs; this module does NOT manufacture
an MT seed, runtime bonus, or eligibility outcome from the displayed Seed.
`lottery_bounds` is explicitly an overapproximation of unobserved MT outputs,
not an assertion that every outcome is reachable from one particular Seed.
"""
from __future__ import annotations
from dataclasses import dataclass
from fractions import Fraction
import math
import struct
from .enemy_state_rng import MT19937, f32, cvtt_i32, A, MASK32


class UnsupportedCurseContext(ValueError):
    pass


def placement_seed(seed: int, mission_modifier: int) -> int:
    if type(seed) is not int or not 0 <= seed <= MASK32 or not 0 <= mission_modifier <= 255:
        raise ValueError('invalid Seed/mission-row+33')
    state = ((seed + mission_modifier) * A + 1) & MASK32
    return (state & 0xFFFF0000) | 1


def selection_probability(*, override_percent: float | None, object_kind: int,
                          config_ae3: bytes | None, config_8a46: bytes | None,
                          bonus_3b37_value: float | None) -> float:
    """DE3FE0 + E66872..E66901. Explicit None bonus = observed absent effect.

    Actual constants: override /100, bonus *0.001 (NOT *0.01).
    All config arguments represent observed lookup results, not missing files.
    """
    if override_percent is not None and override_percent >= 0:
        base = f32(f32(override_percent)/100.0)
    else:
        row = config_8a46 if override_percent is not None and object_kind == 2 else config_ae3
        if row is not None and len(row) != 0x20:
            raise ValueError('optional multiplier row must be 0x20 bytes')
        base = 0.0 if row is None else f32(f32(struct.unpack_from('<i',row,0x10)[0]) * struct.unpack_from('<f',row,0x18)[0])
    bonus = 0.0 if bonus_3b37_value is None else f32(f32(bonus_3b37_value)*f32(0.001))
    return f32(base+bonus)


def _count_parameters(n: int, p: float, already: int):
    p = f32(p)
    # Extreme/NaN values invoke additional uint64 and wrap semantics. Refuse
    # rather than call them a probability and silently clamp them.
    if type(n) is not int or not 0 <= n <= 4096 or not 0 <= already <= 4096:
        raise UnsupportedCurseContext('unsupported candidate/counter range')
    if not math.isfinite(p) or p < 0 or f32(f32(n)*p) >= 2147483648:
        raise UnsupportedCurseContext('count scalar must be finite, nonnegative and fit the native i32 product')
    base = cvtt_i32(f32(f32(n)*p))
    if p == 0:
        return base, None, 0
    reciprocal = f32(1.0/p)
    if reciprocal > 0x7FFFFFFF:
        raise UnsupportedCurseContext('reciprocal requires unported signed/uint64 range branch')
    q = math.trunc(reciprocal)
    remainder = (n-base*q) & 0xFFFFFFFFFFFFFFFF
    return base, q, remainder


def selection_count(n: int, p: float, already: int, mt: MT19937, *, has_bonus_3b37: bool) -> int:
    """Exact E3A900 on the guarded finite ordinary-input domain.

    Draws from [0,q] INCLUSIVE; even zero remainder consumes a draw if p>0.
    This helper is called only for a nonempty weighted pool by E3ADF0.
    """
    base, q, remainder = _count_parameters(n,p,already)
    if q is not None and remainder > mt.inclusive(q):
        base += 1
    base += int(has_bonus_3b37)
    return min(base,8-already)  # Can be negative. Caller checks <=0.


def count_support(n: int, p: float, already: int, *, has_bonus_3b37: bool) -> tuple[tuple[int,Fraction], ...]:
    """Support over one unconstrained integer-lottery output, not per-seed odds."""
    base,q,r = _count_parameters(n,p,already)
    success = Fraction(0) if q is None else Fraction(min(r,q+1),q+1)
    out = {}
    for inc,prob in [(0,1-success),(1,success)]:
        if prob:
            val=min(base+inc+int(has_bonus_3b37),8-already)
            out[val]=out.get(val,Fraction(0))+prob
    return tuple(sorted(out.items()))


@dataclass(frozen=True)
class CurseCandidate:
    occurrence: tuple[int,int]
    selector_class: int
    route: str  # direct / weighted / excluded / unknown
    weight: int | None  # None = known null DB row ONLY for direct

    def __post_init__(self):
        if self.selector_class not in (0,1) or self.route not in ('direct','weighted','excluded','unknown'):
            raise ValueError('invalid selector/route')
        if self.weight is not None and not 0 <= self.weight <= MASK32:
            raise ValueError('weight outside uint32')
        if self.route == 'weighted' and (self.weight is None or self.weight == 0):
            raise ValueError('weighted candidate must have positive observed u32 weight')


@dataclass(frozen=True)
class CurseReplay:
    selected_indices: dict[tuple[int,int],int]
    mt_draws: int
    mt_rejections: int
    trace: tuple[dict,...]
    evidence_grade: str = 'static_replay_given_explicit_late_context'


def replay_curse(candidates: tuple[CurseCandidate,...], *, placement: int,
                 probability: float, has_bonus_3b37: bool) -> CurseReplay:
    """One fresh E667A0 invocation, already-qualified records in manager order.

    Caller must establish the top-level gate, fresh flags, pointer-derived
    eligibility, and stable runtime p/bonus. Placement is the observed/predicted
    late-selector parameter, NOT the Possessed source-assignment seed.
    """
    if placement == 0:
        raise UnsupportedCurseContext('native selector does not run with zero placement')
    if len({x.occurrence for x in candidates}) != len(candidates):
        raise ValueError('duplicate occurrence identity')
    if any(x.route == 'unknown' for x in candidates):
        raise UnsupportedCurseContext('unresolved eligibility; cannot invent late ordering')
    mt = MT19937(placement); out={}; events=[]; assigned=0
    for selector in (0,1):
        pool=[]
        for x in candidates:
            if x.selector_class != selector or x.route == 'excluded':
                continue
            if x.route == 'direct':
                if x.weight == 0:
                    continue
                out[x.occurrence]=assigned & 255
                events.append(dict(kind='direct',selector=selector,occurrence=x.occurrence,index=assigned))
                assigned += 1
            else:
                pool.append(x)
        if not pool:
            continue
        before=mt.draws
        count=selection_count(len(pool),probability,assigned,mt,has_bonus_3b37=has_bonus_3b37)
        events.append(dict(kind='count',selector=selector,count=count,pool_size=len(pool),
                           assigned_before=assigned,mt_start=before,mt_end=mt.draws))
        for _ in range(max(0,count)):
            total=sum(x.weight for x in pool)
            if total>MASK32:
                raise UnsupportedCurseContext('u32 total weight overflow unsupported')
            before=mt.draws
            ticket=mt.inclusive(total-1 if total else MASK32)
            cumulative=0
            for i,x in enumerate(pool):
                cumulative+=x.weight
                if ticket<cumulative:
                    out[x.occurrence]=assigned & 255
                    assigned+=1
                    pool.pop(i)  # stable left removal, never swap-last
                    events.append(dict(kind='weighted',selector=selector,occurrence=x.occurrence,
                                       index=(assigned-1)&255,total=total,ticket=ticket,
                                       mt_start=before,mt_end=mt.draws))
                    break
    return CurseReplay(out,mt.draws,mt.rejections,tuple(events))


def lottery_bounds(candidates: tuple[CurseCandidate,...], *, probability: float,
                   has_bonus_3b37: bool) -> dict:
    """Sound bounds over unknown MT values; no fabricated per-occurrence odds.

    `possible` means not excluded by integer-lottery support. Cross-pass MT
    correlations and the 65,536 allowed placement initializations can narrow
    this set; they cannot invalidate a guaranteed/never bound from this model.
    """
    if len({x.occurrence for x in candidates}) != len(candidates):
        raise ValueError('duplicate occurrence identity')
    if any(x.route=='unknown' for x in candidates):
        return dict(status='unknown',by_occurrence={x.occurrence:'unknown' for x in candidates},
                    count_range=None,domain='missing_eligibility')
    possible_totals={0}; answer={x.occurrence:'never' for x in candidates}
    for selector in (0,1):
        direct=[x for x in candidates if x.selector_class==selector and x.route=='direct' and x.weight!=0]
        pool=[x for x in candidates if x.selector_class==selector and x.route=='weighted']
        if sum(x.weight for x in pool)>MASK32:
            raise UnsupportedCurseContext('u32 total weight overflow unsupported')
        for x in direct:answer[x.occurrence]='guaranteed'
        counts=set();next_totals=set()
        for prev in possible_totals:
            assigned=prev+len(direct)
            support=count_support(len(pool),probability,assigned,has_bonus_3b37=has_bonus_3b37) if pool else ((0,Fraction(1)),)
            for k,_ in support:
                selected=min(len(pool),max(0,k));counts.add(selected);next_totals.add(assigned+selected)
        for x in pool:
            answer[x.occurrence]='never' if max(counts)==0 else ('guaranteed' if min(counts)==len(pool) else 'possible')
        possible_totals=next_totals
    return dict(status='conditional_bounds',by_occurrence=answer,
                count_range=(min(possible_totals),max(possible_totals)),
                count_range_is_exact_for_displayed_seed=False,
                domain='one completed fresh selector invocation; unconstrained lottery support overapproximation')


def classify_null_source_record(raw_record: bytes, occurrence: tuple[int,int], *,
                                mission_key: int, playthrough: int,
                                database_lookup_observed: bool,
                                enemy_row_weight: int | None) -> CurseCandidate:
    """E3ADF0's fully recovered null-source branch from actual 0x158 records.

    None weight with observed=True means the native DB lookup returned null.
    Non-null source/actor references require their external predicate results;
    they remain unknown instead of being replaced with invented false values.
    """
    if len(raw_record)!=0x158:
        raise ValueError('mission record must be exactly 0x158 bytes')
    raw=raw_record;selector=raw[0x90]
    if selector not in (0,1):
        # Other selectors are never visited by these two passes.
        return CurseCandidate(occurrence,0,'excluded',0)
    spawn,mission,lookup=struct.unpack_from('<III',raw,0x20)
    if mission!=mission_key or struct.unpack_from('<I',raw,0x80)[0]==0:
        return CurseCandidate(occurrence,selector,'excluded',0)
    if struct.unpack_from('<Q',raw,0x18)[0]!=0:
        return CurseCandidate(occurrence,selector,'unknown',None)
    if not database_lookup_observed:
        return CurseCandidate(occurrence,selector,'unknown',None)
    if raw[0x8F] and playthrough>=3:
        return CurseCandidate(occurrence,selector,'direct',enemy_row_weight)
    if not 0xF3C<=spawn<=0xF96:
        return CurseCandidate(occurrence,selector,'excluded',0)
    if struct.unpack_from('<Q',raw)[0]!=0:
        return CurseCandidate(occurrence,selector,'unknown',None)
    if enemy_row_weight is None or enemy_row_weight==0:
        return CurseCandidate(occurrence,selector,'excluded',0)
    return CurseCandidate(occurrence,selector,'weighted',enemy_row_weight)
