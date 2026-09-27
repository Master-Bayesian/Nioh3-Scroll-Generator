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
type Finding = NonNullable<CharacterEquipment["audit"]>["findings"][number];
interface Candidate {
  id: number;
  star?: boolean;
  min?: number;
  max?: number;
}
const EMPTY_EFFECT = 0xffffffff;
const PAGE_SIZE = 40;

const effectNames = new Map<number, string>();
for (const row of data.editorEffects as { id: string; name: string }[]) {
  const id = Number(row.id);
  if (Number.isInteger(id) && row.name && !effectNames.has(id))
    effectNames.set(
      id,
      // The game fills the remaining "{}" from the item; the record does not say with what.
      fillTemplateSlots(plainGameText(row.name), "增益效果", "异常状态").replaceAll("{}", "（特定对象）"),
    );
}
const ALL_EFFECTS: Candidate[] = [...effectNames.keys()].map(id => ({ id }));

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

function hex(value: number) {
  return "0x" + value.toString(16).toUpperCase().padStart(4, "0");
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
function rangeText(min?: number, max?: number) {
  if (min == null || max == null || (min === 0 && max === 0)) return "";
  return min === max ? String(min) : min + "–" + max;
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
  missing_innate: "缺少该物品自带的固有词条",
  missing_set: "缺少该物品自带的套装效果",
  missing_grace: "缺少恩宠效果",
  unexpected_fixed: "多出不该有的套装或恩宠效果",
  not_in_pool: "该词条不会出现在这类装备上",
  hell_effect_on_normal: "地狱词条只会出现在地狱武器上",
  missing_hell_effect: "地狱武器缺少地狱词条",
  hell_on_ineligible_item: "该物品不能成为地狱武器",
  star_below_rarity: "星号词条需要更高稀有度",
  multiple_stars: "星号词条超过一条",
  star_flag_mismatch: "星号标记与词条不一致",
  group_conflict: "与另一条词条互斥",
  value_not_natural: "数值不是自然生成能出现的值",
  value_above_formula: "数值高于公式（可能有额外加成）",
  roll_out_of_range: "分位超出该稀有度范围",
};
const VERDICT_LABEL: Record<string, string> = { natural: "自然", unverified: "待确认", unnatural: "非自然" };

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
    if (id === null || value === null) return { error: "请输入有效的词条和数值。" };
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

/**
 * Effect picker: opening it always lists every candidate; only text typed after
 * opening filters the list, so the current effect never hides the others.
 */
function EffectPicker({ value, candidates, label, onPick }: {
  value: string;
  candidates: Candidate[];
  label: (candidate: Candidate) => string;
  onPick: (candidate: Candidate) => void;
}) {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState("");
  const box = useRef<HTMLDivElement>(null);
  const current = parseEffectId(value);
  const currentLabel =
    current === null ? value : current === EMPTY_EFFECT ? "" : label(candidates.find(candidate => candidate.id === current) ?? { id: current });
  const shown = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    const list = needle
      ? candidates.filter(candidate => (label(candidate) + " " + hex(candidate.id)).toLowerCase().includes(needle))
      : candidates;
    return list.slice(0, 400);
  }, [candidates, filter, label]);
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
        value={open ? filter : currentLabel}
        placeholder={open ? "输入名称筛选" : "（空）"}
        onFocus={() => { setFilter(""); setOpen(true); }}
        onChange={event => setFilter(event.target.value)}
        onKeyDown={event => {
          if (event.key === "Escape") setOpen(false);
          if (event.key === "Enter" && shown[0]) { onPick(shown[0]); setOpen(false); }
        }}
      />
      {open && (
        <ul className="effect-picker-list" role="listbox">
          <li><button type="button" onMouseDown={() => { onPick({ id: EMPTY_EFFECT }); setOpen(false); }}>（空）</button></li>
          {shown.map(candidate => (
            <li key={candidate.id}>
              <button type="button" className={candidate.id === current ? "current" : ""}
                onMouseDown={() => { onPick(candidate); setOpen(false); }}>
                {label(candidate)}
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

export function CharacterEditor({ showIds = false }: { showIds?: boolean }) {
  const [mode, setMode] = useState<Mode>("live");
  const [character, setCharacter] = useState<Character | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [currencyDraft, setCurrencyDraft] = useState<Record<Currency, string>>({ amrita: "", gold: "" });
  const [query, setQuery] = useState("");
  const [major, setMajor] = useState("");
  const [middle, setMiddle] = useState("");
  const [minor, setMinor] = useState("");
  const [page, setPage] = useState(0);
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

  const effectText = (id: number) => {
    if (id === EMPTY_EFFECT) return "（空）";
    const name = effectNames.get(id);
    if (!name) return hex(id);
    return showIds ? name + " " + hex(id) : name;
  };
  const itemText = (id: number) => {
    const name = itemCatalog[String(id)]?.[0] || catalog?.entries.get(id) || "";
    if (!name) return "未收录物品 " + hex(id);
    return showIds ? name + " " + hex(id) : name;
  };
  const candidateLabel = (candidate: Candidate) => {
    const range = rangeText(candidate.min, candidate.max);
    return (candidate.star ? "✦ " : "") + effectText(candidate.id) + (range ? "（" + range + "）" : "");
  };

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
  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return (character?.equipment ?? []).filter(entry => {
      const [a, b, c] = itemGroups(entry.item_id, entry.type_class);
      if ((major && a !== major) || (middle && b !== middle) || (minor && c !== minor)) return false;
      if (!needle) return true;
      const text = [itemText(entry.item_id), a, b, c, ...entry.effects.map(effect => effectText(effect.effect_id))].join(" ");
      return text.toLowerCase().includes(needle);
    });
  }, [character, query, catalog, major, middle, minor, showIds]);
  const pages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const shownPage = Math.min(page, pages - 1);
  const rows = filtered.slice(shownPage * PAGE_SIZE, (shownPage + 1) * PAGE_SIZE);
  useEffect(() => setPage(0), [query, major, middle, minor]);

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

  // Legal values of every chosen effect.
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

  /** Natural candidates for one slot, or every effect in modded mode. */
  function candidatesFor(index: number, natural = false): Candidate[] {
    if (!rules?.known || !rules.roles) return natural ? [] : ALL_EFFECTS;
    if (modded && !natural) return ALL_EFFECTS;
    const role = rules.roles[index];
    let list: Candidate[] = [];
    if (role === "innate") list = (rules.innate ?? []).map(id => ({ id }));
    else if (role === "set") list = rules.set_effect == null ? [] : [{ id: rules.set_effect }];
    else if (role === "grace") list = (rules.graces ?? []).map(id => ({ id }));
    else if (role === "hell") list = (rules.hell_pool ?? []).map(entry => ({ id: entry.effect_id, min: entry.min, max: entry.max }));
    else if (role === "random")
      list = (rules.random_pool ?? []).map(entry => ({ id: entry.effect_id, star: entry.star, min: entry.min, max: entry.max }));
    if (natural) return list;
    // Same name, marker and range read identically; keep one of them.
    const seen = new Set<string>();
    return list.filter(candidate => {
      const key = effectNames.get(candidate.id) + "|" + candidate.star + "|" + candidate.min + "|" + candidate.max;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  }

  function setEffect(index: number, next: Partial<DraftEffect>) {
    if (!draft) return;
    const effects = draft.effects.slice();
    effects[index] = { ...effects[index], ...next };
    setDraft({ ...draft, effects });
  }

  function pickEffect(index: number, candidate: Candidate) {
    const id = candidate.id;
    setEffect(index, {
      id: id === EMPTY_EFFECT ? "" : hex(id),
      value: candidate.max != null && !modded ? String(candidate.max) : draft?.effects[index].value ?? "",
      roll: null,
      star: id === EMPTY_EFFECT ? null : candidate.star ?? null,
    });
  }

  /** Match a typed value to its roll so a legal write stays consistent. */
  function chooseValue(index: number, text: string) {
    const legal = values[index]?.values ?? [];
    const match = legal.find(entry => String(entry.value) === text.trim());
    setEffect(index, { value: text, roll: match ? match.roll_max : null, star: values[index]?.star ?? null });
  }

  function best(index: number): LegalValue | undefined {
    const list = values[index]?.values;
    return list?.length ? list[list.length - 1] : undefined;
  }

  function maximize() {
    if (!draft) return;
    const effects = draft.effects.map((effect, index) => {
      const role = rules?.roles?.[index];
      const top = best(index);
      if (!top || role === "set" || role === "grace") return effect;
      return { ...effect, value: String(top.value), roll: top.roll_max, star: values[index]?.star ?? null };
    });
    setDraft({ ...draft, effects });
  }

  /** Why the draft is not natural; empty when it is. */
  const draftNotes = useMemo(() => {
    if (!draft || !rules?.known) return [] as string[];
    const notes: string[] = [];
    const used = draft.effects.filter(effect => parseEffectId(effect.id) !== EMPTY_EFFECT).length;
    if (rules.roles && used !== rules.roles.length) notes.push("词条数量与该稀有度不符" + " (" + used + "/" + rules.roles.length + ")");
    draft.effects.forEach((effect, index) => {
      const id = parseEffectId(effect.id);
      if (id === null || id === EMPTY_EFFECT) return;
      if (!candidatesFor(index, true).some(candidate => candidate.id === id))
        notes.push("#" + (index + 1) + " " + "该词条不会自然出现在这个位置");
      const role = rules.roles?.[index];
      if (role === "set" || role === "grace") return;
      const legal = values[index]?.values;
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

  function planLines(preview: Record<string, unknown>) {
    const currencies = (preview.currencies ?? []) as { currency: string; before: number; after: number }[];
    const equipment = (preview.equipment ?? []) as { slot_index: number; before: CharacterEquipment; after: CharacterEquipment }[];
    const lines: string[] = [];
    for (const change of currencies)
      lines.push((CURRENCY_LABEL[change.currency] ?? change.currency) + "：" + change.before + " → " + change.after);
    for (const change of equipment) {
      const prefix = itemText(change.before.item_id);
      for (const [key, label] of FIELD_LABEL)
        if (change.before[key] !== change.after[key])
          lines.push(prefix + " " + label + "：" + change.before[key] + " → " + change.after[key]);
      change.after.effects.forEach((effect, index) => {
        const before = change.before.effects[index];
        if (!before || before.effect_id !== effect.effect_id || before.value !== effect.value)
          lines.push(prefix + " #" + (index + 1) + "：" +
            effectText(before?.effect_id ?? EMPTY_EFFECT) + " " + (before?.value ?? 0) + " → " +
            effectText(effect.effect_id) + " " + effect.value);
      });
    }
    return lines;
  }

  function findingText(finding: Finding) {
    return (finding.slot == null ? "" : "#" + (finding.slot + 1) + " ") + (FINDING_LABEL[finding.code] ?? finding.code);
  }
  function verdictCell(entry: CharacterEquipment) {
    const audit = entry.audit;
    if (!audit) return <span title="规则未载入">—</span>;
    const verdict = audit.verdict ?? (audit.natural ? "natural" : "unnatural");
    const reasons = [...audit.findings, ...(audit.unverified ?? [])].map(findingText).join("\n");
    return <span className={"character-verdict " + verdict} title={reasons}>{VERDICT_LABEL[verdict]}</span>;
  }

  if (!desktop) return <main className="equipment-page"><Notice text="请在桌面版中使用此功能。" /></main>;
  const middles = major ? [...(groups.get(major)?.keys() ?? [])].filter(Boolean) : [];
  const minors = major
    ? [...new Set(middle
        ? [...(groups.get(major)?.get(middle) ?? [])]
        : [...(groups.get(major)?.values() ?? [])].flatMap(set => [...set]))]
    : [];
  return (
    <main className="equipment-page character-page">
      <div className="character-top">
        <div className="character-modes" role="tablist">
          <button className={mode === "live" ? "active" : ""} onClick={() => switchMode("live")} disabled={busy}>游戏内实时修改</button>
          <button className={mode === "save" ? "active" : ""} onClick={() => switchMode("save")} disabled={busy}>修改存档文件</button>
        </div>
        <button onClick={() => load()} disabled={busy || (mode === "save" && !save?.selected)}>
          {character ? "重新读取" : "读取角色"}
        </button>
        <span className="equipment-description">
          {mode === "live"
            ? "直接修改正在运行的游戏，需要先读档进入游戏。修改后到神社存档即可保存。"
            : "修改存档文件，游戏必须关闭。写入前会自动备份原存档。"}
        </span>
      </div>
      {mode === "save" && <SavePicker compact />}
      <Notice text={message} />
      {character && (
        <>
          <section className="character-currencies">
            {(["amrita", "gold"] as const).map(key => (
              <label key={key}>
                <span>{CURRENCY_LABEL[key]}</span>
                <input inputMode="numeric" value={currencyDraft[key]} onChange={event => setCurrencyDraft({ ...currencyDraft, [key]: event.target.value })} />
                <button onClick={() => applyCurrency(key)} disabled={busy}>{key === "amrita" ? "修改精华" : "修改金钱"}</button>
              </label>
            ))}
          </section>
          <section className="character-equipment">
            <div className="character-filters">
              <div className="character-chips">
                <button className={!major ? "active" : ""} onClick={() => { setMajor(""); setMiddle(""); setMinor(""); }}>全部</button>
                {[...groups.keys()].map(value => (
                  <button key={value} className={major === value ? "active" : ""} onClick={() => { setMajor(value); setMiddle(""); setMinor(""); }}>{value}</button>
                ))}
              </div>
              {middles.length > 1 && (
                <div className="character-chips small">
                  <button className={!middle ? "active" : ""} onClick={() => { setMiddle(""); setMinor(""); }}>全部</button>
                  {middles.map(value => (
                    <button key={value} className={middle === value ? "active" : ""} onClick={() => { setMiddle(value); setMinor(""); }}>{value}</button>
                  ))}
                </div>
              )}
              <div className="character-search">
                {minors.length > 0 && (
                  <select value={minor} onChange={event => setMinor(event.target.value)} aria-label="小类">
                    <option value="">全部小类</option>
                    {minors.map(value => <option key={value} value={value}>{value}</option>)}
                  </select>
                )}
                <input value={query} onChange={event => setQuery(event.target.value)} placeholder="搜索物品名称或词条" />
                <span className="equipment-range">{filtered.length} / {character.equipment.length}</span>
              </div>
            </div>
            <table className="equipment-table character-table">
              <thead>
                <tr><th>类别</th><th>物品</th><th>等级</th><th>+值</th><th>稀有度</th><th>词条</th><th>判定</th></tr>
              </thead>
              <tbody>
                {rows.map(entry => {
                  const [a, b, c] = itemGroups(entry.item_id, entry.type_class);
                  return (
                    <tr key={entry.slot_index} className={entry.slot_index === selected ? "selected" : ""}
                      onClick={() => { setSelected(entry.slot_index); setDraft(draftOf(entry)); setModded(false); }}>
                      <td>{c || b || a}{entry.hell ? <span className="character-hell">地狱</span> : null}</td>
                      <td className="character-item-name">{itemText(entry.item_id)}{showIds ? <small> #{entry.slot_index}</small> : null}</td>
                      <td>{entry.level}</td>
                      <td>{entry.plus}</td>
                      <td>{entry.rarity}</td>
                      <td className="character-effects-cell">{entry.effects.filter(effect => effect.effect_id !== EMPTY_EFFECT).map(effect => effectText(effect.effect_id)).join("、")}</td>
                      <td>{verdictCell(entry)}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
            {pages > 1 && (
              <div className="character-pager">
                <button onClick={() => setPage(Math.max(0, shownPage - 1))} disabled={shownPage === 0}>上一页</button>
                <span>{shownPage + 1} / {pages}</span>
                <button onClick={() => setPage(Math.min(pages - 1, shownPage + 1))} disabled={shownPage >= pages - 1}>下一页</button>
              </div>
            )}
            <details className="character-names">
              <summary>补充物品名称（可选）</summary>
              <LocalCatalogImport onCatalogChange={setCatalog} />
            </details>
            {row && draft && (
              <div className="equipment-detail character-detail">
                <div className="character-detail-head">
                  <h3>{itemText(row.item_id)}</h3>
                  {verdictCell(row)}
                  <div className="character-edit-modes" role="tablist">
                    <button className={!modded ? "active" : ""} onClick={() => setModded(false)}>合法修改</button>
                    <button className={modded ? "active" : ""} onClick={() => setModded(true)}>魔改</button>
                  </div>
                </div>
                <p className="equipment-notes">
                  {modded
                    ? "魔改：任何词条、任何数值都可以填写，不受游戏生成规则约束，结果可能无法自然获得。"
                    : "合法修改：每个位置只列出这件装备能自然出现的词条，数值填在合法范围内。"}
                </p>
                <div className="character-fields">
                  {FIELD_LABEL.map(([key, label]) => (
                    <label key={key}>
                      <span>{label}</span>
                      <input inputMode="numeric" value={draft[key as keyof Draft] as string} onChange={event => setDraft({ ...draft, [key]: event.target.value })} />
                    </label>
                  ))}
                </div>
                <table className="equipment-effects character-effects">
                  <thead><tr><th>位置</th><th>词条</th><th>数值</th><th>合法范围</th></tr></thead>
                  <tbody>
                    {draft.effects.map((effect, index) => {
                      const role = rules?.roles?.[index];
                      const legal = values[index]?.values ?? [];
                      const fixed = role === "set" || role === "grace";
                      const chosen = legal.find(entry => String(entry.value) === effect.value.trim());
                      const top = best(index);
                      const id = parseEffectId(effect.id);
                      return (
                        <tr key={index}>
                          <td>{role ? ROLE_LABEL[role] ?? role : "—"}</td>
                          <td><EffectPicker value={effect.id} candidates={candidatesFor(index)} label={candidateLabel} onPick={candidate => pickEffect(index, candidate)} /></td>
                          <td>
                            {id === EMPTY_EFFECT ? null : fixed ? <span className="character-muted">—</span> : (
                              <span className="character-value">
                                <input inputMode="numeric" value={effect.value} onChange={event => chooseValue(index, event.target.value)} />
                                {top && !modded ? <button type="button" onClick={() => chooseValue(index, String(top.value))}>最高</button> : null}
                              </span>
                            )}
                          </td>
                          <td className="character-range">
                            {id === EMPTY_EFFECT || fixed || !legal.length ? null : (
                              <>
                                {rangeText(legal[0].value, legal[legal.length - 1].value)}
                                {chosen
                                  ? <small>{" · 前 " + percent(chosen.top_fraction)}</small>
                                  : <small className="character-unnatural">{" · 非自然值"}</small>}
                              </>
                            )}
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
                <p className="equipment-notes">
                  数值按游戏内部单位填写，例如百分比词条 15 表示 1.5%。“前 X%”表示自然生成时得到这个值或更好值的概率。
                </p>
                {draftNotes.length > 0 && (
                  <ul className="character-findings">{draftNotes.map(note => <li key={note}>{note}</li>)}</ul>
                )}
                <div className="character-actions">
                  <button onClick={maximize} disabled={busy || !values.some(Boolean)}>全部取理论最高</button>
                  <button onClick={applyEquipment} disabled={busy}>{modded ? "写入魔改" : "写入"}</button>
                </div>
              </div>
            )}
          </section>
          {plan && (
            <section className="character-plan">
              <h3>修改计划</h3>
              <ul className="character-plan-lines">{planLines(plan.preview).map(line => <li key={line}>{line}</li>)}</ul>
              <button onClick={commitPlan} disabled={busy}>写入存档</button>
              <button onClick={() => { setPlan(null); void saveSession!.discard(); }} disabled={busy}>放弃</button>
            </section>
          )}
        </>
      )}
    </main>
  );
}
