"""Additive enemy preview, three-valued search, and inverse prefilters.

No UI/save edits; no full-domain permutation fallback. A supplied existing
inverse candidate stream retains its own coverage. Unknown != absence.
"""
from __future__ import annotations
from dataclasses import dataclass, asdict
from typing import Iterable, Iterator, Literal
from .enemy_variant_generation import generate_enemy_variant, EnemyVariantResult
from .possessed_generation import generate_possessed, EnemyStateTables
from . import auxiliary_generation as ag


@dataclass(frozen=True)
class EnemyStateOccurrence:
    wave_index: int
    position: int
    lookup_key: int
    role: int
    source_row_index: int
    availability: str
    native_spawn_key: int
    possessed: str
    curse: str = 'unknown'
    curse_probability: None = None
    # This is intentionally separate from unconditional product certainty.
    curse_if_fresh_null_source_selector_runs: str = 'unknown'
    evidence_grade: str = 'static_replay'
    curse_evidence: str = 'unknown'


@dataclass(frozen=True)
class EnemyStatePreview:
    seed: int
    playthrough: int
    variant: str
    terrain: int
    occurrences: tuple[EnemyStateOccurrence,...]
    possessed_complete: bool
    missing_inputs: tuple[str,...]
    curse_scope: str = 'late runtime context not captured; no Seed-only certainty claimed'
    curse_count_range: tuple[int, int] | None = None
    curse_count_domain: str | None = None

    def to_dict(self):
        return asdict(self)


def generate_enemy_state_preview(seed: int, playthrough: int, *, variant: str,
                                game_version: str = 'pc_v2_01', tables=None,
                                resource=None, state_tables: EnemyStateTables | None = None) -> EnemyStatePreview:
    if game_version != 'pc_v2_01':
        raise ValueError('unsupported game version; cannot reuse enemy-state semantics')
    roster=generate_enemy_variant(seed,playthrough,variant=variant,tables=tables,resource=resource)
    state_tables=state_tables or EnemyStateTables.load()
    possessed=generate_possessed(roster,tables=state_tables)
    out=[]
    for x in roster.occurrences:
        pos=possessed.by_occurrence[(x.wave_index,x.position)]
        gate=state_tables.eligibility_by_lookup.get(x.lookup_key)
        # E3AF3D..E3AF9B direct branch. This says NOTHING about the top-level
        # selector having run, existing actor flags, or the player's bonus.
        conditional='unknown'
        if playthrough>=3 and pos=='yes' and gate is not None:
            if gate.get('enemy_row_present') is False or gate.get('enemy_weight244',0)>0:
                conditional='guaranteed'
            elif 'enemy_weight244' in gate:
                conditional='never'
        out.append(EnemyStateOccurrence(x.wave_index,x.position,x.lookup_key,x.role,
                   x.source_row_index,x.availability,x.native_spawn_key,pos,
                   curse_if_fresh_null_source_selector_runs=conditional))
    missing=list(possessed.missing)
    missing.append('Curse: selector gates, resolved per-occurrence eligibility, probability/3B37 and placement source for the same invocation')
    return EnemyStatePreview(seed,playthrough,variant,roster.terrain,tuple(out),possessed.status=='exact',tuple(missing))


@dataclass(frozen=True)
class EnemyStateCriterion:
    lookup_key: int
    wave_index: int | None = None
    position: int | None = None
    possessed: bool | None = None
    curse: str | None = None  # guaranteed / possible / never
    min_occurrences: int = 1

    def __post_init__(self):
        if not 0<=self.lookup_key<=0xFFFFFFFF or self.min_occurrences<1:
            raise ValueError('invalid enemy constraint')
        if self.curse not in (None,'guaranteed','possible','never'):
            raise ValueError('invalid Curse certainty query')
        if self.wave_index is not None and self.wave_index<0 or self.position is not None and self.position<0:
            raise ValueError('negative occurrence location')
        if self.possessed is not None and type(self.possessed) is not bool:
            raise ValueError('possessed must be bool/None')

    def evaluate(self, result: EnemyStatePreview) -> str:
        yes=unknown=0
        for x in result.occurrences:
            if x.lookup_key!=self.lookup_key or self.wave_index is not None and x.wave_index!=self.wave_index or self.position is not None and x.position!=self.position:
                continue
            flags=[]
            if self.possessed is not None:
                flags.append('unknown' if x.possessed=='unknown' else 'match' if (x.possessed=='yes')==self.possessed else 'no_match')
            if self.curse is not None:
                # Upper-support membership is not a witnessed existential hit.
                if x.curse=='unknown' or (x.curse=='possible' and x.curse_evidence=='abstract_bound'):
                    flags.append('unknown')
                else:
                    flags.append('match' if (x.curse==self.curse or self.curse=='possible' and x.curse=='guaranteed') else 'no_match')
            if 'no_match' in flags:continue
            if 'unknown' in flags:unknown+=1
            else:yes+=1
        if yes>=self.min_occurrences:return 'match'
        if yes+unknown>=self.min_occurrences:return 'unknown'
        return 'no_match'


