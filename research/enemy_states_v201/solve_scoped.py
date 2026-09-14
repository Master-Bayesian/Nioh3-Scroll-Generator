"""Bounded inverse search of a DECLARED fixed-roster class; not a global solver."""
from __future__ import annotations
import argparse,json,sys,time
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[2]))
from nioh3_scroll_editor.enemy_state_search import solve_possessed_equivalence_class
from nioh3_scroll_editor.possessed_generation import EnemyStateTables

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('reference_seed',type=lambda s:int(s,0));p.add_argument('--playthrough',type=int,default=3)
    p.add_argument('--variant',choices=['solo','expedition'],required=True)
    p.add_argument('--target',nargs=2,type=int,required=True,metavar=('ZERO_BASED_WAVE','POSITION'))
    p.add_argument('--max-trials',type=int,required=True);p.add_argument('--start-after-trial',type=int,default=0)
    p.add_argument('--tables',type=Path);p.add_argument('--output',type=Path,required=True)
    p.add_argument('--scalar',action='store_true');a=p.parse_args();start=time.perf_counter()
    d=solve_possessed_equivalence_class(a.reference_seed,a.playthrough,variant=a.variant,target=tuple(a.target),
                max_trials=a.max_trials,start_after_trial=a.start_after_trial,
                state_tables=EnemyStateTables.load(a.tables),use_numpy=not a.scalar)
    d['elapsed_seconds']=time.perf_counter()-start
    d['empty_result_means']='no hit in this cursor budget and declared equivalence class; NOT global no-solution'
    s=json.dumps(d,indent=2)+'\n';a.output.write_text(s);print(s)
