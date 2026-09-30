import React, { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import type {
  CharacterEquipment,
  CharacterItem,
  EffectValues,
  EquipmentRules,
  EquipmentSeeds,
  LiveCharacter,
  SaveCharacter,
} from "../../packages/contracts/protected-responses";
import { data } from "./model";
import itemNames from "./item-names.json";
import hellSkillNames from "./hell-skill-names.json";
import effectSources from "./effect-sources.json";
import { desktop } from "./desktop-bridge";
import { fillTemplateSlots, plainGameText } from "./game-text";
import { Notice } from "./Notice";
import { LiveEquipmentAdd } from "./LiveEquipmentAdd";
import { errorText, publicError, stripErrorPrefix } from "./public-errors";
import { SavePicker } from "./CartActions";
import { runtimeObserver, saveObserver, saveSession } from "./save-workspace";

type Mode = "live" | "save";
type Tab = "equipment" | "items";
/** Change what the character already has, or add new equipment. */
type Section = "edit" | "add";
type Character = (LiveCharacter | SaveCharacter) & { mode: Mode };
type Currency = "amrita" | "gold";
type Container = CharacterItem["container"];
/** One item id with its held and stored stacks. */
interface ItemRow {
  item_id: number;
  held?: CharacterItem;
  storage?: CharacterItem;
}
/** Hell martial-skill names by skill id (PC v2.02, read from the game). */
const HELL_SKILL_NAMES: Record<string, string> = hellSkillNames.skills;
const CONTAINER_LABEL: Record<Container, string> = { held: "持有", storage: "仓库" };
type LegalValue = EffectValues["values"][number];
type Finding = NonNullable<CharacterEquipment["audit"]>["findings"][number];
interface Candidate {
  id: number;
  star?: boolean;
  min?: number;
  max?: number;
  /** Group and conflict masks of a drawn (random or hell) candidate. */
  group?: number;
  masks?: number[];
}
/** The generator's exclusion: drawn effects sharing a group or any mask bit never appear together. */
function excludes(a: Candidate, b: Candidate) {
  if (a.group == null || b.group == null || !a.masks || !b.masks) return false;
  return a.group === b.group || (a.masks[0] & b.masks[0]) !== 0 || (a.masks[1] & b.masks[1]) !== 0;
}
const EMPTY_EFFECT = 0xffffffff;
const PAGE_SIZE = 40;
/** One item to add to a save (`save.prepare_character_edit` `add`). */
interface NewEquipmentRequest {
  item_id: number;
  level: number;
  plus: number;
  rarity: number;
  hell?: boolean;
  hell_skill?: number;
  effects?: { effect_id: number; value: number; star?: boolean }[];
  /** Generate the item as the game does from this seed (legal adds). */
  seed?: number;
  difficulty?: number;
}
/** One wanted effect: ids that read the same, any of which counts. */
interface WantedEffect {
  key: string;
  ids: number[];
}
/** The selection of an item being added to the save; it has no slot yet. */
const NEW_SLOT = -1;
/** Catalog kinds a new item may be picked from; the rules decide the rest. */
const ADDABLE_KINDS = new Set(["武器", "防具", "防具或饰品", "饰品", "魂核"]);

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
/**
 * Where natural generation can put each effect row, e.g. "武器随机 75". One
 * group has a row per context and every row shows the group's name, so this
 * is what tells same-named effects apart (PC v2.02, rarity 4, level 180).
 */
const EFFECT_SOURCES: Record<string, string> = effectSources.sources;
const NO_DROP = "非装备掉落";
function sourceOf(id: number) {
  return EFFECT_SOURCES[String(id)] ?? "";
}
/** Every effect, same names together and rows no drop can carry last. */
const ALL_EFFECTS: Candidate[] = [...effectNames.keys()]
  .map(id => ({ id }))
  .sort((a, b) =>
    Number(sourceOf(a.id).startsWith(NO_DROP)) - Number(sourceOf(b.id).startsWith(NO_DROP)) ||
    (effectNames.get(a.id) ?? "").localeCompare(effectNames.get(b.id) ?? "", "zh-CN") ||
    a.id - b.id);
/** Catalog kinds each add filter chip shows. */
const ADD_KIND_CHIPS: [string, string[]][] = [
  ["武器", ["武器"]],
  ["防具", ["防具", "防具或饰品"]],
  ["饰品", ["饰品", "防具或饰品"]],
  ["魂核", ["魂核"]],
];
/** Most new items one plan adds (the request contract's limit). */
const ADD_LIMIT = 16;

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
  replaced_effect: "词条被替换过，游戏自己生成不会留下这种痕迹",
};
const VERDICT_LABEL: Record<string, string> = { natural: "自然", unverified: "待确认", unnatural: "非自然" };
const VERDICT_ORDER = ["unnatural", "unverified", "natural"];
type SortKey = "level" | "rarity";

function verdictOf(entry: CharacterEquipment): string | null {
  const audit = entry.audit;
  if (!audit) return null;
  return audit.verdict ?? (audit.natural ? "natural" : "unnatural");
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
  hell: boolean;
  hell_skill: number;
  effects: DraftEffect[];
}
function draftOf(row: CharacterEquipment): Draft {
  return {
    level: String(row.level),
    level_before_forge: String(row.level_before_forge),
    plus: String(row.plus),
    rarity: String(row.rarity),
    familiarity: String(row.familiarity),
    hell: row.hell ?? false,
    hell_skill: row.hell_skill ?? 0,
    effects: row.effects.map(effect => ({
      id: effect.effect_id === EMPTY_EFFECT ? "" : hex(effect.effect_id),
      value: String(effect.value),
      roll: null,
      star: null,
    })),
  };
}

/**
 * The star marker slot `index` will carry: an unchanged effect keeps its own,
 * and so does a slot the game re-rolls in place (`keepsMarker`, a soul core's
 * random slot); a new effect elsewhere takes the marker a drop would give it.
 */
function markerOf(row: CharacterEquipment, draft: Draft, index: number, keepsMarker: boolean, rowStar?: boolean | null) {
  const id = parseEffectId(draft.effects[index].id);
  const before = row.effects[index];
  if (id === null || id === EMPTY_EFFECT) return false;
  if (before?.effect_id === id || keepsMarker) return before?.star ?? false;
  return draft.effects[index].star ?? rowStar ?? false;
}

