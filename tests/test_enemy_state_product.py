from dataclasses import replace
from fractions import Fraction
import json
from pathlib import Path
import struct
import pytest
from nioh3_scroll_editor import auxiliary_generation as ag
from nioh3_scroll_editor.enemy_state_rng import MT19937,LcgStream,native_shuffle,state_after,lottery_10000,f32
from nioh3_scroll_editor.enemy_variant_generation import generate_enemy_variant,generate_roster,append_group_extras,budget_and_extras
from nioh3_scroll_editor.possessed_generation import EnemyStateTables,generate_possessed,position_parent_stream
from nioh3_scroll_editor.curse_generation import (selection_count,count_support,selection_probability,CurseCandidate,
    replay_curse,lottery_bounds,placement_seed,UnsupportedCurseContext)
from nioh3_scroll_editor.enemy_state_search import (generate_enemy_state_preview,EnemyStateCriterion,
    filter_inverse_seed_stream,base_anchor_prefilter,compile_possessed_roster_constraints,
    enemy_occurrence_groups_status,compile_enemy_occurrence_prefilters)

FIX=json.loads((Path(__file__).parent/'fixtures/enemy_states_v201/native_controls.json').read_text())

@pytest.fixture(scope='module')
def deps():return ag.load_default_auxiliary_generation_tables(),ag.load_default_r4_finalizer_resource()


def test_enemy_state_preview_is_a_declared_auxiliary_public_export(deps):
    assert "generate_enemy_state_preview" in ag.__all__
    tables, resource = deps
    preview = ag.generate_enemy_state_preview(
        86872488,
        3,
        variant="solo",
        tables=tables,
        resource=resource,
    )
    assert preview.variant == "solo"

@pytest.mark.parametrize('control',FIX['rosters'],ids=['expedition_native','solo_native'])
def test_roster_raw_native_vector(control,deps):
    t,r=deps;n=generate_enemy_variant(control['seed'],3,variant=control['variant'],tables=t,resource=r)
    assert len(n.waves)==len(control['waves'])
    for expected,actual in zip(control['waves'],n.waves):
        assert len(expected)==len(actual)
        assert [(x['spawn'],x['lookup'],x['role'],x['selector']) for x in expected]==[(x.native_spawn_key,x.lookup_key,x.role,x.selector_class) for x in actual]


def test_156_solo_roster_matches_native_record_identities_not_just_count(deps):
    t,r=deps;n=generate_enemy_variant(156062997,3,variant='solo',tables=t,resource=r)
    ctl=next(c for c in FIX['late_controls'] if c['seed']==156062997)
    native={x['spawn']:(x['lookup'],int.from_bytes(bytes.fromhex(x['raw_hex'])[8:12],'little')) for x in ctl['records']}
    assert native=={x.native_spawn_key:(x.lookup_key,x.role) for x in n.occurrences}
    assert [x.native_spawn_key for x in n.waves[0]]==[0xF40,0xF41]


@pytest.mark.parametrize('variant',['solo','expedition'])
def test_seed_to_possessed_868_control_no_captured_state_input(variant,deps):
    t,r=deps;n=generate_enemy_variant(86872488,3,variant=variant,tables=t,resource=r)
    p=generate_possessed(n)
    assert p.status=='exact'
    assert p.source_entry_draw==35
    assert p.source_entry_state==FIX['run_d_source_entry_state']==state_after(86872488,35)
    control=next(x for x in FIX['rosters'] if x['variant']==variant)
    for wi,w in enumerate(control['waves']):
        for pi,x in enumerate(w):assert p.by_occurrence[wi,pi]==('yes' if x['flag'] else 'no')
    if variant=='expedition':
        trials=[e for e in p.trace if e['reason']=='source-first-success']
        assert len(trials)==7
        assert [(e['spawn'],e['ticket'],e['after'],e['accepted']) for e in trials]==[(e['spawn'],e['ticket'],e['state'],e['accepted']) for e in FIX['run_d_trials']]


def test_required_156_possessed_vector_passes_with_complete_native_tables(deps):
    t,r=deps;p=generate_enemy_state_preview(156062997,3,variant='solo',tables=t,resource=r)
    assert p.possessed_complete
    assert not [e for e in p.missing_inputs if not e.startswith('Curse:')]
    c=next(c for c in FIX['late_controls'] if c['seed']==156062997)
    expected=[x['spawn'] for x in c['records'] if x['flag8f']]
    assert expected==[0xF40]
    assert [x.native_spawn_key for x in p.occurrences if x.possessed=='yes']==expected


