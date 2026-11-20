import React, { useEffect, useMemo, useReducer, useRef, useState, useSyncExternalStore } from "react";
import type {
  LoadoutBundles,
  LoadoutCatalog,
  LoadoutContext,
  LoadoutSearch,
  LoadoutSets,
  SaveCharacter,
  SaveReference,
} from "../../packages/contracts/protected-responses";
import {
  canWearLoadout, currentLoadoutContext, initialLoadoutContextState, loadoutContextReducer, sameLoadoutScope,
  type LoadoutContextEvent, type LoadoutScope, type LoadoutWearBinding,
} from "./loadout-context-state";
import { LoadoutCatalogPicker } from "./LoadoutCatalogPicker";
import { LoadoutSourceCheck } from "./LoadoutSourceCheck";
import { DamageSummary,DamageContacts } from "./LoadoutDamageDetails";
import { loadoutAnswerCoverage, loadoutHasScoreConflict, loadoutIncompleteReasons, loadoutSourceModelScope } from './loadout-answer-coverage';
import { identifiedLoadoutSearchJob, loadoutSearchProgress, loadoutSearchStageLabel, type LoadoutSearchOwner } from "./loadout-search-progress";
import { sameSource } from "../desktop/src/save-session";
import { itemCatalog, nameLabel, setName, setOf } from "./equipment-facets";
import { errorText, publicError } from "./public-errors";
import { localize } from "./presentation";
import { SavePicker, saveLines } from "./CartActions";
import { discoverSaves, saveObserver, saveSession } from "./save-workspace";

import { nativeLoadoutSlot, preferredScene, sceneWeaponKey as weaponKey } from "./loadout-scene-selection";
import { matchesLoadoutUndoItem, parseLoadoutUndo, type LoadoutUndo as Undo } from "./loadout-undo";

type Bundle = LoadoutBundles["bundles"][number];
type AttackScene = NonNullable<Bundle["scene"]>;
type VerifiedBundle = Bundle & { scene: AttackScene };
type Slot = 0 | 4 | 5 | 6 | 7 | 8 | 9 | 10;
type Item = { slot_index: number; slot: Slot };
type Row = LoadoutSearch["best"][number];
type Facts = NonNullable<Row["facts"]>;
type Current = LoadoutSearch["current"];
type SetInfo = LoadoutSets["sets"][number];

const SLOT_LABEL: Record<number, string> = {
  0: "武器", 4: "头部", 5: "身体", 6: "手臂", 7: "腿部", 8: "足部", 9: "饰品1", 10: "饰品2",
};
const SLOT_ORDER = [0, 4, 5, 6, 7, 8, 9, 10];
/** The bundles were exported for items up to this rarity; the page text names it too. */
const MAX_RARITY = 5;
const MAX_RULES = 4;
const MAX_SET_KEYS = 6;
const UNDO_KEY = "nioh3.loadout.undo.";
const STANCE_LABEL: Record<AttackScene["stance"], string> = {
  high: "上段", mid: "中段", low: "下段", none: "无段位",
};
const STANCE_ORDER: AttackScene["stance"][] = ["high", "mid", "low", "none"];
const BUFF_LABEL: Record<AttackScene["buff"], string> = {
  current: "读取角色时的增益",
  untriggered: "无临时增益",
  deflect: "精防增伤已触发",
  arts: "琢技已触发",
  deflect_arts: "精防＋琢技",
};

function attackLabel(bundle: VerifiedBundle): string {
  // Variants have distinct names and ids; the kind alone is not their label.
  return bundle.damage;
}

/** Installed runnable scenes. Discovery is independent of private character captures. */
export async function loadoutDiscovery(): Promise<{ bundles: VerifiedBundle[]; catalog?: LoadoutCatalog }> {
  const result = await saveObserver!.run(() =>
    window.operations.execute({ method: "save.loadout_bundles", params: {} }),
  );
  if (!result || !("bundles" in result) || !Array.isArray(result.bundles)) throw new Error("LOADOUT_DISCOVERY_INVALID_RESPONSE");
  const discovery = result as LoadoutBundles;
  return { bundles: discovery.bundles.filter((bundle): bundle is VerifiedBundle =>
    bundle.scene !== undefined && !bundle.diagnostic), catalog: discovery.catalog };
}

export async function loadoutBundles(): Promise<VerifiedBundle[]> {
  return (await loadoutDiscovery()).bundles;
}

/** Native TYPE admission matches the protected host; names are presentation only. */
export function slotFor(bundle: Bundle, _itemId: number, typeClass: number | null | undefined): Slot | null {
  return nativeLoadoutSlot(bundle, typeClass);
}

/** The character's items that can go into a search for this bundle. */
function eligible(bundle: Bundle, equipment: SaveCharacter["equipment"]): Item[] {
  return equipment.flatMap(entry => {
    if (entry.rarity > MAX_RARITY) return [];
    const slot = slotFor(bundle, entry.item_id, entry.type_class);
    return slot == null ? [] : [{ slot_index: entry.slot_index, slot }];
  });
}

function itemName(id: number) {
  const name = itemCatalog[String(id)]?.[0];
  return name ? nameLabel(name) : <span>未知装备 {id}</span>;
}

function slotText(slot: number | null) {
  return slot != null && SLOT_LABEL[slot] ? SLOT_LABEL[slot] : String(slot);
}

function itemText(id: number) {
  return itemCatalog[String(id)]?.[0] ?? "未知装备 " + id;
}

/** The save the game wrote last: the one being played. */
function newest(saves: SaveReference[]): SaveReference | undefined {
  return [...saves].sort((a, b) => (b.summary?.saved_at_unix ?? 0) - (a.summary?.saved_at_unix ?? 0))[0];
}

/** Why a loadout request failed, with what the player can do about it. */
function loadoutError(error: unknown): string {
  const text = errorText(error);
  if (/LOADOUT_SCORE_VERIFICATION_MISMATCH/.test(text)) return "伤害复核与排名分数不一致，本次结果已作废。请重新读取角色后搜索；仍然出现请反馈。";
  if (/LOADOUT_SOURCE_REBUILD_REFRESH_REQUIRED/.test(text)) return "当前场景的来源数据尚未与这次角色读取绑定，暂不能搜索。请使用已支持的场景。";
  if (/LOADOUT_DOMAIN_COUNT_TOO_LARGE/.test(text)) return "搭配数量超出可精确统计的范围，无法完成这次搜索。请减少参与搜索的装备后重试。";
  if (/LOADOUT_SOURCE_ATTRIBUTES_UNPROVEN/.test(text)) return "当前来源重建尚不支持属性点重分配。请关闭“允许重新分配属性点”后搜索。";
  if (/LOADOUT_BUFF_SOURCE_NOT_ENABLED/.test(text)) return "所选增益需要对应技能已解锁并启用，请检查角色技能后重新读取。";
  if (/LOADOUT_BUFF_REQUIRES_SAMURAI/.test(text)) return "所选增益条件需要武士流派，请切换流派后重新读取。";
  if (/LOADOUT_CONTEXT_WEAPON_MISMATCH/.test(text)) return "当前手持武器与所选攻击的武器类型不符，请切换武器后重新读取。";
  if (/LOADOUT_CONTEXT_CHARACTER_NOT_LOADED/.test(text)) return "请在游戏中载入角色，再刷新角色状态。";
  if (/LOADOUT_CONTEXT_CHANGED/.test(text)) return "读取过程中角色状态发生变化，请保持角色空闲后重试。";
  if (/LOADOUT_CONTEXT_CHARACTER_MISMATCH/.test(text)) return "游戏角色的穿戴与所选存档不一致，请在游戏中保存后重新读取，或选择正确的存档。";
  if (/LOADOUT_CONTEXT_SCHOOL_MISMATCH/.test(text)) return "当前流派与所选攻击不符，请在游戏中切换流派并使用主手武器后重新读取。";
  if (/LOADOUT_CONTEXT_MISSING/.test(text)) return "所选攻击的计算模板缺失，请重新安装完整的计算数据。";
  if (/OPTIMIZER_CONTEXT_STALE/.test(text)) return "角色上下文与当前存档不一致，请重新读取。";
  if (/SAVE_VIEW_STALE|OPTIMIZER_RESULT_STALE/.test(text)) return publicError(text);
  if (/Save changed since it was read|Save changed after preparation/i.test(text))
    return "存档在读取后被游戏改写了。请点“重新读取”后再试。";
  if (/LOADOUT_BUNDLE_MISSING|LOADOUT_BUNDLE_INVALID/.test(text)) return "所选伤害的计算数据缺失或损坏。请重新安装本工具后再试。";
  if (/LOADOUT_SETS_TOO_MANY/.test(text)) return "套装条件太多了：最多 4 条条件、一共 6 种套装。请删掉一些后再搜索。";
  if (/LOADOUT_SET_RULE_INVALID/.test(text)) return "有一条套装条件没有选套装，或“满足其中几套”超过了已选的套装数。请修改后再搜索。";
  if (/LOADOUT_SPACE_TOO_LARGE_CPU/.test(text))
    return "当前搜索无法使用显卡加速，搭配数超过 CPU 的 3000 万套限制。请添加套装条件或提高件数，缩小范围后再搜索。";
  if (/LOADOUT_SPACE_TOO_LARGE/.test(text)) return "要比较的搭配超过 30 亿套，太多了。请加一条套装条件或提高件数，缩小范围后再搜索。";
  if (/LOADOUT_WEAR_INVALID/.test(text)) return "这套装备里有两件要穿在同一个部位，没有写入。请重新搜索后再试。";
  if (/LOADOUT_ITEM_/.test(text))
    return "有装备与它被放入的部位不符，操作已取消。请点“重新读取”后再试；仍然出现请反馈并附上这段说明：" + text;
  return "操作失败。请点“重新读取”后再试；仍然出现请反馈并附上这段说明：" + text;
}

