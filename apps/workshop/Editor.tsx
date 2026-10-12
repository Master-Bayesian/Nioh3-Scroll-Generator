import { Notice } from "./Notice";
import {CountEditor} from "./CountEditor";
import {ScrollCompletion} from "./ScrollCompletion";
import { localize } from "./presentation";
import { useDialogBackdropDismiss } from "./use-dialog-backdrop-dismiss";
import React, { useEffect, useState, useRef } from "react";
import { data, type Sample, toRecordTransferCount } from "./model";
import { desktop } from "./desktop-bridge";
import {
  saveSession,
  runtimeObserver,
  entrySample,
  enrichEntry,
} from "./save-workspace";
import { EMPTY_EFFECT, EffectPicker, hex, type PickerCandidate } from "./EffectPicker";
import { StarIcon } from "./StarIcon";
import { pinnedFirst, togglePinned, usePinned } from "./pinned";
import { SavePicker } from "./CartActions";
import { errorText } from "./public-errors";
import type { ProtectedParams } from "../desktop/src/protected-client";
import rawCatalog from "./editor-values.json";
import type { CountEdit } from "../../packages/contracts/protected-responses";
/** Where a reviewed scroll edit is written: the running game or the save file. */
type WriteMode = "live" | "save";
const WRITE_MODE_KEY = "nioh3-scroll-edit-mode";
function storedWriteMode(): WriteMode {
  try {
    return localStorage.getItem(WRITE_MODE_KEY) === "save" ? "save" : "live";
  } catch {
    return "live";
  }
}
function countEditOf(result: unknown): CountEdit {
  if (!result || typeof result !== "object" || !("count_edit" in result))
    throw Error("COUNT_RESULT_EXPECTED");
  return (result as { count_edit: CountEdit }).count_edit;
}
const rawData = rawCatalog as {
  effects: Record<string, number>;
  patterns: number[][];
  sets: number[][];
};
function nativeValues(
  id: string,
  rarity: number,
  level: number,
): number[] | undefined {
  const pattern = rawData.effects[id];
  if (
    pattern === undefined ||
    !Number.isInteger(level) ||
    level < 0 ||
    level > 180 ||
    ![3, 4, 5].includes(rarity)
  )
    return;
  return rawData.sets[rawData.patterns[pattern][level * 3 + rarity - 3]];
}
/** The natural "min–max" of one effect at this rarity and level, or "". */
function nativeRange(id: string, rarity: number, level: number): string {
  const values = nativeValues(id, rarity, level);
  if (!values?.length) return "";
  const min = values[0], max = values[values.length - 1];
  return min === max ? String(min) : `${min}–${max}`;
}

/**
 * Which effects the slot picker offers. A scroll's effects depend on its
 * playthrough and rarity; offering all 3609 native effects by default made it
 * unclear which ones a scroll can actually carry.
 */
type EffectScope = "context" | "scroll" | "all";
const EFFECT_SCOPE_KEY = "nioh3-scroll-effect-scope";
function storedEffectScope(): EffectScope {
  try {
    const stored = localStorage.getItem(EFFECT_SCOPE_KEY);
    return stored === "scroll" || stored === "all" ? stored : "context";
  } catch {
    return "context";
  }
}
const effectNames = new Map(data.editorEffects.map((e) => [e.id, e.name]));
/** Growth effects (R3 slot 5) are not part of the search contexts. */
const growthIds = [
  ...new Set(
    data.samples.flatMap((s) =>
      s.effects.filter((e) => e.role === "成长词条").map((e) => e.id),
    ),
  ),
];
type Contexts = Record<string, { effects: { id: string }[]; graces: { id: string }[] }>;
function scopeCandidates(scope: EffectScope, ng: string, rarity: string): PickerCandidate[] {
  if (scope === "all") return data.editorEffects.map((e) => ({ id: Number(e.id) }));
  const contexts = data.contexts as Contexts;
  const chosen =
    scope === "context"
      ? [contexts[`${ng}-${rarity}`]].filter(Boolean)
      : Object.values(contexts);
  const ids = new Set<string>();
  for (const context of chosen)
    for (const effect of [...context.effects, ...context.graces]) ids.add(effect.id);
  if (scope === "scroll" || rarity === "3") growthIds.forEach((id) => ids.add(id));
  return [...ids].map((id) => ({ id: Number(id) }));
}
const effectLabel = (candidate: PickerCandidate) =>
  effectNames.get(String(candidate.id)) || "未知词条 " + hex(candidate.id);
/** A star marks one save scroll; the same seed, rarity and playthrough is the same scroll. */
const scrollKey = (s: Sample) =>
  [s.seed, s.rarity, s.saveEntry?.header.playthrough ?? s.playthrough ?? ""].join(":");