def test_both_modes_advance_same_parent_roster_not_local_extra_stream(deps):
    t,r=deps;a=generate_enemy_variant(86872488,3,variant='solo',tables=t,resource=r)
    b=generate_enemy_variant(86872488,3,variant='expedition',tables=t,resource=r)
    assert a.state_after_roster==b.state_after_roster==state_after(86872488,12)
    assert a.parent_draws==b.parent_draws==12
    assert len(a.occurrences)==6 and len(b.occurrences)==10
    pos=position_parent_stream(b,EnemyStateTables.load())
    assert sum(e['reason']=='position-pool-local-MT-seed' for e in pos.events)==23
    assert [sum(e['reason']=='position-pool-local-MT-seed' and e['wave']==w for e in pos.events) for w in range(4)]==[5,6,6,6]


def test_existing_solo_behaviour_516_deterministic_seeds(deps):
    t,r=deps;seen=set();different=[]
    for seed in [1,5,86872488,156062997]+[((i*2654435761)&0x0fffffff) or 1 for i in range(1,513)]:
        b=ag.generate_complete_auxiliary(seed,3,tables=t,resource=r);seen.add(b.mode.branch_class)
        if b.descriptor.selector:
            with pytest.raises(ag.AuxiliaryGenerationError,match='selector-nonzero'):
                generate_enemy_variant(seed,3,variant='solo',tables=t,resource=r)
            different.append(seed);continue
        n=generate_enemy_variant(seed,3,variant='solo',tables=t,resource=r)
        assert [[x.row_index for x in g.entries] for g in b.enemies.groups]==[[x.source_row_index for x in g] for g in n.waves]
        assert b.enemies.random_draws==n.parent_draws
    assert seen=={0,1,2}
    assert different==[220174487,100499291,248425816,235086056,198497377,257501240,43900972,162742433]  # Conservative guard for all selector-nonzero profiles.


def row(lookup=1,group=0,role=1,cost=1):
    b=bytearray(28);struct.pack_into('<I',b,4,lookup);struct.pack_into('<f',b,12,cost);b[24]=group;b[26]=role
    return bytes(b)


def test_extra_same_name_different_row_identity():
    rows=[row(100,7),row(100,7),row(200,7),row(300,8)]
    local=LcgStream(5)
    extra=append_group_extras([0,1,2,3],[0],0,5,rows,local)
    assert set(extra)=={1,2} and 0 not in extra and 3 not in extra
    assert len(extra)==5 and local.draws==1


def test_extra_no_group_repeats_anchor_without_new_draw():
    local=LcgStream(5);assert append_group_extras([0],[0],0,3,[row()],local)==[0,0,0]
    assert local.draws==0


def test_extras_all_group_used_falls_back_to_all():
    local=LcgStream(5);x=append_group_extras([0,1],[0,1],0,3,[row(1,7),row(2,7)],local)
    assert len(x)==3 and set(x)=={0,1}


def test_failed_local_affordability_attempt_still_advances_before_anchor():
    rows=[row(1,0,cost=3)];p=LcgStream(5);s=[]
    budget_and_extras([0],[0],s,4,rows,p,0,1)
    e=[x for x in p.events if x['reason']=='budget-attempt']
    assert len(e)==2 and p.draws==1 and s==[0,0]
    assert next(x for x in p.events if x['reason']=='extra-anchor')['draw']==3


def test_no_anchor_cannot_relabel_base_as_extra():
    rows=[row(role=5)];p=LcgStream(1);s=[]
    b=budget_and_extras([0],[],s,1,rows,p,0,2,first_group=True)
    assert b==len(s)==1


def test_unobserved_enemy_is_not_native_null_lookup():
    partial=Path(__file__).resolve().parents[1]/'nioh3_scroll_editor/data/enemy_states/pc_v2_01/observed_subset.json'
    t=EnemyStateTables.load(partial)
    with pytest.raises(ValueError,match='not captured'):t.eligible(0xDEADBEEF)
    assert replace(t,enemy_index_complete=True).eligible(0xDEADBEEF)
    assert not t.eligible(0x4388)  # Native subtype lookup missing, not subtype flag guess.


def test_default_enemy_state_profile_is_complete_native_capture():
    t=EnemyStateTables.load()
    assert t.enemy_index_complete
    assert 0x8E in t.positions_by_terrain
    assert 'per-Seed result lookup' in t.source_note


