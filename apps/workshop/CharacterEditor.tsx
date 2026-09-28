import React, { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import type {
  CharacterEquipment,
  CharacterItem,
  EffectValues,
  EquipmentRules,
  LiveCharacter,
  SaveCharacter,
} from "../../packages/contracts/protected-responses";
import { data } from "./model";
import itemNames from "./item-names.json";
import hellSkillNames from "./hell-skill-names.json";
import { desktop } from "./desktop-bridge";
import { fillTemplateSlots, plainGameText } from "./game-text";
import { Notice } from "./Notice";
import { publicError, stripErrorPrefix } from "./public-errors";
import { SavePicker } from "./CartActions";
import { runtimeObserver, saveObserver, saveSession } from "./save-workspace";

type Mode = "live" | "save";
type Tab = "equipment" | "items";
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
function EffectPicker({ value, star, candidates, label, onPick }: {
  value: string;
  /** The saved star marker of the current effect, when it is unchanged. */
  star?: boolean;
  candidates: Candidate[];
  label: (candidate: Candidate) => string;
  onPick: (candidate: Candidate) => void;
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
  }, [tab, character]);
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
    if (!follow || mode !== "live" || !character) return;
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
  }, [follow, mode, character]);
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
    const kept = next.equipment.find(entry => entry.slot_index === selected);
    setDraft(kept ? draftOf(kept) : null);
    if (!kept) setSelected(null);
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
    setFollow(next === "live");
    setCharacter(null);
    setSelected(null);
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
      const result = await saveSession!.prepareCharacterEdit(edit);
      if (!("plan_id" in result)) throw new Error("UNEXPECTED_CHARACTER_PLAN");
      setPlan({ plan_id: result.plan_id, preview: result.preview as Record<string, unknown> });
      setMessage("已生成修改计划。核对右侧内容后点击“写入存档”。游戏必须关闭。");
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
      const result = patchOf(row, draft, keepsMarker);
      if ("error" in result) throw new Error(result.error);
      if (!Object.keys(result.patch).length) throw new Error("没有需要修改的内容。");
      await submit({ equipment: [{ slot_index: row.slot_index, patch: result.patch }] });
    });
  }

  /** Remove the selected unworn equipment from the running game, after the dialog confirms it. */
  function removeEquipment() {
    removeDialog.current?.close();
    setRemoveNote("");
    return run(async () => {
      if (!row || character?.mode !== "live") return;
      try {
        if (row.worn) throw new Error("这件装备正在装备中，请先在游戏里卸下。");
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
    const items = (preview.items ?? []) as { container: Container; item_id: number; before: number; after: number }[];
    const lines: string[] = [];
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

  const equipmentDetail = row && draft ? (
    <div className="character-detail">
      <div className="character-detail-head">
        <h3>
          {itemText(row.item_id)}
          {row.hell ? <span className="character-hell">地狱</span> : null}
          {row.worn ? <span className="character-worn" title="正在装备中，不能移除">装备中</span> : null}
        </h3>
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
      {(itemGroups(row.item_id, row.type_class)[0] === "武器" || rules?.hell_capable || draft.hell) && (
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
      <div className="character-actions">
        <button onClick={maximize} disabled={busy || !values.some(Boolean)}>全部取理论最高</button>
        <button className="primary" onClick={applyEquipment} disabled={busy}>{modded ? "写入魔改" : "写入"}</button>
        {dirty ? <button onClick={() => setDraft(draftOf(row))} disabled={busy}>还原</button> : null}
        {character?.mode === "live" && (
          <button className="danger" onClick={() => { setRemoveNote(""); removeDialog.current?.showModal(); }} disabled={busy || !!row.worn}
            title={row.worn ? "这件装备正在装备中，请先在游戏里卸下" : undefined}>移除</button>
        )}
      </div>
      {character?.mode === "live" && row.worn && <p className="equipment-notes">这件装备正在装备中，要移除请先在游戏里卸下。</p>}
      {removeNote && <p className="character-remove-note">{removeNote}</p>}
      <dialog ref={removeDialog} className="remove-dialog">
        <header>
          <h2>移除装备</h2>
          <button aria-label="关闭" onClick={() => removeDialog.current?.close()}>×</button>
        </header>
        <div className="modal-body">
          <p>确定要从游戏里移除「{itemText(row.item_id)}」（Lv.{row.level}）吗？</p>
          <p>移除后它会立即从游戏的装备栏里消失，无法恢复。到神社存档后，这个改动才会写进存档文件。</p>
          <p className="muted">正在装备中的物品不能移除。</p>
          <div className="character-actions">
            <button onClick={() => removeDialog.current?.close()}>取消</button>
            <button className="danger" onClick={removeEquipment}>确认移除</button>
          </div>
        </div>
      </dialog>
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

  return (
    <main className="equipment-page character-page">
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
          : "修改存档文件，游戏必须关闭。写入前会自动备份原存档。"}
        {follow && followNote ? <span className="character-follow-note">{followNote}</span> : null}
      </p>
      {mode === "save" && <SavePicker compact />}
      <Notice text={message} />
      {character && (
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
            {plan && (
              <section className="character-plan">
                <h3>修改计划</h3>
                <ul className="character-plan-lines">{planLines(plan.preview).map(line => <li key={line}>{line}</li>)}</ul>
                <button className="primary" onClick={commitPlan} disabled={busy}>写入存档</button>
                <button onClick={() => { setPlan(null); void saveSession!.discard(); }} disabled={busy}>放弃</button>
              </section>
            )}
            {tab === "equipment" ? equipmentDetail : itemDetail}
          </aside>
        </div>
      )}
    </main>
  );
}
