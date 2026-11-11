import { EffectLabel, EffectValue, effectText, EffectPreview } from "./effect-presentation";
import React, { useEffect, useRef, useState, useSyncExternalStore, type Dispatch, type SetStateAction } from "react";
import { useEquipmentFavorites } from "./equipment-favorites";
import { ADD_CATALOG, nameLabel, setName, setOf } from "./equipment-facets";
import { decodeLoadout, type LoadoutPiece } from "./loadout-code";
import { fitsSlot, savedLoadoutPiece, equipmentOccupiedSlot, setLoadoutEquipment } from "./loadout-equipment";
import type { CharacterEquipment } from "../../packages/contracts/protected-responses";
import { data, type Sample } from "./model";
import { collectionKey } from "./collections";
import { localize, localizeName } from "./presentation";
import { publicError, stripErrorPrefix } from "./public-errors";
import { LoadoutSaveEquipmentSource } from "./loadout-save-equipment-source";
import { loadoutSaveGateway, preferredLoadoutSave, rememberLoadoutSave } from "./loadout-save-workspace";
import { canAddSkill, canSelectClanProtection, clanProtectionValue, setClanProtection, ESSENCE_EFFECTS, clans, clanRanks, essenceLimit, skills, skillUsage, type ClanProtectionChoice, type LoadoutSelection, type LoadoutStyle, type SkillCategory } from "./loadout-state";