def base_anchor_prefilter(lookup_key: int, *, variant: str, tables=None) -> ag.AuxiliarySearchCriteria:
    """Necessary, NOT sufficient, base-roster OR-group for an enemy query.

    For a group-0 extra, the base anchor is that same row. For grouped extras,
    the base contains a general-role row of the same group. Include all such
    lookups to avoid losing expedition-only enemy hits. Final replay mandatory.
    This compiler applies to the supported selector-0 roster profile.
    """
    if variant not in ('solo','expedition'):
        raise ValueError('invalid variant')
    if not 0<=lookup_key<=0xFFFFFFFF:raise ValueError('lookup outside uint32')
    tables=tables or ag.load_default_auxiliary_generation_tables()
    if tables.enemy_candidates is None:raise ag.AuxiliaryGenerationError('enemy table missing')
    anchors={lookup_key}
    if variant=='expedition':
        rows=list(tables.enemy_candidates.rows())
        groups={r[0x18] for r in rows if ag._enemy_lookup_key(r)==lookup_key and r[0x18]}
        anchors.update(ag._enemy_lookup_key(r) for r in rows if r[0x1A] not in (4,5) and r[0x18] in groups)
    return ag.AuxiliarySearchCriteria(required_enemy_lookup_key_groups=(frozenset(anchors),))


def filter_inverse_seed_stream(seeds: Iterable[int], criteria: tuple[EnemyStateCriterion,...], *,
                               playthrough: int, variant: str, state_tables=None,
                               tables=None, resource=None) -> Iterator[tuple[int,str,EnemyStatePreview]]:
    """Exact replay gate on an existing inverse stream; yields unknown explicitly.

    It neither creates an implicit 2^28 scan nor claims all-domain completeness.
    The caller must preserve the original inverse cursor and deduplication.
    """
    for seed in seeds:
        result=generate_enemy_state_preview(seed,playthrough,variant=variant,state_tables=state_tables,tables=tables,resource=resource)
        verdicts=[c.evaluate(result) for c in criteria]
        status='no_match' if 'no_match' in verdicts else 'unknown' if 'unknown' in verdicts else 'match'
        yield seed,status,result


def enemy_occurrence_groups_status(
    result: EnemyStatePreview,
    groups: tuple[tuple[dict, ...], ...],
) -> str:
    """Evaluate mandatory groups of occurrence alternatives without hiding unknowns."""
    group_statuses = []
    for group in groups:
        alternatives = []
        for requirement in group:
            lookup_keys = frozenset(requirement['lookup_keys'])
            state = requirement.get('state', 'any')
            availability = requirement.get('availability', 'any')
            matched = False
            unknown = False
            for occurrence in result.occurrences:
                if occurrence.lookup_key not in lookup_keys:
                    continue
                if availability != 'any' and occurrence.availability != availability:
                    continue
                if state == 'any':
                    matched = True
                elif state == 'possessed':
                    matched = occurrence.possessed == 'yes'
                    unknown = unknown or occurrence.possessed == 'unknown'
                elif state == 'curse':
                    # Default Curse is deliberately not Seed-exact. Keep this
                    # explicit even if a conditional bound exists.
                    matched = occurrence.curse in ('guaranteed', 'possible')
                    unknown = unknown or occurrence.curse == 'unknown'
                else:
                    raise ValueError('invalid enemy occurrence state')
                if matched:
                    break
            alternatives.append('match' if matched else 'unknown' if unknown else 'no_match')
        group_statuses.append(
            'match' if 'match' in alternatives else
            'unknown' if 'unknown' in alternatives else 'no_match'
        )
    if 'no_match' in group_statuses:
        return 'no_match'
    if 'unknown' in group_statuses:
        return 'unknown'
    return 'match'


def compile_enemy_occurrence_prefilters(
    groups: tuple[tuple[dict, ...], ...],
    *,
    variant: str,
    tables=None,
) -> tuple[frozenset[int], ...]:
    """Compile necessary base-roster OR groups for exact final occurrence replay."""
    compiled = []
    for group in groups:
        anchors = set()
        for requirement in group:
            for lookup_key in requirement['lookup_keys']:
                prefilter = base_anchor_prefilter(lookup_key, variant=variant, tables=tables)
                anchors.update(prefilter.required_enemy_lookup_key_groups[0])
        if not anchors:
            raise ValueError('enemy occurrence group cannot be empty')
        compiled.append(frozenset(anchors))
    return tuple(compiled)


