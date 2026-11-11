import { EffectLabel, EffectValue } from "./effect-presentation";
import React, { useEffect, useState, useSyncExternalStore } from "react";
import { PickerDialog } from "./LoadoutPickers";
import { LoadoutSaveEquipmentSource } from "./loadout-save-equipment-source";
import { loadoutSaveGateway, preferredLoadoutSave, rememberLoadoutSave } from "./loadout-save-workspace";
import { savedConfigReview, type SavedLoadoutImport } from "./loadout-save-config";
import { clans } from "./loadout-state";
import { nameLabel } from "./equipment-facets";
import { formatNativeEffectValue, nativeEffectGroup } from "./loadout-effect-summary";
import { publicError, stripErrorPrefix } from "./public-errors";
const none = () => null, subscribeNone = () => () => {};

function SavedConfigDialog({ close, onImport }: { close: () => void; onImport: (value: SavedLoadoutImport) => void }) {
  const [source] = useState(() => loadoutSaveGateway ? new LoadoutSaveEquipmentSource(loadoutSaveGateway) : null);
  const snapshot = useSyncExternalStore(source?.subscribe ?? subscribeNone, source?.getSnapshot ?? none);
  useEffect(() => { void source?.discover(preferredLoadoutSave()); return () => source?.dispose(); }, [source]);
  useEffect(() => { if (snapshot?.selected) rememberLoadoutSave(snapshot.selected); }, [snapshot?.selected]);
  const review = snapshot?.character ? savedConfigReview(snapshot.character) : null;
  const config = review?.config;
  const refs = snapshot?.selected && !snapshot.saves.some(row => row.save_id === snapshot.selected!.save_id)
    ? [...snapshot.saves, snapshot.selected] : snapshot?.saves ?? [];
  return <PickerDialog title="导入存档配置" close={close}>
    <p className="loadout-source-note">读取存档中的两种风格配置。导入后保留当前守护灵和同武家的手动阶级，存档文件保持不变。</p>
    {source ? <>
      <div className="loadout-save-toolbar">
        <label>存档<select aria-label="角色配置存档" disabled={snapshot?.busy} value={snapshot?.selected?.save_id ?? ""}
          onChange={event => void source.select(refs.find(row => row.save_id === event.target.value) ?? null)}>
          <option value="">选择存档</option>{refs.map(row => <option key={row.save_id} value={row.save_id}>{`存档 ${row.save_slot + 1}`}</option>)}
        </select></label>
        <button type="button" disabled={snapshot?.busy} onClick={() => void source.pick()}>浏览文件</button>
        <button type="button" disabled={snapshot?.busy} onClick={() => void (snapshot?.selected ? source.refresh() : source.discover())}>重新读取</button>
      </div>
      {snapshot?.busy && <p role="status">正在读取角色配置…</p>}
      {snapshot?.error && <p role="alert" className="loadout-invalid">{publicError(stripErrorPrefix(snapshot.error))}</p>}
      {!snapshot?.selected && !snapshot?.busy && <p>请选择用于核对的存档。</p>}
      {config && <div className="loadout-config-review">
        <section><h3>已确认字段</h3><dl>
          <div><dt>等级</dt><dd>{config.level ?? "—"}</dd></div>
          <div><dt>属性</dt><dd>{config.attributes?.map((value, index) => `${["体", "心", "刚", "武", "技", "智", "咒"][index]} ${value}`).join(" · ") ?? "—"}</dd></div>
          <div><dt>技能容量</dt><dd>{config.capacities?.map((value, index) => `${["共通", "武士", "忍者"][index]} ${value}`).join(" · ") ?? "—"}</dd></div>
          <div><dt>破坏楔数</dt><dd>{config.destroyed_spike_count ?? "—"}</dd></div>
          <div><dt>装备记录</dt><dd>{snapshot!.character!.equipment.length}</dd></div>
        </dl></section>
        <section><h3>已选配装技能 <small>{review!.selectedSkills.length}</small></h3>
          <p className="loadout-source-note">仅列出当前配装技能目录中已识别且已选择的记录。</p>
          <ul>{review!.selectedSkills.map(skill => <li key={skill.id}>{nameLabel(skill.name)}<span>{`等级 ${skill.rank}`}</span></li>)}</ul>
          {!review!.selectedSkills.length && <p>未发现已选的目录内配装技能。</p>}
        </section>
        <details><summary>武家庇佑原始记录</summary><ul>{config.clan_protections?.map((choice, index) => {
          const clan = clans.find(row => row.clan_id === choice.clan_id);
          const effect = clan?.slots.find(row => row.slot_index === choice.effect_index && row.effect_group_id === choice.group_id);
          const group = nativeEffectGroup(choice.group_id);
          return <li key={index}><span>{`记录 ${index + 1} · `}{clan ? nameLabel(clan.name_zh) : `武家 ID ${choice.clan_id}`} · {effect ? <EffectLabel text={effect.label_zh} /> : `词条组 ${choice.group_id}`}</span><EffectValue display={group ? formatNativeEffectValue(group, choice.raw) : null} raw={choice.raw} /></li>;
        }) ?? <li>记录不可读取</li>}</ul><p>武家词条使用存档实际数值，不从缓存恢复阶级。已有同武家的手动阶级保留，其余可在导入后手动选择。</p></details>
      </div>}
      {review && <section className="loadout-config-pending"><h3>{review.canRestore ? "配置可导入" : "暂不能完整导入"}</h3><ul>{[...review.issues, ...review.notes].map(issue => <li key={issue}>{issue}</li>)}</ul>
        {review.canRestore && <p>将恢复等级与属性、装备与绘卷、魂核、武家庇佑及已选技能；守护灵保持当前配置，武家阶级缓存不导入。</p>}
      </section>}
    </> : <p>网页预览不能访问本地存档。桌面工具中可只读核对角色配置。</p>}
    <footer className="loadout-picker-footer"><span>读取与取消保留当前配装</span><button type="button" disabled={snapshot?.busy || !review?.restoration} onClick={() => {
      if (!review?.restoration) return;
      onImport(structuredClone(review.restoration)); close();
    }}>导入角色配置</button></footer>
  </PickerDialog>;
}
export function LoadoutSaveConfig({ onImport }: { onImport: (value: SavedLoadoutImport) => void }) {
  const [open, setOpen] = useState(false);
  return <><button type="button" className="loadout-config-entry" onClick={() => setOpen(true)}>导入存档配置</button>{open && <SavedConfigDialog onImport={onImport} close={() => setOpen(false)} />}</>;
}
