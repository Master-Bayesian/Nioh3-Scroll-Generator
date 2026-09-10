"""Verify exported editor sets against the existing native-value implementation."""
import json
import random
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from nioh3_scroll_editor.effect_generation_tables import load_default_effect_generation_tables
raw=json.loads(Path(__file__).with_name('editor-values.json').read_text(encoding='utf-8'))
tables=load_default_effect_generation_tables()
randomizer=random.Random(20260909)
cases=[]
for effect_id in randomizer.sample(sorted(raw['effects']),100):
    for level in (0,1,90,179,180):
        for rarity in (3,4,5):
            row=tables.rarity_generation[rarity]
            expected=sorted({tables.resolved_effect_value(int(effect_id),roll_percent=roll,level=level)&0xFFFFFFFF for roll in range(row.minimum_roll_percent,row.maximum_roll_percent+1)})
            actual=raw['sets'][raw['patterns'][raw['effects'][effect_id]][level*3+rarity-3]]
            assert actual==expected,(effect_id,level,rarity)
            cases.append([effect_id,level,rarity])
output=Path('deliverables/frontend-v2/search-ui-demo-v2/editor-value-verification.json')
output.write_text(json.dumps({'verified_contexts':len(cases),'exported_effects':len(raw['effects']),'levels':[0,180],'rarities':[3,4,5],'seed':20260909},indent=2),encoding='utf-8')
print(f'{len(cases)} native raw-set comparisons passed')