def compile_possessed_roster_constraints(roster: EnemyVariantResult, *, target: tuple[int,int],
                                        state_tables: EnemyStateTables | None=None):
    """Fixed-roster equivalence-class inversion, not an all-context solver.

    Returns native project's real DrawConstraints. After inversion, REPLAY and
    recheck signature, target and full query. Class membership is not encoded
    by these source-assignment constraints alone.
    """
    from .joint_solver import DrawConstraint,U16Runs
    from .enemy_state_rng import lottery_10000,threshold_from_config
    from .possessed_generation import position_parent_stream
    state_tables=state_tables or EnemyStateTables.load()
    stream=position_parent_stream(roster,state_tables)
    threshold=threshold_from_config(state_tables.config_4543)
    h=[i for i in range(65536) if lottery_10000(i)<=threshold]
    if not h:raise ValueError('source assignment success unreachable for config')
    success=U16Runs.from_values(h);fail=U16Runs.from_ranges([(h[-1]+1,65535)]) if h[-1]<65535 else None
    sequence=[x for cls in ((0,1) if roster.variant=='expedition' else (0,)) for x in roster.occurrences
              if x.selector_class==cls and state_tables.eligible(x.lookup_key)]
    at=next((i for i,x in enumerate(sequence) if (x.wave_index,x.position)==target),None)
    if at is None:raise ValueError('target absent/ineligible')
    if at and fail is None:raise ValueError('earlier eligible occurrence always succeeds')
    constraints=tuple(DrawConstraint(f'source_{i}',stream.draws+i+1,success if i==at else fail) for i in range(at+1))
    signature=_scope_signature(roster,state_tables)
    return signature,constraints


def apply_curse_context(preview: EnemyStatePreview, candidates, *,
                        selector_will_run: bool, candidate_universe_complete: bool,
                        probability: float, has_bonus_3b37: bool, placement: int | None=None) -> EnemyStatePreview:
    """Optional explicit-late-context extension. Never silently inferred from Seed.

    Exact when placement and all candidate inputs are given; otherwise sound
    lottery-support bounds. Generated-only records are insufficient if other
    mission records can consume the same index counter.
    """
    from dataclasses import replace
    from .curse_generation import lottery_bounds,replay_curse
    if not selector_will_run or not candidate_universe_complete:
        raise ValueError('complete native selection context required; leave preview unknown')
    occurrence_keys={(x.wave_index,x.position) for x in preview.occurrences}
    candidate_keys={c.occurrence for c in candidates}
    if not occurrence_keys.issubset(candidate_keys):
        raise ValueError('late context omits preview occurrences')
    if placement is None:
        bounds=lottery_bounds(tuple(candidates),probability=probability,has_bonus_3b37=has_bonus_3b37)
        statuses=bounds['by_occurrence']
        count_range=bounds['count_range']
        scope='explicit complete late context; conservative integer-lottery bounds, not fixed-Seed probabilities'
    else:
        replay=replay_curse(tuple(candidates),placement=placement,probability=probability,has_bonus_3b37=has_bonus_3b37)
        statuses={k:'guaranteed' if k in replay.selected_indices else 'never' for k in candidate_keys}
        count_range=(len(replay.selected_indices),len(replay.selected_indices))
        scope='exact given explicit complete late context including placement; not a Seed-only claim'
    return replace(preview,occurrences=tuple(replace(x,curse=statuses[x.wave_index,x.position],curse_evidence='abstract_bound' if placement is None else 'explicit_context_replay') for x in preview.occurrences),curse_scope=scope,curse_count_range=count_range,
                   curse_count_domain='entire explicitly supplied fresh selector candidate universe; not Seed-only')


def _scope_signature(roster, state_tables):
    import hashlib,json
    gate=[(x.lookup_key,state_tables.eligible(x.lookup_key)) for x in roster.occurrences]
    payload={'text':state_tables.text_sha256,'positions':[r.hex() for r in state_tables.positions_by_terrain[roster.terrain]],
             'config':state_tables.config_4543.hex() if state_tables.config_4543 is not None else None,'gates':gate}
    digest=hashlib.sha256(json.dumps(payload,sort_keys=True).encode()).hexdigest()
    return (roster.playthrough,roster.variant,roster.terrain,roster.branch_class,roster.parent_draws,
            tuple((x.lookup_key,x.role,x.selector_class,x.wave_index,x.position) for x in roster.occurrences),
            roster.auxiliary_mode,digest)