def test_f32_lottery_boundary_not_integer_shortcut():
    assert lottery_10000(29039)==4431
    assert 29039*10000//65536==4430


def test_mt_reference_known_sequence_and_shuffle():
    mt=MT19937(5489)
    assert [mt.u32() for _ in range(5)]==[3499211612,581869302,3890346734,3586334585,545404204]
    values=list(range(10));mt=MT19937(5489);native_shuffle(values,mt)
    assert sorted(values)==list(range(10)) and mt.draws>=9


def test_mt_zero_range_consumes_nothing_and_rejection_branch(monkeypatch):
    mt=MT19937(1);assert mt.inclusive(0)==0 and mt.draws==0
    it=iter([0xFFFFFFFF,12]);monkeypatch.setattr(mt,'u32',lambda:next(it))
    assert mt.inclusive(9)==2 and mt.rejections==1


@pytest.mark.parametrize('n,p,a,b,expected',[(10,0,0,False,{0}),(10,1,0,False,{8}),
 (3,.5,0,False,{1,2}),(3,.5,7,True,{1}),(6,1,9,False,{-1}),(9,.03,0,False,{0,1})])
def test_count_support(n,p,a,b,expected):
    assert {x for x,_ in count_support(n,p,a,has_bonus_3b37=b)}==expected
    assert sum(prob for _,prob in count_support(n,p,a,has_bonus_3b37=b))==1


def test_count_is_not_binomial_or_usual_fractional_rounding():
    assert dict(count_support(3,.5,0,has_bonus_3b37=False))=={1:Fraction(2,3),2:Fraction(1,3)}
    mt=MT19937(1);selection_count(6,1,0,mt,has_bonus_3b37=False);assert mt.draws==1


@pytest.mark.parametrize('p',[float('nan'),float('inf'),-1,1e20,1e-30])
def test_unknown_probability_domain_rejected(p):
    with pytest.raises(UnsupportedCurseContext):selection_count(2,p,0,MT19937(1),has_bonus_3b37=False)


def test_probability_uses_correct_two_float_scales():
    p=selection_probability(override_percent=50,object_kind=0,config_ae3=None,config_8a46=None,bonus_3b37_value=50)
    assert p==f32(.5+f32(50*f32(.001)))
    assert p!=1.0


def test_placement_prefix_is_not_possessed_source_rng():
    assert placement_seed(86872488,3)==0x078E0001
    assert placement_seed(86872488,3)!=state_after(86872488,35)


def candidates():
    return tuple([CurseCandidate((0,i),0,'weighted',w) for i,w in enumerate((200,200,100,200,100,200))]+
                 [CurseCandidate((1,0),1,'direct',200)]+[CurseCandidate((1,i),1,'weighted',200) for i in (1,2,3)])


def test_conditional_curse_bounds_shared_counter_and_no_fake_odds():
    b=lottery_bounds(candidates(),probability=1,has_bonus_3b37=False)
    # class0 selects 6; class1 direct increments to7; only1 of3 can follow.
    assert b['count_range']==(8,8)
    assert all(b['by_occurrence'][0,i]=='guaranteed' for i in range(6))
    assert b['by_occurrence'][1,0]=='guaranteed'
    assert all(b['by_occurrence'][1,i]=='possible' for i in (1,2,3))
    assert not b['count_range_is_exact_for_displayed_seed']


def test_weighted_replay_varies_when_explicit_late_input_changes():
    a=replay_curse(candidates(),placement=0x078E0001,probability=1,has_bonus_3b37=False)
    b=replay_curse(candidates(),placement=0x078E0001,probability=0,has_bonus_3b37=False)
    assert len(a.selected_indices)==8 and len(b.selected_indices)==1
    assert a==replay_curse(candidates(),placement=0x078E0001,probability=1,has_bonus_3b37=False)
    # No claim these synthetic p inputs are the actual two live repeats.


def test_every_actual_mt_replay_obeys_abstract_bounds():
    for p in [0,.03,.2,.5,1]:
        c=candidates();bound=lottery_bounds(c,probability=p,has_bonus_3b37=False)
        for h in range(48):
            out=replay_curse(c,placement=(h<<16)|1,probability=p,has_bonus_3b37=False)
            for k,status in bound['by_occurrence'].items():
                if status=='guaranteed':assert k in out.selected_indices
                if status=='never':assert k not in out.selected_indices
            assert bound['count_range'][0]<=len(out.selected_indices)<=bound['count_range'][1]