export type SelectionSetter = Dispatch<SetStateAction<LoadoutSelection>>;
const categoryTitles: Record<SkillCategory, string> = { common: "共通技能", samurai: "武士技能", ninja: "忍者技能" };
const essenceNames: Record<SkillCategory, string> = { common: "精髓并存·共通", samurai: "精髓并存·武士", ninja: "精髓并存·忍者" };
const knownEffects = new Set(data.editorEffects.map(effect => Number(effect.id)));
const equipmentCatalog = new Map(ADD_CATALOG.map(item => [item.id, item]));
const equipmentEffects = new Map(data.editorEffects.map(effect => [Number(effect.id), effect.name]));
const matchesName = (name: string, query: string, extra = "") =>
  `${name} ${localizeName(name) ?? ""} ${extra}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase());

export function PickerDialog({ title, close, children }: { title: string; close: () => void; children: React.ReactNode }) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => { ref.current!.showModal(); }, []);
  return <dialog ref={ref} className="loadout-picker" aria-label={title} onCancel={close} onClick={event => { if (event.target === ref.current) close(); }}>
    <header><h2>{title}</h2><button type="button" aria-label="关闭" onClick={close}>×</button></header>
    {children}
  </dialog>;
}

export function ClanPanel({ style, selection, setSelection }: { style: LoadoutStyle; selection: LoadoutSelection; setSelection: SelectionSetter }) {
  const [picker, setPicker] = useState<{ slot: number; kind: "clan" | "effect" | "rank" } | null>(null);
  const [query, setQuery] = useState("");
  const chooseClan = (id: number | null) => {
    setSelection(current => current.clans[style][picker!.slot]?.clanId === id ? current : setClanProtection(current, style, picker!.slot, id === null ? null : { clanId: id, rankIndex: 0, slotIndex: null }));
    setPicker(null);
  };
  const activeChoice = picker && selection.clans[style][picker.slot];
  const activeClan = activeChoice && clans.find(clan => clan.clan_id === activeChoice.clanId);
  return <section className="loadout-subsection loadout-clan-section" aria-label="武家庇佑">
    <h3>武家</h3>
    <div className="loadout-clan-head"><span>武家</span><span>阶级</span><span>词条</span></div>
    <div className="loadout-clan-rows">
      {selection.clans[style].map((choice, index) => {
        const clan = choice && clans.find(clan => clan.clan_id === choice.clanId);
        const effect = choice && clanProtectionValue(selection, choice);
        return <div className="loadout-clan-card" key={index} data-clan-row={index}>
          <button type="button" className="loadout-clan-name" aria-label="选择武家" aria-haspopup="dialog" onClick={() => { setQuery(""); setPicker({ slot: index, kind: "clan" }); }}><span>{clan ? nameLabel(clan.name_zh) : "选择武家"}</span><span aria-hidden="true">⌄</span></button>
          <button type="button" className="loadout-clan-rank" aria-label="武家阶级" aria-haspopup="dialog" disabled={!choice} onClick={() => setPicker({ slot: index, kind: "rank" })}>
            <span>{choice?.rankIndex === null ? "存档数值" : clanRanks[choice?.rankIndex ?? 0].name_zh}</span><span aria-hidden="true">⌄</span>
          </button>
          <button type="button" className="loadout-clan-effect" disabled={!clan} aria-label="选择武家词条" aria-haspopup="dialog" title={effect ? `${effectText(effect.label_zh)} ${effect.display}` : localize("选择词条")} onClick={() => setPicker({ slot: index, kind: "effect" })}>
            <span>{effect ? <EffectLabel text={effect.label_zh} /> : "选择词条"}</span>{effect && <EffectValue display={effect.display} />}
          </button>
        </div>;
      })}
    </div>
    {picker?.kind === "rank" && activeChoice && <PickerDialog title="选择武家阶级" close={() => setPicker(null)}>
      <div className="loadout-rank-options">{clanRanks.map(rank => <button type="button" key={rank.rank_index}
        data-clan-rank={rank.rank_index} aria-pressed={activeChoice.rankIndex === rank.rank_index} onClick={() => {
          setSelection(current => setClanProtection(current, style, picker.slot, { ...current.clans[style][picker.slot]!, rankIndex: rank.rank_index, savedRaw: undefined }));
          setPicker(null);
        }}>{rank.name_zh}</button>)}</div>
    </PickerDialog>}
    {picker?.kind === "clan" && <PickerDialog title="选择武家" close={() => setPicker(null)}>
      <input aria-label="搜索武家或词条" placeholder="搜索武家或词条" value={query} onChange={event => setQuery(event.target.value)} />
      <button type="button" onClick={() => chooseClan(null)}>清空栏位</button>
      {activeClan && <label className="loadout-clan-membership"><input type="checkbox" checked={selection.joinedClan === activeClan.clan_id} onChange={event => {
        const joinedClan = event.target.checked ? activeClan.clan_id : null;
        setSelection(current => ({ ...current, joinedClan }));
      }} /><span>当前所属武家</span> · {nameLabel(activeClan.name_zh)}</label>}
      <div className="loadout-picker-list loadout-clan-list">{clans.filter(clan => matchesName(clan.name_zh, query, clan.slots.map(effect => `${effect.label_zh} ${effectText(effect.label_zh)}`).join(" "))).map(clan =>
        <button type="button" key={clan.clan_id} data-clan-id={clan.clan_id} onClick={() => chooseClan(clan.clan_id)}>
          <strong>{nameLabel(clan.name_zh)}</strong><ul>{clan.slots.map(effect => <li key={effect.slot_index}><EffectLabel text={effect.label_zh} /></li>)}</ul>
        </button>)}</div>
    </PickerDialog>}
    {picker?.kind === "effect" && activeClan && activeChoice && <PickerDialog title="选择武家词条" close={() => setPicker(null)}>
      <p>{nameLabel(activeClan.name_zh)} · {clanRanks.find(rank => rank.rank_index === activeChoice.rankIndex)?.name_zh}</p>
      <div className="loadout-picker-list">{activeClan.slots.map(native => {
        const choice: ClanProtectionChoice = { ...activeChoice, slotIndex: native.slot_index, rankIndex: activeChoice.rankIndex ?? 0, savedRaw: undefined };
        const effect = clanProtectionValue(selection, choice)!;
        const available = canSelectClanProtection(selection, style, picker.slot, choice);
        return <button type="button" key={native.slot_index} data-clan-slot={native.slot_index} disabled={!available} title={available ? native.description_zh : "另一栏位已选择此词条"} onClick={() => {
          setSelection(current => setClanProtection(current, style, picker.slot, choice)); setPicker(null);
        }}>{<EffectLabel text={effect.label_zh} />}<EffectValue display={effect.display} /></button>;
      })}</div>
    </PickerDialog>}
  </section>;
}

export function NativeSkillPanel({ category, style, selection, setSelection }: { category: SkillCategory; style: LoadoutStyle; selection: LoadoutSelection; setSelection: SelectionSetter }) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const used = skillUsage(selection, category);
  const limit = essenceLimit(selection, style, category);
  const capacity = selection.capacities[category];
  const invalid = used.essences > limit || (capacity !== null && used.cost > capacity);
  const remove = (id: number) => setSelection(current => ({ ...current, skills: { ...current.skills, [category]: current.skills[category].filter(value => value !== id) } }));
  const add = (id: number) => setSelection(current => canAddSkill(current, style, id) ? { ...current, skills: { ...current.skills, [category]: [...current.skills[category], id] } } : current);
  const headingId = `loadout-skills-${category}-heading`;
  return <section className="loadout-subsection" data-skill-category={category} aria-labelledby={headingId}>
    <h4 id={headingId}>{categoryTitles[category]}</h4>
    <div className="loadout-skill-status">
      <label className="loadout-capacity"><span>容量</span><strong>{used.cost}</strong><span>/</span><input type="number" min={0} step={1} placeholder="—" title="容量上限" aria-label={categoryTitles[category] + "容量上限"} value={capacity ?? ""} onChange={event => { const text = event.target.value; const value = text === "" ? null : Number(text); if (value === null || Number.isSafeInteger(value) && value >= 0) setSelection(current => ({ ...current, capacities: { ...current.capacities, [category]: value } })); }} /></label>
      <span className="loadout-essence-count">精髓 <strong>{`${used.essences} / ${limit}`}</strong></span>
    </div>
    {invalid && <p role="alert" className="loadout-invalid">已选技能超出当前上限，请移除技能或调整装备与容量。</p>}
    <ul className="loadout-skill-list" tabIndex={used.chosen.length ? 0 : undefined} aria-labelledby={headingId}>{used.chosen.map(skill => <li key={skill.id} data-skill-id={skill.id}>
      <span className={`loadout-skill-name rarity-${skill.rarity_tag}`} title={localizeName(skill.name) ?? skill.name}>
        {nameLabel(skill.name)}
      </span><small className="loadout-skill-kind" data-skill-type={skill.type} title={localize(skill.type_label)}>{skill.type_label}</small><small>{`Cost ${skill.cost}`}</small><button type="button" aria-label="移除技能" onClick={() => remove(skill.id)}>×</button>
    </li>)}</ul>
    <button className="loadout-add-skill" type="button" onClick={() => { setQuery(""); setOpen(true); }}>+ 添加技能</button>
    {open && <PickerDialog title={categoryTitles[category]} close={() => setOpen(false)}>
      <input aria-label="搜索技能" placeholder="搜索技能" value={query} onChange={event => setQuery(event.target.value)} />
      <dl className="loadout-skill-metrics"><div><dt>容量</dt><dd>{`${used.cost} / ${capacity ?? "—"}`}</dd></div><div><dt>精髓</dt><dd>{`${used.essences} / ${limit}`}</dd></div></dl>
      <div className="loadout-picker-list">{skills.filter(skill => skill.category === category && matchesName(skill.name, query, localize(skill.type_label))).map(skill => {
        const selected = selection.skills[category].includes(skill.id);
        return <button type="button" data-skill-id={skill.id} key={skill.id} className={selected ? "selected" : ""} disabled={!selected && !canAddSkill(selection, style, skill.id)} onClick={() => selected ? remove(skill.id) : add(skill.id)}>
          <span className={`loadout-skill-name rarity-${skill.rarity_tag}`}>{nameLabel(skill.name)}<small className="loadout-skill-kind" data-skill-type={skill.type}>{skill.type_label}</small></span>
          <small>{`Cost ${skill.cost}`}{selected ? " ✓" : ""}</small>
        </button>;
      })}</div>
    </PickerDialog>}
  </section>;
}

export function PlannerScrollRow({ style, selection, setSelection, favorites, onInspect }: { style: LoadoutStyle; selection: LoadoutSelection; setSelection: SelectionSetter; favorites: Sample[]; onInspect?: () => void }) {
  const [open, setOpen] = useState(false);
  const chosen = selection.scrolls[style];
  const choose = (sample: Sample | null) => {
    setSelection(current => ({ ...current, scrolls: { ...current.scrolls, [style]: sample } }));
    setOpen(false);
  };
  return <div className="loadout-equipment-rows">
    <div className="loadout-equipment-entry"><button type="button" data-equipment-slot="scroll" aria-label="绘卷" className="loadout-equipment-row" onClick={() => { onInspect?.(); if (!chosen || !onInspect) setOpen(true); }}>
      <span className="loadout-equipment-name">{chosen ? `Seed ${chosen.seed}` : "—"}</span><span className="loadout-equipment-set">—</span>
    </button>{chosen && onInspect && <button type="button" className="loadout-replace-equipment" aria-label="更换绘卷" onClick={() => { onInspect(); setOpen(true); }}>更换</button>}</div>
    {open && <PickerDialog title="选择配装绘卷" close={() => setOpen(false)}>
      <p>从绘卷收藏选择</p>
      <button type="button" onClick={() => choose(null)}>清空栏位</button>
      {!favorites.length && <p>没有收藏绘卷。请先在绘卷搜索或编辑中收藏绘卷。</p>}
      <div className="loadout-picker-list">{favorites.map(sample => <button type="button" data-scroll-seed={sample.seed} key={collectionKey(sample)} onClick={() => choose(sample)}>
        <span>{`Seed ${sample.seed} · Lv.${sample.level ?? 180}`}</span>
        <EffectPreview effects={sample.effects.map(effect => ({ effectId: Number(effect.id), raw: effect.raw }))} />
      </button>)}</div>
    </PickerDialog>}
  </div>;
}

const graceNames = new Map(Object.values(data.contexts).flatMap(context => context.graces).map(grace => [Number(grace.id), grace.name]));
function equipmentSetLabel(piece: LoadoutPiece): string | null {
  const fixed = setOf(piece.item_id);
  return fixed !== null ? setName(fixed) : piece.effects.map(effect => graceNames.get(effect.effect_id)).find(name => name !== undefined) ?? null;
}

type EquipmentSource = "save" | "favorites" | "code";
const noSubscription = () => () => {};
const noSnapshot = () => null;
function EquipmentPicker({ style, slot, selection, owner, choose }: { style: LoadoutStyle; slot: string; selection: LoadoutSelection; owner: LoadoutStyle | "shared"; choose: (piece: LoadoutPiece | null) => void }) {
  const favorites = useEquipmentFavorites();
  const [source, setSource] = useState<EquipmentSource>(loadoutSaveGateway ? "save" : "favorites");
  const [query, setQuery] = useState("");
  const [code, setCode] = useState("");
  const [imported, setImported] = useState<LoadoutPiece[]>([]);
  const [codeError, setCodeError] = useState("");
  const [saveSource] = useState(() => loadoutSaveGateway ? new LoadoutSaveEquipmentSource(loadoutSaveGateway) : null);
  const save = useSyncExternalStore(saveSource?.subscribe ?? noSubscription, saveSource?.getSnapshot ?? noSnapshot);
  const discovered = useRef(false);
  useEffect(() => {
    if (source === "save" && saveSource && !discovered.current) {
      discovered.current = true;
      void saveSource.discover(preferredLoadoutSave());
    }
  }, [source, saveSource]);
  useEffect(() => () => saveSource?.dispose(), [saveSource]);
  useEffect(() => { if (save?.selected) rememberLoadoutSave(save.selected); }, [save?.selected]);
  const entries = source === "save" ? (save?.character?.equipment ?? []).map(piece => ({ piece, saveSlot: piece.slot_index, worn: piece.worn }))
    : (source === "favorites" ? favorites : imported).map((piece, index) => ({ piece, saveSlot: undefined, worn: false, index }));
  const candidates = entries.filter(({ piece }) => fitsSlot(piece, style, slot)).filter(({ piece }) => {
    const item = equipmentCatalog.get(piece.item_id)!;
    const set = equipmentSetLabel(piece) ?? "";
    const effects = piece.effects.map(effect => equipmentEffects.get(effect.effect_id) ?? "");
    return matchesName(item.name, query, [set, localizeName(set) ?? "", ...effects.map(effectText)].join(" "));
  });
  const references = save?.selected && !save.saves.some(reference => reference.save_id === save.selected!.save_id) ? [...save.saves, save.selected] : save?.saves ?? [];
  const saveBusy = source === "save" && !!save?.busy;
  const readCode = () => {
    try {
      const pieces = decodeLoadout(code.trim());
      if (pieces.some(piece => !ADD_CATALOG.some(item => item.id === piece.item_id) || piece.effects.some(effect => effect.effect_id !== 0xffffffff && !knownEffects.has(effect.effect_id)))) throw new Error("配装码包含未知装备或词条。");
      setImported(pieces); setCodeError("");
    } catch (error) { setImported([]); setCodeError(stripErrorPrefix(String(error))); }
  };
  return <div className="loadout-equipment-picker">
    <div className="loadout-source-tabs" role="tablist" aria-label="装备来源">
      {([["save", "存档装备"], ["favorites", "装备收藏"], ["code", "装备码"]] as const).map(([value, label]) => <button type="button" role="tab" key={value} id={`equipment-source-${value}`} aria-controls="equipment-source-panel" aria-selected={source === value} onClick={() => setSource(value)}>{label}</button>)}
    </div>
    <div id="equipment-source-panel" role="tabpanel" aria-labelledby={`equipment-source-${source}`}>
      {source === "save" && <div className="loadout-save-source">
        {saveSource ? <>
          <div className="loadout-save-toolbar">
            <label>存档<select aria-label="配装存档" disabled={save?.busy} value={save?.selected?.save_id ?? ""} onChange={event => void saveSource.select(references.find(reference => reference.save_id === event.target.value) ?? null)}>
              <option value="">选择存档</option>{references.map(reference => <option value={reference.save_id} key={reference.save_id}>{`${localize("存档")} ${reference.save_slot + 1} · ${reference.path.split(/[\\/]/).pop()}`}</option>)}
            </select></label>
            <button type="button" disabled={save?.busy} onClick={() => void saveSource.pick()}>浏览文件</button>
            <button type="button" disabled={save?.busy} onClick={() => void (save?.selected ? saveSource.refresh() : saveSource.discover())}>重新读取</button>
          </div>
          {save?.busy && <p role="status">正在读取存档装备…</p>}
          {save?.error && <p role="alert" className="loadout-invalid">{publicError(stripErrorPrefix(save.error))}</p>}
          {!save?.busy && !save?.error && !save?.selected && <p className="loadout-source-note">请选择存档后读取装备，也可以浏览存档文件。</p>}
          {save?.character && <p className="loadout-source-note">已读取存档装备<span> · {save.character.equipment.length}</span></p>}
        </> : <p className="loadout-source-note">网页预览无法访问本地存档，请在桌面工具中选择存档装备。</p>}
      </div>}
      {source === "favorites" && <p className="loadout-source-note">从已收藏的装备中选择。</p>}
      {source === "code" && <div className="loadout-code-source">
        <label>装备码<textarea aria-label="装备码" placeholder="粘贴 N3E1- 开头的装备码" value={code} onChange={event => setCode(event.target.value)} /></label>
        <button type="button" disabled={!code.trim()} onClick={readCode}>读取装备码</button>
        {codeError && <p role="alert" className="loadout-invalid">{codeError}</p>}
      </div>}
      <div className="loadout-picker-filter"><input aria-label="搜索配装装备" placeholder="搜索装备名称、套装或词条" value={query} onChange={event => setQuery(event.target.value)} /><small>{candidates.length}</small></div>
      {!candidates.length && !saveBusy && <p className="loadout-source-note">没有符合当前栏位与搜索条件的装备。</p>}
      <div className="loadout-picker-list loadout-equipment-options">{candidates.map(({ piece, saveSlot, worn }, index) => {
        const candidate: LoadoutPiece = source === "save" && save?.character ? savedLoadoutPiece(piece as CharacterEquipment, save.character)
          : { item_id: piece.item_id, level: piece.level, level_before_forge: piece.level_before_forge, plus: piece.plus, rarity: piece.rarity, hell: piece.hell, hell_skill: piece.hell_skill, origin: source, effects: piece.effects.map(effect => ({ ...effect })) };
        const occupied = equipmentOccupiedSlot(selection, style, owner, slot, candidate);
        const set = equipmentSetLabel(piece);
        const effects = piece.effects.filter(effect => effect.effect_id !== 0xffffffff).map(effect => ({ effectId: effect.effect_id, raw: effect.value }));
        return <button type="button" key={saveSlot ?? index} data-item-id={piece.item_id} data-save-slot={saveSlot} disabled={saveBusy || !!occupied} onClick={() => choose(candidate)}>
          <span className="loadout-option-name">{nameLabel(ADD_CATALOG.find(item => item.id === piece.item_id)!.name)}{worn && <small>已装备</small>}</span><small>{`Lv.${piece.level} +${piece.plus}`}</small>
          <span className="loadout-option-set">{set ? nameLabel(set) : "—"}</span>
          {occupied && <span className="loadout-source-note">已用于其他栏位</span>}
          {!!effects.length && <EffectPreview effects={effects} />}
        </button>;
      })}</div>
    </div>
    <footer className="loadout-picker-footer"><span>选择装备后仅更新配装方案。</span><button type="button" onClick={() => choose(null)}>清空栏位</button></footer>
  </div>;
}

export function PlannerEquipmentRows({ rows, style, shared = false, selection, setSelection, onInspect, inspectedSlot }: { rows: { slot: string; key: string }[]; style: LoadoutStyle; shared?: boolean; selection: LoadoutSelection; setSelection: SelectionSetter; onInspect?: (slot: string) => void; inspectedSlot?: string }) {
  const [slot, setSlot] = useState<string | null>(null);
  const owner = shared ? "shared" : style;
  const choose = (piece: LoadoutPiece | null) => {
    setSelection(current => setLoadoutEquipment(current, style, owner, slot!, piece));
    setSlot(null);
  };
  return <div className="loadout-equipment-rows">
    {rows.map(row => { const piece = selection.equipment[owner][row.key]; const item = ADD_CATALOG.find(item => item.id === piece?.item_id); const deferred = row.key === "guardian" || row.key === "scroll"; const set = piece && equipmentSetLabel(piece);
      return <div className="loadout-equipment-entry" key={row.key}><button type="button" data-equipment-slot={row.key} data-inspected={inspectedSlot === row.key || undefined} className="loadout-equipment-row" aria-label={row.slot} aria-disabled={deferred || undefined} title={deferred ? "装备选择功能尚未接入。" : [item ? localizeName(item.name) ?? item.name : localize("从存档、装备收藏或装备码选择"), set ? localizeName(set) ?? set : ""].join(" · ")} onClick={() => { if (!deferred) { onInspect?.(row.key); if (!piece || !onInspect) setSlot(row.key); } }}>
        <span className="loadout-equipment-name">{item ? nameLabel(item.name) : "—"}</span>
        <span className="loadout-equipment-set">{set ? nameLabel(set) : "—"}</span>
      </button>{piece && onInspect && !deferred && <button type="button" data-replace-slot={row.key} className="loadout-replace-equipment" aria-label={`更换${row.slot}`} onClick={() => { onInspect(row.key); setSlot(row.key); }}>更换</button>}</div>;
    })}
    {slot !== null && <PickerDialog title="选择配装装备" close={() => setSlot(null)}>
      <EquipmentPicker style={style} slot={slot} selection={selection} owner={owner} choose={choose} />
    </PickerDialog>}
  </div>;
}
