import React, {useEffect, useMemo, useRef, useState} from "react";
import type {EquipmentRules, EquipmentSeeds} from "../../packages/contracts/protected-responses";
import {EMPTY_EFFECT, EffectPicker} from "./EffectPicker";
import {excludes, seedOptions, type SeedCandidate, type WantedEffect} from "./equipment-seed-options";
import {Notice} from "./Notice";
import {errorText} from "./public-errors";
import {runtimeObserver, saveObserver} from "./save-workspace";

/** Search is read-only and save-scoped. Only the subsequent native preview is authoritative. */
export function LiveEquipmentSeedSearch({item, level, plus, rarity, seed, saveId, source, locked, wanted, onWanted, onSeed, onBusy, effectName}: {
  item: number | null; level: number | null; plus: number | null; rarity: number | null; seed: string;
  saveId: string | null; source: string; locked: boolean; wanted: WantedEffect[];
  onWanted: (wanted: WantedEffect[]) => void; onSeed: (seed: string) => void;
  onBusy: (busy: boolean) => void; effectName: (id: number) => string;
}) {
  const [rules, setRules] = useState<EquipmentRules | null>(null);
  const [message, setMessage] = useState("");
  const [searching, setSearching] = useState(false);
  const [search, setSearch] = useState<{identity: string; result: EquipmentSeeds} | null>(null);
  const running = useRef(false), mounted = useRef(false);
  const callbacks = useRef({onBusy, onWanted, onSeed}); callbacks.current = {onBusy, onWanted, onSeed};
  const rulesIdentity = JSON.stringify([item, level, rarity]);
  const identity = JSON.stringify([rulesIdentity, plus, saveId, source, wanted]);
  const current = useRef(identity); current.current = identity;
  useEffect(() => {
    mounted.current = true;
    return () => {mounted.current = false; if (running.current) callbacks.current.onBusy(false);};
  }, []);
  useEffect(() => {
    let active = true;
    setRules(null); setMessage(""); callbacks.current.onWanted([]);
    if (item !== null && level !== null && rarity !== null) {
      void window.operations.execute({method: "runtime.equipment_rules", params: {item_id: item, level, rarity, hell: false}})
        .then(reply => {
          if (!active) return;
          if (!reply || !("known" in reply)) throw Error("EQUIPMENT_RULES_EXPECTED");
          setRules(reply as EquipmentRules);
        }).catch(error => {if (active) setMessage(errorText(error));});
    }
    return () => {active = false;};
  }, [rulesIdentity]);
  useEffect(() => {setSearch(null); setMessage("");}, [identity]);
  const options = useMemo(() => seedOptions(rules, rarity, effectName), [rules, rarity, effectName]);
  const candidates = [...options.entries()].filter(([key, option]) =>
    !wanted.some(entry => entry.key === key) &&
    !wanted.some(entry => {const other = options.get(entry.key); return other && excludes(option.candidate, other.candidate);}),
  ).map(([, option]) => option.candidate);
  const label = (candidate: SeedCandidate) => (candidate.star ? "✦ " : "") + effectName(candidate.id) +
    (candidate.min == null || candidate.max == null ? "" : "（" + candidate.min + "–" + candidate.max + "）");
  function pick(candidate: {id: number}) {
    if (locked || candidate.id === EMPTY_EFFECT || wanted.length >= 7) return;
    for (const [key, option] of options) {
      if (option.candidate.id === candidate.id && candidates.includes(option.candidate)) {
        onWanted([...wanted, {key, ids: option.ids}]); return;
      }
    }
  }
  async function find() {
    if (locked || running.current || !saveId || item === null || level === null || plus === null || rarity === null || !rules?.known) return;
    const requested = identity;
    running.current = true; setSearching(true); onBusy(true); setSearch(null); setMessage("");
    try {
      // Re-read the selected save on every search; do not substitute its state for the live character's.
      const read = () => window.operations.execute({method: "save.character", params: {save_id: saveId}});
      const saved = saveObserver ? await saveObserver.run(read) : await read();
      if (!mounted.current || current.current !== requested) return;
      if (!saved || !("save_id" in saved) || saved.save_id !== saveId || !("generation" in saved)) throw Error("无法读取目标存档的难度和进度，请重新读取后再查找。");
      const state = saved.generation?.difficulties.find(entry => entry.difficulty === saved.generation?.difficulty);
      if (!state) throw Error("无法读取目标存档的难度和进度，请重新读取后再查找。");
      const submit = () => window.operations.execute({method: "runtime.equipment_seeds", params: {
        item_id: item, level, plus, rarity, difficulty: state.difficulty, progress: state.progress,
        want: wanted.map(entry => entry.ids) as never, limit: 40,
      }});
      const reply = runtimeObserver ? await runtimeObserver.run(submit) : await submit();
      if (!mounted.current || current.current !== requested) return;
      if (!reply || !("outcomes" in reply) || !("matches" in reply)) throw Error("EQUIPMENT_SEEDS_EXPECTED");
      const result = reply as EquipmentSeeds;
      setSearch({identity: requested, result});
      if (result.outcomes[0]) callbacks.current.onSeed(String(result.outcomes[0].seed));
    } catch (error) {
      if (mounted.current && current.current === requested) setMessage(errorText(error));
    } finally {
      running.current = false;
      if (mounted.current) {setSearching(false); callbacks.current.onBusy(false);}
    }
  }
  const result = search?.identity === identity ? search.result : null;
  return <fieldset className="seed-panel live-equipment-seed-search" disabled={locked}>
    <legend>按词条查找种子</legend>
    <p className="equipment-notes">搜索使用目标存档的难度和进度，不包含敌人或地区掉落加成；实际词条以游戏生成的预览为准。</p>
    {!saveId && <p className="equipment-notes">请先选择当前角色的目标存档，再按词条查找。</p>}
    {rules && !rules.known && <p className="equipment-notes">这件物品暂不支持按词条查找，可手动填写种子后生成实时预览。</p>}
    <div className="seed-want">
      <span className="seed-want-label">想要的词条（可不选，最多 7 个）</span>
      <div className="seed-want-chips">
        {wanted.map(entry => <span key={entry.key} className="seed-chip">
          {label(options.get(entry.key)?.candidate ?? {id: entry.ids[0]})}
          <button type="button" aria-label="移除" onClick={() => onWanted(wanted.filter(other => other.key !== entry.key))}>×</button>
        </span>)}
        {rules?.known && wanted.length < 7 && <EffectPicker value="" candidates={candidates} label={label} onPick={pick} placeholder="点这里添加想要的词条"/>}
      </div>
    </div>
    <button type="button" data-action="find-equipment-seeds" onClick={() => void find()} disabled={locked || !saveId || !rules?.known || plus === null}>{searching ? "查找中…" : "查找"}</button>
    <Notice text={message}/>
    {result && <>
      <p className="seed-summary"><span>目标存档难度</span> {result.difficulty} · <span>匹配种子数</span> {result.matches} / {result.seeds}</p>
      {result.matches === 0 && <p className="equipment-notes">目标存档的生成条件下没有匹配结果。可减少所选词条，或到神社保存当前进度后重新查找。</p>}
      {result.outcomes.length > 0 && <ul className="seed-outcomes" role="listbox" aria-label="生成结果">
        {result.outcomes.map(outcome => <li key={outcome.seed}>
          <button type="button" role="option" aria-selected={seed === String(outcome.seed)} className={seed === String(outcome.seed) ? "chosen" : ""} onClick={() => onSeed(String(outcome.seed))}>
            <small className="seed-id"><span>生成种子</span> {outcome.seed}</small>
            {outcome.effects.map((effect, index) => <span key={index} className={"seed-effect" + (wanted.some(entry => entry.ids.includes(effect.effect_id)) ? " wanted" : "")}>
              {effect.star ? "✦ " : ""}{effectName(effect.effect_id)}{effect.role === "set" || effect.role === "grace" ? null : <small> {effect.value}</small>}
            </span>)}
          </button>
        </li>)}
      </ul>}
    </>}
  </fieldset>;
}
