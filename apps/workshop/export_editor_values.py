"""Export exact native raw sets for the editor without reimplementing numeric rules."""
import json
import sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from nioh3_scroll_editor.effect_generation_tables import load_default_effect_generation_tables
from nioh3_scroll_editor.catalog import native_effect_definitions

def export():
    tables = load_default_effect_generation_tables()
    sets, set_ids, patterns, pattern_ids, effects = [], {}, [], {}, {}
    for definition in native_effect_definitions():
        effect_id = definition.effect_id
        if effect_id not in tables.effects_by_id:
            continue
        try:
            tables.resolved_effect_value(effect_id, roll_percent=100, level=180)
        except (KeyError, ValueError):
            continue
        pattern = []
        for level in range(181):
            values = [tables.resolved_effect_value(effect_id, roll_percent=roll, level=level) & 0xFFFFFFFF for roll in range(60, 101)]
            for start in (0, 20, 30):
                native = tuple(sorted(set(values[start:])))
                if native not in set_ids:
                    set_ids[native] = len(sets)
                    sets.append(native)
                pattern.append(set_ids[native])
        key = tuple(pattern)
        if key not in pattern_ids:
            pattern_ids[key] = len(patterns)
            patterns.append(pattern)
        effects[str(effect_id)] = pattern_ids[key]
    path = Path(__file__).with_name('editor-values.json')
    path.write_text(json.dumps({'effects': effects, 'patterns': patterns, 'sets': sets}, separators=(',', ':')), encoding='utf-8')
    print(f'Exported exact raw sets: {len(effects)} effects, {len(patterns)} patterns, {len(sets)} sets')
if __name__ == '__main__':
    export()
