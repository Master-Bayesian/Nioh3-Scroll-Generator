import React from "react";
import itemNames from "./item-names.json";
import itemSets from "./item-sets.json";
import { data } from "./model";
import { plainGameText } from "./game-text";
import { localizeName } from "./presentation";

/**
 * A catalog name for display: its exact translation when the locale has one,
 * otherwise the Chinese name. React.createElement keeps it out of the JSX
 * translator, which would replace substrings inside the name.
 */
export function nameLabel(name: string) {
  const translated = localizeName(name);
  return translated != null
    ? React.createElement("span", null, translated)
    : React.createElement("span", { lang: "zh-CN" }, name);
}

/** Equipment grouping shared by the inventory, the save-file add list and the live add list. */
export const itemCatalog = (itemNames as { items: Record<string, string[]> }).items;
/** Catalog kinds a new item may be picked from; the rules decide the rest. */
const ADDABLE_KINDS = new Set(["武器", "防具", "防具或饰品", "饰品", "魂核"]);

/** The class chip that groups set pieces across weapons, armor and accessories (#30). */
export const SET_MAJOR = "套装";
const SET_OF = (itemSets as { items: Record<string, number> }).items;
/** The set effect an item carries in the item table, or null. */
export function setOf(id: number): number | null {
  return SET_OF[String(id)] ?? null;
}
const SET_NAMES = new Map<number, string>();
for (const row of data.editorEffects as { id: string; name: string }[])
  if (!SET_NAMES.has(Number(row.id))) SET_NAMES.set(Number(row.id), plainGameText(row.name));
/** A set's name is its set effect's name. */
export function setName(set: number): string {
  return SET_NAMES.get(set) || "套装 " + set;
}
/** Whether an item passes the class/type filter; under 套装 the type is a set id. */
export function matchesClass(id: number, facets: Facets, major: string, type: string): boolean {
  if (major === SET_MAJOR) {
    const set = setOf(id);
    return set != null && (!type || String(set) === type);
  }
  return (!major || facets.major === major) && (!type || facets.type === type);
}

/** Coarse item group from the shipped item table's type class. */
export function kindOf(typeClass: number | null | undefined): string {
  if (typeClass == null) return "其他";
  if (typeClass <= 22) return "武器";
  if (typeClass >= 24 && typeClass <= 38) return "防具";
  if (typeClass === 39 || typeClass === 40) return "饰品";
  if (typeClass >= 54 && typeClass <= 57) return "魂核";
  return "其他";
}

/** Equipment classes in the order every filter shows them. */
export const MAJOR_ORDER = ["武器", "防具", "饰品", "魂核"];
/** Weapon types by school, then armor slots and accessory kinds, in the game's menu order. */
export const TYPE_ORDER = [
  "刀", "双刀", "枪", "大太刀", "斧", "薙刀镰", "手甲", "盾矛",
  "忍刀", "忍双刀", "锁镰", "手甲钩", "手斧", "旋棍", "机关棍", "忍盾矛",
  "弓", "火枪", "大炮",
  "头部", "身体", "手臂", "腿部", "足部",
  "武士饰品", "忍者饰品", "通用饰品",
];
export const SCHOOLS = ["武士", "忍者"];
export interface Facets {
  /** 武器 / 防具 / 饰品 / 魂核 / 其他 */
  major: string;
  /** Weapon type, armor slot or accessory kind; "" when the catalog does not say. */
  type: string;
  /** 武士 / 忍者 for armor (weapon types already imply it); "" otherwise. */
  school: string;
}
/** What the equipment filters group an item by, from the bundled catalog. */
export function facetsOf(id: number, typeClass: number | null | undefined): Facets {
  const entry = itemCatalog[String(id)];
  let major = entry?.[1] || kindOf(typeClass);
  if (major === "防具或饰品") major = kindOf(typeClass) === "防具" ? "防具" : "饰品";
  const sub = entry?.[2] ?? "", group = entry?.[3] ?? "";
  if (major === "饰品") return { major, type: sub || group, school: "" };
  if (major === "防具") return { major, type: sub, school: SCHOOLS.find(school => group.startsWith(school)) ?? "" };
  if (major === "武器") return { major, type: sub, school: "" };
  return { major, type: "", school: "" };
}
export function byOrder(order: string[]) {
  const rank = (value: string) => (order.indexOf(value) + 1 || order.length + 1);
  return (left: string, right: string) => rank(left) - rank(right) || left.localeCompare(right, "zh-CN");
}