def test_unknown_gate_does_not_become_never():
    c=(CurseCandidate((0,0),0,'unknown',None),)
    assert lottery_bounds(c,probability=1,has_bonus_3b37=False)['status']=='unknown'
    with pytest.raises(UnsupportedCurseContext):replay_curse(c,placement=1,probability=1,has_bonus_3b37=False)


def test_possessed_and_curse_must_match_same_occurrence(deps):
    t,r=deps;p=generate_enemy_state_preview(86872488,3,variant='expedition',tables=t,resource=r)
    assert EnemyStateCriterion(0xDCB98,possessed=True).evaluate(p)=='match'
    assert EnemyStateCriterion(0xDCB98,wave_index=3,possessed=True).evaluate(p)=='no_match'
    assert EnemyStateCriterion(0xDCB98,possessed=True,curse='guaranteed').evaluate(p)=='unknown'
    assert EnemyStateCriterion(0xDCB98,min_occurrences=4).evaluate(p)=='match'
    assert EnemyStateCriterion(0xDCB98,min_occurrences=5).evaluate(p)=='no_match'
    assert sum(x.curse_if_fresh_null_source_selector_runs=='guaranteed' for x in p.occurrences)==1
    assert all(x.curse=='unknown' for x in p.occurrences)


def test_product_occurrence_groups_keep_variant_state_and_unknown_separate(deps):
    t,r=deps
    expedition=generate_enemy_state_preview(86872488,3,variant='expedition',tables=t,resource=r)
    possessed=(({'lookup_keys':[0xDCB98],'state':'possessed','availability':'expedition_only'},),)
    curse=(({'lookup_keys':[0xDCB98],'state':'curse','availability':'any'},),)
    solo=generate_enemy_state_preview(86872488,3,variant='solo',tables=t,resource=r)
    assert enemy_occurrence_groups_status(expedition,possessed)=='match'
    assert enemy_occurrence_groups_status(solo,possessed)=='no_match'
    assert enemy_occurrence_groups_status(expedition,curse)=='unknown'
    anchors=compile_enemy_occurrence_prefilters(possessed,variant='expedition',tables=t)
    assert len(anchors)==1 and 0xDCB98 in anchors[0]


def test_inverse_filter_preserves_unknown_and_no_implicit_seed_scan(deps):
    t,r=deps;c=(EnemyStateCriterion(0xDCB98,possessed=True),)
    out=list(filter_inverse_seed_stream([86872488],c,playthrough=3,variant='expedition',tables=t,resource=r))
    assert len(out)==1 and out[0][1]=='match'
    c=(EnemyStateCriterion(0x8BC34,possessed=True),)
    assert list(filter_inverse_seed_stream([156062997],c,playthrough=3,variant='solo',tables=t,resource=r))[0][1]=='match'
    assert list(filter_inverse_seed_stream([],c,playthrough=3,variant='solo',tables=t,resource=r))==[]


def test_fixed_roster_inverse_constraints_recover_native_winner(deps):
    t,r=deps;n=generate_enemy_variant(86872488,3,variant='expedition',tables=t,resource=r)
    signature,constraints=compile_possessed_roster_constraints(n,target=(1,1))
    assert [c.draw_index for c in constraints]==list(range(36,43))
    assert all(c.matches(86872488) for c in constraints)
    assert signature[0:3]==(3,'expedition',0xD4)
    with pytest.raises(ValueError,match='ineligible'):compile_possessed_roster_constraints(n,target=(2,1))


def test_anchor_inverse_prefilter_is_necessary_not_solo_only(deps):
    t,r=deps
    tested=0
    for seed in [86872488]+[(i*747796405)&0x0FFFFFFF for i in range(1,65)]:
        try:n=generate_enemy_variant(seed,3,variant='expedition',tables=t,resource=r)
        except ag.AuxiliaryGenerationError as e:
            assert 'selector-nonzero' in str(e);continue
        baseline=ag.generate_complete_auxiliary(seed,3,tables=t,resource=r)
        for key in {x.lookup_key for x in n.occurrences}:
            c=base_anchor_prefilter(key,variant='expedition',tables=t)
            assert c.matches_enemies(baseline.enemies);tested+=1
    assert tested>100


@pytest.mark.parametrize('kw',[{'variant':'co-op'}, {'variant':'solo','game_version':'pc_v2_02'}])
def test_unsupported_input_rejected(kw):
    with pytest.raises(ValueError):generate_enemy_state_preview(1,3,**kw)


