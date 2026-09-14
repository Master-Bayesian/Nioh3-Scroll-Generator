"""Run mandatory possessed native controls with an actual captured table profile."""
from __future__ import annotations
import argparse,json,sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[2]))
from nioh3_scroll_editor.possessed_generation import EnemyStateTables
from nioh3_scroll_editor.enemy_state_search import generate_enemy_state_preview

def verify(path=None):
    tables=EnemyStateTables.load(path);out=[]
    for seed,variant,expected in [(86872488,'solo',[]),(86872488,'expedition',[0xF3F]),(156062997,'solo',[0xF40])]:
        p=generate_enemy_state_preview(seed,3,variant=variant,state_tables=tables)
        actual=[x.native_spawn_key for x in p.occurrences if x.possessed=='yes']
        status='pass' if p.possessed_complete and actual==expected else 'missing_tables' if not p.possessed_complete else 'mismatch'
        out.append(dict(seed=seed,variant=variant,expected=expected,actual=actual,status=status,
                        missing=[x for x in p.missing_inputs if not x.startswith('Curse:')]))
    return dict(controls=out,all_required_possessed_controls_pass=all(x['status']=='pass' for x in out),
                scope='offline replay versus provided native fixtures; not a new game run')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--tables',type=Path);p.add_argument('--output',type=Path)
    a=p.parse_args();d=verify(a.tables);s=json.dumps(d,indent=2)+'\n'
    if a.output:a.output.write_text(s)
    print(s);raise SystemExit(0 if d['all_required_possessed_controls_pass'] else 2)