/** One set condition as the player builds it: any `atLeast` of the options must be active. */
interface RuleDraft {
  options: { key: number; pieces: number }[];
  atLeast: number;
}

/** Undo for the last outfit this page put on, kept per save until it is used or the outfit changes again. */
/** How to undo a wear: every record it moved, with the set word it had before (null: not worn). */
function readUndo(saveId: string): Undo | null {
  try {
    const text = localStorage.getItem(UNDO_KEY + saveId);
    return text ? parseLoadoutUndo(JSON.parse(text) as unknown) : null;
  } catch {
    return null;
  }
}

function writeUndo(saveId: string, undo: Undo | null) {
  try {
    if (undo) localStorage.setItem(UNDO_KEY + saveId, JSON.stringify(undo));
    else localStorage.removeItem(UNDO_KEY + saveId);
  } catch {
    // Without storage the undo lasts until the page closes; the automatic backup still holds the old save.
  }
}

/** A wear plan waiting for the player to confirm the game is at the title screen. */
type Pending =
  | { kind: "none" }
  | { kind: "planned"; planId: string; lines: string[]; undo: Undo | null; purpose: "wear" | "undo"; binding: LoadoutWearBinding | null }
  | { kind: "retiring"; planId: string; reason: "changed" | "cancelled" }
  | { kind: "retirement_failed"; planId: string; reason: "changed" | "cancelled"; message: string };

type WornLine = { slot_index: number; item_id: number; before: number | null; after: number | null };

function percent(after: number, before: number) {
  if (!before) return "";
  const p = ((after - before) / before) * 100;
  return (p >= 0 ? "+" : "−") + Math.abs(p).toFixed(Math.abs(p) < 10 ? 1 : 0) + "%";
}

/** Match the protected request schema before starting a search. UI load is a percentage. */
export function loadoutLimitsFromText(maxLoad: string, minDefense: string, minHealth: string):
  { max_load?: number; min_defense?: number; min_health?: number } | null {
  const limits: { max_load?: number; min_defense?: number; min_health?: number } = {};
  if (maxLoad.trim()) {
    const percentValue = Number(maxLoad);
    if (!Number.isFinite(percentValue) || percentValue <= 0 || percentValue > 1000) return null;
    limits.max_load = percentValue / 100;
  }
  for (const [key, text] of [["min_defense", minDefense], ["min_health", minHealth]] as const) {
    if (!text.trim()) continue;
    const value = Number(text);
    if (!Number.isInteger(value) || value < 0 || value > 1000000) return null;
    limits[key] = value;
  }
  return limits;
}

/**
 * What wearing `row` instead of the current outfit gains and costs, as short sentences. Damage leads; a cost is
 * called out when it is larger than the damage gained, since that is the trade a player most often misses.
 */
export function describeChange(row: Row, current: Current): string[] {
  if (current.status === "ambiguous") return ["你现在同一部位穿着两件可参与计算的装备，无法和当前穿戴对比。"];
  if (current.status !== "answered" || current.damage == null)
    return ["你现在穿的这套暂时算不出伤害，只能看下面的排名。"];
  const lines: string[] = [];
  const gain = current.damage ? (row.damage - current.damage) / current.damage : 0;
  const changed = SLOT_ORDER.filter(slot => {
    const a = row.items.find(i => i.slot === slot)?.slot_index;
    const b = current.items.find(i => i.slot === slot)?.slot_index;
    return a !== b;
  });
  if (!changed.length && !current.kept_slots.length && row.damage === current.damage &&
      (!row.facts || !current.facts || (row.facts.load === current.facts.load &&
        row.facts.defense === current.facts.defense && row.facts.health === current.facts.health)))
    return ["就是你现在穿的这套。"];
  lines.push(
    (current.damage === 0 && row.damage > 0 ? "伤害提高" :
      Math.abs(gain) < 0.0005 ? "伤害不变" : "伤害" + " " + percent(row.damage, current.damage)) +
      "（" + current.damage.toFixed(0) + " → " + row.damage.toFixed(0) + "）",
  );
  lines.push(changed.length ? "要换的部位：" + changed.map(slot => SLOT_LABEL[slot]).join("、") :
    "装备不变；数值按本次搜索的属性与强化设置计算。");
  const now = current.facts, next = row.facts;
  if (now && next) {
    const better: string[] = [], worse: string[] = [];
    const compare = (label: string, a: number, b: number) => {
      if (a === b) return;
      (b > a ? better : worse).push(label + " " + percent(b, a));
    };
    compare("防御", now.defense, next.defense);
    compare("体力", now.health, next.health);
    const load = (f: Facts) => f.load * 100;
    if (Math.abs(load(next) - load(now)) >= 0.05)
      (load(next) < load(now) ? better : worse).push(
        "负重" + " " + load(now).toFixed(1) + "% → " + load(next).toFixed(1) + "%",
      );
    if (better.length) lines.push("同时变好：" + better.join("、"));
    if (worse.length) lines.push("代价：" + worse.join("、"));
    const biggest = [
      now.defense ? (now.defense - next.defense) / now.defense : 0,
      now.health ? (now.health - next.health) / now.health : 0,
    ].reduce((a, b) => Math.max(a, b), 0);
    if (biggest > Math.max(gain, 0) && gain > 0)
      lines.push("注意：生存能力下降的幅度比伤害提升还大，打不过的地方可以先试试这套的下一名。");
    if (!worse.length && gain > 0) lines.push("其他各项都不比现在差。");
  }
  if (current.kept_slots.length)
    lines.push("参考穿戴的这些部位沿用角色快照中的状态：" + current.kept_slots.map(slot => SLOT_LABEL[slot]).join("、"));
  return lines;
}

