"""Integration with the existing frontier collector. Real Lua, mock CE; not live."""
from tests.test_materialization_frontier import fixture,arm,prepare,close,complete,validate


def test_real_frontier_expansion_capture_is_valid_evidence_and_static_counterexample():
    l=fixture(6);arm(l);l.run('enter()');prepare(l,10);l.run('pre()')
    for i in range(10):l.run(f'lookup({i});link({i})')
    data,cleanup=close(l);r=validate(data,cleanup)
    assert r['observation_validated']
    inv=r['invocations'][0]
    assert inv['before_descriptor_count']==6 and inv['after_descriptor_count']==10
    assert inv['static_pre_iteration_contract']['status']=='contradicted'
    assert not inv['static_pre_iteration_contract']['known_helper_augmentation_proved']
    assert not r['physical_actor_join'] and not r['whole_manager_state_known']


def test_existing_matching_six_records_check_only_local_boundary():
    l=fixture(6);arm(l);complete(l,6,reuse=True);data,cleanup=close(l)
    r=validate(data,cleanup);inv=r['invocations'][0]
    assert inv['static_pre_iteration_contract']['status']=='consistent'
    assert inv['all_post_descriptors_linked'] and inv['linked_records']==6
    assert inv['reuse_mismatches']==[]
    assert r['unobserved_generator_excluded'] is False


def test_missing_post_boundary_is_not_a_successful_no_augmentation_proof():
    l=fixture();arm(l);l.run('enter()');data,cleanup=close(l)
    inv=validate(data,cleanup)['invocations'][0]
    assert inv['after_descriptor_count'] is None
    assert inv['static_pre_iteration_contract']['status']=='not_observed'