/** Named equipment a new item may be picked from, by class, type and name. */
export const ADD_CATALOG = Object.entries(itemCatalog)
  .filter(([, entry]) => entry[0] && ADDABLE_KINDS.has(entry[1] ?? ""))
  .map(([id, entry]) => ({ id: Number(id), name: entry[0], ...facetsOf(Number(id), null) }))
  .sort((left, right) =>
    byOrder(MAJOR_ORDER)(left.major, right.major) ||
    byOrder(TYPE_ORDER)(left.type, right.type) ||
    left.name.localeCompare(right.name, "zh-CN") ||
    left.id - right.id);
/** Sets with pieces in the add catalog, by name. */
export const ADD_SETS: [string, number][] = [...ADD_CATALOG.reduce((sets, item) => {
  const set = setOf(item.id);
  if (set != null) sets.set(String(set), (sets.get(String(set)) ?? 0) + 1);
  return sets;
}, new Map<string, number>())].sort(([left], [right]) => setName(Number(left)).localeCompare(setName(Number(right)), "zh-CN"));
/** Types present in each class of the add catalog, in menu order. */
export const ADD_TYPES = new Map(MAJOR_ORDER.map(major => [
  major,
  [...new Set(ADD_CATALOG.filter(item => item.major === major && item.type).map(item => item.type))].sort(byOrder(TYPE_ORDER)),
]));

/**
 * Class chips, then the weapon types / armor slots / accessory kinds of the
 * chosen class, and for armor a samurai/ninja choice.
 */
export function FacetFilter({ majors, major, onMajor, types, type, onType, school, onSchool }: {
  majors: [string, number | null][];
  major: string;
  onMajor: (value: string) => void;
  types: [string, number | null][];
  type: string;
  onType: (value: string) => void;
  school: string;
  onSchool: ((value: string) => void) | null;
}) {
  const chip = (label: string, count: number | null, active: boolean, onClick: () => void) => (
    <button key={label} className={active ? "active" : ""} onClick={onClick}>
      {label}{count != null ? <small>{count}</small> : null}
    </button>
  );
  return (
    <>
      <div className="character-chips">
        {chip("全部", null, !major, () => onMajor(""))}
        {majors.map(([value, count]) => chip(value, count, major === value, () => onMajor(value)))}
      </div>
      {major === SET_MAJOR && (
        <div className="character-chips small">
          <select className="character-set-select" value={type} onChange={event => onType(event.target.value)} aria-label="套装">
            <option value="">全部套装（{types.length}）</option>
            {types.map(([value, count]) => (
              <option key={value} value={value}>{setName(Number(value))}{count != null ? "（" + count + "）" : ""}</option>
            ))}
          </select>
        </div>
      )}
      {major && major !== SET_MAJOR && (types.length > 1 || onSchool) && (
        <div className="character-chips small">
          {types.length > 1 && chip("全部", null, !type, () => onType(""))}
          {types.length > 1 && types.map(([value, count]) => chip(value, count, type === value, () => onType(value)))}
          {onSchool && (
            <span className="character-school" role="group" aria-label="流派">
              {["", ...SCHOOLS].map(value => (
                <button key={value || "all"} className={school === value ? "active" : ""} onClick={() => onSchool(value)}>
                  {value || "全部流派"}
                </button>
              ))}
            </span>
          )}
        </div>
      )}
    </>
  );
}
