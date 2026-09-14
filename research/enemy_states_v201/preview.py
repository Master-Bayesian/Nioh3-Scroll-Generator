"""Preview explicit variants, or compile scoped fixed-roster inverse constraints."""
from __future__ import annotations
import argparse,json,sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[2]))
from nioh3_scroll_editor.enemy_state_search import generate_enemy_state_preview,compile_possessed_roster_constraints
from nioh3_scroll_editor.enemy_variant_generation import generate_enemy_variant
from nioh3_scroll_editor.possessed_generation import EnemyStateTables

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('seed',type=lambda x:int(x,0));p.add_argument('--playthrough',type=int,default=3)
    p.add_argument('--variant',choices=['solo','expedition','both'],default='both');p.add_argument('--tables',type=Path)
    p.add_argument('--target',nargs=2,type=int,metavar=('WAVE_ZERO_BASED','POSITION_ZERO_BASED'))
    a=p.parse_args();t=EnemyStateTables.load(a.tables);out=[]
    for v in ['solo','expedition'] if a.variant=='both' else [a.variant]:
        q=generate_enemy_state_preview(a.seed,a.playthrough,variant=v,state_tables=t).to_dict()
        if a.target:
            r=generate_enemy_variant(a.seed,a.playthrough,variant=v);sig,c=compile_possessed_roster_constraints(r,target=tuple(a.target),state_tables=t)
            q['scoped_inverse']={'scope_signature':sig,'must_replay_scope_and_full_query':True,'global_seed_completeness':False,
                                 'constraints':[{'name':x.name,'draw':x.draw_index,'high16_runs':x.allowed_u16.runs} for x in c]}
        out.append(q)
    print(json.dumps(out,ensure_ascii=False,indent=2))