export function LoadoutOptimizer({ bundles, catalog }: { bundles: VerifiedBundle[]; catalog?: LoadoutCatalog }) {
  const save = useSyncExternalStore(saveSession!.subscribe, saveSession!.getSnapshot, saveSession!.getSnapshot);
  const observer = useSyncExternalStore(saveObserver!.subscribe, saveObserver!.getSnapshot, saveObserver!.getSnapshot);
  const observerPhase = observer.phase;
  const backendIdle = saveObserver!.canStart();
  const [bundleId, setBundleId] = useState(() => bundles.find(({ scene }) =>
    scene.stance === "mid" && scene.kind === "quick" && scene.buff === "current",
  )?.id ?? bundles[0]?.id ?? "");
  const bundle = bundles.find(b => b.id === bundleId) ?? bundles[0];
  const schools = [...new Set(bundles.map(b => b.school))];
  const formScenes = bundles.filter(b => b.school === bundle?.school);
  const weapons = new Map(formScenes.map(b => [weaponKey(b), b]));
  const weaponScenes = bundle ? bundles.filter(b => weaponKey(b) === weaponKey(bundle)) : [];
  const stances = STANCE_ORDER.filter(stance => weaponScenes.some(b => b.scene.stance === stance));
  const stanceScenes = weaponScenes.filter(b => b.scene.stance === bundle?.scene.stance);
  const attacks = new Map(stanceScenes.map(b => [b.scene.attack_id, b]));
  const buffScenes = stanceScenes.filter(b => b.scene.attack_id === bundle?.scene.attack_id);
  function selectScene(choices: VerifiedBundle[]) {
    const next = preferredScene(choices, bundle);
    if (next) setBundleId(next.id);
  }
  const [changingSave, setChangingSave] = useState(false);
  const [character, setCharacter] = useState<SaveCharacter | null>(null);
  const [sets, setSets] = useState<LoadoutSets | null>(null);
  const [rules, setRules] = useState<RuleDraft[]>([]);
  const [maxLoad, setMaxLoad] = useState("");
  const [minDefense, setMinDefense] = useState("");
  const [minHealth, setMinHealth] = useState("");
  const [movePoints, setMovePoints] = useState(false);
  const [enhanced, setEnhanced] = useState(false);
  const [contextState, dispatchContext] = useReducer(loadoutContextReducer, initialLoadoutContextState);
  const contextStateRef = useRef(contextState);
  contextStateRef.current = contextState;
  const contextRequest = useRef(0);
  const searchOwner = useRef<(LoadoutSearchOwner & { controller: AbortController; source: SaveCharacter }) | null>(null);
  const transition = (event: LoadoutContextEvent) => {
    contextStateRef.current = loadoutContextReducer(contextStateRef.current, event);
    dispatchContext(event);
  };
  const [saveBusy, setSaveBusy] = useState("");
  const [pending, setPending] = useState<Pending>({ kind: "none" });
  const retiringPlan = useRef<string | null>(null);
  const busy = saveBusy || (pending.kind === "retiring" ? "正在撤销换装计划…" :
    pending.kind === "retirement_failed" ? "换装计划尚未撤销，请重试。" : "") || (contextState.kind === "Reading" ? "正在读取角色上下文…" :
    contextState.kind === "Interrupted" ? "搜索连接中断；尚未确认是否停止。" :
    contextState.kind === "Cancelling" ? "正在停止搜索；等待当前批次清理完成…" :
    contextState.kind === "Searching" ? "正在搜索。用时取决于搭配数量、连段长度和后台负载；持续连段可能需要数分钟或更久。" :
    !backendIdle ? "正在处理上一项操作…" : "");
  const [message, setMessage] = useState("");
  const [notice, setNotice] = useState("");
  const [titleConfirmed, setTitleConfirmed] = useState(false);
  const [undo, setUndo] = useState<Undo | null>(null);
  const readGeneration = useRef(0);
  const operationGeneration = useRef(0);
  const requestedOwner = useRef<string | null>(null);
  const viewOwner = useRef("");
  viewOwner.current = (save.selected?.save_id ?? "") + ":" + bundle?.id;
  const mounted = useRef(true);
  const ownedPlan = useRef<string | null>(null);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      readGeneration.current++;
      searchOwner.current?.controller.abort();
      if (ownedPlan.current) {
        void saveSession!.discardReviewedPlan(ownedPlan.current).catch(error => window.review.log(String(error)));
      }
    };
  }, []);

  const items = useMemo(() => (character && bundle ? eligible(bundle, character.equipment) : []), [character, bundle]);
  const counts = useMemo(() => {
    const by = new Map<number, number>();
    for (const item of items) by.set(item.slot, (by.get(item.slot) ?? 0) + 1);
    return by;
  }, [items]);
  const byIndex = useMemo(
    () => new Map((character?.equipment ?? []).map(entry => [entry.slot_index, entry])),
    [character],
  );

  // The page opens on the save the game wrote last; "更换" picks another for this visit. Saves are read only once
  // that discovery has settled: the protected host runs one operation at a time, so a read started alongside it
  // would be refused as busy.
  const [opening, setOpening] = useState<"discovering" | "ready">("discovering");
  useEffect(() => {
    let live = true;
    void discoverSaves()
      .then(saves => {
        const pick = newest(saves);
        if (live && pick && !saveSession!.getSnapshot().selected)
          return saveSession!.select(pick);
      })
      .catch(error => live && setMessage(loadoutError(error)))
      .finally(() => live && setOpening("ready"));
    return () => { live = false; };
  }, []);

  const selectedId = save.selected?.save_id ?? "";
  const sourceSha = save.inventory?.save_id === selectedId ? save.inventory.source_sha256 : "";
  const scope = useMemo<LoadoutScope | null>(() => selectedId && bundle ?
    { saveId: selectedId, sourceSha256: sourceSha, bundleId: bundle.id } : null, [selectedId, sourceSha, bundle?.id]);
  const inputKey = JSON.stringify({ rules, maxLoad, minDefense, minHealth, movePoints, enhanced });
  const liveView = useRef({ scope, inputKey });
  liveView.current = { scope, inputKey };
  const context = currentLoadoutContext(contextState, scope);
  const readSources = contextState.kind === "SourcesRead" && sameLoadoutScope(contextState.scope, scope) ? contextState.sources : null;
  const result = contextState.kind === "Results" && context && contextState.inputKey === inputKey ?
    { ...contextState.result, source: contextState.source, bundleId: contextState.scope.bundleId } : null;
  // A prepared plan is displayable only while the exact result and service plan
  // still belong to this view. Retirement keeps its own ID until discard settles.
  const pendingIsCurrent = pending.kind === "planned" && save.plan?.plan_id === pending.planId &&
    (!pending.binding || canWearLoadout(contextState, pending.binding, scope, inputKey));
  useEffect(() => {
    if (pending.kind !== "planned" || pendingIsCurrent) return;
    setTitleConfirmed(false);
    setPending({ kind: "retiring", planId: pending.planId, reason: "changed" });
  }, [pending, pendingIsCurrent]);
  useEffect(() => {
    if (pending.kind !== "retiring" || save.busy || (!backendIdle && observerPhase !== "interrupted") || retiringPlan.current !== null) return;
    const { planId, reason } = pending;
    retiringPlan.current = planId;
    void saveSession!.discardReviewedPlan(planId).then(() => {
      setPending(current => current.kind === "retiring" && current.planId === planId ? { kind: "none" } : current);
      if (reason === "changed") setNotice("条件已改变，旧换装计划已撤销。请重新搜索。");
    }, error => {
      setPending(current => current.kind === "retiring" && current.planId === planId ?
        { kind: "retirement_failed", planId, reason, message: loadoutError(error) } : current);
    }).finally(() => { if (retiringPlan.current === planId) retiringPlan.current = null; });
  }, [pending, save.busy, backendIdle, observerPhase]);
  useEffect(() => {
    const owner = searchOwner.current;
    if (owner && !sameLoadoutScope(owner.binding, scope)) owner.controller.abort();
    transition({ type: "ScopeChanged", scope });
  }, [selectedId, sourceSha, bundle?.id]);
  useEffect(() => {
    transition({ type: "InputsChanged", inputKey });
    if (searchOwner.current && searchOwner.current.binding.inputKey !== inputKey) searchOwner.current.controller.abort();
  }, [inputKey]);
  useEffect(() => {
    readGeneration.current++;
    setCharacter(null);
    setSets(null);
    transition({ type: "ScopeChanged", scope: null });
    setPending({ kind: "none" });
    setTitleConfirmed(false);
    setUndo(selectedId ? readUndo(selectedId) : null);
    requestedOwner.current = null;
  }, [opening, selectedId, bundle?.id]);
  useEffect(() => {
    if (opening === "ready" && selectedId && bundle && !save.busy && !busy &&
      requestedOwner.current !== viewOwner.current) void read();
  }, [opening, selectedId, bundle?.id, sourceSha, save.busy, busy, observerPhase]);

  useEffect(() => {
    if (character && (!save.inventory || !sameSource(character, save.inventory))) {
      readGeneration.current++;
      requestedOwner.current = null;
      setCharacter(null);
      setSets(null);
      transition({ type: "ScopeChanged", scope: null });
      setPending({ kind: "none" });
      setTitleConfirmed(false);
    }
  }, [character, save.inventory]);

  async function run(label: string, task: () => Promise<void>) {
    const operation = ++operationGeneration.current;
    setSaveBusy(label);
    setMessage("");
    try {
      await task();
    } catch (error) {
      if (operation === operationGeneration.current) setMessage(loadoutError(error));
    } finally {
      if (operation === operationGeneration.current) setSaveBusy("");
    }
  }

  function read() {
    const generation = ++readGeneration.current;
    const owner = viewOwner.current;
    requestedOwner.current = owner;
    const currentRead = () => generation === readGeneration.current && owner === viewOwner.current;
    let requestId = ++contextRequest.current;
    let requestScope = liveView.current.scope;
    if (requestScope) transition({ type: "ReadStarted", scope: requestScope, requestId, inputKey: liveView.current.inputKey });
    setPending({ kind: "none" });
    setTitleConfirmed(false);
    setCharacter(null);
    setSets(null);
    return run("正在读取存档装备…", async () => {
      try {
        const selected = saveSession!.getSnapshot().selected;
        if (!selected || !bundle) return;
        await saveSession!.refresh();
        if (!currentRead()) return;
        const loaded = await saveObserver!.run(() =>
          window.operations.execute({ method: "save.character", params: { save_id: selected.save_id } }),
        );
        if (!loaded || !("equipment_slots" in loaded)) throw new Error("UNEXPECTED_CHARACTER_SNAPSHOT");
        const next = loaded as SaveCharacter;
        if (!currentRead()) return;
        const snapshot = saveSession!.getSnapshot().inventory;
        if (!snapshot || !sameSource(next, snapshot)) throw new Error("SAVE_VIEW_STALE");
        requestScope = { saveId: next.save_id, sourceSha256: next.source_sha256, bundleId: bundle.id };
        requestId = ++contextRequest.current;
        transition({ type: "ReadStarted", scope: requestScope, requestId, inputKey: liveView.current.inputKey });
        const rawContext = await saveObserver!.run(() =>
          window.operations.execute({ method: "save.loadout_context", params: { save_id: next.save_id, source_sha256: next.source_sha256, bundle: bundle.id } }),
        );
        if (!currentRead()) return;
        if (!rawContext || !("state" in rawContext)) throw new Error("UNEXPECTED_LOADOUT_CONTEXT");
        const nextContext = rawContext as LoadoutContext;
        if (nextContext.state === "sources_read") {
          if (nextContext.save_id !== next.save_id || nextContext.source_sha256 !== next.source_sha256 ||
              nextContext.bundle !== bundle.id || nextContext.school !== bundle.school) throw new Error("OPTIMIZER_CONTEXT_STALE");
          const latest = saveSession!.getSnapshot().inventory;
          if (!latest || !sameSource(next, latest)) throw new Error("SAVE_VIEW_STALE");
          setCharacter(next);
          transition({type:"SourcesRead", scope:requestScope, requestId, sources:nextContext});
          return;
        }
        if (nextContext.state === "unavailable") {
          setCharacter(next);
          transition({ type: "ReadFailed", scope: requestScope, requestId, message: loadoutError(nextContext.reason) });
          return;
        }
        if (nextContext.state !== "ready" || nextContext.save_id !== next.save_id || nextContext.source_sha256 !== next.source_sha256 ||
            nextContext.school !== bundle.school || nextContext.bundle !== bundle.id) throw new Error("OPTIMIZER_CONTEXT_STALE");
        const found = await saveObserver!.run(() =>
          window.operations.execute({ method: "save.loadout_sets", params: {
            save_id: next.save_id, source_sha256: next.source_sha256, bundle: bundle.id,
            context_id: nextContext.context_id, items: eligible(bundle, next.equipment),
          } }),
        );
        if (!found || !("sets" in found)) throw new Error("UNEXPECTED_LOADOUT_SETS");
        if (!currentRead()) return;
        const latest = saveSession!.getSnapshot().inventory;
        if (!latest || !sameSource(next, latest)) throw new Error("SAVE_VIEW_STALE");
        const list = found as LoadoutSets;
        if (list.context_id !== nextContext.context_id || list.bundle !== bundle.id) throw new Error("OPTIMIZER_CONTEXT_STALE");
        setCharacter(next);
        // One candidate can combine with retained gear or repeated native sources.
        setSets(list);
        setRules(current => current.map(rule => {
          const options = rule.options.filter(option => list.sets.some(set => set.key === option.key));
          return { options, atLeast: Math.min(rule.atLeast, options.length) };
        }).filter(rule => rule.options.length));
        transition({ type: "ReadReady", scope: requestScope, requestId, context: nextContext });
      } catch (error) {
        if (requestScope) transition({ type: "ReadFailed", scope: requestScope, requestId, message: loadoutError(error) });
        if (currentRead()) throw error;
      }
    });
  }

  async function search() {
    const requestScope = liveView.current.scope;
    const ready = currentLoadoutContext(contextStateRef.current, requestScope);
    const firstRule = rules[0];
    if (!character || !bundle || !requestScope || !ready || !firstRule) return;
    const searchRule = (rule: RuleDraft) => ({ options: rule.options, at_least: rule.atLeast });
    const limits = loadoutLimitsFromText(maxLoad, minDefense, minHealth);
    if (!limits) return;
    const requestId = ++contextRequest.current;
    const requestedInputs = liveView.current.inputKey;
    const source = structuredClone(character);
    transition({ type: "InputsChanged", inputKey: requestedInputs });
    transition({ type: "SearchStarted", scope: requestScope, contextId: ready.context_id, requestId, inputKey: requestedInputs });
    if (contextStateRef.current.kind !== "Searching" || contextStateRef.current.requestId !== requestId) return;
    setMessage("");
    const controller = new AbortController();
    const owner = { controller, jobId: null as string | null, clientRequestId: crypto.randomUUID(), source,
      binding: { ...requestScope, contextId: ready.context_id, requestId, inputKey: requestedInputs } };
    searchOwner.current = owner;
    try {
      const found = await saveObserver!.run(async () => {
        const accepted = await window.operations.execute({ method: "save.loadout_search", params: {
        save_id: source.save_id, source_sha256: source.source_sha256, bundle: bundle.id,
        context_id: ready.context_id, client_request_id: owner.clientRequestId, items, sets: [searchRule(firstRule), ...rules.slice(1).map(searchRule)],
        limits, move_points: movePoints, enhanced, keep_current:false,
        } });
        if (accepted && "job_id" in accepted) owner.jobId = accepted.job_id;
        return accepted;
      }, null, { signal: controller.signal });
      finishSearch(owner, found);
    } catch (error) { failSearch(owner, error); }
    finally {
      if (searchOwner.current === owner && saveObserver!.getSnapshot().phase !== "interrupted") searchOwner.current = null;
    }
  }

  function finishSearch(owner: NonNullable<typeof searchOwner.current>, found: unknown) {
    if (searchOwner.current !== owner) return;
    if (!found || typeof found !== "object" || !("best" in found)) throw new Error("UNEXPECTED_LOADOUT_SEARCH");
    if (!("client_request_id" in found) || found.client_request_id !== owner.clientRequestId) throw new Error("OPTIMIZER_RESULT_STALE");
    if (!sameLoadoutScope(owner.binding, liveView.current.scope)) return;
    const snapshot = saveSession!.getSnapshot().inventory;
    if (!snapshot || !sameSource(owner.source, snapshot)) throw new Error("OPTIMIZER_RESULT_STALE");
    transition({ type: "SearchReady", ...owner.binding, scope: owner.binding, result: found as LoadoutSearch, source: owner.source });
  }
  function failSearch(owner: NonNullable<typeof searchOwner.current>, error: unknown) {
    if (searchOwner.current !== owner) return;
    if (saveObserver!.getSnapshot().phase === "interrupted") {
      transition({ type: "SearchInterrupted", ...owner.binding, scope: owner.binding, message: loadoutError(error) });
    } else if (/LOADOUT_SEARCH_CANCELLED|OPERATION_CANCELLED_BEFORE_SUBMISSION/.test(errorText(error))) {
      transition({ type: "SearchCancelled", ...owner.binding, scope: owner.binding });
      if (/OPERATION_CANCELLED_BEFORE_SUBMISSION/.test(errorText(error))) searchOwner.current = null;
    } else transition({ type: "SearchFailed", ...owner.binding, scope: owner.binding, message: loadoutError(error) });
  }
  function cancelSearch() {
    const owner = searchOwner.current, state = contextStateRef.current;
    if (!owner || (state.kind !== "Searching" && state.kind !== "Interrupted")) return;
    transition({ type: "SearchCancelRequested", ...owner.binding, scope: owner.binding });
    owner.controller.abort();
    if (state.kind === "Interrupted" && observer.job?.job_id === owner.jobId && observer.phase === "running") void saveObserver!.cancel();
  }
  useEffect(() => {
    const owner = searchOwner.current;
    if (!owner || !observer.recovered || observer.phase === "interrupted" || observer.phase === "submitting") return;
    const identified = identifiedLoadoutSearchJob(observer.job, owner);
    if (identified) {
      owner.jobId = identified;
      transition({ type: "SearchResumed", ...owner.binding, scope: owner.binding });
      if (observer.phase === "running" && owner.controller.signal.aborted && observer.job?.state === "running") void saveObserver!.cancel();
      if (observer.phase === "completed" || observer.phase === "failed") {
        try {
          if (observer.phase === "completed") finishSearch(owner, observer.result);
          else failSearch(owner, observer.error ?? "LOADOUT_SEARCH_FAILED");
        } catch (error) { failSearch(owner, error); }
        if (searchOwner.current === owner) searchOwner.current = null;
      }
    } else if (saveObserver!.canStart()) {
      transition({ type: "SearchRetired", ...owner.binding, scope: owner.binding });
      if (sameLoadoutScope(owner.binding, liveView.current.scope)) setNotice("未能恢复这次搜索结果，请重新搜索。");
      if (searchOwner.current === owner) searchOwner.current = null;
    }
  }, [observer.job, observer.phase, observer.recovered]);
  const searchProgress = loadoutSearchProgress(observer.job, searchOwner.current);
  /** Plan putting `wear` on (and taking `clear` off) in the bundle's school, for the player to confirm. */
  type WornSlot = 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10;
  function planWear(
    change: { wear: Row } | { restore: Undo },
    purpose: "wear" | "undo",
  ) {
    return run(purpose === "wear" ? "正在准备换装…" : "正在准备换回…", async () => {
      if (!bundle || !character) return;
      const generation = readGeneration.current;
      const state = contextStateRef.current;
      const binding: LoadoutWearBinding | null = "wear" in change && state.kind === "Results" ? {
        ...state.scope, contextId: state.context.context_id, requestId: state.requestId, inputKey: state.inputKey,
      } : null;
      if ("wear" in change && (!binding || !canWearLoadout(state, binding, liveView.current.scope, liveView.current.inputKey)))
        throw new Error("OPTIMIZER_RESULT_STALE");
      const source = "wear" in change ? result?.source : character;
      const snapshot = saveSession!.getSnapshot().inventory;
      if (!source || !snapshot || !sameSource(source, snapshot) || !sameSource(source, character))
        throw new Error("OPTIMIZER_RESULT_STALE");
      if ("wear" in change) {
        if (!result || result.bundleId !== bundle.id || !result.best.includes(change.wear))
          throw new Error("OPTIMIZER_RESULT_STALE");
        for (const item of change.wear.items) {
          const original = source.equipment.find(entry => entry.slot_index === item.slot_index);
          const current = byIndex.get(item.slot_index);
          const slot = current && slotFor(bundle, current.item_id, current.type_class);
          if (!original || !current || original.item_id !== item.item_id ||
            JSON.stringify(original) !== JSON.stringify(current) || current.rarity > MAX_RARITY ||
            (slot !== item.slot && !(slot === 9 && item.slot === 10)))
            throw new Error("OPTIMIZER_RESULT_STALE");
        }
      }
      const plan = await saveSession!.prepareCharacterEdit({
        ...(binding ? { context_id: binding.contextId } : {}),
        wear:
          "restore" in change
            ? {
                school: change.restore.school,
                items: [],
                restore: change.restore.restore.map(r => ({ slot_index: r.slot_index, slot: r.slot as WornSlot | null })),
              }
            : {
                school: bundle.school,
                items: change.wear.items.map(item => ({ slot_index: item.slot_index, slot: item.slot as WornSlot })),
                ...(change.wear.move_previous_weapon_to_slot === undefined ? {} : {
                  move_previous_weapon_to_slot: change.wear.move_previous_weapon_to_slot,
                }),
              },
      }, snapshot);
      ownedPlan.current = plan.plan_id;
      if (!mounted.current) {
        await saveSession!.discardReviewedPlan(plan.plan_id);
        return;
      }
      if (generation !== readGeneration.current || binding && !canWearLoadout(contextStateRef.current, binding, liveView.current.scope, liveView.current.inputKey)) {
        // Keep ownership through a failed/lost discard acknowledgement too.
        setPending({ kind: "retiring", planId: plan.plan_id, reason: "changed" });
        return;
      }
      const worn = ((plan.preview as Record<string, unknown>).worn ?? []) as WornLine[];
      if (!worn.length) {
        setPending({ kind: "retiring", planId: plan.plan_id, reason: "cancelled" });
        setNotice("你身上已经是这套装备，不需要写入。");
        return;
      }
      const lines = worn.map(line =>
        (line.after != null ? "穿上的装备：" : "卸下的装备：") + slotText(line.after ?? line.before) + " " + itemText(line.item_id),
      );
      const restore: Undo["restore"] = [];
      for (const line of worn) {
        const original = source.equipment.find(item => item.slot_index === line.slot_index);
        if (!original || original.item_id !== line.item_id) {
          // The service already owns this plan. Keep its exact ID until discard
          // succeeds, even when the claimed preview cannot identify an item.
          setPending({ kind: "retiring", planId: plan.plan_id, reason: "changed" });
          setMessage(loadoutError("OPTIMIZER_RESULT_STALE"));
          return;
        }
        restore.push({ slot_index: line.slot_index, slot: line.before, item_id: line.item_id,
          inventory_key: original.inventory_key, seed: original.seed });
      }
      const back: Undo = { school: bundle.school, restore };
      setTitleConfirmed(false);
      setPending({ kind: "planned", planId: plan.plan_id, lines, undo: purpose === "wear" ? back : null, purpose, binding });
    });
  }

  function wearRow(row: Row) {
    void planWear({ wear: row }, "wear");
  }

  function undoWear() {
    if (!undo || !character || !selectedId) return;
    const moved = undo.restore.find(item => !matchesLoadoutUndoItem(item, byIndex.get(item.slot_index)));
    if (moved) {
      setMessage("原来的装备已经不在背包原来的位置（可能被卖掉或分解了），无法自动换回。请在游戏里手动换装，或到“存档管理”恢复换装前的自动备份。缺少的装备：" + itemText(moved.item_id));
      return;
    }
    void planWear({ restore: undo }, "undo");
  }

  function commit() {
    if (pending.kind !== "planned") return;
    const planned = pending;
    return run("正在写入存档…", async () => {
      if (planned.binding && !canWearLoadout(contextStateRef.current, planned.binding, liveView.current.scope, liveView.current.inputKey)) throw new Error("OPTIMIZER_RESULT_STALE");
      if (!titleConfirmed) throw new Error("请先让游戏回到标题界面，再勾选“游戏已回到标题界面”。");
      const receipt = await saveSession!.commit(planned.planId);
      setPending({ kind: "none" });
      if (receipt.commit_status !== "committed")
        throw new Error("写入结果不确定，请到“存档管理”核对这次操作的结果。");
      writeUndo(selectedId, planned.undo);
      setUndo(planned.undo);
      // The worn outfit changed, so the shown comparison with it no longer holds.
      transition({ type: "ScopeChanged", scope: null });
      await read();
      setNotice(
        planned.purpose === "wear"
          ? "已换上这套装备，原存档已自动备份。进游戏读取这个存档即可；不满意可以点“换回原来的装备”。"
          : "已换回原来的装备。",
      );
    });
  }

  function discard() {
    if (pending.kind !== "planned") return;
    setTitleConfirmed(false);
    setPending({ kind: "retiring", planId: pending.planId, reason: "cancelled" });
  }

  const setLabel = (key: number) => {
    const s = sets?.sets.find(x => x.key === key);
    const named = s?.effect ?? s?.item_ids.map(id => setOf(id)).find(id => id != null);
    return named != null ? setName(named) : "套装 " + key.toString(16);
  };
  const usedKeys = new Set(rules.flatMap(r => r.options.map(o => o.key)));
  const invalidLimits = loadoutLimitsFromText(maxLoad, minDefense, minHealth) === null;
  const reduction = (sets?.reduction_items ?? 0) > 0;
  // Native admission includes retained gear and repeated sources; candidate
  // slot counts cannot prove that an active tier is unreachable.

  if (!bundle) return null;
  return (
    <main className="equipment-page loadout-page">
      {!context && !readSources && <p className="loadout-beta">
        角色上下文尚未就绪，暂不能搜索。不会使用样本角色代替。
      </p>}
      <section className="character-hint" data-testid="loadout-context" data-state={contextState.kind} data-context-save-id={context?.save_id} data-context-id={context?.context_id} aria-label="角色计算上下文">
        <strong>{context || readSources ? "已读取角色状态" : "请先读取角色状态"}</strong>
        {context ? <>
          <p>{context.source === "live" ? "来源：当前游戏" : "来源：已验证的角色快照"} · <span>等级 {context.level}</span> · <span>{context.school === "ninja" ? "忍者" : "武士"}</span></p>
          <p>{ATTRIBUTE_NAMES.map((name, index) => <span className="loadout-count" key={name}>{name} {context.attrs[index]}</span>)}</p>
          {bundle.scene.buff !== "current" && <p>增益条件为本次计算的假设，角色与装备来自同一次读取。</p>}
          <p>读取时间：<time dateTime={context.captured_at}>{context.captured_at}</time></p>
          <p>技能：{context.skills.length ? context.skills.map(skill => `${skill.name} (${skill.rank})`).join("、") : "未记录"}</p>
          <p>增益：{context.buffs.length ? context.buffs.map(buff => buff.name).join("、") : context.source === "live" ? "未检测到临时增益" : "未记录"}</p>
          <LoadoutSourceCheck context={context} />
          {context.notes.length > 0 && <ul>{context.notes.map((note, index) => <li key={index}>{note}</li>)}</ul>}
        </> : readSources ? <>
          <p>{readSources.source === "live" ? "来源：当前游戏" : "来源：已验证的角色快照"} · <span>等级 {readSources.level}</span> · <span>{readSources.school === "ninja" ? "忍者" : "武士"}</span></p>
          <p>{ATTRIBUTE_NAMES.map((name, i) => <span className="loadout-count" key={name}>{name} {readSources.attrs[i]}</span>)}</p>
          <p>{readSources.source_state === "current_maps_matched" ? "当前装备的加成已与游戏核对。此攻击仍缺少计算伤害所需的数据。" : "重算的装备加成与游戏读数有差异，目前无法给出可靠排名。"}</p>
        </> : <p>读取成功后才能使用当前角色搜索。</p>}
        <button className="link-button" onClick={read} disabled={!!busy || !save.selected}>刷新角色状态</button>
      </section>
      <div className="loadout-form loadout-choices">
        <div className="loadout-field loadout-save">
          <span>存档</span>
          <b>{save.selected ? saveLines(save.selected)[0] : "正在检测存档…"}</b>
          <button className="link-button" onClick={() => setChangingSave(v => !v)} disabled={!!busy}>
            {changingSave ? "收起" : "更换"}
          </button>
          <button className="link-button" onClick={read} disabled={!!busy || !save.selected}>重新读取</button>
        </div>
        <label className="loadout-field">
          <span>流派</span>
          <select aria-label="流派" value={bundle.school} disabled={!!busy}
            onChange={event => selectScene(bundles.filter(b => b.school === event.target.value))}>
            {schools.map(school => <option key={school} value={school}>{school === "ninja" ? "忍者" : "武士"}</option>)}
          </select>
        </label>
        <label className="loadout-field">
          <span>武器类型</span>
          <select aria-label="武器类型" value={weaponKey(bundle)} disabled={!!busy}
            onChange={event => selectScene(bundles.filter(b => weaponKey(b) === event.target.value))}>
            {[...weapons].map(([key, b]) => (
              <option key={key} value={key}>{b.weapon}</option>
            ))}
          </select>
        </label>
        <label className="loadout-field">
          <span>段位</span>
          <select aria-label="段位" value={bundle.scene.stance} disabled={!!busy}
            onChange={event => selectScene(weaponScenes.filter(b => b.scene.stance === event.target.value))}>
            {stances.map(stance => <option key={stance} value={stance}>{STANCE_LABEL[stance]}</option>)}
          </select>
        </label>
        <label className="loadout-field">
          <span>攻击</span>
          <select aria-label="攻击" value={bundle.scene.attack_id} disabled={!!busy}
            onChange={event => selectScene(stanceScenes.filter(b => b.scene.attack_id === event.target.value))}>
            {[...attacks].map(([id, b]) => <option key={id} value={id}>{attackLabel(b)}</option>)}
          </select>
        </label>
        <label className="loadout-field">
          <span>增益条件</span>
          <select aria-label="增益条件" value={bundle.id} disabled={!!busy}
            onChange={event => setBundleId(event.target.value)}>
            {buffScenes.map(b => <option key={b.id} value={b.id}>{BUFF_LABEL[b.scene.buff]}</option>)}
          </select>
        </label>
      </div>
      <LoadoutCatalogPicker catalog={catalog} disabled={!!busy || !backendIdle}
        onSelect={ids => selectScene(bundles.filter(candidate => ids.includes(candidate.id)))} />
      {changingSave && <SavePicker compact />}
      {character && (
        <p className="character-hint">
          可参与搜索的装备：
          {SLOT_ORDER.filter(slot => slot !== 10 && (slot !== 0 || (bundle.selection_weapon_class ?? bundle.weapon_class) != null)).map(slot => (
            <span key={slot} className="loadout-count">
              {slot === 9 ? "饰品（两栏任选两件）" : SLOT_LABEL[slot]} {counts.get(slot) ?? 0}
            </span>
          ))}
          {(bundle.selection_weapon_class ?? bundle.weapon_class) == null && <span className="loadout-count">武器沿用计算上下文中的装备</span>}
          稀有度 6 及以上的装备暂不参与。某个部位没有装备时，沿用计算上下文中的对应装备。
        </p>
      )}
      {sets && (
        <section className="loadout-rules">
          <h3>套装条件</h3>
          {reduction && (
            <p className="character-hint">
              你的背包里有带“组合效果的所需装备数 -1”的饰品（如八尺琼勾玉）。这里的件数都指生效的件数：戴着它时，穿 2 件就按 3 件生效。搜索会自动比较戴和不戴它。
            </p>
          )}

          {rules.map((rule, r) => (
            <div key={r} className="loadout-rule">
              {rule.options.length > 1 && (
                <label className="loadout-field">
                  <span>其中要满足的套数</span>
                  <select value={rule.atLeast} onChange={event => setRules(rules.map((x, i) => i === r ? { ...x, atLeast: Number(event.target.value) } : x))} disabled={!!busy}>
                    {rule.options.map((_, n) => <option key={n} value={n + 1}>{n + 1 === rule.options.length ? "全部" : String(n + 1)}</option>)}
                  </select>
                </label>
              )}
              {rule.options.map((option, o) => (
                <span key={option.key} className="loadout-chip">
                  {setLabel(option.key)}
                  <select aria-label="生效件数" value={option.pieces} disabled={!!busy}
                    onChange={event => setRules(rules.map((x, i) => i === r ? { ...x, options: x.options.map((y, j) => j === o ? { ...y, pieces: Number(event.target.value) } : y) } : x))}>
                    {Array.from({ length: 6 }, (_, n) => n + 2).map(n => <option key={n} value={n}>{"≥ " + n}</option>)}
                  </select>
                  <button className="link-button" aria-label="删除这个套装" disabled={!!busy}
                    onClick={() => setRules(rules.flatMap((x, i) => {
                      if (i !== r) return [x];
                      const options = x.options.filter((_, j) => j !== o);
                      return options.length ? [{ options, atLeast: Math.min(x.atLeast, options.length) }] : [];
                    }))}>×</button>
                </span>
              ))}
              {usedKeys.size < MAX_SET_KEYS && (
                <select value="" disabled={!!busy} aria-label="加入套装"
                  onChange={event => {
                    const key = Number(event.target.value);
                    setRules(rules.map((x, i) => i === r ? { options: [...x.options, { key, pieces: 2 }], atLeast: x.atLeast === x.options.length ? x.options.length + 1 : x.atLeast } : x));
                  }}>
                  <option value="">＋ 加入套装</option>
                  {sets.sets.filter(s => !rule.options.some(o => o.key === s.key)).map(s => <option key={s.key} value={s.key}>{setLabel(s.key)}</option>)}
                </select>
              )}
            </div>
          ))}
          {rules.length < MAX_RULES && usedKeys.size < MAX_SET_KEYS && sets.sets.length > 0 && (
            <select value="" disabled={!!busy} aria-label="添加套装条件"
              onChange={event => setRules([...rules, { options: [{ key: Number(event.target.value), pieces: 3 }], atLeast: 1 }])}>
              <option value="">＋ 添加一条套装条件</option>
              {sets.sets.filter(s => !usedKeys.has(s.key)).map(s => <option key={s.key} value={s.key}>{setLabel(s.key)}</option>)}
            </select>
          )}
          {rules.length > 1 && <p className="character-hint">几条条件需要同时满足。</p>}
        </section>
      )}
      {sets && (
        <div className="loadout-form">
          <label className="loadout-field">
            <span>负重上限（%）</span>
            <input inputMode="decimal" placeholder="不限" value={maxLoad} onChange={event => setMaxLoad(event.target.value)} />
          </label>
          <label className="loadout-field">
            <span>防御至少</span>
            <input inputMode="numeric" placeholder="不限" value={minDefense} onChange={event => setMinDefense(event.target.value)} />
          </label>
          <label className="loadout-field">
            <span>体力至少</span>
            <input inputMode="numeric" placeholder="不限" value={minHealth} onChange={event => setMinHealth(event.target.value)} />
          </label>
          <label className="loadout-check">
            <input type="checkbox" checked={movePoints} onChange={event => setMovePoints(event.target.checked)} />
            <span>允许重新分配属性点</span>
          </label>
          <label className="loadout-check" title="按铁匠魂合能达到的上限估算：装备等级升到 180，已有 + 值的装备升到角色的 + 值上限。实际穿上后要去铁匠魂合才有这个伤害。">
            <input type="checkbox" checked={enhanced} onChange={event => setEnhanced(event.target.checked)} />
            <span>按强化到上限计算</span>
          </label>
          <button data-testid="loadout-search-start" className="primary" onClick={search} disabled={!!busy || !context || invalidLimits || rules.length === 0 || items.length === 0 || pending.kind !== "none"}>开始搜索</button>
          {rules.length === 0 && <p className="character-message">请指定至少一个套装及其生效件数。</p>}
          {invalidLimits && <p className="character-message" role="alert">负重上限请填大于 0 且不超过 1000 的百分数；防御和体力请填 0 到 1000000 的整数。不设限制时留空。</p>}
        </div>
      )}
      {undo && pending.kind === "none" && (
        <p className="character-hint">
          上次在这里换过装备。
          <button className="link-button" onClick={undoWear} disabled={!!busy}>换回原来的装备</button>
        </p>
      )}
      {busy && <p className="character-hint" role="status">{busy}</p>}
      {(contextState.kind === "Searching" || contextState.kind === "Cancelling" || contextState.kind === "Interrupted") && (
        <section className="character-hint loadout-search-progress" data-testid="loadout-search-progress" data-search-state={contextState.kind} data-search-job-id={searchOwner.current?.jobId ?? ""}>
          {searchProgress && <>
            <p><span>{loadoutSearchStageLabel(searchProgress.stage)}</span>{searchProgress.backend !== "unselected" &&
              <span className="loadout-count">{" · "}{searchProgress.backend === "card" ? "显卡加速" : "CPU 计算"}</span>}</p>
            <p><span>已完成搭配：</span> {searchProgress.completed.toLocaleString()}{searchProgress.total != null && <> / {searchProgress.total.toLocaleString()}</>}</p>
            {searchProgress.batchTotal != null && searchProgress.batchTotal > 0 ?
              <progress aria-label="当前批次进度" max={searchProgress.batchTotal} value={searchProgress.batchCompleted} /> :
              searchProgress.total != null && searchProgress.total > 0 &&
              <progress aria-label="搜索完成进度" max={searchProgress.total} value={searchProgress.completed} />}
            {searchProgress.batchTotal != null && <p><span>当前批次：</span> {searchProgress.batchCompleted.toLocaleString()} / {searchProgress.batchTotal.toLocaleString()}</p>}
          </>}
          <button data-testid="loadout-search-cancel" type="button" onClick={cancelSearch} disabled={contextState.kind === "Cancelling"}>取消搜索</button>
        </section>
      )}
      {contextState.kind === "Cancelled" && <p className="character-hint" role="status" data-testid="loadout-search-cancelled">搜索已取消，可以重新搜索。</p>}
      {observer.phase === "interrupted" && searchOwner.current &&
        <button data-testid="loadout-search-recover" type="button" onClick={() => { void saveObserver!.recover(); }}>重新连接搜索状态</button>}
      {(message || contextState.kind === "Failed") && <p className="character-message" role="alert">{message || (contextState.kind === "Failed" ? contextState.message : "")}</p>}
      {notice && !busy && <p className="character-hint" role="status">{notice}</p>}
      {pending.kind === "retirement_failed" && <section className="character-message" role="alert" data-testid="loadout-plan-retirement-failed">
        <p>{pending.message}</p>
        <button disabled={save.busy || (!backendIdle && observerPhase !== "interrupted")} onClick={() => setPending({ kind: "retiring", planId: pending.planId, reason: pending.reason })}>重试撤销换装计划</button>
      </section>}
      {pending.kind === "planned" && pendingIsCurrent && (
        <section className="character-plan character-save-plan">
          <h3>{pending.purpose === "wear" ? "换上这套装备" : "换回原来的装备"}</h3>
          <ul className="character-plan-lines">{pending.lines.map((line, i) => <li key={i}>{line}</li>)}</ul>
          <label className="character-toggle character-title-confirm">
            <input type="checkbox" checked={titleConfirmed} disabled={!!busy} onChange={event => setTitleConfirmed(event.target.checked)} />
            游戏已回到标题界面
          </label>
          <p className="equipment-notes">只改装备栏，不增减任何物品，所以背包满了也没关系。游戏在读档状态下会自己保存，写入的内容会被覆盖。写入前会自动备份原存档；写入后进游戏重新读取这个存档即可。</p>
          <div className="character-plan-actions">
            <button className="primary" onClick={commit} disabled={!!busy || !titleConfirmed}>写入存档</button>
            <button onClick={discard} disabled={!!busy}>放弃</button>
          </div>
        </section>
      )}
      {result && (
        <LoadoutResults result={result} byIndex={byIndex} movePoints={movePoints} referenceOutfit={context?.source === "captured"} onWear={wearRow} disabled={!!busy || pending.kind !== "none"} />
      )}
    </main>
  );
}