type Slot = {
  id: string;
  name: string;
  raw: string;
  prefix: string;
  metadata: string;
  tail0: string;
  tail1: string;
};
type Temporary = {
  enemies: number[];
  originalEnemies: number[];
  capacity: number;
  capacityEnabled?: boolean;
  terrain: string;
  rules: number[];
  enemyEnabled?: boolean;
  terrainEnabled?: boolean;
  rulesEnabled?: boolean;
};
function temporaryFromSample(s: Sample): Temporary {
  return {
    enemies: [...s.enemySlotKeys],
    originalEnemies: [...s.enemySlotKeys],
    capacity: s.capacity,
    terrain:
      data.terrains.find(
        (t) =>
          !t.aggregate &&
          t.effect_keys.length === s.terrainKeys.length &&
          t.effect_keys.every((k) => s.terrainKeys.includes(k)),
      )?.option_id || "",
    rules: Array.from({ length: 3 }, (_, i) => s.rules[i]?.key || 0),
  };
}
type History = { draft: Draft; temporary: Temporary };
type Draft = {
  seed: string;
  ng: string;
  rarity: string;
  level: string;
  recommended: string;
  transfers: string;
  slots: Slot[];
  /** The game's protection (lock) and 喜好项目 (favourite) marks. */
  locked: boolean;
  favorite: boolean;
};
function fromSample(s: Sample): Draft {
  const entry = s.saveEntry;
  return {
    seed: s.seed,
    ng: String(entry?.header.playthrough || s.playthrough || 3),
    rarity: String(s.rarity),
    // A stored level of 0 is a legal value the editor accepts; `||` used to
    // turn it into 180 and rewrite an untouched header field.
    level: String(entry?.header.level ?? s.level ?? 180),
    recommended: String(entry?.derived.recommended_displayed_level ?? 350),
    transfers: entry
      ? String(
          entry.header.transfer_count === 0xffffffff
            ? -1
            : entry.header.transfer_count,
        )
      : "-1",
    locked: !!entry?.locked,
    favorite: !!entry?.favorite,
    slots: Array.from({ length: 7 }, (_, i) => {
      const slot = entry?.effects[i];
      return {
        id: String(slot?.effect_id ?? s.effects[i]?.id ?? 4294967295),
        name: slot
          ? data.editorEffects.find((e) => e.id === String(slot.effect_id))
              ?.name || (slot.effect_id === 0xffffffff ? "空槽" : "未知词条")
          : s.effects[i]?.name || "空槽",
        raw: String(slot?.value ?? s.effects[i]?.raw ?? 0),
        prefix: String(slot?.prefix || 0),
        metadata: String(slot?.metadata || 0),
        tail0: String(slot?.tail_0 || 0),
        tail1: String(slot?.tail_1 || 0),
      };
    }),
  };
}
export function Editor({ cart }: { cart: Sample[] }) {
  const [saveState, setSaveState] = useState(() => saveSession?.getSnapshot());
  const [writeMode, setWriteModeRaw] = useState<WriteMode>(storedWriteMode);
  const [livePlan, setLivePlan] = useState<{ operation_id: string; plan_digest: string } | null>(null);
  const [reviewedPlan, setReviewedPlan] = useState<string | null>(null),
    [saveStateConfirmed, setSaveStateConfirmed] = useState(false),
    [backendBusy, setBackendBusy] = useState(false);
  const [effectScope, setEffectScopeRaw] = useState<EffectScope>(storedEffectScope);
  function setEffectScope(next: EffectScope) {
    setEffectScopeRaw(next);
    try {
      localStorage.setItem(EFFECT_SCOPE_KEY, next);
    } catch {
      // Remembering the scope is a convenience only.
    }
  }
  // The desktop reads the game's own 喜好项目 marks from the save; the browser
  // preview, which has no save, keeps its stars locally.
  const localPins = usePinned("scroll");
  const isFavorite = (s: Sample) => (desktop ? !!s.saveEntry?.favorite : localPins.has(scrollKey(s)));
  const [deleteLockedPrompt, setDeleteLockedPrompt] = useState(0);
  const [deleteLockedConfirmed, setDeleteLockedConfirmed] = useState(false);
  useEffect(() => {
    if (deleteLockedConfirmed) void prepareDelete();
  }, [deleteLockedConfirmed]);
  const [pinnedOnly, setPinnedOnly] = useState(false);
  const activeRead = useRef(0);
  const [deleteSlots, setDeleteSlots] = useState<number[]>([]);
  useEffect(() => setDeleteSlots([]), [saveState?.inventory?.snapshot_id]);
  useEffect(
    () =>
      saveSession?.subscribe(() => setSaveState(saveSession!.getSnapshot())),
    [],
  );
  const cartDialog = useRef<HTMLDialogElement>(null);
  const cartDialogBackdropDismiss = useDialogBackdropDismiss();
  const [inventory, setInventory] = useState<Sample[]>(() =>
    data.samples.filter((s) => s.rarity === 4).slice(0, 12),
  );
  const [current, setCurrent] = useState<Sample>(inventory[0]);
  const [saved, setSaved] = useState<Draft>(() => fromSample(inventory[0]));
  const [draft, setDraftRaw] = useState<Draft>(saved),
    [find, setFind] = useState("");
  const draftIdentity = useRef("");
  draftIdentity.current = JSON.stringify([
    draft,
    current.saveEntry?.slot_index,
    saveState?.inventory?.snapshot_id,
  ]);
  const [message, setMessage] = useState(""),
    [review, setReview] = useState(false),
    [pending, setPending] = useState<Sample | null>(null);
  // The confirmation lives in the side panel; bring it into view whenever a
  // reviewed plan opens it, so a successful check is never invisible.
  const reviewPrompt = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (review) reviewPrompt.current?.scrollIntoView?.({ block: "nearest" });
  }, [review]);
  const [records, setRecords] = useState<Record<string, Draft>>({});
  useEffect(() => {
    setReview(false);
    setReviewedPlan(null);
    setLivePlan(null);
    setSaveStateConfirmed(false);
  }, [draft, writeMode]);
  function setWriteMode(next: WriteMode) {
    setWriteModeRaw(next);
    try {
      localStorage.setItem(WRITE_MODE_KEY, next);
    } catch {
      // Remembering the mode is a convenience only.
    }
  }
  const [undoStack, setUndoStack] = useState<History[]>([]),
    [redoStack, setRedoStack] = useState<History[]>([]);
  function setDraft(next: Draft) {
    setUndoStack((stack) => [...stack, { draft, temporary }].slice(-100));
    setRedoStack([]);
    setDraftRaw(next);
  }
  function undo() {
    if (!undoStack.length) return;
    setRedoStack([...redoStack, { draft, temporary }]);
    setDraftRaw(undoStack[undoStack.length - 1].draft);
    setTemporaryRaw(undoStack[undoStack.length - 1].temporary);
    setUndoStack(undoStack.slice(0, -1));
  }
  function redo() {
    if (!redoStack.length) return;
    setUndoStack([...undoStack, { draft, temporary }]);
    setDraftRaw(redoStack[redoStack.length - 1].draft);
    setTemporaryRaw(redoStack[redoStack.length - 1].temporary);
    setRedoStack(redoStack.slice(0, -1));
  }
  const [temporary, setTemporaryRaw] = useState<Temporary>(() =>
    temporaryFromSample(current),
  );
  function setTemporary(next: Temporary) {
    setUndoStack((stack) => [...stack, { draft, temporary }].slice(-100));
    setRedoStack([]);
    setTemporaryRaw(next);
  }
  useEffect(() => {
    if (!desktop || !saveState?.inventory) return;
    const entries = saveState.inventory.entries.map(entrySample);
    setInventory(entries);
    if (entries.length)
      choose(
        entries.find(
          (s) => s.saveEntry?.slot_index === current.saveEntry?.slot_index,
        ) || entries[0],
      );
    else setMessage("当前存档没有绘卷。");
  }, [saveState?.inventory?.snapshot_id]);
  // The preview shows the dungeon content of the drafted seed and playthrough;
  // a changed seed is read again shortly after typing stops.
  const [previewSample, setPreviewSample] = useState<Sample | null>(null);
  const [previewFailed, setPreviewFailed] = useState(false);
  useEffect(() => {
    setPreviewSample(null);
    setPreviewFailed(false);
    const seed = Number(draft.seed),
      ng = Number(draft.ng);
    if (
      !desktop ||
      (draft.seed === current.seed && ng === (current.playthrough || 3)) ||
      !Number.isInteger(seed) ||
      seed < 0 ||
      seed > 0xffffffff ||
      !Number.isInteger(ng) ||
      ng < 1 ||
      ng > 5
    )
      return;
    let live = true;
    const timer = setTimeout(() => {
      enrichEntry({ ...current, seed: String(seed), playthrough: ng })
        .then((sample) => live && setPreviewSample(sample))
        .catch(() => live && setPreviewFailed(true));
    }, 400);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [draft.seed, draft.ng, current]);
  const seedChanged =
    draft.seed !== current.seed || Number(draft.ng) !== (current.playthrough || 3);
  // Never show the old seed's content under a new seed.
  const shown =
    desktop && seedChanged
      ? (previewSample ?? { ...current, enemies: [], rules: [], terrainKeys: [] })
      : current;
  const contentPending = previewFailed
    ? "无法读取这个绘卷 ID 的副本内容。"
    : seedChanged && !previewSample
      ? "正在读取新绘卷 ID 的副本内容…"
      : "正在读取副本内容…";
  const changed = JSON.stringify(draft) !== JSON.stringify(saved);
  const fields: [keyof Omit<Draft, "slots" | "locked" | "favorite">, string][] = [
    ["seed", "绘卷 ID（种子）"],
    ["ng", "周目"],
    ["rarity", "稀有度"],
    ["level", "绘卷等级"],
    ["recommended", "推荐等级（敌人等级）"],
    ["transfers", "转手次数"],
  ];
  const patch = (index: number, key: keyof Slot, value: string) =>
    setDraft({
      ...draft,
      slots: draft.slots.map((s, i) =>
        i === index ? { ...s, [key]: value } : s,
      ),
    });
  function choose(s: Sample) {
    setCurrent(s);
    const next = records[s.seed] || fromSample(s);
    setSaved(next);
    setDraftRaw(next);
    setUndoStack([]);
    setRedoStack([]);
    setTemporaryRaw(temporaryFromSample(s));
    setPending(null);
    setReview(false);
    if (desktop) {
      const read = ++activeRead.current;
      void enrichEntry(s)
        .then((enriched) => {
          if (activeRead.current !== read) return;
          setCurrent(enriched);
          setTemporaryRaw(temporaryFromSample(enriched));
        })
        .catch((e) => setMessage(errorText(e)));
    }
  }
  async function validate() {
    const identity = draftIdentity.current;
    setMessage("正在核对修改…");
    try {
      toRecordTransferCount(Number(draft.transfers));
      for (const [key] of fields) {
        if (!draft[key].trim() || !Number.isInteger(Number(draft[key])))
          throw Error("请填写有效整数");
      }
      if (Number(draft.seed) < 0 || Number(draft.seed) > 0xffffffff)
        throw Error("绘卷 ID 超出范围");
      if (
        Number(draft.ng) < 1 ||
        Number(draft.ng) > 5 ||
        Number(draft.rarity) < 3 ||
        Number(draft.rarity) > 5
      )
        throw Error("请选择有效周目与稀有度");
      if (Number(draft.level) < 0 || Number(draft.level) > 180)
        throw Error("绘卷等级应为 0–180");
      if (!(draft.recommended in data.levels))
        throw Error("请填写有效的敌人等级");
      // A record stored above the current bound stays editable: when the
      // displayed level is untouched, keep its existing raw value instead of
      // rewriting it to a different canonical raw with the same display.
      const storedRaw = current.saveEntry?.derived.recommended_raw_level;
      const storedDisplay = current.saveEntry?.derived.recommended_displayed_level;
      const preservingStoredRaw =
        typeof storedRaw === "number" &&
        saved.recommended === draft.recommended &&
        String(storedDisplay ?? "") === String(draft.recommended);
      for (const slot of draft.slots)
        for (const k of [
          "id",
          "raw",
          "prefix",
          "metadata",
          "tail0",
          "tail1",
        ] as const) {
          const v = Number(slot[k]);
          if (
            !slot[k].trim() ||
            !Number.isInteger(v) ||
            v < 0 ||
            v > 0xffffffff
          )
            throw Error("词条字段应为 0–4294967295 的整数");
        }
      if (desktop) {
        if (!current.saveEntry) throw Error("请先读取真实存档。");
        setBackendBusy(true);
        // An untouched recommended level keeps its stored raw value, so it
        // needs no conversion; only a changed level asks the search worker.
        let recommendedRaw = storedRaw;
        if (!preservingStoredRaw) {
          setMessage("正在转换推荐等级…");
          const level = await window.nioh.resolveRecommendedLevel(
            Number(draft.recommended),
          );
          if (
            level.status !== "exact" ||
            level.selected_internal_level === null
          )
            throw Error("推荐等级无法转换。");
          recommendedRaw = level.selected_internal_level;
        }
        const edit = {
          slot_index: current.saveEntry.slot_index,
          header: {
            seed: Number(draft.seed),
            playthrough: Number(draft.ng),
            rarity: Number(draft.rarity),
            level: Number(draft.level),
            recommended_level: recommendedRaw!,
            transfer_count: toRecordTransferCount(Number(draft.transfers)),
          },
          effects: draft.slots.map((slot, i) => ({
            slot_index: i,
            effect_id: Number(slot.id),
            value: Number(slot.raw),
            prefix: Number(slot.prefix),
            metadata: Number(slot.metadata),
            tail_0: Number(slot.tail0),
            tail_1: Number(slot.tail1),
          })),
          ...(draft.locked !== saved.locked || draft.favorite !== saved.favorite
            ? { marks: { locked: draft.locked, favorite: draft.favorite } }
            : {}),
        } as ProtectedParams<"save.prepare_edit">["edits"][number];
        if (writeMode === "live") {
          const inventory = saveState?.inventory;
          if (!inventory) throw Error("请先读取真实存档。");
          setMessage("正在核对游戏里的这张绘卷…");
          const { slot_index, ...change } = edit;
          const plan = countEditOf(
            await runtimeObserver!.run(() =>
              window.operations.prepareScrollEdit({
                save_id: inventory.save_id,
                snapshot_id: inventory.snapshot_id,
                slot_index,
                edit: change,
              }),
            ),
          );
          if (plan.state !== "prepared") throw Error(plan.error || "核对失败，请重新读取后再试。");
          if (identity !== draftIdentity.current)
            throw Error("内容已改变，请重新核对修改。");
          setLivePlan({ operation_id: plan.operation_id, plan_digest: plan.plan_digest });
        } else {
          setMessage("正在生成写入计划…");
          const plan = await saveSession!.prepareEdit([edit]);
          if (identity !== draftIdentity.current)
            throw Error("内容已改变，请重新核对修改。");
          setReviewedPlan(plan.plan_id);
          setSaveStateConfirmed(false);
        }
      }
      setReview(true);
      setMessage(
        desktop
          ? writeMode === "live"
            ? `已核对 ${changes.length} 项变化，已自动备份存档，尚未写入游戏。请在右侧“确认修改”中确认后写入。`
            : `已核对 ${changes.length} 项变化，尚未写入存档。请在右侧“确认修改”中确认后写入。`
          : "",
      );
    } catch (e) {
      // Backend failures reject with plain strings; never leave the status
      // line blank, or the button looks like it did nothing.
      setReview(false);
      setMessage(errorText(e, "核对失败，也没有返回原因；请导出反馈文件发给开发者。"));
    } finally {
      setBackendBusy(false);
    }
  }
  async function previewDraft() {
    setBackendBusy(true);
    try {
      const seed = Number(draft.seed),
        ng = Number(draft.ng);
      if (
        !Number.isInteger(seed) ||
        seed < 0 ||
        seed > 0xffffffff ||
        !Number.isInteger(ng) ||
        ng < 1 ||
        ng > 5
      )
        throw Error("请输入有效的绘卷 ID 和周目。");
      const sample = await enrichEntry({
        ...current,
        seed: String(seed),
        playthrough: ng,
      });
      setTemporary(temporaryFromSample(sample));
      setMessage("已预览当前种子的副本内容。");
    } catch (e) {
      setMessage(errorText(e));
    } finally {
      setBackendBusy(false);
    }
  }
  async function inspectTemporary() {
    try {
      const result = await window.operations.execute({
        method: "runtime.status",
        params: {},
      });
      if (result && "override_state" in result)
        setMessage(
          `临时修改：${({ stopped: "未开启", armed_no_hit: "等待打开绘卷", applied_hit: "已生效", unknown: "状态待核对" } as Record<string, string>)[result.override_state] || result.override_state} · 触发 ${result.hit_count} 次 · 待完成调用 ${result.pending_remote_calls}`,
        );
    } catch (e) {
      setMessage(errorText(e));
    }
  }
  const slotNative = (slot: Slot) =>
    slot.id === String(EMPTY_EFFECT)
      ? undefined
      : nativeValues(slot.id, Number(draft.rarity), Number(draft.level));
  const slotRange = (slot: Slot) =>
    slot.id === String(EMPTY_EFFECT) ? "" : nativeRange(slot.id, Number(draft.rarity), Number(draft.level));
  const candidates = React.useMemo(
    () => scopeCandidates(effectScope, draft.ng, draft.rarity),
    [effectScope, draft.ng, draft.rarity],
  );
  const inScope = React.useMemo(
    () => new Set(candidates.map((c) => String(c.id))),
    [candidates],
  );
  function replaceEffect(index: number, candidate: PickerCandidate) {
    if (candidate.id === EMPTY_EFFECT) {
      patchSlot(index, { id: String(EMPTY_EFFECT), name: "空槽", raw: "0" });
      return;
    }
    const effect = { id: String(candidate.id), name: effectLabel(candidate) };
    const identity = (
      data.editorIdentities as Record<
        string,
        { prefix: number; category: number; star?: boolean }
      >
    )[effect.id];
    // A new effect starts at the top of its native values, so the value the
    // row shows is one the game can roll.
    const values = nativeValues(effect.id, Number(draft.rarity), Number(draft.level));
    // The star (+0x0E bit 0x04, metadata bit 18) belongs to the effect, not
    // the slot: the game shows it on every …之深奥 and on no ordinary effect.
    const starred = (metadata: number) =>
      identity?.star === undefined ? metadata : identity.star ? metadata | 0x40000 : metadata & ~0x40000;
    setDraft({
      ...draft,
      slots: draft.slots.map((slot, i) =>
        i === index
          ? {
              ...slot,
              id: effect.id,
              name: effect.name,
              ...(values?.length && !values.includes(Number(slot.raw))
                ? { raw: String(values[values.length - 1]) }
                : {}),
              ...(identity
                ? {
                    prefix: String(identity.prefix),
                    metadata: String(
                      starred(
                        (Number(slot.metadata) & 0xffff00ff) |
                          ((((Number(slot.metadata) >>> 8) & 0xc0) |
                            identity.category) <<
                            8),
                      ) >>> 0,
                    ),
                  }
                : {}),
            }
          : slot,
      ),
    });
  }
  function patchSlot(index: number, change: Partial<Slot>) {
    setDraft({
      ...draft,
      slots: draft.slots.map((slot, i) => (i === index ? { ...slot, ...change } : slot)),
    });
  }
  const changes = [
    ...fields
      .filter(([k]) => draft[k] !== saved[k])
      .map(([k, n]) => `${n}：${saved[k]} → ${draft[k]}`),
    ...draft.slots.flatMap((s, i) =>
      JSON.stringify(s) !== JSON.stringify(saved.slots[i])
        ? [
            `第 ${i + 1} 槽：` +
              (
                [
                  "name",
                  "id",
                  "raw",
                  "prefix",
                  "metadata",
                  "tail0",
                  "tail1",
                ] as const
              )
                .filter((k) => s[k] !== saved.slots[i][k])
                .map(
                  (k) =>
                    `${{ name: "词条", id: "ID", raw: "原始数值", prefix: "prefix", metadata: "metadata", tail0: "tail0", tail1: "tail1" }[k]} ${saved.slots[i][k]} → ${s[k]}`,
                )
                .join("；"),
          ]
        : [],
    ),
    ...(draft.locked !== saved.locked ? [draft.locked ? "加上保护" : "解除保护"] : []),
    ...(draft.favorite !== saved.favorite ? [draft.favorite ? "加入喜好项目" : "移出喜好项目"] : []),
  ];
  /** Favouriting also protects, as the game does; removing it keeps protection. */
  function toggleMark(s: Sample, mark: "locked" | "favorite") {
    const isCurrent = current.saveEntry
      ? current.saveEntry.slot_index === s.saveEntry?.slot_index
      : current.seed === s.seed;
    if (!isCurrent && changed) {
      setPending(s);
      return;
    }
    const base = isCurrent ? draft : records[s.seed] || fromSample(s);
    const next =
      mark === "favorite"
        ? { ...base, favorite: !base.favorite, locked: base.favorite ? base.locked : true }
        : { ...base, locked: !base.locked };
    if (isCurrent) setDraft(next);
    else {
      choose(s);
      setDraftRaw(next);
    }
    setMessage("已加入草稿。点“核对修改”后写入" + (writeMode === "live" ? "游戏。" : "存档。"));
  }
  /** Select the scroll under the in-game inventory cursor. */
  async function locateSelected() {
    try {
      const result = await window.operations.execute({ method: "runtime.menu_selection", params: {} });
      if (!result || !("menu_open" in result)) throw Error("没有读到游戏里的选中物品，请确认游戏正在运行后再试。");
      if (!result.menu_open)
        return setMessage("游戏里的背包没有打开：请在游戏里打开背包，把光标停在一张绘卷上，再点“定位游戏里选中的绘卷”。");
      if (result.container !== "scroll" || result.slot_index == null)
        return setMessage("游戏里选中的不是绘卷：请把光标停在一张绘卷上，再点“定位游戏里选中的绘卷”。");
      const entry = inventory.find((s) => s.saveEntry?.slot_index === result.slot_index);
      const record = entry?.saveEntry?.record_hex;
      const recordType = record ? Number.parseInt(record.slice(2, 4) + record.slice(0, 2), 16) : null;
      if (!entry || recordType !== result.item_id)
        return setMessage("游戏里选中的这张绘卷还没有存进存档：请在游戏里存档（例如在神社休息）后点“重新读取”，再定位一次。");
      setFind("");
      setPinnedOnly(false);
      if (changed) setPending(entry);
      else choose(entry);
      setMessage("已定位到游戏里选中的绘卷（栏位 " + (result.slot_index + 1) + "）。");
      requestAnimationFrame(() =>
        document.querySelector(".inventory-item.is-located")?.scrollIntoView({ block: "nearest" }),
      );
    } catch (error) {
      setMessage(errorText(error));
    }
  }
  async function applyTemporary() {
    setBackendBusy(true);
    try {
      if (
        temporary.capacityEnabled &&
        (!Number.isInteger(temporary.capacity) ||
          temporary.capacity < 1 ||
          temporary.capacity > 7)
      )
        throw Error("挑战次数上限应为 1–7。");
      if (!current.saveEntry) throw Error("请先读取存档绘卷。");
      if (temporary.enemyEnabled !== false && !temporary.enemies.length)
        throw Error("请先等待副本内容读取完成。");
      const terrain = (data.runtimeTerrains as Record<string, number>)[
        temporary.terrain
      ];
      if (temporary.terrainEnabled !== false && terrain === undefined)
        throw Error("请选择有效地形。");
      // Applying again replaces the active change: the backend only starts
      // from a stopped override, so stop the current one first. A stop that
      // cannot be confirmed fails here and nothing new is applied.
      const status = await window.operations.execute({
        method: "runtime.status",
        params: {},
      });
      const replacing =
        !!status &&
        "override_state" in status &&
        status.override_state !== "stopped";
      if (replacing)
        await runtimeObserver!.run(() =>
          window.operations.execute({
            method: "runtime.stop_override",
            params: {},
          }),
        );
      await runtimeObserver!.run(() =>
        window.operations.execute({
          method: "runtime.start_override",
          params: {
            profile: {
              seed: Number(draft.seed),
              enemy_keys: (temporary.enemyEnabled === false
                ? []
                : temporary.enemies) as ProtectedParams<"runtime.start_override">["profile"]["enemy_keys"],
              special_rule_keys:
                temporary.rulesEnabled === false
                  ? null
                  : (temporary.rules as [number, number, number]),
              terrain_value:
                temporary.terrainEnabled === false ? null : terrain,
              challenge_capacity: temporary.capacityEnabled
                ? temporary.capacity
                : null,
            },
          },
        }),
      );
      setMessage(
        replacing
          ? "临时修改已更新为当前内容。停止修改后重新打开绘卷，或退出游戏即可恢复。"
          : "临时修改已开启。停止修改后重新打开绘卷，或退出游戏即可恢复。",
      );
    } catch (error) {
      setMessage(errorText(error));
    } finally {
      setBackendBusy(false);
    }
  }
  async function stopTemporary() {
    setBackendBusy(true);
    try {
      await runtimeObserver!.run(() =>
        window.operations.execute({
          method: "runtime.stop_override",
          params: {},
        }),
      );
      setMessage("临时修改已停止。");
    } catch (error) {
      setMessage(errorText(error));
    } finally {
      setBackendBusy(false);
    }
  }
  async function applyLive() {
    if (!livePlan) return;
    setBackendBusy(true);
    try {
      let result = countEditOf(
        await runtimeObserver!.run(() =>
          window.operations.execute({
            method: "runtime.count_execute",
            params: livePlan,
          }),
        ),
      );
      // A lost reply is settled by observing the game, never by writing again.
      if (result.state === "uncertain")
        result = countEditOf(
          await runtimeObserver!.run(() =>
            window.operations.execute({
              method: "runtime.count_recover",
              params: { operation_id: livePlan.operation_id },
            }),
          ),
        );
      setReview(false);
      setLivePlan(null);
      if (result.state === "verified") {
        setSaved(structuredClone(draft));
        setRecords({ ...records, [current.seed]: structuredClone(draft) });
        setMessage(
          "已写入游戏。在游戏里重新打开绘卷菜单即可看到；到神社休息（游戏存档）后才会写进存档文件。",
        );
      } else if (result.state === "rejected")
        setMessage(errorText(result.error || "没有写入，请重新核对。"));
      else
        setMessage("写入结果尚未确认。请不要重复写入：点“重新读取”后核对这张绘卷，或导出反馈文件发给开发者。");
    } catch (error) {
      setMessage(errorText(error));
    } finally {
      setBackendBusy(false);
    }
  }
  async function applyReal() {
    if (!reviewedPlan || !saveStateConfirmed) return;
    setBackendBusy(true);
    try {
      const receipt = await saveSession!.commit(reviewedPlan);
      setReview(false);
      setReviewedPlan(null);
      // A warning never means "not written": do not invite a second write.
      setMessage(
        receipt.commit_status === "committed_with_warning"
          ? `修改已写入存档，但有警告：${receipt.warning || "请核对操作回执"}。请勿重复写入。`
          : receipt.commit_status === "committed"
            ? "修改已写入存档。"
            : "写入结果尚未确认，请核对操作回执。",
      );
      if (receipt.commit_status.startsWith("committed"))
        await saveSession!.refresh();
    } catch (error) {
      setMessage(errorText(error));
    } finally {
      setBackendBusy(false);
    }
  }
  async function prepareDelete() {
    const targets = deleteSlots.length ? deleteSlots : current.saveEntry ? [current.saveEntry.slot_index] : [];
    const locked = inventory.filter(
      (s) => s.saveEntry && targets.includes(s.saveEntry.slot_index) && s.saveEntry.locked,
    ).length;
    if (locked && !deleteLockedConfirmed) {
      setDeleteLockedPrompt(locked);
      return;
    }
    setDeleteLockedPrompt(0);
    setDeleteLockedConfirmed(false);
    setBackendBusy(true);
    try {
      if (!current.saveEntry) return;
      const plan = await saveSession!.prepareDelete(
        (deleteSlots.length ? deleteSlots : [current.saveEntry.slot_index]) as [
          number,
          ...number[],
        ],
      );
      setReviewedPlan(plan.plan_id);
      setReview(true);
      setSaveStateConfirmed(false);
      setMessage(`将删除 ${deleteSlots.length || 1} 张绘卷，请核对后确认。`);
    } catch (error) {
      setMessage(errorText(error));
    } finally {
      setBackendBusy(false);
    }
  }
  if (desktop && !saveState?.inventory?.entries.length)
    return (
      <section className="editor-empty">
        <h2>选择存档中的绘卷</h2>
        <SavePicker />
        <p>
          {saveState?.inventory
            ? "当前存档没有绘卷，可以先从购物车添加。"
            : "读取后可编辑词条和修改副本内容。"}
        </p>
      </section>
    );
  return (
    <div
      className="editor-page"
      onKeyDown={(e) => {
        if (e.ctrlKey && (e.key === "z" || e.key === "y")) {
          e.preventDefault();
          e.key === "y" || e.shiftKey ? redo() : undo();
        }
      }}
    >
      <section className="inventory-pane">
        <h2>我的绘卷</h2>
        {desktop ? (
          <SavePicker />
        ) : (
          <>
            <label>
              账户与角色
              <select aria-label="编辑账户">
                <option>示例角色 · 存档 1</option>
              </select>
            </label>
            <button
              onClick={() =>
                setMessage("此预览展示示例绘卷，暂不读取游戏存档。")
              }
            >
              读取存档
            </button>
          </>
        )}
        <div className="inventory-find">
          <input
            aria-label="查找已有绘卷"
            placeholder="查找绘卷 ID 或词条"
            value={find}
            onChange={(e) => setFind(e.target.value)}
          />
          <label className="pinned-filter">
            <input
              type="checkbox"
              checked={pinnedOnly}
              onChange={(e) => setPinnedOnly(e.target.checked)}
            />
            只看收藏{inventory.some(isFavorite) ? `（${inventory.filter(isFavorite).length}）` : ""}
          </label>
        </div>
        <div className="inventory-list">
          {pinnedFirst(
            inventory
              .map((s, index) => ({ s, slot: (s.saveEntry?.slot_index ?? index) + 1 }))
              .filter(
                ({ s }) =>
                  (!pinnedOnly || isFavorite(s)) &&
                  (s.seed + s.effects.map((e) => e.name).join()).includes(find),
              ),
            ({ s }) => isFavorite(s),
          ).map(({ s, slot }) => (
              <div
                className={
                  "inventory-item" +
                  (isFavorite(s) ? " is-pinned" : "") +
                  (current.saveEntry && current.saveEntry.slot_index === s.saveEntry?.slot_index ? " is-located" : "")
                }
                key={s.saveEntry?.slot_index ?? s.seed}
              >
                {desktop && s.saveEntry && (
                  <input
                    type="checkbox"
                    aria-label={"选择删除栏位" + (s.saveEntry.slot_index + 1)}
                    checked={deleteSlots.includes(s.saveEntry.slot_index)}
                    onChange={(e) => {
                      setReview(false);
                      setReviewedPlan(null);
                      const slot = s.saveEntry!.slot_index;
                      setDeleteSlots(
                        e.target.checked
                          ? [...deleteSlots, slot]
                          : deleteSlots.filter((v) => v !== slot),
                      );
                    }}
                  />
                )}
                <button
                  className={
                    current.saveEntry
                      ? current.saveEntry.slot_index === s.saveEntry?.slot_index
                        ? "selected"
                        : ""
                      : current.seed === s.seed
                        ? "selected"
                        : ""
                  }
                  onClick={() => (changed ? setPending(s) : choose(s))}
                >
                  <small>
                    栏位 {slot} · R{s.rarity}
                    <i className={"rarity-dot rarity-" + s.rarity} aria-hidden="true" />
                  </small>
                  <b>
                    {s.effects[0]?.name || "空词条绘卷"}
                    {s.saveEntry?.locked && (
                      <span className="lock-mark" title="游戏里已保护：不能卖掉或丢弃" aria-label="已保护">
                        <svg viewBox="0 0 24 24" width="13" height="13" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="2" strokeLinejoin="round">
                          <rect x="5" y="11" width="14" height="10" rx="2" />
                          <path d="M8 11V8a4 4 0 0 1 8 0v3" />
                        </svg>
                      </span>
                    )}
                  </b>
                  <span>绘卷 ID {s.seed}</span>
                </button>
                <button
                  type="button"
                  className={"pin-toggle" + (isFavorite(s) ? " active" : "")}
                  aria-pressed={isFavorite(s)}
                  aria-label={(isFavorite(s) ? "移出喜好项目" : "加入喜好项目") + "绘卷 " + s.seed}
                  title={
                    desktop
                      ? isFavorite(s)
                        ? "游戏里的喜好项目。点一下移出（保护保留）"
                        : "加入游戏里的喜好项目，同时加上保护；总是排在最上面"
                      : isFavorite(s)
                        ? "取消收藏"
                        : "收藏：总是排在最上面"
                  }
                  onClick={() => (desktop ? toggleMark(s, "favorite") : togglePinned("scroll", scrollKey(s)))}
                >
                  <StarIcon filled={isFavorite(s)} />
                </button>
              </div>
            ))}
          {pinnedOnly && !inventory.some(isFavorite) && (
            <p className="inventory-empty">还没有收藏的绘卷。点绘卷右边的星标即可收藏。</p>
          )}
        </div>
        {desktop && (
          <button onClick={() => void locateSelected()} disabled={backendBusy}>
            定位游戏里选中的绘卷
          </button>
        )}
        {deleteLockedPrompt > 0 && (
          <div className="editor-prompt" role="alert">
            要删除的绘卷里有 {deleteLockedPrompt} 张在游戏里加了保护。确定仍要删除吗？
            <button onClick={() => setDeleteLockedConfirmed(true)}>仍然删除</button>
            <button onClick={() => setDeleteLockedPrompt(0)}>取消</button>
          </div>
        )}
        <button
          disabled={backendBusy || !!saveState?.busy || (desktop && !current.saveEntry)}
          onClick={() =>
            desktop
              ? void prepareDelete()
              : setMessage("此预览暂不删除游戏存档中的绘卷。")
          }
        >
          删除选中绘卷{deleteSlots.length ? `（${deleteSlots.length}）` : ""}
        </button>
      </section>
      <section className="editor-work">
        <div className="editor-heading">
          <h2>编辑绘卷</h2>
          <span>{changed ? "有未应用的修改" : "尚未修改"}</span>
          <button disabled={!undoStack.length} onClick={undo}>
            撤销
          </button>
          <button disabled={!redoStack.length} onClick={redo}>
            重做
          </button>
          <button
            disabled={!cart.length}
            onClick={() => cartDialog.current?.showModal()}
          >
            选择购物车中绘卷种子
          </button>
        </div>
        {desktop && (
          <div className="editor-write-mode">
            <div className="character-modes" role="group" aria-label="修改方式">
              <button className={writeMode === "live" ? "active" : ""} onClick={() => setWriteMode("live")}>
                游戏内实时修改
              </button>
              <button className={writeMode === "save" ? "active" : ""} onClick={() => setWriteMode("save")}>
                修改存档文件
              </button>
            </div>
            <span>
              {writeMode === "live"
                ? "直接改正在运行的游戏，改完在游戏里存档（例如在神社休息）即可保存。"
                : "改存档文件，写入时让游戏停在标题界面。"}
            </span>
          </div>
        )}
        <p className="editor-info">
          基础信息和主副词条会保存下来；副本内容只是临时改游戏，不会存进存档。传给其他玩家后，词条会按种子重新生成。
        </p>
        {pending && (
          <div className="editor-prompt">
            切换绘卷会放弃当前修改。
            <button onClick={() => choose(pending)}>放弃并切换</button>
            <button onClick={() => setPending(null)}>继续编辑</button>
          </div>
        )}
        <details
          className="module sand editor-section editor-basics"
          name="editor-section"
        >
          <summary>基础信息</summary>
          <div className="editor-section-body">
            <div className="editor-fields">
              {fields.map(([k, n]) => (
                <label key={k}>
                  {n}
                  <input
                    aria-label={"编辑" + n}
                    value={draft[k]}
                    onChange={(e) => {
                      setDraft({ ...draft, [k]: e.target.value });
                      setReview(false);
                    }}
                  />
                </label>
              ))}
            </div>
            <div className="editor-marks">
              <label>
                <input
                  type="checkbox"
                  checked={draft.locked}
                  onChange={(e) => setDraft({ ...draft, locked: e.target.checked })}
                />
                保护（游戏里不能卖掉或丢弃）
              </label>
              <label>
                <input
                  type="checkbox"
                  checked={draft.favorite}
                  onChange={(e) =>
                    setDraft({
                      ...draft,
                      favorite: e.target.checked,
                      locked: e.target.checked ? true : draft.locked,
                    })
                  }
                />
                喜好项目（加入时同时保护，和游戏一致）
              </label>
            </div>
          </div>
        </details>
        <details
          className="module sand editor-section editor-effects"
          name="editor-section"
        >
          <summary>主副词条</summary>
          <div className="editor-section-body">
            <div className="effect-scope">
              <fieldset aria-label="可选词条范围">
                <legend>可选词条</legend>
                {(
                  [
                    ["context", "当前周目和稀有度的绘卷能出现的词条和恩宠"],
                    ["scroll", "任意绘卷能出现的词条"],
                    ["all", "全部词条（含绘卷上不会出现的）"],
                  ] as [EffectScope, string][]
                ).map(([value, label]) => (
                  <label key={value}>
                    <input
                      type="radio"
                      name="effect-scope"
                      checked={effectScope === value}
                      onChange={() => setEffectScope(value)}
                    />
                    <span>{label}</span>
                  </label>
                ))}
              </fieldset>
              <span>
                {effectScope === "all"
                  ? "游戏里所有词条都可选，绘卷上不会自然出现的词条也能写进去。"
                  : "范围外的词条请切换到“全部词条”。"}
              </span>
            </div>
            <table className="equipment-effects scroll-effects">
              <thead>
                <tr>
                  <th>位置</th>
                  <th>词条</th>
                  <th>数值</th>
                  <th>原生范围</th>
                </tr>
              </thead>
              <tbody>
                {draft.slots.map((s, i) => {
                  const native = slotNative(s);
                  const empty = s.id === String(EMPTY_EFFECT);
                  return (
                    <tr key={i} className={JSON.stringify(s) === JSON.stringify(saved.slots[i]) ? "" : "is-changed"}>
                      <td>
                        {i + 1}
                        {i === 0 && <small>主词条</small>}
                      </td>
                      <td>
                        <EffectPicker
                          value={s.id}
                          candidates={candidates}
                          label={effectLabel}
                          onPick={(candidate) => replaceEffect(i, candidate)}
                          placeholder="空槽"
                        />
                        {!empty && !inScope.has(s.id) && (
                          <small className="character-unnatural">不在所选范围内</small>
                        )}
                      </td>
                      <td>
                        {empty ? null : (
                          <span className="scroll-value">
                            <input
                              aria-label={"词条数值 " + (i + 1)}
                              inputMode="numeric"
                              value={s.raw}
                              onChange={(e) => patch(i, "raw", e.target.value)}
                            />
                            {native && native.length > 1 && (
                              <select
                                aria-label={"原生数值 " + (i + 1)}
                                value={native.includes(Number(s.raw)) ? s.raw : ""}
                                onChange={(e) => patch(i, "raw", e.target.value)}
                              >
                                <option value="" disabled>
                                  原生值
                                </option>
                                {native.map((value) => (
                                  <option key={value} value={value}>
                                    {value}
                                  </option>
                                ))}
                              </select>
                            )}
                          </span>
                        )}
                      </td>
                      <td className="character-range">
                        {empty ? null : native?.length ? (
                          <>
                            {slotRange(s)}
                            {!native.includes(Number(s.raw)) && (
                              <small className="character-unnatural"> · {"非原生值"}</small>
                            )}
                          </>
                        ) : (
                          <small>无可核对的范围</small>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
            <details className="advanced">
              <summary>高级原始字段</summary>
              <p>
                不检查词条冲突或组合合法性。修改这些字段前，请确认它们的含义。
              </p>
              <table className="equipment-effects scroll-raw-fields">
                <thead>
                  <tr>
                    <th>位置</th>
                    <th>prefix</th>
                    <th>metadata</th>
                    <th>tail0</th>
                    <th>tail1</th>
                  </tr>
                </thead>
                <tbody>
                  {draft.slots.map((s, i) => (
                    <tr key={i}>
                      <td>{i + 1}</td>
                      {(["prefix", "metadata", "tail0", "tail1"] as const).map((k) => (
                        <td key={k}>
                          <input
                            aria-label={k + " " + (i + 1)}
                            value={s[k]}
                            onChange={(e) => patch(i, k, e.target.value)}
                          />
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </details>
          </div>
        </details>
        {desktop && current.saveEntry && <CountEditor sample={current} />}
        {desktop && <ScrollCompletion
          recordHex={current.saveEntry?.record_hex}
          identity={JSON.stringify([saveState?.inventory?.snapshot_id,current.saveEntry?.slot_index])}
          disabled={changed || backendBusy || !!saveState?.busy}
        />}
        <details className="module blue editor-section" name="editor-section">
          <summary>副本内容 · 临时，不进存档</summary>
          <div className="editor-section-body">
            <p className="temporary-warning">
              只修改游戏内存，不保存进绘卷。停止修改后重新打开绘卷，或退出游戏即可恢复。
            </p>
            <div className="temporary-fields">
              {desktop && (
                <div>
                  <button
                    disabled={backendBusy}
                    onClick={() => void previewDraft()}
                  >
                    按当前种子预览副本
                  </button>
                  <button
                    disabled={backendBusy}
                    onClick={() => void inspectTemporary()}
                  >
                    检查生效状态
                  </button>
                </div>
              )}
              <fieldset>
                <legend>
                  <label>
                    <input
                      type="checkbox"
                      checked={temporary.enemyEnabled !== false}
                      onChange={(e) =>
                        setTemporary({
                          ...temporary,
                          enemyEnabled: e.target.checked,
                        })
                      }
                    />
                    修改敌人
                  </label>
                </legend>
                <label>
                  敌人组数
                  <select
                    aria-label="临时敌人组数"
                    disabled={temporary.enemyEnabled === false}
                    value={temporary.enemies.length}
                    onChange={(e) =>
                      setTemporary({
                        ...temporary,
                        enemies: Array.from(
                          { length: Number(e.target.value) },
                          (_, i) =>
                            temporary.enemies[i] ??
                            temporary.originalEnemies[i],
                        ),
                      })
                    }
                  >
                    {temporary.originalEnemies.map((_, i) => (
                      <option key={i} value={i + 1}>
                        {i + 1}
                      </option>
                    ))}
                  </select>
                </label>
                {temporary.enemies.map((key, i) => {
                  const roles = data.enemyRoles as Record<string, number>;
                  const tier = (key: number) =>
                    Math.min(4, roles[String(key)]) === 4
                      ? roles[String(key)]
                      : 0;
                  const allowed = data.enemies
                    .map((e) => ({
                      ...e,
                      key: e.keys.find(
                        (k) => tier(k) === tier(temporary.originalEnemies[i]),
                      ),
                    }))
                    .filter((e) => e.key !== undefined);
                  return (
                    <label key={i}>
                      敌人组 {i + 1}
                      <select
                        disabled={temporary.enemyEnabled === false}
                        aria-label={"临时修改敌人" + (i + 1)}
                        value={key}
                        onChange={(e) =>
                          setTemporary({
                            ...temporary,
                            enemies: temporary.enemies.map((v, j) =>
                              i === j ? Number(e.target.value) : v,
                            ),
                          })
                        }
                      >
                        <option value={key}>
                          {data.enemies.find((e) => e.keys.includes(key))
                            ?.name || "当前敌人"}
                        </option>
                        {allowed
                          .filter((e) => !e.keys.includes(key))
                          .map((e) => (
                            <option key={e.id} value={e.key}>
                              {e.name}
                            </option>
                          ))}
                      </select>
                    </label>
                  );
                })}
              </fieldset>
              <label>
                <span>
                  <input
                    type="checkbox"
                    aria-label="修改地形"
                    checked={temporary.terrainEnabled !== false}
                    onChange={(e) =>
                      setTemporary({
                        ...temporary,
                        terrainEnabled: e.target.checked,
                      })
                    }
                  />
                  修改地形
                </span>
                <select
                  disabled={temporary.terrainEnabled === false}
                  aria-label="临时修改地形影响"
                  value={temporary.terrain}
                  onChange={(e) =>
                    setTemporary({ ...temporary, terrain: e.target.value })
                  }
                >
                  {data.terrains
                    .filter((t) => !t.aggregate)
                    .map((t) => (
                      <option key={t.option_id} value={t.option_id}>
                        {t.name}
                      </option>
                    ))}
                </select>
              </label>
              <fieldset>
                <legend>
                  <label>
                    <input
                      type="checkbox"
                      checked={temporary.rulesEnabled !== false}
                      onChange={(e) =>
                        setTemporary({
                          ...temporary,
                          rulesEnabled: e.target.checked,
                        })
                      }
                    />
                    修改特殊规则
                  </label>
                </legend>
                {temporary.rules.map((key, i) => (
                  <label key={i}>
                    规则 {i + 1}
                    <select
                      disabled={temporary.rulesEnabled === false}
                      aria-label={"临时修改特殊规则" + (i + 1)}
                      value={key}
                      onChange={(e) =>
                        setTemporary({
                          ...temporary,
                          rules: temporary.rules.map((v, j) =>
                            i === j ? Number(e.target.value) : v,
                          ),
                        })
                      }
                    >
                      <option value={0}>无</option>
                      {data.rules.map((r) => (
                        <optgroup key={r.id} label={r.name}>
                          {r.variants.map((v) => (
                            <option key={v.key} value={v.key}>
                              {r.name} · {v.label}
                            </option>
                          ))}
                        </optgroup>
                      ))}
                    </select>
                  </label>
                ))}
              </fieldset>
              <label>
                <span>
                  <input
                    type="checkbox"
                    aria-label="可挑战次数上限"
                    checked={!!temporary.capacityEnabled}
                    onChange={(e) =>
                      setTemporary({
                        ...temporary,
                        capacityEnabled: e.target.checked,
                      })
                    }
                  />
                  可挑战次数上限
                </span>
                <input
                  type="number"
                  min="1"
                  max="7"
                  aria-label="临时修改挑战次数上限"
                  value={temporary.capacity}
                  disabled={!temporary.capacityEnabled}
                  onChange={(e) =>
                    setTemporary({
                      ...temporary,
                      capacity: e.target.valueAsNumber,
                    })
                  }
                />
              </label>
            </div>
            <button
              disabled={backendBusy}
              onClick={() =>
                desktop
                  ? void applyTemporary()
                  : setMessage("临时修改草稿已准备好；此预览暂未连接游戏。")
              }
            >
              应用临时修改
            </button>
            {desktop && (
              <button
                disabled={backendBusy}
                onClick={() => void stopTemporary()}
              >
                停止临时修改
              </button>
            )}
            <button
              onClick={() => {
                setTemporary(temporaryFromSample(current));
                setMessage("已恢复示例副本内容。");
              }}
            >
              恢复原始内容
            </button>
          </div>
        </details>
        <div className="editor-bottom">
          <button
            disabled={!changed}
            onClick={() => {
              setDraft(structuredClone(saved));
              setReview(false);
            }}
          >
            放弃修改
          </button>
          <button
            disabled={!changed || backendBusy || !!saveState?.busy}
            onClick={() => void validate()}
          >
            核对修改
          </button>
        </div>
        <Notice text={message} className="editor-notice" />
      </section>
      <aside className="editor-review">
        <h2>修改预览</h2>
        <p>绘卷 ID {draft.seed}</p>
        <div className={"scroll rarity-" + draft.rarity}>
          <h2>百境百怪绘卷</h2>
          <p>
            等级 {draft.level} · 敌人等级 {draft.recommended}
          </p>
          {draft.slots.map((s, i) => (
            <div className="effect-line" key={i}>
              <span>{i + 1}</span>
              <div>{s.name}</div>
              <strong title="原始数值">
                {s.raw}
                {slotRange(s) && <small className="value-range">范围 {slotRange(s)}</small>}
              </strong>
            </div>
          ))}
          <section className="editor-review-content">
            <h3>出现敌人</h3>
            {shown.enemies.length ? (
              <div className="enemy-lines">
                {shown.enemies.map((name, i) => (
                  <span key={i}>◇ {name}</span>
                ))}
              </div>
            ) : (
              <p className="editor-review-pending">{contentPending}</p>
            )}
            <h3>特殊规则</h3>
            {shown.rules.length ? (
              shown.rules.map((r, i) => (
                <p className="scroll-rule" key={i}>
                  <span title={r.name}>{r.name}</span>
                  <strong>{r.value}</strong>
                </p>
              ))
            ) : (
              <p className="editor-review-pending">{shown.enemies.length ? "无" : contentPending}</p>
            )}
            <p className="scroll-rule">
              <span>地形影响</span>
              <strong>
                {shown.enemies.length
                  ? data.terrains.find(
                      (t) =>
                        !t.aggregate &&
                        t.effect_keys.length === shown.terrainKeys.length &&
                        t.effect_keys.every((k) => shown.terrainKeys.includes(k)),
                    )?.name || "未解析"
                  : "—"}
              </strong>
            </p>
            <p className="scroll-rule">
              <span>可挑战次数上限</span>
              <strong>{shown.enemies.length ? shown.capacity : "—"}</strong>
            </p>
          </section>
        </div>
        <h3>本次变化</h3>
        {changes.length ? (
          <ul>
            {changes.map((c) => (
              <li key={c}>{c}</li>
            ))}
          </ul>
        ) : (
          <p>选择词条或修改基础信息，即可在这里核对。</p>
        )}
        {review && (
          <div className="editor-prompt" ref={reviewPrompt} role="region" aria-label="确认修改">
            <h3>确认修改</h3>
            <p>{desktop ? message : `本次将应用 ${changes.length} 项变化。`}</p>
            {desktop && writeMode === "live" && livePlan ? (
              <button disabled={backendBusy} onClick={() => void applyLive()}>
                确认写入游戏
              </button>
            ) : desktop ? (
              <>
                <label>
                  <input
                    type="checkbox"
                    checked={saveStateConfirmed}
                    onChange={(e) => setSaveStateConfirmed(e.target.checked)}
                  />
                  游戏已回到标题界面
                </label>
                <button
                  disabled={!saveStateConfirmed || !reviewedPlan || backendBusy}
                  onClick={() => void applyReal()}
                >
                  确认写入存档
                </button>
              </>
            ) : (
              <button
                onClick={() => {
                  setSaved(structuredClone(draft));
                  setRecords({
                    ...records,
                    [current.seed]: structuredClone(draft),
                  });
                  setReview(false);
                  setMessage("修改已应用到本次预览。");
                }}
              >
                应用到预览
              </button>
            )}
          </div>
        )}
        <p className="editor-boundary">
          {desktop
            ? writeMode === "live"
              ? "左侧列表来自存档文件；实时修改要在游戏里存档后，点“重新读取”才会显示在列表里。"
              : "当前存档绘卷"
            : "示例绘卷 · 不写入游戏存档"}
        </p>
      </aside>
      <dialog
        ref={cartDialog}
        className="seed-cart-dialog"
        {...cartDialogBackdropDismiss}
      >
        <header>
          <h2>选择购物车中绘卷种子</h2>
          <button
            aria-label="关闭种子选择"
            onClick={() => cartDialog.current?.close()}
          >
            ×
          </button>
        </header>
        <div className="seed-cart-list">
          {cart.map((s) => (
            <button
              key={s.rarity + ":" + s.seed}
              onClick={() => {
                setDraft({ ...draft, seed: s.seed });
                cartDialog.current?.close();
                setMessage("已选用绘卷种子 " + s.seed);
              }}
            >
              <b>绘卷 ID {s.seed}</b>
              <span>
                R{s.rarity} · {s.effects[0]?.name}
              </span>
            </button>
          ))}
        </div>
      </dialog>
    </div>
  );
}
