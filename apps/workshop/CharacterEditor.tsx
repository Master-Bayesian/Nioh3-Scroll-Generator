import React, { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import type {
  CharacterEquipment,
  EffectValues,
  EquipmentRules,
  LiveCharacter,
  SaveCharacter,
} from "../../packages/contracts/protected-responses";
import { data } from "./model";
import itemNames from "./item-names.json";
import { desktop } from "./desktop-bridge";
import { fillTemplateSlots, plainGameText } from "./game-text";
import { LocalCatalogImport, type ActiveLocalCatalog } from "./LocalCatalogImport";
import { Notice } from "./Notice";
import { SavePicker } from "./CartActions";
import { runtimeObserver, saveObserver, saveSession } from "./save-workspace";

type Mode = "live" | "save";
type Character = (LiveCharacter | SaveCharacter) & { mode: Mode };
type Currency = "amrita" | "gold";
type LegalValue = EffectValues["values"][number];
const EMPTY_EFFECT = 0xffffffff;

const effectNames = new Map<number, string>();
for (const row of data.editorEffects as { id: string; name: string }[]) {
  const id = Number(row.id);
  if (Number.isInteger(id) && row.name && !effectNames.has(id))
    effectNames.set(id, fillTemplateSlots(plainGameText(row.name), "增益效果", "异常状态"));
}
const ALL_EFFECTS = [...effectNames.keys()];

/** Coarse item group from the shipped item table's type class. */
function kindOf(typeClass: number | null | undefined): string {
  if (typeClass == null) return "其他";
  if (typeClass <= 22) return "武器";
  if (typeClass >= 24 && typeClass <= 38) return "防具";
  if (typeClass === 39 || typeClass === 40) return "饰品";
  if (typeClass >= 54 && typeClass <= 57) return "魂核";
  return "其他";
}

const itemCatalog = (itemNames as { items: Record<string, string[]> }).items;
/** `[major, middle, minor]` from the bundled catalog, falling back to the type class. */
function itemGroups(id: number, typeClass: number | null | undefined): [string, string, string] {
  const entry = itemCatalog[String(id)];
  return [entry?.[1] || kindOf(typeClass), entry?.[3] ?? "", entry?.[2] ?? ""];
}
function itemName(id: number, catalog: ActiveLocalCatalog | null): string {
  return itemCatalog[String(id)]?.[0] || catalog?.entries.get(id) || "";
}

function hex(value: number) {
  return "0x" + value.toString(16).toUpperCase().padStart(4, "0");
}
function effectLabel(id: number) {
  if (id === EMPTY_EFFECT) return "（空）";
  return effectNames.get(id) ?? hex(id);
}
/** An effect field accepts `0xA166`, `41318`, or a picker entry that starts with either. */
function parseEffectId(text: string): number | null {
  const token = text.trim().split(/\s+/)[0] ?? "";
  if (!token) return EMPTY_EFFECT;
  const value = /^0x[0-9a-f]+$/i.test(token) ? Number.parseInt(token, 16) : /^\d+$/.test(token) ? Number(token) : NaN;
  return Number.isInteger(value) && value >= 0 && value <= EMPTY_EFFECT ? value : null;
}
function parseAmount(text: string, max: number): number | null {
  if (!/^\d+$/.test(text.trim())) return null;
  const value = Number(text.trim());
  return Number.isSafeInteger(value) && value <= max ? value : null;
}
function percent(fraction: number) {
  const value = fraction * 100;
  return (value >= 10 ? value.toFixed(0) : value >= 1 ? value.toFixed(1) : value.toFixed(2)) + "%";
}

const ROLE_LABEL: Record<string, string> = {
  innate: "固有",
  hell: "地狱",
  random: "随机",
  set: "套装",
  grace: "恩宠",
};
const FINDING_LABEL: Record<string, string> = {
  unknown_item: "物品不在当前版本的物品表中",
  unsupported_rarity: "稀有度超出自然范围",
  effect_count: "词条数量与该稀有度不符",
  unknown_effect: "词条 ID 不存在",
  wrong_innate: "固有词条不是该物品自带的",
  wrong_set: "套装效果不是该物品自带的",
  not_grace: "该位置应为恩宠效果",
  not_in_pool: "该词条不会出现在这类装备上",
  hell_effect_outside_hell_slot: "地狱词条只能出现在地狱武器第一条",
  missing_hell_effect: "地狱武器第一条应为地狱词条",
  hell_on_ineligible_item: "该物品不能成为地狱武器",
  star_below_rarity: "星号词条需要更高稀有度",
  multiple_stars: "星号词条超过一条",
  star_flag_mismatch: "星号标记与词条不一致",
  group_conflict: "与另一条词条互斥",
  value_not_natural: "数值不是自然生成能出现的值",
  roll_out_of_range: "分位超出该稀有度范围",
  hell_skill_not_natural: "地狱武技不属于该武器",
};
function findingText(finding: { code: string; slot?: number; other?: number }) {
  const where = finding.slot == null ? "" : "#" + (finding.slot + 1) + " ";
  return where + (FINDING_LABEL[finding.code] ?? finding.code);
}

interface DraftEffect {
  id: string;
  value: string;
  roll: number | null;
  star: boolean | null;
}
interface Draft {
  level: string;
  level_before_forge: string;
  plus: string;
  rarity: string;
  familiarity: string;
  effects: DraftEffect[];
}
function draftOf(row: CharacterEquipment): Draft {
  return {
    level: String(row.level),
    level_before_forge: String(row.level_before_forge),
    plus: String(row.plus),
    rarity: String(row.rarity),
    familiarity: String(row.familiarity),
    effects: row.effects.map(effect => ({
      id: effect.effect_id === EMPTY_EFFECT ? "" : hex(effect.effect_id),
      value: String(effect.value),
      roll: null,
      star: null,
    })),
  };
}

/** The changed fields of one record, or an error message when a field is invalid. */
function patchOf(row: CharacterEquipment, draft: Draft) {
  const patch: Record<string, unknown> = {};
  const fields: [keyof Draft & keyof CharacterEquipment, number, number][] = [
    ["level", 1, 65535],
    ["level_before_forge", 1, 65535],
    ["plus", 0, 65535],
    ["rarity", 0, 255],
    ["familiarity", 0, 4294967295],
  ];
  for (const [key, min, max] of fields) {
    const value = parseAmount(draft[key] as string, max);
    if (value === null || value < min) return { error: "请输入有效的数值。" };
    if (value !== row[key]) patch[key] = value;
  }
  const effects = [];
  for (const [index, effect] of draft.effects.entries()) {
    const id = parseEffectId(effect.id);
    const value = id === EMPTY_EFFECT ? 0 : parseAmount(effect.value, 4294967295);
    if (id === null || value === null) return { error: "请输入有效的词条 ID 和数值。" };
    const before = row.effects[index];
    if (!before || before.effect_id !== id || (id !== EMPTY_EFFECT && before.value !== value) || effect.roll !== null)
      effects.push({
        index,
        effect_id: id,
        value,
        ...(effect.roll !== null && id !== EMPTY_EFFECT ? { roll: effect.roll } : {}),
        ...(effect.star !== null && id !== EMPTY_EFFECT ? { star: effect.star } : {}),
      });
  }
  if (effects.length) patch.effects = effects;
  return { patch };
}

const CURRENCY_LABEL: Record<string, string> = { amrita: "精华", gold: "持有金钱" };
const FIELD_LABEL: [keyof CharacterEquipment, string][] = [
  ["level", "等级"],
  ["level_before_forge", "锻造前等级"],
  ["plus", "+值"],
  ["rarity", "稀有度"],
  ["familiarity", "爱用度"],
];

/** The reviewed plan in words: every changed value, before and after. */
function PlanPreview({ preview }: { preview: Record<string, unknown> }) {
  const currencies = (preview.currencies ?? []) as { currency: string; before: number; after: number }[];
  const equipment = (preview.equipment ?? []) as { slot_index: number; before: CharacterEquipment; after: CharacterEquipment }[];
  const lines: string[] = [];
  for (const change of currencies)
    lines.push((CURRENCY_LABEL[change.currency] ?? change.currency) + "：" + change.before + " → " + change.after);
  for (const change of equipment) {
    const prefix = "槽位 " + change.slot_index + "（" + hex(change.before.item_id) + "）";
    for (const [key, label] of FIELD_LABEL)
      if (change.before[key] !== change.after[key])
        lines.push(prefix + " " + label + "：" + change.before[key] + " → " + change.after[key]);
    change.after.effects.forEach((effect, index) => {
      const before = change.before.effects[index];
      if (!before || before.effect_id !== effect.effect_id || before.value !== effect.value)
        lines.push(prefix + " " + "词条" + " " + (index + 1) + "：" +
          effectLabel(before?.effect_id ?? EMPTY_EFFECT) + " " + (before?.value ?? 0) + " → " +
          effectLabel(effect.effect_id) + " " + effect.value);
    });
  }
  return <ul className="character-plan-lines">{lines.map(line => <li key={line}>{line}</li>)}</ul>;
}

/**
 * Effect picker: opening it always lists every candidate; only text typed after
 * opening filters the list, so the current effect never hides the others.
 */
function EffectPicker({ value, candidates, onPick }: {
  value: string;
  candidates: { id: number; star?: boolean }[];
  onPick: (id: number | null) => void;
}) {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState("");
  const box = useRef<HTMLDivElement>(null);
  const current = parseEffectId(value);
  const label = current === null ? value : current === EMPTY_EFFECT ? "" : hex(current) + " " + effectLabel(current);
  const shown = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    const list = needle
      ? candidates.filter(candidate => (hex(candidate.id) + " " + effectLabel(candidate.id)).toLowerCase().includes(needle))
      : candidates;
    return list.slice(0, 400);
  }, [candidates, filter]);
  useEffect(() => {
    if (!open) return;
    const close = (event: MouseEvent) => {
      if (box.current && !box.current.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);
  return (
    <div className="effect-picker" ref={box}>
      <input
        value={open ? filter : label}
        placeholder={open ? "输入名称或 ID 筛选" : "（空）"}
        onFocus={() => { setFilter(""); setOpen(true); }}
        onChange={event => setFilter(event.target.value)}
        onKeyDown={event => {
          if (event.key === "Escape") setOpen(false);
          if (event.key === "Enter" && shown[0]) { onPick(shown[0].id); setOpen(false); }
        }}
      />
      {open && (
        <ul className="effect-picker-list" role="listbox">
          <li><button type="button" onMouseDown={() => { onPick(EMPTY_EFFECT); setOpen(false); }}>（空）</button></li>
          {shown.map(candidate => (
            <li key={candidate.id}>
              <button type="button" className={candidate.id === current ? "current" : ""}
                onMouseDown={() => { onPick(candidate.id); setOpen(false); }}>
                {candidate.star ? "✦ " : ""}{hex(candidate.id)} {effectLabel(candidate.id)}
              </button>
            </li>
          ))}
          {shown.length === 0 && <li className="effect-picker-empty">没有匹配的词条</li>}
        </ul>
      )}
    </div>
  );
}

const valueCache = new Map<string, Promise<EffectValues | null>>();
function legalValues(effectId: number, rarity: number, level: number): Promise<EffectValues | null> {
  const key = effectId + ":" + rarity + ":" + level;
  let pending = valueCache.get(key);
  if (!pending) {
    pending = window.operations
      .execute({ method: "runtime.effect_values", params: { effect_id: effectId, rarity, level } })
      .then(result => (result && "values" in result ? (result as EffectValues) : null))
      .catch(() => null);
    valueCache.set(key, pending);
  }
  return pending;
}

export function CharacterEditor() {
  const [mode, setMode] = useState<Mode>("live");
  const [character, setCharacter] = useState<Character | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [currencyDraft, setCurrencyDraft] = useState<Record<Currency, string>>({ amrita: "", gold: "" });
  const [query, setQuery] = useState("");
  const [major, setMajor] = useState("");
  const [middle, setMiddle] = useState("");
  const [minor, setMinor] = useState("");
  const [selected, setSelected] = useState<number | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [modded, setModded] = useState(false);
  const [rules, setRules] = useState<EquipmentRules | null>(null);
  const [values, setValues] = useState<(EffectValues | null)[]>([]);
  const [plan, setPlan] = useState<{ plan_id: string; preview: Record<string, unknown> } | null>(null);
  const [catalog, setCatalog] = useState<ActiveLocalCatalog | null>(null);
  const save = useSyncExternalStore(
    saveSession ? saveSession.subscribe : () => () => {},
    saveSession ? saveSession.getSnapshot : () => null,
  );

  const row = character?.equipment.find(entry => entry.slot_index === selected) ?? null;
  const groups = useMemo(() => {
    const tree = new Map<string, Map<string, Set<string>>>();
    for (const entry of character?.equipment ?? []) {
      const [a, b, c] = itemGroups(entry.item_id, entry.type_class);
      if (!tree.has(a)) tree.set(a, new Map());
      const middles = tree.get(a)!;
      if (!middles.has(b)) middles.set(b, new Set());
      if (c) middles.get(b)!.add(c);
    }
    return tree;
  }, [character]);
  const rows = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const all = (character?.equipment ?? []).filter(entry => {
      const [a, b, c] = itemGroups(entry.item_id, entry.type_class);
      return (!major || a === major) && (!middle || b === middle) && (!minor || c === minor);
    });
    if (!needle) return all.slice(0, 300);
    return all
      .filter(entry => {
        const name = itemName(entry.item_id, catalog) + " " + itemGroups(entry.item_id, entry.type_class).join(" ");
        const text = [hex(entry.item_id), name, ...entry.effects.map(effect => effectLabel(effect.effect_id))].join(" ");
        return text.toLowerCase().includes(needle);
      })
      .slice(0, 300);
  }, [character, query, catalog, major, middle, minor]);

  const rarity = draft ? parseAmount(draft.rarity, 255) : null;
  const level = draft ? parseAmount(draft.level, 65535) : null;

  // Rules follow the item, rarity and level being edited.
  useEffect(() => {
    setRules(null);
    if (!row || rarity === null || level === null) return;
    let live = true;
    window.operations
      .execute({ method: "runtime.equipment_rules", params: { item_id: row.item_id, rarity, level, hell: row.hell ?? false } })
      .then(result => { if (live && result && "known" in result) setRules(result as EquipmentRules); })
      .catch(() => {});
    return () => { live = false; };
  }, [row?.slot_index, row?.item_id, rarity, level]);

  // Legal values of every chosen effect, for the value pickers.
  const effectKey = draft?.effects.map(effect => effect.id).join(",") ?? "";
  useEffect(() => {
    if (!draft || rarity === null || level === null || rarity > 5) { setValues([]); return; }
    let live = true;
    Promise.all(draft.effects.map(effect => {
      const id = parseEffectId(effect.id);
      return id === null || id === EMPTY_EFFECT || id > 0xffff ? Promise.resolve(null) : legalValues(id, rarity, level);
    })).then(list => { if (live) setValues(list); });
    return () => { live = false; };
  }, [effectKey, rarity, level]);

  /** Candidates for one slot; `natural` ignores the modded switch. */
  function candidatesFor(index: number, natural = false): { id: number; star?: boolean }[] {
    if (!rules?.known || !rules.roles) return natural ? [] : ALL_EFFECTS.map(id => ({ id }));
    if (modded && !natural) return ALL_EFFECTS.map(id => ({ id }));
    const role = rules.roles[index];
    if (role === "innate") return (rules.innate ?? []).map(id => ({ id }));
    if (role === "set") return rules.set_effect == null ? [] : [{ id: rules.set_effect }];
    if (role === "grace") return (rules.graces ?? []).map(id => ({ id }));
    if (role === "hell") return (rules.hell_pool ?? []).map(id => ({ id }));
    if (role === "random") return (rules.random_pool ?? []).map(entry => ({ id: entry.effect_id, star: entry.star }));
    return [];
  }

  function setEffect(index: number, next: Partial<DraftEffect>) {
    if (!draft) return;
    const effects = draft.effects.slice();
    effects[index] = { ...effects[index], ...next };
    setDraft({ ...draft, effects });
  }

  function pickEffect(index: number, id: number | null) {
    if (id === null) return;
    const star = rules?.random_pool?.find(entry => entry.effect_id === id)?.star ?? null;
    setEffect(index, { id: id === EMPTY_EFFECT ? "" : hex(id), roll: null, star: id === EMPTY_EFFECT ? null : star });
  }

  function pickValue(index: number, value: LegalValue) {
    setEffect(index, { value: String(value.value), roll: value.roll_max, star: values[index]?.star ?? null });
  }

  function maximize() {
    if (!draft) return;
    const effects = draft.effects.map((effect, index) => {
      const list = values[index]?.values;
      const role = rules?.roles?.[index];
      if (!list?.length || role === "set" || role === "grace") return effect;
      const best = list[list.length - 1];
      return { ...effect, value: String(best.value), roll: best.roll_max, star: values[index]?.star ?? null };
    });
    setDraft({ ...draft, effects });
  }

  /** Client-side notes for the draft; the saved record is re-audited on reload. */
  const draftNotes = useMemo(() => {
    if (!draft || !rules?.known) return [] as string[];
    const notes: string[] = [];
    const used = draft.effects.filter(effect => parseEffectId(effect.id) !== EMPTY_EFFECT).length;
    if (rules.roles && used !== rules.roles.length) notes.push("词条数量与该稀有度不符" + " (" + used + "/" + rules.roles.length + ")");
    draft.effects.forEach((effect, index) => {
      const id = parseEffectId(effect.id);
      if (id === null || id === EMPTY_EFFECT) return;
      const natural = candidatesFor(index, true);
      if (!natural.some(candidate => candidate.id === id))
        notes.push("#" + (index + 1) + " " + "该词条不会自然出现在这个位置");
      const legal = values[index]?.values;
      const role = rules.roles?.[index];
      if (role === "set" || role === "grace") return;
      if (legal && !legal.some(entry => String(entry.value) === effect.value.trim()))
        notes.push("#" + (index + 1) + " " + "数值不是自然生成能出现的值");
    });
    return notes;
  }, [draft, rules, values, modded]);

  function adopt(next: Character) {
    setCharacter(next);
    setCurrencyDraft({
      amrita: next.currencies.amrita == null ? "" : String(next.currencies.amrita),
      gold: next.currencies.gold == null ? "" : String(next.currencies.gold),
    });
    const kept = next.equipment.find(entry => entry.slot_index === selected);
    setDraft(kept ? draftOf(kept) : null);
    if (!kept) setSelected(null);
  }

  async function run(task: () => Promise<void>) {
    setBusy(true);
    setMessage("");
    try {
      await task();
    } catch (error) {
      setMessage(String(error instanceof Error ? error.message : error));
    } finally {
      setBusy(false);
    }
  }

  function load(target: Mode = mode) {
    return run(async () => {
      setPlan(null);
      if (target === "live") {
        const result = await window.operations.execute({ method: "runtime.character_snapshot", params: {} });
        if (!result || !("source" in result) || result.source !== "runtime") throw new Error("UNEXPECTED_CHARACTER_SNAPSHOT");
        adopt({ ...result, mode: "live" });
      } else {
        const selectedSave = saveSession?.getSnapshot().selected;
        if (!selectedSave) throw new Error("请先选择存档。");
        const result = await saveObserver!.run(() =>
          window.operations.execute({ method: "save.character", params: { save_id: selectedSave.save_id } }),
        );
        if (!result || !("equipment_slots" in result) || !("save_id" in result)) throw new Error("UNEXPECTED_CHARACTER_SNAPSHOT");
        adopt({ ...(result as SaveCharacter), mode: "save" });
      }
    });
  }

  function switchMode(next: Mode) {
    setMode(next);
    setCharacter(null);
    setSelected(null);
    setDraft(null);
    setPlan(null);
    setMessage("");
  }

  async function submit(edit: { currencies?: Record<string, number>; equipment?: { slot_index: number; patch: Record<string, unknown> }[] }) {
    if (!character) return;
    if (character.mode === "live") {
      const live = character as LiveCharacter & { mode: Mode };
      const equipment = (edit.equipment ?? []).map(item => ({
        ...item,
        expected_record_sha256: live.equipment.find(entry => entry.slot_index === item.slot_index)?.record_sha256 ?? "",
      }));
      const expected: Record<string, number> = {};
      for (const key of Object.keys(edit.currencies ?? {})) expected[key] = Number(live.currencies[key as Currency]);
      const result = await runtimeObserver!.run(() =>
        window.operations.execute({
          method: "runtime.character_edit",
          params: {
            process_id: live.process_id,
            ...(edit.currencies ? { currencies: edit.currencies, expected_currencies: expected } : {}),
            ...(equipment.length ? { equipment } : {}),
          },
        }),
      );
      const outcome = result && "character_edit" in result ? result.character_edit : null;
      if (!outcome) throw new Error("UNEXPECTED_CHARACTER_EDIT");
      if (outcome.state === "verified") setMessage("已写入游戏。到神社存档即可保存到存档文件。");
      else if (outcome.state === "rejected") setMessage("没有写入：" + (outcome.error ?? ""));
      else setMessage("写入结果不确定，请重新读取后核对：" + (outcome.error ?? ""));
      const refreshed = await window.operations.execute({ method: "runtime.character_snapshot", params: {} });
      if (refreshed && "source" in refreshed && refreshed.source === "runtime") adopt({ ...refreshed, mode: "live" });
    } else {
      const result = await saveSession!.prepareCharacterEdit(edit);
      if (!("plan_id" in result)) throw new Error("UNEXPECTED_CHARACTER_PLAN");
      setPlan({ plan_id: result.plan_id, preview: result.preview as Record<string, unknown> });
      setMessage("已生成修改计划。核对下方内容后点击“写入存档”。游戏必须关闭。");
    }
  }

  function applyCurrency(key: Currency) {
    return run(async () => {
      if (!character) return;
      const value = parseAmount(currencyDraft[key], 9007199254740991);
      if (value === null) throw new Error("请输入有效的数值。");
      if (value === character.currencies[key]) throw new Error("没有需要修改的内容。");
      await submit({ currencies: { [key]: value } });
    });
  }

  function applyEquipment() {
    return run(async () => {
      if (!row || !draft) return;
      if (draftNotes.length && !modded) throw new Error("当前修改不符合自然规则。如需保留，请切换到“魔改”。");
      const result = patchOf(row, draft);
      if ("error" in result) throw new Error(result.error);
      if (!Object.keys(result.patch).length) throw new Error("没有需要修改的内容。");
      await submit({ equipment: [{ slot_index: row.slot_index, patch: result.patch }] });
    });
  }

  function commitPlan() {
    return run(async () => {
      if (!plan) return;
      const receipt = await saveSession!.commit(plan.plan_id);
      setPlan(null);
      setMessage(
        receipt.commit_status === "committed"
          ? "已写入存档，并已自动备份原存档。"
          : "写入结果不确定，请在备份与管理中核对操作结果。",
      );
      await saveSession!.refresh();
      await load("save");
    });
  }

  if (!desktop) return <main className="equipment-page"><Notice text="请在桌面版中使用此功能。" /></main>;
  const middles = major ? [...(groups.get(major)?.keys() ?? [])].filter(Boolean) : [];
  const minors = major ? [...(groups.get(major)?.get(middle)?.values() ?? (middle ? [] : [...(groups.get(major)?.values() ?? [])].flatMap(set => [...set])))] : [];
  return (
    <main className="equipment-page character-page">
      <div className="character-modes" role="tablist">
        <button className={mode === "live" ? "active" : ""} onClick={() => switchMode("live")} disabled={busy}>游戏内实时修改</button>
        <button className={mode === "save" ? "active" : ""} onClick={() => switchMode("save")} disabled={busy}>修改存档文件</button>
      </div>
      <p className="equipment-description">
        {mode === "live"
          ? "直接修改正在运行的游戏，需要先读档进入游戏。修改后到神社存档即可保存。"
          : "修改存档文件，游戏必须关闭。写入前会自动备份原存档。"}
      </p>
      {mode === "save" && <SavePicker compact />}
      <div className="equipment-toolbar">
        <button onClick={() => load()} disabled={busy || (mode === "save" && !save?.selected)}>
          {character ? "重新读取" : "读取角色"}
        </button>
      </div>
      <Notice text={message} />
      {character && (
        <>
          <section className="character-currencies">
            <h3>货币</h3>
            {(["amrita", "gold"] as const).map(key => (
              <label key={key}>
                <span>{CURRENCY_LABEL[key]}</span>
                <input inputMode="numeric" value={currencyDraft[key]} onChange={event => setCurrencyDraft({ ...currencyDraft, [key]: event.target.value })} />
                <button onClick={() => applyCurrency(key)} disabled={busy}>{key === "amrita" ? "修改精华" : "修改金钱"}</button>
              </label>
            ))}
          </section>
          <section className="character-equipment">
            <h3>装备</h3>
            <div className="equipment-toolbar">
              <label className="equipment-search">
                <span>搜索</span>
                <input value={query} onChange={event => setQuery(event.target.value)} placeholder="物品 ID、名称或词条" />
              </label>
              <label className="equipment-search">
                <span>大类</span>
                <select value={major} onChange={event => { setMajor(event.target.value); setMiddle(""); setMinor(""); }}>
                  <option value="">全部</option>
                  {[...groups.keys()].map(value => <option key={value} value={value}>{value}</option>)}
                </select>
              </label>
              {middles.length > 0 && (
                <label className="equipment-search">
                  <span>中类</span>
                  <select value={middle} onChange={event => { setMiddle(event.target.value); setMinor(""); }}>
                    <option value="">全部</option>
                    {middles.map(value => <option key={value} value={value}>{value}</option>)}
                  </select>
                </label>
              )}
              {minors.length > 0 && (
                <label className="equipment-search">
                  <span>小类</span>
                  <select value={minor} onChange={event => setMinor(event.target.value)}>
                    <option value="">全部</option>
                    {[...new Set(minors)].map(value => <option key={value} value={value}>{value}</option>)}
                  </select>
                </label>
              )}
              <span className="equipment-range">
                {character.equipment.length} / {character.equipment_slots}
              </span>
            </div>
            <details className="character-names">
              <summary>补充物品名称（可选）</summary>
              <LocalCatalogImport onCatalogChange={setCatalog} />
            </details>
            <table className="equipment-table">
              <thead>
                <tr><th>槽位</th><th>类别</th><th>物品</th><th>等级</th><th>+值</th><th>稀有度</th><th>词条</th><th>自然</th></tr>
              </thead>
              <tbody>
                {rows.map(entry => {
                  const [a, b, c] = itemGroups(entry.item_id, entry.type_class);
                  const audit = entry.audit;
                  return (
                    <tr key={entry.slot_index} className={entry.slot_index === selected ? "selected" : ""}>
                      <td><button onClick={() => { setSelected(entry.slot_index); setDraft(draftOf(entry)); setModded(false); }}>{entry.slot_index}</button></td>
                      <td>{c || b || a}</td>
                      <td>{itemName(entry.item_id, catalog) ? <span className="equipment-item-name character-item-name">{itemName(entry.item_id, catalog)}</span> : <code>{hex(entry.item_id)}</code>}</td>
                      <td>{entry.level}</td>
                      <td>{entry.plus}</td>
                      <td>{entry.rarity}</td>
                      <td>{entry.effects.filter(effect => effect.effect_id !== EMPTY_EFFECT).map(effect => effectLabel(effect.effect_id)).join("、")}</td>
                      <td title={audit ? audit.findings.map(findingText).join("\n") : "规则未载入"}>
                        {audit == null ? "—" : audit.natural ? "自然" : <span className="character-unnatural">非自然</span>}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
            {row && draft && (
              <div className="equipment-detail character-detail">
                <h3>{"编辑装备"}{itemName(row.item_id, catalog) ? "：" + itemName(row.item_id, catalog) : ""}</h3>
                {row.audit && !row.audit.natural && (
                  <ul className="character-findings">
                    {row.audit.findings.map((finding, index) => <li key={index}>{findingText(finding)}</li>)}
                  </ul>
                )}
                <div className="character-edit-modes" role="tablist">
                  <button className={!modded ? "active" : ""} onClick={() => setModded(false)}>合法修改</button>
                  <button className={modded ? "active" : ""} onClick={() => setModded(true)}>魔改</button>
                </div>
                <p className="equipment-notes">
                  {modded
                    ? "魔改：任何词条、任何数值都可以填写，不受游戏生成规则约束，结果可能无法自然获得。"
                    : "合法修改：每个位置只列出这件装备能自然出现的词条，数值从自然生成能出现的值中选择。"}
                </p>
                <div className="character-fields">
                  {FIELD_LABEL.map(([key, label]) => (
                    <label key={key}>
                      <span>{label}</span>
                      <input inputMode="numeric" value={draft[key as keyof Draft] as string} onChange={event => setDraft({ ...draft, [key]: event.target.value })} />
                    </label>
                  ))}
                </div>
                <table className="equipment-effects">
                  <thead><tr><th>#</th><th>位置</th><th>词条</th><th>数值</th><th>当前</th></tr></thead>
                  <tbody>
                    {draft.effects.map((effect, index) => {
                      const role = rules?.roles?.[index];
                      const legal = values[index]?.values ?? [];
                      const chosen = legal.find(entry => String(entry.value) === effect.value.trim());
                      return (
                        <tr key={index}>
                          <td>{index + 1}</td>
                          <td>{role ? ROLE_LABEL[role] ?? role : "—"}</td>
                          <td><EffectPicker value={effect.id} candidates={candidatesFor(index)} onPick={id => pickEffect(index, id)} /></td>
                          <td>
                            {!modded && legal.length > 0 && role !== "set" && role !== "grace" ? (
                              <select value={chosen ? String(chosen.value) : ""} onChange={event => {
                                const next = legal.find(entry => String(entry.value) === event.target.value);
                                if (next) pickValue(index, next);
                              }}>
                                {!chosen && <option value="">{effect.value}（非自然）</option>}
                                {legal.slice().reverse().map(entry => (
                                  <option key={entry.value} value={String(entry.value)}>
                                    {entry.value + " （前 " + percent(entry.top_fraction) + "）"}
                                  </option>
                                ))}
                              </select>
                            ) : (
                              <input inputMode="numeric" value={effect.value} onChange={event => setEffect(index, { value: event.target.value, roll: null })} />
                            )}
                          </td>
                          <td>{effectLabel(row.effects[index]?.effect_id ?? EMPTY_EFFECT)}</td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
                <p className="equipment-notes">
                  原始数值按游戏内部单位填写，例如百分比词条 15 表示 1.5%。“前 X%”表示自然生成时得到这个值或更好值的概率。
                </p>
                {draftNotes.length > 0 && (
                  <ul className="character-findings">{draftNotes.map(note => <li key={note}>{note}</li>)}</ul>
                )}
                <div className="equipment-toolbar">
                  <button onClick={maximize} disabled={busy || !values.some(Boolean)}>全部取理论最高</button>
                  <button onClick={applyEquipment} disabled={busy}>{modded ? "写入魔改" : "写入"}</button>
                </div>
              </div>
            )}
          </section>
          {plan && (
            <section className="character-plan">
              <h3>修改计划</h3>
              <PlanPreview preview={plan.preview} />
              <button onClick={commitPlan} disabled={busy}>写入存档</button>
              <button onClick={() => { setPlan(null); void saveSession!.discard(); }} disabled={busy}>放弃</button>
            </section>
          )}
        </>
      )}
    </main>
  );
}