function factsText(f: Facts) {
  return "负重" + " " + (f.load * 100).toFixed(1) + "% · " + "防御" + " " + f.defense + " · " + "体力" + " " + f.health;
}

/** Rows of equal damage, best first; the first of each is the one shown. */
function grouped(best: Row[]): Row[][] {
  const out: Row[][] = [];
  for (const row of best) {
    const last = out[out.length - 1];
    if (last && last[0].damage === row.damage) last.push(row);
    else out.push([row]);
  }
  return out;
}

export function LoadoutResults({ result, byIndex, movePoints, referenceOutfit, onWear, disabled }: {
  result: LoadoutSearch;
  byIndex: Map<number, SaveCharacter["equipment"][number]>;
  movePoints: boolean;
  referenceOutfit: boolean;
  onWear: (row: Row) => void;
  disabled: boolean;
}) {
  const [open, setOpen] = useState<number | null>(0);
  const [showSame, setShowSame] = useState<number | null>(null);
  if (loadoutHasScoreConflict(result)) return <section className="character-message" role="alert" data-testid="loadout-result-coverage" data-coverage="unanswered">
    伤害复核与排名分数不一致，本次结果已作废。请重新读取角色后搜索；仍然出现请反馈。
  </section>;
  const groups = grouped(result.best);
  const current = result.current;
  const unanswered = result.no_path + result.refused;
  const sourceUncomputed = result.source_uncomputed ?? null;
  const sourceModelScope = loadoutSourceModelScope(result.source_model_scope);
  const sourceModelBound = sourceModelScope === "runtime_bound_rebuild";
  const coverage = loadoutAnswerCoverage(result.best.length, unanswered, result.complete === true, sourceUncomputed);
  const incompleteReasons = loadoutIncompleteReasons(unanswered, result.complete === true, movePoints, sourceUncomputed).join(" ");
  const sourceModelNotice = <p className={sourceModelBound ? "character-hint" : "character-message"} data-testid="loadout-source-model-scope" data-source-model-scope={sourceModelScope}>
    {sourceModelScope === "legacy_unverified" ? "旧版来源模型尚未通过完整重建验证。伤害与排名均为当前模型的暂定结果。" :
      sourceModelScope === "conditional_rebuild" ? "来源重建仍受已声明条件限制，尚未与当前角色的主机重建过程绑定。伤害与排名均为当前模型的暂定结果。" :
        "本次搜索的来源重建已与当前角色读取绑定。"}
  </p>;
  const sourceCoverage = sourceUncomputed === null ? <span className="loadout-count" data-testid="loadout-source-uncomputed">来源覆盖范围未报告</span> : sourceUncomputed > 0 && <span className="loadout-count" data-testid="loadout-source-uncomputed">
    <span>来源未计算的搭配</span> {sourceUncomputed.toLocaleString()}<span>（套装筛选前）</span>
  </span>;
  const weaponDomain = result.weapon_domain && <p className="character-hint" data-testid="loadout-weapon-domain" data-policy={result.weapon_domain.policy}>
    {result.weapon_domain.type_class !== null && localize("同类型武器 %owned% 件，已纳入计算 %admitted% 件。")
      .replace("%owned%", result.weapon_domain.requested_items.toLocaleString())
      .replace("%admitted%", result.weapon_domain.admitted_items.toLocaleString())}
    {result.weapon_domain.uncomputed_items > 0 && <span>{" "}{localize("另有 %count% 件武器的效果来源待验证。")
      .replace("%count%", result.weapon_domain.uncomputed_items.toLocaleString())}</span>}
    {result.weapon_domain.retained_current && <span>同时保留当前武器作为选择。</span>}
  </p>;
  switch (coverage) {
    case "infeasible":
      return <section className="character-message" data-testid="loadout-result-coverage" data-coverage={coverage} data-source-model-scope={sourceModelScope}>
        {sourceModelNotice}{weaponDomain}
        <p>{sourceModelBound ? "没有满足条件的搭配。请放宽负重、防御或体力的要求，或减少套装件数后再试。" : "当前模型未找到满足条件的搭配。请放宽负重、防御或体力的要求，或减少套装件数后再试。"}</p>
      </section>;
    case "unanswered":
      return <section className="character-message" data-testid="loadout-result-coverage" data-coverage={coverage} data-source-model-scope={sourceModelScope}>{sourceModelNotice}<p>{incompleteReasons}{" "}暂时无法判断是否存在满足条件的搭配。{unanswered > 0 && <span className="loadout-count">无法计算的搭配 {unanswered.toLocaleString()}</span>}{sourceCoverage}</p>{weaponDomain}</section>;
    case "partial":
    case "complete":
      break;
  }
  return (
    <section className="loadout-results" data-client-request-id={result.client_request_id} data-testid="loadout-result-coverage" data-coverage={coverage} data-source-model-scope={sourceModelScope}>
      {sourceModelNotice}
      {weaponDomain}
      <div className="loadout-cards">
        <div><small data-testid="loadout-baseline-label">{referenceOutfit ? "参考穿戴" : "当前穿戴"}</small><b>{current.status === "answered" && current.damage != null ? current.damage.toFixed(0) : "—"}</b>
          <DamageSummary details={current.status==="answered"?current.damage_details:undefined}/></div>
        <div>
          <small>{coverage === "complete" ? (sourceModelBound ? "最佳搭配" : "当前模型中的最佳搭配") : (sourceModelBound ? "已计算搭配中的最高伤害" : "当前模型已计算搭配中的最高伤害")}</small>
          <b>{result.best[0].damage.toFixed(0)}</b>
          <DamageSummary details={result.best[0].damage_details}/>
          {current.status === "answered" && current.damage ? <em>{percent(result.best[0].damage, current.damage)}</em> : null}
        </div>
        <div><small>已计算</small><b data-testid="loadout-answered-count">{result.answered.toLocaleString()}</b></div>
        <div>
          <small>用时（秒）</small>
          <b>{result.seconds.toFixed(1)}</b>
          <em>{result.backend === "card" ? "显卡加速" : "CPU 计算"}</em>
        </div>
      </div>
      {result.enhanced && <p className="character-hint">以下伤害和对照数值都按强化到上限估算。穿上后去铁匠魂合提高等级和 + 值，才能达到这些数值。</p>}
      {movePoints && <p className="character-hint">伤害按各搭配列出的属性点计算。“穿上这套”只更换装备；需要调整的属性点请在游戏中重新分配。</p>}
      {coverage === "partial" && (
        <p className="character-message">
          {unanswered > 0 && <span className="loadout-count">无法计算的搭配 {unanswered.toLocaleString()}</span>}
          {sourceCoverage}
          {incompleteReasons}{" "}暂不能确认全局最优。
        </p>
      )}
      <ol className="loadout-list">
        {groups.map((group, g) => {
          const row = group[0];
          const expanded = open === g;
          return (
            <li key={g} className={expanded ? "open" : ""}>
              <button className="loadout-row" onClick={() => setOpen(expanded ? null : g)}>
                <b>#{g + 1} · {row.damage.toFixed(0)}</b>
                {group.length > 1 && <span className="loadout-tag"><span>同伤害的搭配</span> +{group.length - 1}</span>}
                <span className="loadout-row-facts">
                  {row.facts ? factsText(row.facts) : ""}
                </span>
              </button>
              <ul className="loadout-sentences">{describeChange(row, current).map((line, i) => <li key={i}>{line}</li>)}</ul>
              {expanded && (
                <div className="loadout-detail">
                  <DamageSummary details={row.damage_details}/>
                  <DamageContacts details={row.damage_details} baseline={current.status==="answered"?current.damage_details:undefined} referenceOutfit={referenceOutfit}/>
                  <Outfit row={row} current={current} byIndex={byIndex} />
                  {movePoints && <Points row={row} start={result.start_attrs} />}
                  <div className="character-plan-actions">
                    <button className="primary" onClick={() => onWear(row)} disabled={disabled}>穿上这套（写入存档）</button>
                    {group.length > 1 && (
                      <button onClick={() => setShowSame(showSame === g ? null : g)}>
                        {showSame === g ? "收起同伤害的搭配" : "看看同伤害的其他搭配"}
                      </button>
                    )}
                  </div>
                  {showSame === g && (
                    <ul className="loadout-same">
                      {group.slice(1).map((other, i) => (
                        <li key={i}>
                          <span>{other.facts ? factsText(other.facts) : ""}</span>
                          <span className="loadout-diff">
                            {other.items.filter(item => row.items.find(x => x.slot === item.slot)?.slot_index !== item.slot_index)
                              .map(item => <span key={item.slot}>{SLOT_LABEL[item.slot]}：{itemName(item.item_id)}</span>)}
                          </span>
                          <DamageSummary details={other.damage_details}/>
                          {movePoints && <Points row={other} start={result.start_attrs} />}
                          <button onClick={() => onWear(other)} disabled={disabled}>穿上这套</button>
                        </li>
                      ))}
                    </ul>
                  )}
                </div>
              )}
            </li>
          );
        })}
      </ol>
    </section>
  );
}