def test_count_above_one_and_exhausted_pool_keep_rng_consumption():
    c=(CurseCandidate((0,0),0,'weighted',200),)
    r=replay_curse(c,placement=1,probability=2,has_bonus_3b37=False)
    assert len(r.selected_indices)==1 and r.mt_draws==3
    # q=0 means the count helper draws no MT; one weighted draw and
    # two full-range draws after the pool is exhausted, exactly as native.


def make_task(flag=0,actor=0,source=0):
    b=bytearray(0x158);struct.pack_into('<Q',b,0,actor);struct.pack_into('<Q',b,0x18,source)
    struct.pack_into('<III',b,0x20,0xF3C,1234,100);struct.pack_into('<I',b,0x80,0xF3C)
    b[0x8F]=flag;return bytes(b)


@pytest.mark.parametrize('flag,observed,weight,actor,source,route',[
    (1,True,None,0,0,'direct'),(1,True,0,0,0,'direct'),
    (0,True,None,0,0,'excluded'),(0,True,0,0,0,'excluded'),
    (0,True,100,0,0,'weighted'),(1,False,None,0,0,'unknown'),
    (0,True,100,123,0,'unknown'),(1,True,100,0,123,'unknown')])
def test_native_null_source_classification(flag,observed,weight,actor,source,route):
    from nioh3_scroll_editor.curse_generation import classify_null_source_record
    c=classify_null_source_record(make_task(flag,actor,source),(0,0),mission_key=1234,playthrough=3,
                                 database_lookup_observed=observed,enemy_row_weight=weight)
    assert c.route==route


def test_curse_context_integration_is_explicit_and_filters_work(deps):
    from nioh3_scroll_editor.enemy_state_search import apply_curse_context
    t,r=deps;p=generate_enemy_state_preview(86872488,3,variant='expedition',tables=t,resource=r)
    c=tuple(CurseCandidate((x.wave_index,x.position),int(x.availability=='expedition_only'),
                          'direct' if x.possessed=='yes' else 'weighted',200) for x in p.occurrences)
    q=apply_curse_context(p,c,selector_will_run=True,candidate_universe_complete=True,
                         probability=1,has_bonus_3b37=False)
    assert EnemyStateCriterion(0xDCB98,possessed=True,curse='guaranteed').evaluate(q)=='match'
    assert EnemyStateCriterion(0xDCB98,wave_index=3,position=2,curse='possible').evaluate(q)=='unknown'
    assert all(x.curse_probability is None for x in q.occurrences)
    assert 'not fixed-Seed' in q.curse_scope
    with pytest.raises(ValueError,match='complete native'):
        apply_curse_context(p,c,selector_will_run=True,candidate_universe_complete=False,probability=1,has_bonus_3b37=False)

@pytest.mark.parametrize('draw,stride,start,stop',[(1,1,0,2000),(42,0x9E37,3100,4800),(3,65535,50000,55000)])
def test_numpy_preimage_collector_exactly_matches_scalar_cursor(draw,stride,start,stop):
    pytest.importorskip('numpy')
    from nioh3_scroll_editor.enemy_state_search import collect_numpy_pivot
    from nioh3_scroll_editor.enemy_state_rng import affine
    values=(0,65535,19,3283,2170,40000);a,c=affine(draw);inv=pow(a,-1,1<<32)
    expected=[]
    for i in range(start,stop):
        low_index,b=divmod(i,len(values));h=values[(low_index%len(values)+b)%len(values)]
        state=(h<<16)|((low_index*stride)&65535)
        seed=(inv*(state-c))&0xFFFFFFFF
        if seed<0x10000000 and seed&65535:expected.append((seed,i+1))
    assert collect_numpy_pivot(values,start_index=start,stop_index=stop,low16_stride=stride,draw_index=draw)==tuple(expected)


def reference_cursor(deps):
    from nioh3_scroll_editor.joint_solver import choose_pivot,permuted_pivot_values
    t,r=deps;n=generate_enemy_variant(86872488,3,variant='expedition',tables=t,resource=r)
    _,cs=compile_possessed_roster_constraints(n,target=(1,1));pivot=choose_pivot(cs);vs=permuted_pivot_values(pivot.allowed_u16)
    state=state_after(86872488,pivot.draw_index);low=((state&65535)*pow(0x9E37,-1,65536))&65535
    bucket=(vs.index(state>>16)-low%len(vs))%len(vs)
    return low*len(vs)+bucket