/** The changed fields of one record, or an error message when a field is invalid. */
function patchOf(row: CharacterEquipment, draft: Draft, keepsMarker: (index: number) => boolean = () => false) {
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
  if (draft.hell !== (row.hell ?? false)) patch.hell = draft.hell;
  if (draft.hell_skill !== (row.hell_skill ?? 0)) patch.hell_skill = draft.hell_skill;
  const effects = [];
  for (const [index, effect] of draft.effects.entries()) {
    const id = parseEffectId(effect.id);
    const value = id === EMPTY_EFFECT ? 0 : parseAmount(effect.value, 4294967295);
    if (id === null || value === null) return { error: "请输入有效的词条和数值。" };
    const before = row.effects[index];
    // An unchanged effect keeps the star marker it has: a re-rolled soul-core
    // effect is unmarked even on a star row, and the game shows it that way.
    const star = before?.effect_id === id ? null : keepsMarker(index) ? markerOf(row, draft, index, true) : effect.star;
    if (!before || before.effect_id !== id || (id !== EMPTY_EFFECT && before.value !== value) || effect.roll !== null)
      effects.push({
        index,
        effect_id: id,
        value,
        ...(effect.roll !== null && id !== EMPTY_EFFECT ? { roll: effect.roll } : {}),
        ...(star !== null && id !== EMPTY_EFFECT ? { star } : {}),
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
function EffectPicker({ value, star, candidates, label, onPick, placeholder = "（空）" }: {
  value: string;
  /** The saved star marker of the current effect, when it is unchanged. */
  star?: boolean;
  candidates: Candidate[];
  label: (candidate: Candidate) => string;
  onPick: (candidate: Candidate) => void;
  placeholder?: string;
}) {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState("");
  const box = useRef<HTMLDivElement>(null);
  const current = parseEffectId(value);
  const currentLabel =
    current === null
      ? value
      : current === EMPTY_EFFECT
        ? ""
        : label({ ...(candidates.find(candidate => candidate.id === current) ?? { id: current }), ...(star === undefined ? {} : { star }) });
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
        placeholder={open ? "输入名称筛选" : placeholder}
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

/** Previous / page number box / next; the box jumps on Enter or blur. */
function Pager({ page, pages, onChange }: { page: number; pages: number; onChange: (page: number) => void }) {
  const [text, setText] = useState(String(page + 1));
  useEffect(() => setText(String(page + 1)), [page]);
  const commit = () => {
    const value = Number(text);
    if (Number.isInteger(value) && value >= 1 && value <= pages) onChange(value - 1);
    else setText(String(page + 1));
  };
  if (pages <= 1) return null;
  return (
    <div className="character-pager">
      <button onClick={() => onChange(Math.max(0, page - 1))} disabled={page === 0}>上一页</button>
      <span>
        第
        <input aria-label="页码" inputMode="numeric" value={text}
          onChange={event => setText(event.target.value)} onBlur={commit}
          onKeyDown={event => { if (event.key === "Enter") commit(); }} />
        / {pages} 页
      </span>
      <button onClick={() => onChange(Math.min(pages - 1, page + 1))} disabled={page >= pages - 1}>下一页</button>
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
  const [tab, setTab] = useState<Tab>("equipment");
  const [itemQuery, setItemQuery] = useState("");
  const [itemMajor, setItemMajor] = useState("");
  const [itemPage, setItemPage] = useState(0);
  const [selectedItem, setSelectedItem] = useState<number | null>(null);
  const [itemDraft, setItemDraft] = useState<Record<Container, string>>({ held: "", storage: "" });
  /** The item being added (save mode), shown as a row without a slot. */
  const [newItem, setNewItem] = useState<CharacterEquipment | null>(null);
  const [addQuery, setAddQuery] = useState("");
  const [section, setSection] = useState<Section>("edit");
  const [addKind, setAddKind] = useState("");
  const [addPage, setAddPage] = useState(0);
  /** Items waiting to be added together in one plan. */
  const [queue, setQueue] = useState<{ key: number; request: NewEquipmentRequest; modded: boolean }[]>([]);
  const queueKey = useRef(0);
  /** The player confirmed the game is at the title screen. */
  const [titleConfirmed, setTitleConfirmed] = useState(false);
  /** Legal adds: the effects wanted, the difficulty, and the seed search. */
  const [wanted, setWanted] = useState<WantedEffect[]>([]);
  const [difficulty, setDifficulty] = useState<number | null>(null);
  const [seedResult, setSeedResult] = useState<(EquipmentSeeds & { plus: number }) | null>(null);
  const [chosenSeed, setChosenSeed] = useState<number | null>(null);
  const seedRequestId = useRef(0);
  /** The slot to select once the character is read again after an add. */
  const pendingSelect = useRef<number | null>(null);
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
    const name = itemCatalog[String(id)]?.[0] || "";
    if (!name) return "未收录物品 " + hex(id);
    return showIds ? name + " " + hex(id) : name;
  };
  const skillText = (id: number) => {
    if (!id) return "无";
    const name = HELL_SKILL_NAMES[String(id)];
    if (!name) return "其他武技 " + hex(id);
    return showIds ? name + " " + hex(id) : name;
  };
  const candidateLabel = (candidate: Candidate) => {
    const range = rangeText(candidate.min, candidate.max);
    const source = modded && !range ? sourceOf(candidate.id) : "";
    return (candidate.star ? "✦ " : "") + effectText(candidate.id) +
      (range ? "（" + range + "）" : source ? "〔" + source + "〕" : "");
  };

  const row = selected === NEW_SLOT ? newItem : character?.equipment.find(entry => entry.slot_index === selected) ?? null;
  const adding = selected === NEW_SLOT;
  const addCandidates = useMemo(() => {
    const needle = addQuery.trim().toLowerCase();
    const kinds = ADD_KIND_CHIPS.find(([label]) => label === addKind)?.[1];
    return Object.entries(itemCatalog)
      .map(([id, entry]) => ({ id: Number(id), name: entry[0] ?? "", kind: entry[1] ?? "", sub: entry[3] || entry[2] || "" }))
      .filter(item => item.name && ADDABLE_KINDS.has(item.kind) && (!kinds || kinds.includes(item.kind)))
      .filter(item => !needle || (item.name + " " + item.kind + " " + item.sub + " " + hex(item.id)).toLowerCase().includes(needle));
  }, [addQuery, addKind]);
  useEffect(() => setAddPage(0), [addQuery, addKind]);
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
  const [verdictFilter, setVerdictFilter] = useState("");
  const [sort, setSort] = useState<{ key: SortKey; descending: boolean } | null>(null);
  const verdictCounts = useMemo(() => {
    const counts: Record<string, number> = {};
    for (const entry of character?.equipment ?? []) {
      const verdict = verdictOf(entry);
      if (verdict) counts[verdict] = (counts[verdict] ?? 0) + 1;
    }
    return counts;
  }, [character]);
  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const matches = (character?.equipment ?? []).filter(entry => {
      const [a, b, c] = itemGroups(entry.item_id, entry.type_class);
      if ((major && a !== major) || (middle && b !== middle) || (minor && c !== minor)) return false;
      if (verdictFilter && verdictOf(entry) !== verdictFilter) return false;
      if (!needle) return true;
      const text = [itemText(entry.item_id), a, b, c, ...entry.effects.map(effect => effectText(effect.effect_id))].join(" ");
      return text.toLowerCase().includes(needle);
    });
    if (!sort) return matches;
    // Ties keep the game's inventory order (the sort is stable).
    const sign = sort.descending ? -1 : 1;
    const key = (entry: CharacterEquipment) => sort.key === "level" ? entry.level * 100 + (entry.plus ?? 0) : entry.rarity;
    return [...matches].sort((left, right) => sign * (key(left) - key(right)));
  }, [character, query, major, middle, minor, verdictFilter, sort, showIds]);
  // Descending first, then ascending, then back to the game's order.
  const toggleSort = (key: SortKey) =>
    setSort(current => current?.key !== key ? { key, descending: true } : current.descending ? { key, descending: false } : null);
  const sortHeader = (key: SortKey, label: string) => {
    const active = sort?.key === key;
    return (
      <th aria-sort={active ? (sort.descending ? "descending" : "ascending") : "none"}>
        <button type="button" className={"character-sort" + (active ? " active" : "")} onClick={() => toggleSort(key)}
          title="点击排序：从高到低 → 从低到高 → 游戏顺序">
          {label}<span aria-hidden="true">{active ? (sort.descending ? " ▼" : " ▲") : " ↕"}</span>
        </button>
      </th>
    );
  };
  // Rows per page: as many as fit the list area, so a page never needs the
  // wheel. The single-column layout scrolls the page instead.
  const tableWrap = useRef<HTMLDivElement>(null);
  const [pageSize, setPageSize] = useState(PAGE_SIZE);
  useEffect(() => {
    const wrap = tableWrap.current;
    if (!wrap || typeof ResizeObserver === "undefined") return;
    const measure = () => {
      if (window.matchMedia?.("(max-width: 1100px)").matches) {
        setPageSize(PAGE_SIZE);
        return;
      }
      const head = wrap.querySelector("thead")?.getBoundingClientRect().height ?? 28;
      const heights = [...wrap.querySelectorAll("tbody tr")].map(row => row.getBoundingClientRect().height);
      const row = heights.length ? Math.max(...heights) : 30;
      setPageSize(Math.max(5, Math.floor((wrap.clientHeight - head - 2) / row)));
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(wrap);
    return () => observer.disconnect();
  }, [tab, character, section]);
  const pages = Math.max(1, Math.ceil(filtered.length / pageSize));
  const shownPage = Math.min(page, pages - 1);
  const rows = filtered.slice(shownPage * pageSize, (shownPage + 1) * pageSize);
  useEffect(() => setPage(0), [query, major, middle, minor, verdictFilter, sort]);

  // Items: one row per item id, held and stored stacks side by side.
  const itemRows = useMemo(() => {
    const byId = new Map<number, ItemRow>();
    for (const stack of character?.items ?? []) {
      const entry = byId.get(stack.item_id) ?? { item_id: stack.item_id };
      if (!entry[stack.container]) entry[stack.container] = stack;
      byId.set(stack.item_id, entry);
    }
    // Books are learned recipes, not counted stock; single-count records have no count to edit.
    return [...byId.values()].filter(
      entry =>
        itemGroups(entry.item_id, null)[0] !== "书籍与指南" &&
        (entry.held?.quantity != null || entry.storage?.quantity != null),
    );
  }, [character]);
  const itemMajors = useMemo(
    () => [...new Set(itemRows.map(entry => itemGroups(entry.item_id, null)[0]))],
    [itemRows],
  );
  const filteredItems = useMemo(() => {
    const needle = itemQuery.trim().toLowerCase();
    return itemRows.filter(entry => {
      const [a, b] = itemGroups(entry.item_id, null);
      if (itemMajor && a !== itemMajor) return false;
      return !needle || [itemText(entry.item_id), a, b].join(" ").toLowerCase().includes(needle);
    });
  }, [itemRows, itemQuery, itemMajor, showIds]);
  const itemPages = Math.max(1, Math.ceil(filteredItems.length / pageSize));
  const shownItemPage = Math.min(itemPage, itemPages - 1);
  const pagedItems = filteredItems.slice(shownItemPage * pageSize, (shownItemPage + 1) * pageSize);
  useEffect(() => setItemPage(0), [itemQuery, itemMajor]);
  const itemRow = itemRows.find(entry => entry.item_id === selectedItem) ?? null;
  const itemDraftOf = (entry: ItemRow | null): Record<Container, string> => ({
    held: entry?.held?.quantity == null ? "" : String(entry.held.quantity),
    storage: entry?.storage?.quantity == null ? "" : String(entry.storage.quantity),
  });
  function selectItem(entry: ItemRow) {
    setSelectedItem(entry.item_id);
    setItemDraft(itemDraftOf(entry));
  }

  // Follow the item under the in-game inventory cursor (live mode, read-only).
  const removeDialog = useRef<HTMLDialogElement>(null);
  /** The outcome of the last removal attempt, shown beside the button that started it. */
  const [removeNote, setRemoveNote] = useState("");
  const [follow, setFollow] = useState(true);
  const [followNote, setFollowNote] = useState("");
  // Opening the page, switching mode or picking a save reads the character once;
  // the toolbar button re-reads it.
  const saveId = mode === "save" ? save?.selected?.save_id ?? null : null;
  useEffect(() => {
    if (character || busy || (mode === "save" && !saveId)) return;
    void load();
  }, [mode, saveId]);
  const revealRef = useRef<{ tab: Tab; key: number } | null>(null);
  const missingRef = useRef<string | null>(null);
  const dirty =
    !!(draft && row && JSON.stringify(draft) !== JSON.stringify(draftOf(row))) ||
    !!(itemRow && JSON.stringify(itemDraft) !== JSON.stringify(itemDraftOf(itemRow)));
  const latest = useRef({ selected, selectedItem, dirty, character, busy, filtered, filteredItems, itemRows });
  latest.current = { selected, selectedItem, dirty, character, busy, filtered, filteredItems, itemRows };
  useEffect(() => {
    if (!follow || mode !== "live" || !character || section === "add") return;
    let inFlight = false;
    let stopped = false;
    const reloadOnce = (key: string) => {
      setFollowNote("游戏内选中的物品不在已读取的列表中，正在重新读取");
      if (missingRef.current !== key) {
        missingRef.current = key;
        load("live");
      }
    };
    const timer = window.setInterval(async () => {
      if (inFlight || latest.current.busy) return;
      inFlight = true;
      try {
        const result = await window.operations.execute({ method: "runtime.menu_selection", params: {} });
        if (stopped || !result || !("menu_open" in result)) return;
        const now = latest.current;
        if (!result.menu_open) { setFollowNote("游戏内的持有物品菜单未打开"); return; }
        if (result.slot_index == null || result.item_id == null) { setFollowNote("游戏内选中的物品无法修改"); return; }
        if (result.container === "held" || result.container === "storage") {
          const entry = now.itemRows.find(item => item.item_id === result.item_id);
          if (!entry) {
            if (now.character?.items?.some(stack => stack.item_id === result.item_id)) {
              setFollowNote("游戏内选中的物品没有可修改的数量");
              return;
            }
            return reloadOnce(result.container + result.slot_index);
          }
          missingRef.current = null;
          if (entry.item_id === now.selectedItem) { setFollowNote(""); setTab("items"); return; }
          if (now.dirty) { setFollowNote("当前有未应用的修改，已暂停跟随"); return; }
          setFollowNote("");
          setTab("items");
          if (!now.filteredItems.some(item => item.item_id === entry.item_id)) {
            setItemQuery("");
            setItemMajor("");
          }
          revealRef.current = { tab: "items", key: entry.item_id };
          selectItem(entry);
          return;
        }
        const entry = now.character?.equipment.find(item => item.slot_index === result.slot_index);
        if (!entry) return reloadOnce("equipment" + result.slot_index);
        missingRef.current = null;
        if (entry.slot_index === now.selected) { setFollowNote(""); setTab("equipment"); return; }
        if (now.dirty) { setFollowNote("当前有未应用的修改，已暂停跟随"); return; }
        setFollowNote("");
        setTab("equipment");
        if (!now.filtered.some(item => item.slot_index === entry.slot_index)) {
          setQuery("");
          setMajor(itemGroups(entry.item_id, entry.type_class)[0]);
          setMiddle("");
          setMinor("");
        }
        revealRef.current = { tab: "equipment", key: entry.slot_index };
        setSelected(entry.slot_index);
        setDraft(draftOf(entry));
        setModded(false);
      } catch (error) {
        setFollow(false);
        setFollowNote("");
        setMessage(String(error instanceof Error ? error.message : error));
      } finally {
        inFlight = false;
      }
    }, 300);
    return () => { stopped = true; window.clearInterval(timer); };
  }, [follow, mode, character, section]);
  // In the single-column layout the editor sits below the list; bring it up.
  useEffect(() => {
    if (selected == null && selectedItem == null) return;
    if (!window.matchMedia?.("(max-width: 1100px)").matches) return;
    document.querySelector(".character-side")?.scrollIntoView({ block: "start", behavior: "smooth" });
  }, [selected, selectedItem]);
  // Turn to the page holding a followed row and bring it into view.
  useEffect(() => {
    const target = revealRef.current;
    if (!target) return;
    const list = target.tab === "equipment"
      ? filtered.map(item => item.slot_index)
      : filteredItems.map(item => item.item_id);
    const index = list.indexOf(target.key);
    if (index < 0) return;
    revealRef.current = null;
    (target.tab === "equipment" ? setPage : setItemPage)(Math.floor(index / pageSize));
    window.setTimeout(
      () => document.querySelector(`tr[data-row="${target.tab}-${target.key}"]`)?.scrollIntoView({ block: "nearest" }),
      50,
    );
  }, [filtered, filteredItems, selected, selectedItem, pageSize]);

  const rarity = draft ? parseAmount(draft.rarity, 255) : null;
  const level = draft ? parseAmount(draft.level, 65535) : null;
  const draftHell = draft?.hell ?? false;

  // Rules follow the item, rarity, level and hell state being edited.
  useEffect(() => {
    setRules(null);
    if (!row || rarity === null || level === null) return;
    let live = true;
    window.operations
      .execute({ method: "runtime.equipment_rules", params: { item_id: row.item_id, rarity, level, hell: draftHell } })
      .then(result => { if (live && result && "known" in result) setRules(result as EquipmentRules); })
      .catch(() => {});
    return () => { live = false; };
  }, [row?.slot_index, row?.item_id, rarity, level, draftHell]);

  // A new item takes the slot layout natural generation gives it: innate and
  // set slots are fixed, the others keep what was already picked.
  const rolesKey = rules?.roles?.join(",") ?? "";
  useEffect(() => {
    if (!adding || !newItem || !draft || !rules?.known || !rules.roles) return;
    let innate = 0;
    const effects = rules.roles.map((role, index): DraftEffect => {
      const kept = draft.effects[index] ?? { id: "", value: "", roll: null, star: null };
      const fixed = role === "innate" ? rules.innate?.[innate++] : role === "set" ? rules.set_effect ?? undefined : undefined;
      if (fixed == null) return kept;
      return parseEffectId(kept.id) === fixed ? kept : { id: hex(fixed), value: "", roll: null, star: null };
    });
    if (JSON.stringify(effects) !== JSON.stringify(draft.effects)) setDraft({ ...draft, effects });
  }, [adding, newItem, rolesKey, rules]);

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

  // A new item's effects start at their best natural value.
  useEffect(() => {
    if (!adding || !draft || !values.length) return;
    let changed = false;
    const effects = draft.effects.map((effect, index) => {
      const list = values[index]?.values;
      const role = rules?.roles?.[index];
      if (effect.value.trim() || !list?.length || role === "set" || role === "grace") return effect;
      changed = true;
      const top = list[list.length - 1];
      return { ...effect, value: String(top.value), roll: top.roll_max, star: values[index]?.star ?? null };
    });
    if (changed) setDraft({ ...draft, effects });
  }, [values]);

  /** The generator's player state, read from the save. */
  const generation = character && "generation" in character ? character.generation ?? null : null;
  /** A legal add is generated from a seed, as the game does. */
  const seedMode = adding && !modded;
  const draftPlus = draft ? parseAmount(draft.plus, 65535) : null;
  const wantedKey = wanted.map(entry => entry.key).join(";");
  const seedDifficulty = generation?.difficulties.find(entry => entry.difficulty === (difficulty ?? generation.difficulty));
  const seedSearchIdentity = JSON.stringify({
    item_id: newItem?.item_id ?? null,
    rarity: draft ? parseAmount(draft.rarity, 5) : null,
    level: draft ? parseAmount(draft.level, 180) : null,
    plus: draftPlus,
    difficulty: seedDifficulty?.difficulty ?? null,
    progress: seedDifficulty?.progress ?? null,
    want: wanted.map(entry => entry.ids),
    limit: 40,
  });
  const currentSeedSearchIdentity = useRef(seedSearchIdentity);
  currentSeedSearchIdentity.current = seedSearchIdentity;
  useEffect(() => setDifficulty(generation?.difficulty ?? null), [generation?.difficulty]);
  useEffect(() => setWanted([]), [newItem?.item_id]);
  // A search answers one item, rarity, level, + and difficulty; any change needs a new one.
  useEffect(() => {
    seedRequestId.current += 1;
    setSeedResult(null);
    setChosenSeed(null);
  }, [seedSearchIdentity]);

  /** Effects a legal add can ask for, rows that read the same merged into one choice. */
  const wantOptions = useMemo(() => {
    const options = new Map<string, { candidate: Candidate; ids: number[] }>();
    if (!rules?.known) return options;
    const pool: Candidate[] = (rules.random_pool ?? []).map(entry => ({
      id: entry.effect_id, star: entry.star, min: entry.min, max: entry.max, group: entry.group, masks: entry.masks,
    }));
    const graces: Candidate[] = rarity !== null && rarity >= 4 && rules.set_effect == null && !rules.soul_core
      ? (rules.graces ?? []).map(id => ({ id }))
      : [];
    for (const candidate of [...pool, ...graces]) {
      const key = effectNames.get(candidate.id) + "|" + !!candidate.star + "|" + candidate.min + "|" + candidate.max;
      const option = options.get(key);
      if (option) option.ids.push(candidate.id);
      else options.set(key, { candidate, ids: [candidate.id] });
    }
    return options;
  }, [rules, rarity]);
  const wantCandidates = useMemo(() => {
    const chosen = wanted.flatMap(entry => wantOptions.get(entry.key)?.candidate ?? []);
    return [...wantOptions.entries()]
      .filter(([key, option]) => !wanted.some(entry => entry.key === key) && !chosen.some(other => excludes(option.candidate, other)))
      .map(([, option]) => option.candidate);
  }, [wantOptions, wantedKey]);
  function pickWanted(candidate: Candidate) {
    if (candidate.id === EMPTY_EFFECT || wanted.length >= 7) return;
    for (const [key, option] of wantOptions) {
      if (option.candidate.id !== candidate.id) continue;
      if (!wanted.some(entry => entry.key === key)) setWanted([...wanted, { key, ids: option.ids }]);
      return;
    }
  }

  /** Natural candidates for one slot, or every effect in modded mode. */
  function candidatesFor(index: number, natural = false): Candidate[] {
    if (!rules?.known || !rules.roles) return natural ? [] : ALL_EFFECTS;
    if (modded && !natural) {
      // What this slot can naturally hold comes first, then everything else.
      const own = candidatesFor(index, true);
      const ids = new Set(own.map(candidate => candidate.id));
      return [...own, ...ALL_EFFECTS.filter(candidate => !ids.has(candidate.id))];
    }
    const role = rules.roles[index];
    let list: Candidate[] = [];
    if (role === "innate") list = (rules.innate ?? []).map(id => ({ id }));
    else if (role === "set") list = rules.set_effect == null ? [] : [{ id: rules.set_effect }];
    else if (role === "grace") list = (rules.graces ?? []).map(id => ({ id }));
    else if (role === "hell")
      list = (rules.hell_pool ?? []).map(entry => ({ id: entry.effect_id, min: entry.min, max: entry.max, group: entry.group, masks: entry.masks }));
    else if (role === "random")
      list = (rules.random_pool ?? []).map(entry => ({ id: entry.effect_id, star: entry.star, min: entry.min, max: entry.max, group: entry.group, masks: entry.masks }));
    if (natural) return list;
    // Drop candidates the other drawn slots exclude.
    const others = drawnOthers(index);
    list = list.filter(candidate => !others.some(other => excludes(candidate, other)));
    // Same name, marker and range read identically; keep one of them.
    const seen = new Set<string>();
    return list.filter(candidate => {
      const key = effectNames.get(candidate.id) + "|" + candidate.star + "|" + candidate.min + "|" + candidate.max;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  }

  /** The drawn (random or hell) effects of every slot but `index`, with their exclusion keys. */
  function drawnOthers(index: number): Candidate[] {
    if (!draft || !rules?.roles) return [];
    const keyed = new Map<number, Candidate>();
    for (const entry of [...(rules.random_pool ?? []), ...(rules.hell_pool ?? [])])
      keyed.set(entry.effect_id, { id: entry.effect_id, group: entry.group, masks: entry.masks });
    return draft.effects.flatMap((effect, slot) => {
      const role = rules.roles?.[slot];
      const id = parseEffectId(effect.id);
      if (slot === index || (role !== "random" && role !== "hell") || id === null) return [];
      const key = keyed.get(id);
      return key ? [key] : [];
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

  /** A soul core's random slot keeps its star marker when the game re-rolls it. */
  const keepsMarker = (index: number) => !!rules?.soul_core && rules.roles?.[index] === "random";

  /** Slots whose shown star disagrees with the effect: the game's re-roll bug. */
  const markerNotes = useMemo(() => {
    if (!draft || !row || !rules?.soul_core) return [] as string[];
    const notes: string[] = [];
    draft.effects.forEach((effect, index) => {
      const starRow = values[index]?.star;
      if (!keepsMarker(index) || starRow == null) return;
      const marked = markerOf(row, draft, index, true);
      if (marked === starRow) return;
      notes.push("#" + (index + 1) + " " + (starRow
        ? "这是星号词条，但游戏里不显示星号：游戏洗魂核词条时不会更新星号标记（游戏自身的 bug），这是正常结果。"
        : "这不是星号词条，但游戏里仍显示星号：游戏洗魂核词条时不会去掉原来的星号标记（游戏自身的 bug），这是正常结果。"));
    });
    return notes;
  }, [draft, row, rules, values]);

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
    draft.effects.forEach((effect, index) => {
      const id = parseEffectId(effect.id);
      const mine = candidatesFor(index, true).find(candidate => candidate.id === id);
      const clash = mine && draft.effects.findIndex((other, slot) =>
        slot < index && drawnOthers(index).some(key => key.id === parseEffectId(other.id) && excludes(mine, key)));
      if (clash != null && clash >= 0) notes.push("#" + (index + 1) + " 与 #" + (clash + 1) + " 互斥，游戏不会同时生成这两条");
    });
    // An unedited slot that only had its id swapped; writing it again repairs the marker.
    for (const finding of row?.audit?.findings ?? [])
      if (finding.code === "replaced_effect" && finding.slot != null
        && draft.effects[finding.slot] && parseEffectId(draft.effects[finding.slot].id) === row!.effects[finding.slot]?.effect_id)
        notes.push(findingText(finding));
    if (row) {
      const markers = draft.effects.filter((_, index) => markerOf(row, draft, index, keepsMarker(index), values[index]?.star)).length;
      if (markers > 1) notes.push("星号标记超过一个，游戏不会生成这样的装备");
    }
    if (draft.hell && !rules.hell_capable) notes.push("这件装备不会自然成为地狱武器");
    if (draft.hell && !(rules.hell_skills ?? []).includes(draft.hell_skill))
      notes.push("这个地狱武技不会出现在这类武器上");
    return notes;
  }, [draft, rules, values, modded, row]);

  /** Turn the draft into (or out of) a hell weapon, keeping a legal skill. */
  function setHell(on: boolean) {
    if (!draft) return;
    const skills = rules?.hell_skills ?? [];
    const skill = on ? (skills.includes(draft.hell_skill) ? draft.hell_skill : skills[0] ?? draft.hell_skill) : 0;
    setDraft({ ...draft, hell: on, hell_skill: skill });
  }

  function adopt(next: Character) {
    setCharacter(next);
    setCurrencyDraft({
      amrita: next.currencies.amrita == null ? "" : String(next.currencies.amrita),
      gold: next.currencies.gold == null ? "" : String(next.currencies.gold),
    });
    // After an add, select the new record; while adding, keep the add form.
    const target = pendingSelect.current ?? selected;
    pendingSelect.current = null;
    if (target !== NEW_SLOT) {
      const kept = next.equipment.find(entry => entry.slot_index === target);
      setSelected(kept ? target : null);
      setDraft(kept ? draftOf(kept) : null);
      setNewItem(null);
    }
    const stacks = (next.items ?? []).filter(stack => stack.item_id === selectedItem);
    if (stacks.length) {
      const entry: ItemRow = { item_id: stacks[0].item_id };
      for (const stack of stacks) entry[stack.container] ??= stack;
      setItemDraft(itemDraftOf(entry));
    } else setSelectedItem(null);
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
        try {
          const result = await window.operations.execute({ method: "runtime.character_snapshot", params: {} });
          if (!result || !("source" in result) || result.source !== "runtime") throw new Error("UNEXPECTED_CHARACTER_SNAPSHOT");
          adopt({ ...result, mode: "live" });
        } catch (error) {
          const detail = errorText(error);
          const missing = /no running process matches Nioh3\.exe/i.test(detail);
          if (!missing && !/character layout: no character is loaded/i.test(detail)) throw error;
          // An offline or unloaded game is a normal state. Do not leave a
          // former live snapshot editable after the character becomes unavailable.
          setCharacter(null);
          setMessage(missing
            ? "未检测到正在运行的仁王3。可以直接使用“修改存档文件”；实时修改需要启动游戏并读档。"
            : "当前没有可读取的游戏角色。可以使用“修改存档文件”，或进入角色存档后点“重新读取”。");
        }
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
    setFollow(next === "live");
    setCharacter(null);
    setSelected(section === "add" ? NEW_SLOT : null);
    setNewItem(null);
    setSelectedItem(null);
    setDraft(null);
    setPlan(null);
    setMessage("");
  }

  async function submit(edit: {
    currencies?: Record<string, number>;
    equipment?: { slot_index: number; patch: Record<string, unknown> }[];
    items?: { container: Container; slot_index: number; quantity: number }[];
    remove?: number[];
    add?: NewEquipmentRequest[];
  }) {
    if (!character) return;
    if (character.mode === "live") {
      const live = character as LiveCharacter & { mode: Mode };
      const equipment = (edit.equipment ?? []).map(item => ({
        ...item,
        expected_record_sha256: live.equipment.find(entry => entry.slot_index === item.slot_index)?.record_sha256 ?? "",
      }));
      const items = (edit.items ?? []).map(item => ({
        ...item,
        expected_record_sha256:
          live.items?.find(stack => stack.container === item.container && stack.slot_index === item.slot_index)?.record_sha256 ?? "",
      }));
      const remove = (edit.remove ?? []).map(slot_index => ({
        slot_index,
        expected_record_sha256: live.equipment.find(entry => entry.slot_index === slot_index)?.record_sha256 ?? "",
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
            ...(items.length ? { items } : {}),
            ...(remove.length ? { remove } : {}),
          },
        }),
      );
      const outcome = result && "character_edit" in result ? result.character_edit : null;
      if (!outcome) throw new Error("UNEXPECTED_CHARACTER_EDIT");
      if (outcome.state === "verified" && remove.length) {
        setSelected(null);
        setDraft(null);
        setMessage("已从游戏中移除。在游戏里切换一下菜单页即可看到它消失；到神社存档即可保存到存档文件。");
      } else if (outcome.state === "verified") setMessage("已写入游戏。到神社存档即可保存到存档文件。");
      else if (outcome.state === "rejected") setMessage("没有写入：" + (outcome.error ?? ""));
      else setMessage("写入结果不确定，请重新读取后核对：" + (outcome.error ?? ""));
      const refreshed = await window.operations.execute({ method: "runtime.character_snapshot", params: {} });
      if (refreshed && "source" in refreshed && refreshed.source === "runtime") adopt({ ...refreshed, mode: "live" });
    } else {
      const { remove, add, ...rest } = edit;
      type SaveEdit = Parameters<NonNullable<typeof saveSession>["prepareCharacterEdit"]>[0];
      const result = await saveSession!.prepareCharacterEdit({
        ...rest,
        ...(remove?.length ? { remove: remove.map(slot_index => ({ slot_index })) } : {}),
        // At most seven effects each, which the request contract spells as tuples.
        ...(add?.length ? { add: add as unknown as NonNullable<SaveEdit["add"]> } : {}),
      });
      if (!("plan_id" in result)) throw new Error("UNEXPECTED_CHARACTER_PLAN");
      setPlan({ plan_id: result.plan_id, preview: result.preview as Record<string, unknown> });
      setMessage("已生成修改计划。核对右侧内容，让游戏回到标题界面后勾选确认，再点“写入存档”。");
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
      if (row.slot_index === NEW_SLOT) {
        const level = parseAmount(draft.level, 65535);
        const plus = parseAmount(draft.plus, 65535);
        const rarity = parseAmount(draft.rarity, 5);
        if (level === null || level < 1 || plus === null || rarity === null) throw new Error("请输入有效的数值。");
        const effects: NewEquipmentRequest["effects"] = [];
        for (const effect of draft.effects) {
          const id = parseEffectId(effect.id);
          if (id === EMPTY_EFFECT) continue;
          const value = parseAmount(effect.value || "0", 4294967295);
          if (id === null || id > 0xffff || value === null) throw new Error("请输入有效的词条和数值。");
          effects.push({ effect_id: id, value, ...(effect.star !== null ? { star: effect.star } : {}) });
        }
        if (queue.length >= ADD_LIMIT) throw new Error("待添加清单最多 " + ADD_LIMIT + " 件。请先写入存档，再继续添加。");
        const request: NewEquipmentRequest = {
          item_id: row.item_id,
          level,
          plus,
          rarity,
          ...(draft.hell ? { hell: true, hell_skill: draft.hell_skill } : {}),
          effects,
        };
        setQueue([...queue, { key: ++queueKey.current, request, modded: modded || draftNotes.length > 0 }]);
        setPlan(null);
        setMessage("已加入待添加清单。可以继续挑选其他装备，全部选好后点“生成修改计划”。");
        return;
      }
      const result = patchOf(row, draft, keepsMarker);
      if ("error" in result) throw new Error(result.error);
      if (!Object.keys(result.patch).length) throw new Error("没有需要修改的内容。");
      await submit({ equipment: [{ slot_index: row.slot_index, patch: result.patch }] });
    });
  }

  /** Run the game's generator over every seed and keep the ones with every wanted effect. */
  function findSeeds() {
    return run(async () => {
      if (!row || !draft) return;
      if (!generation) throw new Error("没有从存档里读到难度和进度，不能按游戏规则生成。请点“重新读取”后再试；仍然不行的话，可以改用“魔改”添加。");
      const level = parseAmount(draft.level, 180);
      const plus = parseAmount(draft.plus, 65535);
      const rarity = parseAmount(draft.rarity, 5);
      if (level === null || level < 1 || plus === null || rarity === null) throw new Error("请填写有效的等级（1–180）、+ 数值和稀有度（0–5）。");
      const chosen = generation.difficulties.find(entry => entry.difficulty === (difficulty ?? generation.difficulty));
      if (!chosen) throw new Error("存档里没有这个难度的进度。请换一个难度后重新查找。");
      const params = {
        item_id: row.item_id, rarity, level, plus,
        difficulty: chosen.difficulty,
        progress: chosen.progress as [number, number, number, number],
        want: wanted.map(entry => entry.ids) as never,
        limit: 40,
      };
      const requestId = ++seedRequestId.current;
      const requestIdentity = JSON.stringify(params);
      setSeedResult(null);
      setChosenSeed(null);
      let result: Awaited<ReturnType<typeof window.operations.execute>>;
      try {
        result = await window.operations.execute({ method: "runtime.equipment_seeds", params });
      } catch (error) {
        if (requestId !== seedRequestId.current || requestIdentity !== currentSeedSearchIdentity.current) return;
        throw error;
      }
      if (requestId !== seedRequestId.current || requestIdentity !== currentSeedSearchIdentity.current) return;
      if (!result || !("outcomes" in result) || !("matches" in result)) throw new Error("UNEXPECTED_SEED_SEARCH");
      const found = result as EquipmentSeeds;
      setSeedResult({ ...found, plus });
      setChosenSeed(found.outcomes[0]?.seed ?? null);
    });
  }

  /** Queue the item the chosen seed gives. */
  function addSeeded() {
    return run(async () => {
      if (!row || !seedResult || chosenSeed === null) return;
      if (queue.length >= ADD_LIMIT) throw new Error("待添加清单最多 " + ADD_LIMIT + " 件。请先写入存档，再继续添加。");
      const request: NewEquipmentRequest = {
        item_id: row.item_id,
        level: seedResult.level,
        plus: seedResult.plus,
        rarity: seedResult.rarity,
        seed: chosenSeed,
        difficulty: seedResult.difficulty,
      };
      setQueue([...queue, { key: ++queueKey.current, request, modded: false }]);
      setPlan(null);
      setMessage("已加入待添加清单。可以继续挑选其他装备，全部选好后点“生成修改计划”。");
    });
  }

  /**
   * Remove the selected equipment after the dialog confirms it: live for an
   * unworn item, or as a save plan, where a worn item also leaves its sets.
   */
  function removeEquipment() {
    removeDialog.current?.close();
    setRemoveNote("");
    return run(async () => {
      if (!row || !character) return;
      try {
        if (character.mode === "live" && row.worn) throw new Error("这件装备正在装备中，请先在游戏里卸下。");
        await submit({ remove: [row.slot_index] });
      } catch (error) {
        setRemoveNote(publicError(stripErrorPrefix(String(error instanceof Error ? error.message : error))));
        throw error;
      }
    });
  }

  function applyItems() {
    return run(async () => {
      if (!itemRow) return;
      const items: { container: Container; slot_index: number; quantity: number }[] = [];
      for (const container of ["held", "storage"] as const) {
        const stack = itemRow[container];
        if (!stack || stack.quantity == null) continue;
        const value = parseAmount(itemDraft[container], stack.limit);
        if (value === null) throw new Error("请输入有效的数量。");
        if (value !== stack.quantity) items.push({ container, slot_index: stack.slot_index, quantity: value });
      }
      if (!items.length) throw new Error("没有需要修改的内容。");
      await submit({ items });
    });
  }

  /** Turn the waiting list into one save plan. */
  function planQueue() {
    return run(async () => {
      if (!queue.length) throw new Error("待添加清单是空的。请先在左侧选一件装备，设置好后点“加入待添加清单”。");
      await submit({ add: queue.map(entry => entry.request) });
    });
  }

  /**
   * Settle a commit that failed: a refused write (for example because the
   * game wrote the save in the meantime) is proven from the bytes on disk, so
   * the save is not left locked behind a question the player cannot answer.
   */
  async function settleFailedCommit(error: unknown): Promise<string> {
    const reason = publicError(stripErrorPrefix(String(error instanceof Error ? error.message : error)));
    if (!saveSession!.getSnapshot().uncertainOperationId) return reason;
    try {
      const receipt = await saveSession!.recoverReceipt();
      if (receipt.commit_status === "not_committed") return reason + "（已核对：存档没有改动。）";
      if (receipt.commit_status.startsWith("committed")) return "";
    } catch {
      // The banner above the list stays and explains how to check by hand.
    }
    return reason + " 写入结果暂时无法确认，请按上方黄色提示核对。";
  }

  function commitPlan() {
    return run(async () => {
      if (!plan) return;
      if (!titleConfirmed) throw new Error("请先让游戏回到标题界面，再勾选“游戏已回到标题界面”。");
      const added = (plan.preview.added ?? []) as { slot_index: number }[];
      let committed = false;
      let failure = "";
      try {
        const receipt = await saveSession!.commit(plan.plan_id);
        committed = receipt.commit_status === "committed";
        if (!committed) failure = "写入结果不确定，请在备份与管理中核对操作结果。";
      } catch (error) {
        failure = await settleFailedCommit(error);
        committed = !failure;
      }
      setPlan(null);
      setTitleConfirmed(false);
      if (committed && added.length) setQueue([]);
      // Reading the save again clears the message, so it is set afterwards.
      if (!saveSession!.getSnapshot().uncertainOperationId) {
        await saveSession!.refresh();
        await load("save");
      }
      setMessage(
        !committed
          ? failure
          : added.length
            ? "已写入存档：新增 " + added.length + " 件装备，原存档已自动备份。进游戏读取这个存档即可看到（带“新”标记）；游戏如果一直开着，要回到标题界面重新读取。"
            : "已写入存档，并已自动备份原存档。",
      );
    });
  }

  function planLines(preview: Record<string, unknown>) {
    const currencies = (preview.currencies ?? []) as { currency: string; before: number; after: number }[];
    const equipment = (preview.equipment ?? []) as { slot_index: number; before: CharacterEquipment; after: CharacterEquipment }[];
    const items = (preview.items ?? []) as { container: Container; item_id: number; before: number; after: number }[];
    const removed = (preview.removed ?? []) as { slot_index: number; before: CharacterEquipment }[];
    const added = (preview.added ?? []) as { slot_index: number; after: CharacterEquipment; audit?: CharacterEquipment["audit"] | null; seeded?: boolean }[];
    const lines: string[] = [];
    for (const change of added) {
      const item = change.after;
      lines.push("新增 " + itemText(item.item_id) + "（Lv." + item.level + (item.plus ? " +" + item.plus : "") +
        "，稀有度 " + item.rarity + (item.hell ? "，地狱武器" : "") + "）" +
        (change.seeded ? "，按游戏发放物品的规则生成" : "") +
        (change.audit ? (change.audit.natural ? "，判定为自然" : "，判定为非自然") : ""));
      item.effects.forEach((effect, index) => {
        if (effect.effect_id !== EMPTY_EFFECT)
          lines.push("　#" + (index + 1) + " " + effectText(effect.effect_id) + " " + effect.value);
      });
    }
    for (const change of removed)
      lines.push("移除 " + itemText(change.before.item_id) + "（Lv." + change.before.level + "）" +
        (change.before.worn ? "，并从装备栏卸下" : ""));
    for (const change of currencies)
      lines.push((CURRENCY_LABEL[change.currency] ?? change.currency) + "：" + change.before + " → " + change.after);
    for (const change of items)
      lines.push(itemText(change.item_id) + " " + CONTAINER_LABEL[change.container] + "数量：" + change.before + " → " + change.after);
    for (const change of equipment) {
      const prefix = itemText(change.before.item_id);
      if ((change.before.hell ?? false) !== (change.after.hell ?? false))
        lines.push(prefix + " 地狱武器：" + (change.before.hell ? "是" : "否") + " → " + (change.after.hell ? "是" : "否"));
      if ((change.before.hell_skill ?? 0) !== (change.after.hell_skill ?? 0))
        lines.push(prefix + " 地狱武技：" + skillText(change.before.hell_skill ?? 0) + " → " + skillText(change.after.hell_skill ?? 0));
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
    const slot = finding.slot == null ? "" : "#" + (finding.slot + 1) + " ";
    if (finding.code === "replaced_effect" && finding.original != null)
      return slot + "词条被替换过（槽位里还留着原词条「" + effectText(finding.original) + "」的标记），游戏自己生成不会这样";
    return slot + (FINDING_LABEL[finding.code] ?? finding.code);
  }
  function verdictCell(entry: CharacterEquipment) {
    const audit = entry.audit;
    const verdict = verdictOf(entry);
    if (!audit || !verdict) return <span title="规则未载入">—</span>;
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
  const selectEquipment = (entry: CharacterEquipment) => {
    setSelected(entry.slot_index);
    setDraft(draftOf(entry));
    setModded(false);
    setNewItem(null);
  };
  /** Switch between changing what the character has and adding new equipment. */
  const openSection = (next: Section) => {
    if (next === section) return;
    setSection(next);
    setSelected(next === "add" ? NEW_SLOT : null);
    setNewItem(null);
    setDraft(null);
    setModded(false);
  };
  const chooseNewItem = (itemId: number) => {
    // Start at the highest level the character's equipment has reached.
    const level = Math.max(0, ...(character?.equipment ?? []).map(entry => entry.level)) || 170;
    const item: CharacterEquipment = {
      slot_index: NEW_SLOT,
      item_id: itemId,
      appearance_id: itemId,
      quantity: 1,
      level,
      level_before_forge: level,
      plus: 0,
      familiarity: 0,
      inventory_key: 0,
      seed: 0,
      rarity: 4,
      hell: false,
      hell_skill: 0,
      effects: [],
    };
    // The previous item's rules must not lay out this one: its innate effect
    // would be kept in a slot that is random here.
    setRules(null);
    setValues([]);
    setNewItem(item);
    setDraft(draftOf(item));
  };
  const quantityText = (stack?: CharacterItem) => (stack ? (stack.quantity == null ? "1" : String(stack.quantity)) : "—");

  const equipmentList = (
    <>
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
          <input value={query} onChange={event => setQuery(event.target.value)} placeholder="搜索物品名称或词条" />
          {minors.length > 0 && (
            <select value={minor} onChange={event => setMinor(event.target.value)} aria-label="小类">
              <option value="">全部小类</option>
              {minors.map(value => <option key={value} value={value}>{value}</option>)}
            </select>
          )}
          <select value={verdictFilter} onChange={event => setVerdictFilter(event.target.value)} aria-label="判定">
            <option value="">全部判定</option>
            {VERDICT_ORDER.map(value => (
              <option key={value} value={value}>{VERDICT_LABEL[value]}（{verdictCounts[value] ?? 0}）</option>
            ))}
          </select>
          <span className="equipment-range">{filtered.length} / {character?.equipment.length ?? 0}</span>
        </div>
      </div>
      <div className="character-table-wrap" ref={tableWrap}>
      <table className="equipment-table character-table">
        <thead>
          <tr><th>物品</th><th>类别</th>{sortHeader("level", "等级")}{sortHeader("rarity", "稀有度")}<th>判定</th></tr>
        </thead>
        <tbody>
          {rows.map(entry => {
            const [a, b, c] = itemGroups(entry.item_id, entry.type_class);
            const effects = entry.effects.filter(effect => effect.effect_id !== EMPTY_EFFECT).map(effect => effectText(effect.effect_id)).join("、");
            return (
              <tr key={entry.slot_index} data-row={"equipment-" + entry.slot_index}
                className={entry.slot_index === selected ? "selected" : ""} onClick={() => selectEquipment(entry)}>
                <td className="character-item-cell">
                  <span className="character-item-name">
                    {itemText(entry.item_id)}
                    {entry.hell ? <span className="character-hell">地狱</span> : null}
                    {entry.worn ? <span className="character-worn" title="正在装备中，不能移除">装备中</span> : null}
                    {showIds ? <small> #{entry.slot_index}</small> : null}
                  </span>
                  <span className="character-item-effects" title={effects}>{effects}</span>
                </td>
                <td className="character-kind">{c || b || a}</td>
                <td className="character-num">{entry.level}{entry.plus ? <small> +{entry.plus}</small> : null}</td>
                <td className="character-num">{entry.rarity}</td>
                <td>{verdictCell(entry)}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      </div>
      <Pager page={shownPage} pages={pages} onChange={setPage} />
    </>
  );

  const itemList = character?.items == null ? (
    <p className="character-empty">道具区域没有通过布局校验，暂时无法读取。装备和精华不受影响。</p>
  ) : (
    <>
      <div className="character-filters">
        <div className="character-chips">
          <button className={!itemMajor ? "active" : ""} onClick={() => setItemMajor("")}>全部</button>
          {itemMajors.map(value => (
            <button key={value} className={itemMajor === value ? "active" : ""} onClick={() => setItemMajor(value)}>{value}</button>
          ))}
        </div>
        <div className="character-search">
          <input value={itemQuery} onChange={event => setItemQuery(event.target.value)} placeholder="搜索道具名称" />
          <span className="equipment-range">{filteredItems.length} / {itemRows.length}</span>
        </div>
      </div>
      <div className="character-table-wrap" ref={tableWrap}>
      <table className="equipment-table character-table">
        <thead>
          <tr><th>道具</th><th>类别</th><th>持有</th><th>仓库</th></tr>
        </thead>
        <tbody>
          {pagedItems.map(entry => {
            const [a, b] = itemGroups(entry.item_id, null);
            return (
              <tr key={entry.item_id} data-row={"items-" + entry.item_id}
                className={entry.item_id === selectedItem ? "selected" : ""} onClick={() => selectItem(entry)}>
                <td className="character-item-name">{itemText(entry.item_id)}</td>
                <td className="character-kind">{b || a}</td>
                <td className="character-num">{quantityText(entry.held)}</td>
                <td className="character-num">{quantityText(entry.storage)}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      </div>
      <Pager page={shownItemPage} pages={itemPages} onChange={setItemPage} />
    </>
  );

  const addPages = Math.max(1, Math.ceil(addCandidates.length / pageSize));
  const shownAddPage = Math.min(addPage, addPages - 1);
  const addList = (
    <>
      <div className="character-filters">
        <div className="character-chips">
          <button className={!addKind ? "active" : ""} onClick={() => setAddKind("")}>全部</button>
          {ADD_KIND_CHIPS.map(([label]) => (
            <button key={label} className={addKind === label ? "active" : ""} onClick={() => setAddKind(label)}>{label}</button>
          ))}
        </div>
        <div className="character-search">
          <input value={addQuery} onChange={event => setAddQuery(event.target.value)} placeholder="搜索装备名称，例如 八尺琼勾玉" />
          <span className="equipment-range">{addCandidates.length} 件</span>
        </div>
      </div>
      <div className="character-table-wrap" ref={tableWrap}>
        <table className="equipment-table character-table">
          <thead>
            <tr><th>装备</th><th>类别</th></tr>
          </thead>
          <tbody>
            {addCandidates.slice(shownAddPage * pageSize, (shownAddPage + 1) * pageSize).map(item => (
              <tr key={item.id} data-row={"add-" + item.id}
                className={newItem?.item_id === item.id ? "selected" : ""} onClick={() => chooseNewItem(item.id)}>
                <td className="character-item-name">{item.name}{showIds ? <small> {hex(item.id)}</small> : null}</td>
                <td className="character-kind">{item.sub || item.kind}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {addCandidates.length === 0 && <p className="character-empty">没有匹配的装备</p>}
      </div>
      <Pager page={shownAddPage} pages={addPages} onChange={setAddPage} />
    </>
  );

  const addHint = (
    <p className="character-empty">
      在左侧列表中点选要添加的装备。“合法”按游戏发放物品的规则生成，不包含敌人或地区掉落加成：选等级、稀有度和难度，挑想要的词条后查找，再选一个结果；“魔改”可以随意填写词条和数值。设置好后加入待添加清单，可以一次添加多件。
    </p>
  );

  /** The waiting list and the button that turns it into one plan. */
  const queuePanel = queue.length ? (
    <section className="character-plan character-queue">
      <h3>待添加清单（{queue.length} / {ADD_LIMIT}）</h3>
      <ul className="character-plan-lines">
        {queue.map(entry => (
          <li key={entry.key}>
            {itemText(entry.request.item_id)}（Lv.{entry.request.level}，稀有度 {entry.request.rarity}
            {entry.request.hell ? "，地狱武器" : ""}
            {entry.request.seed != null ? "，第 " + entry.request.difficulty + " 难度生成" : ""}）
            {entry.modded ? <span className="character-unnatural">魔改</span> : <span className="character-muted">合法</span>}
            <button type="button" className="character-queue-remove" aria-label="从清单移除"
              onClick={() => { setQueue(queue.filter(other => other.key !== entry.key)); setPlan(null); }} disabled={busy}>×</button>
          </li>
        ))}
      </ul>
      <button className="primary" onClick={planQueue} disabled={busy || !!save?.busy}>生成修改计划</button>
      <button onClick={() => { setQueue([]); setPlan(null); }} disabled={busy}>清空</button>
    </section>
  ) : null;

  const seedSummary = !seedResult
    ? ""
    : seedResult.empty === seedResult.seeds
      ? "当前生成路线下，这件装备在稀有度 " + seedResult.rarity + " 时无法生成有效装备：全部 " + seedResult.seeds + " 个种子得到的都是空装备。请换一个稀有度。"
      : seedResult.matches === 0
        ? "当前生成路线无匹配：第 " + seedResult.difficulty + " 难度、稀有度 " + seedResult.rarity + " 时，没有任何种子能让这件装备同时带有所选的词条。可以减少词条，或换稀有度、难度后再查找。"
        : (wanted.length
          ? "全部 " + seedResult.seeds + " 个种子中，有 " + seedResult.matches + " 个（" + percent(seedResult.matches / seedResult.seeds) + "）同时带有所选词条。"
          : "当前生成路线下，这件装备有 " + seedResult.matches + " 种种子结果。") +
          "下面按随机词条的分位和星级评分列出前 " + seedResult.outcomes.length + " 个，点选一个后加入清单。";
  const seedPanel = (
    <div className="seed-panel">
      {!generation ? (
        <p className="character-remove-note">没有从存档里读到难度和进度，不能按游戏规则生成。请点“重新读取”后再试；仍然不行的话，可以改用“魔改”添加。</p>
      ) : null}
      <div className="seed-want">
        <span className="seed-want-label">想要的词条（可不选，最多 7 个）</span>
        <div className="seed-want-chips">
          {wanted.map(entry => (
            <span key={entry.key} className="seed-chip">
              {candidateLabel(wantOptions.get(entry.key)?.candidate ?? { id: entry.ids[0] })}
              <button type="button" aria-label="移除" onClick={() => setWanted(wanted.filter(other => other.key !== entry.key))}>×</button>
            </span>
          ))}
          {wanted.length < 7 ? (
            <EffectPicker value="" candidates={wantCandidates} label={candidateLabel} onPick={pickWanted} placeholder="点这里添加想要的词条" />
          ) : null}
        </div>
      </div>
      <div className="character-actions">
        <button className="primary" onClick={findSeeds} disabled={busy || !generation || !rules?.known}>查找</button>
        {seedResult ? (
          <button onClick={addSeeded} disabled={busy || chosenSeed === null}>加入待添加清单</button>
        ) : null}
      </div>
      {seedResult ? <p className="seed-summary">{seedSummary}</p> : null}
      {seedResult?.outcomes.length ? (
        <ul className="seed-outcomes" role="listbox" aria-label="生成结果">
          {seedResult.outcomes.map(outcome => (
            <li key={outcome.seed}>
              <button type="button" role="option" aria-selected={chosenSeed === outcome.seed}
                className={chosenSeed === outcome.seed ? "chosen" : ""} onClick={() => setChosenSeed(outcome.seed)}>
                {outcome.effects.map((effect, index) => (
                  <span key={index} className={"seed-effect" + (wanted.some(entry => entry.ids.includes(effect.effect_id)) ? " wanted" : "")}>
                    {effect.star ? "✦ " : ""}{effectText(effect.effect_id)}
                    {effect.role === "set" || effect.role === "grace" ? null : <small> {effect.value}</small>}
                  </span>
                ))}
                {showIds ? <small className="seed-id">种子 {hex(outcome.seed)}</small> : null}
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );

  const equipmentDetail = adding && !newItem ? addHint : row && draft ? (
    <div className="character-detail">
      <div className="character-detail-head">
        <h3>
          {itemText(row.item_id)}
          {adding ? <span className="character-new">新增</span> : null}
          {row.hell ? <span className="character-hell">地狱</span> : null}
          {row.worn ? <span className="character-worn" title="正在装备中，不能移除">装备中</span> : null}
        </h3>
        {adding ? null : verdictCell(row)}
        <div className="character-edit-modes" role="tablist">
          <button className={!modded ? "active" : ""} onClick={() => setModded(false)}>{adding ? "合法" : "合法修改"}</button>
          <button className={modded ? "active" : ""} onClick={() => setModded(true)}>魔改</button>
        </div>
      </div>
      <p className="equipment-notes">
        {modded
          ? "魔改：任何词条、任何数值都可以填写，不受游戏生成规则约束，结果可能无法自然获得。同名词条后面的〔〕注明它会出现在哪类装备的哪种位置，以及它的数值（稀有度 4、等级 180 时）；“非装备掉落”表示掉落的装备上不会出现。"
          : adding
            ? "合法：按游戏发放物品的规则生成，不包含敌人或地区掉落加成。选好词条后搜索全部 65536 个种子；“无匹配”仅表示当前难度、进度和稀有度下，这条生成路线没有符合条件的结果。选中的装备会按种子生成后写入存档。"
            : "合法修改：每个位置只列出这件装备能自然出现的词条，数值填在合法范围内。"}
      </p>
      {adding && rules && !rules.known ? (
        <p className="character-remove-note">这件物品不在游戏的装备表里，不能添加。请换一件（名字相同的物品可能有多个，其中一个是书籍或关键道具）。</p>
      ) : null}
      <div className="character-fields">
        {FIELD_LABEL.filter(([key]) => !adding || key === "level" || key === "plus" || key === "rarity").map(([key, label]) => (
          <label key={key}>
            <span>{label}</span>
            <input inputMode="numeric" value={draft[key as keyof Draft] as string} onChange={event => setDraft({ ...draft, [key]: event.target.value })} />
          </label>
        ))}
        {seedMode && generation ? (
          <label>
            <span>掉落难度</span>
            <select value={difficulty ?? generation.difficulty} onChange={event => setDifficulty(Number(event.target.value))}>
              {generation.difficulties.map(entry => (
                <option key={entry.difficulty} value={entry.difficulty}>
                  第 {entry.difficulty} 难度{entry.difficulty === generation.difficulty ? "（当前）" : ""}
                </option>
              ))}
            </select>
          </label>
        ) : null}
      </div>
      {!seedMode && (itemGroups(row.item_id, row.type_class)[0] === "武器" || rules?.hell_capable || draft.hell) && (
        <div className="character-hell-row">
          <label className="character-toggle">
            <input type="checkbox" checked={draft.hell}
              disabled={!draft.hell && !rules?.hell_capable && !modded}
              onChange={event => setHell(event.target.checked)} />
            地狱武器
          </label>
          {draft.hell && (
            <select value={draft.hell_skill} aria-label="地狱武技"
              onChange={event => setDraft({ ...draft, hell_skill: Number(event.target.value) })}>
              {[...new Set([...(rules?.hell_skills ?? []), ...(draft.hell_skill ? [draft.hell_skill] : [])])].map(skill => (
                <option key={skill} value={skill}>
                  {skillText(skill)}
                  {(rules?.hell_skills ?? []).includes(skill) ? "" : "（不会自然出现）"}
                </option>
              ))}
            </select>
          )}
          <span className="character-muted">
            {!rules?.known
              ? ""
              : !rules.hell_capable
                ? "这件装备不会自然成为地狱武器，只能在魔改模式下切换。"
                : draft.hell && !row.hell
                  ? "切换后第一个位置变为地狱词条，请在下方选择。"
                  : ""}
          </span>
        </div>
      )}
      {seedMode ? seedPanel : <>
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
                <td><EffectPicker value={effect.id} star={row && (keepsMarker(index) || row.effects[index]?.effect_id === parseEffectId(effect.id)) ? markerOf(row, draft, index, true) : undefined} candidates={candidatesFor(index)} label={candidateLabel} onPick={candidate => pickEffect(index, candidate)} /></td>
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
      {draftNotes.length > 0 && (
        <ul className="character-findings">{draftNotes.map(note => <li key={note}>{note}</li>)}</ul>
      )}
      {markerNotes.length > 0 && (
        <ul className="character-marker-notes">{markerNotes.map(note => <li key={note}>{note}</li>)}</ul>
      )}
      </>}
      {adding && !seedMode ? (
        <div className="character-actions">
          <button onClick={maximize} disabled={busy || !values.some(Boolean)}>全部取理论最高</button>
          <button className="primary" onClick={applyEquipment} disabled={busy || !rules?.known}>
            {modded ? "加入待添加清单（魔改）" : "加入待添加清单"}
          </button>
        </div>
      ) : null}
      {adding ? (
        <p className="equipment-notes">
          新装备会放进存档里的空位，按游戏自己的方式分配物品编号，并带有“新”标记。清单里的装备会一起生成一份修改计划，核对后点“写入存档”才会真正写入。
        </p>
      ) : null}
      {adding ? null : <>
      <div className="character-actions">
        <button onClick={maximize} disabled={busy || !values.some(Boolean)}>全部取理论最高</button>
        <button className="primary" onClick={applyEquipment} disabled={busy}>{modded ? "写入魔改" : "写入"}</button>
        {dirty ? <button onClick={() => setDraft(draftOf(row))} disabled={busy}>还原</button> : null}
        <button className="danger" onClick={() => { setRemoveNote(""); removeDialog.current?.showModal(); }}
          disabled={busy || (character?.mode === "live" && !!row.worn)}
          title={character?.mode === "live" && row.worn ? "这件装备正在装备中，请先在游戏里卸下" : undefined}>移除</button>
      </div>
      {character?.mode === "live" && row.worn && (
        <p className="equipment-notes">这件装备正在装备中，要移除请先在游戏里卸下；卸不下来的话，可以切换到“修改存档文件”移除。</p>
      )}
      {removeNote && <p className="character-remove-note">{removeNote}</p>}
      <dialog ref={removeDialog} className="remove-dialog">
        <header>
          <h2>移除装备</h2>
          <button aria-label="关闭" onClick={() => removeDialog.current?.close()}>×</button>
        </header>
        <div className="modal-body">
          {character?.mode === "live" ? (
            <>
              <p>确定要从游戏里移除「{itemText(row.item_id)}」（Lv.{row.level}）吗？</p>
              <p>移除后它会立即从游戏的装备栏里消失，无法恢复。到神社存档后，这个改动才会写进存档文件。</p>
              <p className="muted">正在装备中的物品不能移除。</p>
            </>
          ) : (
            <>
              <p>确定要从存档里移除「{itemText(row.item_id)}」（Lv.{row.level}）吗？</p>
              {row.worn && <p>它正在装备中，移除时会一并从装备栏卸下。</p>}
              <p>点“确认移除”后会先生成修改计划；核对后点“写入存档”才会真正写入。写入前会自动备份原存档，游戏停在标题界面即可，无需关闭。</p>
            </>
          )}
          <div className="character-actions">
            <button onClick={() => removeDialog.current?.close()}>取消</button>
            <button className="danger" onClick={removeEquipment}>确认移除</button>
          </div>
        </div>
      </dialog>
      </>}
      <p className="equipment-notes">
        数值按游戏内部单位填写，例如百分比词条 15 表示 1.5%。“前 X%”表示自然生成时得到这个值或更好值的概率。
      </p>
    </div>
  ) : (
    <p className="character-empty">在左侧列表中点选一件装备，这里会显示它的词条和修改选项。</p>
  );

  const itemDetail = itemRow ? (
    <div className="character-detail">
      <div className="character-detail-head">
        <h3>{itemText(itemRow.item_id)}</h3>
        <span className="character-muted">{itemGroups(itemRow.item_id, null).filter(Boolean).slice(0, 2).join(" · ")}</span>
      </div>
      <div className="character-fields">
        {(["held", "storage"] as const).map(container => {
          const stack = itemRow[container];
          return (
            <label key={container}>
              <span>{CONTAINER_LABEL[container]}数量</span>
              {stack && stack.quantity != null ? (
                <input inputMode="numeric" value={itemDraft[container]}
                  onChange={event => setItemDraft({ ...itemDraft, [container]: event.target.value })} />
              ) : (
                <span className="character-muted character-no-stack">{stack ? "固定为 1" : "没有这条记录"}</span>
              )}
            </label>
          );
        })}
      </div>
      <p className="equipment-notes">
        只修改已有的记录，不会新增：持有或仓库里没有这件道具时，对应一栏不能填写。游戏里各道具有自己的携带上限，超出的数量可能被游戏自动调整。
      </p>
      <div className="character-actions">
        <button className="primary" onClick={applyItems} disabled={busy}>写入</button>
        {dirty ? <button onClick={() => setItemDraft(itemDraftOf(itemRow))} disabled={busy}>还原</button> : null}
      </div>
    </div>
  ) : (
    <p className="character-empty">在左侧列表中点选一件道具，这里可以修改它的持有数量和仓库数量。</p>
  );

  const planPanel = plan ? (
    <section className="character-plan character-save-plan">
      <h3>修改计划</h3>
      <div className="character-plan-details">
        <ul className="character-plan-lines">{planLines(plan.preview).map((line, index) => <li key={index}>{line}</li>)}</ul>
      </div>
      <div className="character-plan-footer">
        <label className="character-toggle character-title-confirm">
          <input data-action="confirm-character-plan" type="checkbox" checked={titleConfirmed} disabled={busy || !!save?.busy} onChange={event => setTitleConfirmed(event.target.checked)} />
          游戏已回到标题界面
        </label>
        <p className="equipment-notes">游戏在读档状态下会自己保存，写入的内容会被覆盖或被拒绝。写入后进游戏重新读取这个存档即可看到。</p>
        <div className="character-plan-actions">
          <button data-action="commit-character-plan" className="primary" onClick={commitPlan} disabled={busy || !!save?.busy || !titleConfirmed}>写入存档</button>
          <button data-action="discard-character-plan" onClick={() => { setPlan(null); setTitleConfirmed(false); void saveSession!.discard(); }} disabled={busy || !!save?.busy}>放弃</button>
        </div>
      </div>
    </section>
  ) : null;

  return (
    <main className="equipment-page character-page">
      <div className="character-sections" role="tablist" aria-label="功能">
        <button role="tab" aria-selected={section === "edit"} className={section === "edit" ? "active" : ""}
          onClick={() => openSection("edit")} disabled={busy}>
          修改已有物品<small>装备的词条与数值、道具数量、精华与金钱</small>
        </button>
        <button role="tab" aria-selected={section === "add"} className={section === "add" ? "active" : ""}
          onClick={() => openSection("add")} disabled={busy}>
          添加新装备<small>像游戏掉落一样生成一件新装备，或自由魔改</small>
        </button>
      </div>
      <div className="character-toolbar">
        <div className="character-modes" role="tablist">
          <button className={mode === "live" ? "active" : ""} onClick={() => switchMode("live")} disabled={busy}>游戏内实时修改</button>
          <button className={mode === "save" ? "active" : ""} onClick={() => switchMode("save")} disabled={busy}>修改存档文件</button>
        </div>
        <button onClick={() => load()} disabled={busy || (mode === "save" && !save?.selected)}>
          重新读取
        </button>
        {mode === "live" && character && (
          <button className={follow ? "active" : ""} onClick={() => { setFollow(!follow); setFollowNote(""); }}>
            {follow ? "停止跟随" : "跟随游戏内选中"}
          </button>
        )}
        {character && (
          <div className="character-currencies">
            {(["amrita", "gold"] as const).map(key => (
              <label key={key}>
                <span>{CURRENCY_LABEL[key]}</span>
                <input inputMode="numeric" value={currencyDraft[key]} onChange={event => setCurrencyDraft({ ...currencyDraft, [key]: event.target.value })} />
                <button onClick={() => applyCurrency(key)} disabled={busy}>修改</button>
              </label>
            ))}
          </div>
        )}
      </div>
      <p className="character-hint">
        {mode === "live"
          ? "直接修改正在运行的游戏，需要先读档进入游戏。修改后到神社存档即可保存。"
          : "修改存档文件，写入时让游戏停在标题界面即可，无需关闭。写入前会自动备份原存档。"}
        {follow && followNote ? <span className="character-follow-note">{followNote}</span> : null}
      </p>
      {mode === "save" && <SavePicker compact />}
      <Notice text={message} />
      {section === "add" && mode === "live" ? (
        <LiveEquipmentAdd onBusy={setBusy} />
      ) : character && section === "add" ? (
        <div className="character-body">
          <section className="character-list">{addList}</section>
          <aside className="character-side">
            {planPanel}
            {queuePanel}
            {equipmentDetail}
          </aside>
        </div>
      ) : character && (
        <div className="character-body">
          <section className="character-list">
            <div className="character-tabs" role="tablist">
              <button className={tab === "equipment" ? "active" : ""} onClick={() => setTab("equipment")}>
                装备 <small>{character.equipment.length}</small>
              </button>
              <button className={tab === "items" ? "active" : ""} onClick={() => setTab("items")}>
                道具 <small>{itemRows.length}</small>
              </button>
            </div>
            {tab === "equipment" ? equipmentList : itemList}
          </section>
          <aside className="character-side">
            {planPanel}
            {tab === "equipment" ? equipmentDetail : itemDetail}
          </aside>
        </div>
      )}
    </main>
  );
}