def collect_numpy_pivot(values, *, start_index, stop_index, low16_stride, draw_index):
    """Concrete optional CPU vectorized LCG preimage collector, not an oracle.

    Same resumable layout as existing joint_solver. No scanning outside the
    supplied high16 preimage. Missing numpy returns None for its Python path.
    """
    try:import numpy as np
    except ImportError:return None
    from .enemy_state_rng import affine
    a,c=affine(draw_index);inverse=pow(a,-1,1<<32)
    flat=np.arange(start_index,stop_index,dtype=np.uint64)
    low=flat//len(values);bucket=flat%len(values)
    high=np.asarray(values,dtype=np.uint64)[((low%len(values)+bucket)%len(values)).astype(np.intp)]
    x=(high<<np.uint64(16))|((low*np.uint64(low16_stride))&np.uint64(65535))
    seeds=(np.uint64(inverse)*(x-np.uint64(c)))&np.uint64(0xFFFFFFFF)
    keep=(seeds<np.uint64(0x10000000))&((seeds&np.uint64(65535))!=0)
    return tuple(zip(map(int,seeds[keep]),map(int,flat[keep]+1)))


def solve_possessed_equivalence_class(reference_seed: int, playthrough: int, *,
                                      variant: str, target: tuple[int,int],
                                      max_trials: int, start_after_trial: int=0,
                                      state_tables=None, tables=None, resource=None,
                                      use_numpy: bool=True) -> dict:
    """Real bounded inverse search inside a declared roster equivalence class.

    Cannot be used to claim all Seed solutions for arbitrary target enemy:
    other rosters/draw paths form other classes. A finite cursor budget is
    explicit, and exhaustion/completeness are reported without guessing.
    """
    from .joint_solver import choose_pivot,iter_constraint_intersection
    if type(max_trials) is not int or max_trials<=0 or type(start_after_trial) is not int or start_after_trial<0:
        raise ValueError('explicit positive trial budget and nonnegative cursor required')
    tables=tables or ag.load_default_auxiliary_generation_tables()
    resource=resource or ag.load_default_r4_finalizer_resource()
    state_tables=state_tables or EnemyStateTables.load()
    ref=generate_enemy_variant(reference_seed,playthrough,variant=variant,tables=tables,resource=resource)
    signature,constraints=compile_possessed_roster_constraints(ref,target=target,state_tables=state_tables)
    pivot=choose_pivot(constraints);total=pivot.allowed_u16.bucket_count*65536
    def collector(values,**kwargs):return collect_numpy_pivot(values,draw_index=pivot.draw_index,**kwargs)
    seeds=[];math_count=replayed=0
    for item in iter_constraint_intersection(constraints,natural_only=True,start_after_trial=start_after_trial,
            max_trials=max_trials,use_native_acceleration=use_numpy,
            pivot_seed_collector=collector if use_numpy else None,pivot_seed_collector_chunk_trials=100000):
        math_count+=1
        mode=ag.generate_auxiliary_mode(item.seed,resource=resource)
        if mode.value!=ref.auxiliary_mode:continue
        terrain=ag.generate_terrain(item.seed,mode.value,tables=tables,resource=resource)
        if terrain.value!=ref.terrain:continue
        desc=ag.generate_auxiliary_descriptor_flags(item.seed,mode.value,tables=tables,resource=resource)
        if desc.selector!=0:continue  # Outside supported equivalence-class domain.
        n=generate_enemy_variant(item.seed,playthrough,variant=variant,tables=tables,resource=resource)
        replayed+=1
        # Compare roster shape first; unrelated lookup rows may lack a captured
        # subtype gate, but are already outside the class and must not abort it.
        shape=(n.playthrough,n.variant,n.terrain,n.branch_class,n.parent_draws,
               tuple((x.lookup_key,x.role,x.selector_class,x.wave_index,x.position) for x in n.occurrences))
        if shape!=signature[:6]:continue
        if _scope_signature(n,state_tables)!=signature:continue
        p=generate_possessed(n,tables=state_tables)
        if p.status!='exact':raise ValueError('scope matched but state replay lost table coverage')
        if p.by_occurrence.get(target)=='yes':seeds.append(item.seed)
    cursor=min(total,start_after_trial+max_trials)
    return dict(seeds=sorted(set(seeds)),scope_signature=signature,target=target,
                pivot_total_trials=total,completed_trials=max(0,cursor-min(start_after_trial,total)),
                next_trial=cursor,exhaustive_over_equivalence_class=start_after_trial==0 and cursor==total,
                full_query_global_completeness=False,mathematical_candidates=math_count,
                full_roster_replays=replayed,algorithm='LCG-preimage -> native-table mode/terrain -> exact roster/source replay')
