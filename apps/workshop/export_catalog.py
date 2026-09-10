"""Export local read-only catalogs and a bounded NG3 preview corpus; no GPU probe."""
import json
from collections import defaultdict
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from nioh3_scroll_editor.catalog import searchable_scroll_effect_definitions, native_effect_name, contextual_effect_name, native_effect_definitions, R4_FINAL_GRACE_IDS
from nioh3_scroll_editor.catalog_application import auxiliary_catalog, resolve_terrain_selections
from nioh3_scroll_editor.auxiliary_catalog import load_auxiliary_name_catalog
from nioh3_scroll_editor.enemy_variants import split_enemy_variant_display_groups
from nioh3_scroll_editor.grace_map import load_grace_output_map
from nioh3_scroll_editor.effect_sequence import generate_ng3_certified_effect_sequence, generate_challenge_attempt_count
from nioh3_scroll_editor.auxiliary_generation import generate_complete_auxiliary, load_default_auxiliary_generation_tables
from nioh3_scroll_editor.recommended_level import resolve_recommended_level
from nioh3_scroll_editor.version import APP_AUTHORS, CONTACT_QQ_GROUP, PROJECT_GITHUB_URL

from nioh3_scroll_editor.effect_generation_tables import load_default_effect_generation_tables
tables = load_default_effect_generation_tables()

names = load_auxiliary_name_catalog("zh-CN")
catalog = auxiliary_catalog(3, "zh-CN")
roles = {e["lookup_key"]: e["role"] for e in catalog["enemy_options"]}
groups = split_enemy_variant_display_groups({name: keys.intersection(roles) for name, keys in names.enemy_key_groups().items() if keys.intersection(roles)})
enemies = []
for name, keys in groups.items():
    native_roles = {roles[key] for key in keys}
    tier = "低手" if native_roles <= {0,1,2,3} else "中手" if native_roles == {4} else "高手" if native_roles == {5} else "中／高手"
    enemies.append({"id": str(min(keys)), "name": name, "keys": sorted(keys), "tier": tier})

def rule_value(v):
    value = v["display_value"]
    if value is None:
        return v["display_grade"] or "固定变体"
    return f'{value:g}' + ("%" if v["display_unit"] == "percent" else " 秒" if v["display_unit"] == "seconds" else "")

rule_entries = {e["key"]: e for e in catalog["special_rule_options"]}
families = []
for family in catalog["special_rule_families"]:
    keys = [k for k in family["keys"] if k]
    if not keys:
        continue
    name = family["name"]
    base = "造成的属性伤害增加" if name.startswith("造成的") and "属性伤害增加" in name else name.split("（")[0]
    families.append({"id":str(min(keys)),"name":name,"category":base,"keys":keys,
        "variants":[{"key":k,"label":rule_value(rule_entries[k]["variant"])} for k in keys]})

contexts = {}
for ng in range(1,6):
    for rarity in (3,4,5):
        effects = [{"id":str(e.effect_id),"name":e.name} for e in searchable_scroll_effect_definitions(ng,rarity)]
        grace_ids = []
        if (rarity == 4 and ng <= 3) or (rarity == 5 and ng >= 3):
            mapping = load_grace_output_map(rarity=rarity)
            grace_ids = sorted(R4_FINAL_GRACE_IDS) if ng in (1, 2) and rarity == 4 else sorted({r.grace_id for r in mapping.ranges if rarity != 4 or r.grace_id in R4_FINAL_GRACE_IDS})
        contexts[f'{ng}-{rarity}'] = {"effects":effects,"graces":[{"id":str(i),"name":native_effect_name(i,"zh-CN") or f'0x{i:X}'} for i in grace_ids]}

samples=[]
seeds=[10030565,43723117,36526331]+list(range(12000,12197))
for rarity in (3,4,5):
    for seed in seeds:
        sequence=generate_ng3_certified_effect_sequence(seed,rarity=rarity,level=180)
        aux=generate_complete_auxiliary(seed,3)
        enemy_keys=[entry.lookup_key for group in aux.enemies.groups for entry in group.entries]
        grace_ids={g['id'] for g in contexts[f'3-{rarity}']['graces']}
        effects=[]
        for effect in sequence.effects:
            if effect.effect_id == 0xFFFFFFFF:
                continue
            effect_id=str(effect.effect_id)
            effects.append({"id":effect_id,"name":contextual_effect_name(effect.effect_id,rarity=rarity,slot=effect.slot),"roll":effect.roll_percent,"raw":effect.resolved_value,
                "role":"主词条" if effect.slot==1 else "恩宠" if effect_id in grace_ids and effect.slot==len(sequence.effects) else "成长词条" if rarity==3 and effect.slot==5 else "副词条"})
        samples.append({"seed":str(seed),"rarity":rarity,"effects":effects,"capacity":generate_challenge_attempt_count(seed),"enemyKeys":enemy_keys,"enemySlotKeys":[group.entries[0].lookup_key for group in aux.enemies.groups],
            "enemies":list(dict.fromkeys(names.enemy_name(k) for k in enemy_keys)),"terrainKeys":list(aux.terrain.display_effect_keys),
            "rules":[{"key":e.key,"name":names.special_rule_name(e.key),"value":rule_value({"display_value":e.display_value,"display_unit":e.display_unit,"display_grade":e.display_grade})} for e in aux.special_rules.entries]})
levels={str(level):resolve_recommended_level(level).selected_internal_level for level in range(142,701)}
identities={str(key):{"prefix":row.group_key,"category":tables.group_for_effect(key).category_key} for key,row in tables.effects_by_id.items()}
aux_tables=load_default_auxiliary_generation_tables()
runtime_terrains={t["option_id"]:aux_tables.terrain.row(min(resolve_terrain_selections([t["option_id"]])))[0x30] for t in catalog["terrain_options"] if not t["aggregate"]}
output={"runtimeTerrains":runtime_terrains,"editorIdentities":identities,"enemyRoles":roles,"editorEffects":[{"id":str(e.effect_id),"name":e.name} for e in native_effect_definitions()],"contexts":contexts,"enemies":sorted(enemies,key=lambda e:e['name']),"rules":families,"terrains":catalog["terrain_options"],"samples":samples,"levels":levels,
    "authors":list(APP_AUTHORS),"qq":CONTACT_QQ_GROUP,"github":PROJECT_GITHUB_URL,"qqInvite":None}
(Path(__file__).parent / "catalog.json").write_text(json.dumps(output,ensure_ascii=False,separators=(',',':')),encoding='utf-8')
print(f'Exported {len(enemies)} enemy identities, {len(families)} rule families, {len(samples)} exact offline previews')