function Outfit({ row, current, byIndex }: {
  row: Row;
  current: Current;
  byIndex: Map<number, SaveCharacter["equipment"][number]>;
}) {
  const worn = current.status === "ambiguous" ? [] : current.items;
  return (
    <ul className="loadout-items">
      {[...row.items].sort((x, y) => SLOT_ORDER.indexOf(x.slot) - SLOT_ORDER.indexOf(y.slot)).map(item => {
        const entry = byIndex.get(item.slot_index);
        const before = worn.find(w => w.slot === item.slot);
        const same = before?.slot_index === item.slot_index;
        return (
          <li key={item.slot} className={same ? "same" : "changed"}>
            <small>{SLOT_LABEL[item.slot] ?? item.slot}</small>
            {itemName(item.item_id)}
            {entry && <small> Lv{entry.level}{entry.plus ? " +" + entry.plus : ""}</small>}
            {!same && before && <small className="loadout-was" title="原来穿的装备">← {itemText(before.item_id)}</small>}
          </li>
        );
      })}
    </ul>
  );
}

/** The game's attribute order in the character record (STAT+0x844), confirmed from each index's defense-reflection group. */
const ATTRIBUTE_NAMES = ["体", "心", "刚", "武", "技", "智", "咒"] as const;

function Points({ row, start }: { row: Row; start: number[] }) {
  const deltas = row.attrs.map((value, i) => value - start[i]);
  if (deltas.every(d => d === 0)) return <p className="character-hint">属性点不用改。</p>;
  return (
    <p className="character-hint">
      属性点：
      {deltas.map((delta, i) => delta ? (
        <span key={i} className="loadout-count">
          {ATTRIBUTE_NAMES[i]} {delta > 0 ? "+" : ""}{delta}（{start[i]}→{row.attrs[i]}）
        </span>
      ) : null)}
    </p>
  );
}