def test_scoped_inverse_actually_recovers_control_by_modular_cursor(deps):
    from nioh3_scroll_editor.enemy_state_search import solve_possessed_equivalence_class
    t,r=deps;i=reference_cursor(deps)
    kw=dict(variant='expedition',target=(1,1),max_trials=101,start_after_trial=i-50,tables=t,resource=r)
    scalar=solve_possessed_equivalence_class(86872488,3,use_numpy=False,**kw)
    vector=solve_possessed_equivalence_class(86872488,3,use_numpy=True,**kw)
    assert vector==scalar
    assert 86872488 in scalar['seeds'] and scalar['completed_trials']==101
    assert not scalar['full_query_global_completeness'] and not scalar['exhaustive_over_equivalence_class']


def test_scoped_inverse_resume_does_not_replay_or_lose_cursor(deps):
    from nioh3_scroll_editor.enemy_state_search import solve_possessed_equivalence_class
    t,r=deps;i=reference_cursor(deps)
    kw=dict(variant='expedition',target=(1,1),tables=t,resource=r,use_numpy=False)
    a=solve_possessed_equivalence_class(86872488,3,max_trials=50,start_after_trial=i-50,**kw)
    b=solve_possessed_equivalence_class(86872488,3,max_trials=51,start_after_trial=a['next_trial'],**kw)
    whole=solve_possessed_equivalence_class(86872488,3,max_trials=101,start_after_trial=i-50,**kw)
    assert a['next_trial']==i and b['next_trial']==whole['next_trial']
    assert sorted(set(a['seeds']+b['seeds']))==whole['seeds']
    assert a['mathematical_candidates']+b['mathematical_candidates']==whole['mathematical_candidates']


def test_inverse_scope_binds_table_content_not_only_version_name(deps):
    t,r=deps;n=generate_enemy_variant(86872488,3,variant='expedition',tables=t,resource=r)
    tab=EnemyStateTables.load();before,_=compile_possessed_roster_constraints(n,target=(1,1),state_tables=tab)
    b=bytearray(tab.config_4543);struct.pack_into('<i',b,16,499)
    after,_=compile_possessed_roster_constraints(n,target=(1,1),state_tables=replace(tab,config_4543=bytes(b)))
    assert before!=after


@pytest.mark.parametrize('query',['guaranteed','possible','never'])
def test_abstract_possible_is_not_a_proof_of_any_exact_curse_predicate(query,deps):
    from nioh3_scroll_editor.enemy_state_search import apply_curse_context
    t,r=deps;p=generate_enemy_state_preview(86872488,3,variant='expedition',tables=t,resource=r)
    c=tuple(CurseCandidate((x.wave_index,x.position),int(x.availability=='expedition_only'),
             'direct' if x.possessed=='yes' else 'weighted',200) for x in p.occurrences)
    q=apply_curse_context(p,c,selector_will_run=True,candidate_universe_complete=True,
                         probability=1,has_bonus_3b37=False)
    assert q.curse_count_range==(8,8)
    assert EnemyStateCriterion(0xDCB98,wave_index=3,position=2,curse=query).evaluate(q)=='unknown'


def test_explicit_curse_replay_exposes_exact_count_but_not_seed_only(deps):
    from nioh3_scroll_editor.enemy_state_search import apply_curse_context
    t,r=deps;p=generate_enemy_state_preview(86872488,3,variant='expedition',tables=t,resource=r)
    c=tuple(CurseCandidate((x.wave_index,x.position),int(x.availability=='expedition_only'),
             'direct' if x.possessed=='yes' else 'weighted',200) for x in p.occurrences)
    q=apply_curse_context(p,c,selector_will_run=True,candidate_universe_complete=True,
                         probability=1,has_bonus_3b37=False,placement=0x078E0001)
    assert q.curse_count_range==(8,8)
    assert sum(x.curse=='guaranteed' for x in q.occurrences)==8
    assert 'not Seed-only' in q.curse_count_domain


def test_curse_bounds_refuse_unported_weight_overflow_like_replay():
    c=(CurseCandidate((0,0),0,'weighted',0xFFFFFFFF),CurseCandidate((0,1),0,'weighted',1))
    with pytest.raises(UnsupportedCurseContext,match='overflow'):
        lottery_bounds(c,probability=1,has_bonus_3b37=False)
    with pytest.raises(UnsupportedCurseContext,match='overflow'):
        replay_curse(c,placement=1,probability=1,has_bonus_3b37=False)
