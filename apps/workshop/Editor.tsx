import {CountEditor} from "./CountEditor";
import { localize } from "./presentation";
import { useDialogBackdropDismiss } from "./use-dialog-backdrop-dismiss";
import React, { useEffect, useState, useRef } from "react";
import { data, type Sample, toRecordTransferCount } from "./model";
import { desktop } from "./desktop-bridge";
import {
  saveSession,
  saveObserver,
  runtimeObserver,
  entrySample,
  enrichEntry,
} from "./save-workspace";
import { SavePicker } from "./CartActions";
import type { ProtectedParams } from "../desktop/src/protected-client";
import rawCatalog from "./editor-values.json";
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
};
function fromSample(s: Sample): Draft {
  const entry = s.saveEntry;
  return {
    seed: s.seed,
    ng: String(entry?.header.playthrough || s.playthrough || 3),
    rarity: String(s.rarity),
    level: String(entry?.header.level || s.level || 180),
    recommended: String(entry?.derived.recommended_displayed_level || 350),
    transfers: entry
      ? String(
          entry.header.transfer_count === 0xffffffff
            ? -1
            : entry.header.transfer_count,
        )
      : "-1",
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
  const [reviewedPlan, setReviewedPlan] = useState<string | null>(null),
    [saveStateConfirmed, setSaveStateConfirmed] = useState(false),
    [backendBusy, setBackendBusy] = useState(false);
  const [backups, setBackups] = useState<
    { backup_id: string; timestamp: string; action: string }[]
  >([]);
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
    [active, setActive] = useState(0),
    [find, setFind] = useState(""),
    [catalogFind, setCatalogFind] = useState("");
  const draftIdentity = useRef("");
  draftIdentity.current = JSON.stringify([
    draft,
    current.saveEntry?.slot_index,
    saveState?.inventory?.snapshot_id,
  ]);
  const [message, setMessage] = useState(""),
    [review, setReview] = useState(false),
    [pending, setPending] = useState<Sample | null>(null);
  const [records, setRecords] = useState<Record<string, Draft>>({});
  useEffect(() => {
    setReview(false);
    setReviewedPlan(null);
    setSaveStateConfirmed(false);
  }, [draft]);
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
  const changed = JSON.stringify(draft) !== JSON.stringify(saved);
  const fields: [keyof Omit<Draft, "slots">, string][] = [
    ["seed", "绘卷 ID（种子）"],
    ["ng", "周目"],
    ["rarity", "稀有度"],
    ["level", "绘卷等级"],
    ["recommended", "推荐等级（敌人等级）"],
    ["transfers", "转手次数"],
  ];
  const patch = (key: keyof Slot, value: string) =>
    setDraft({
      ...draft,
      slots: draft.slots.map((s, i) =>
        i === active ? { ...s, [key]: value } : s,
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
    setActive(0);
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
        .catch((e) => setMessage(String(e)));
    }
  }
  async function validate() {
    const identity = draftIdentity.current;
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
        const level = await window.nioh.resolveRecommendedLevel(
          Number(draft.recommended),
        );
        if (level.status !== "exact" || level.selected_internal_level === null)
          throw Error("推荐等级无法转换。");
        const edit = {
          slot_index: current.saveEntry.slot_index,
          header: {
            seed: Number(draft.seed),
            playthrough: Number(draft.ng),
            rarity: Number(draft.rarity),
            level: Number(draft.level),
            recommended_level: level.selected_internal_level,
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
        } as ProtectedParams<"save.prepare_edit">["edits"][number];
        const plan = await saveSession!.prepareEdit([edit]);
        if (identity !== draftIdentity.current)
          throw Error("内容已改变，请重新核对修改。");
        setReviewedPlan(plan.plan_id);
        setSaveStateConfirmed(false);
      }
      setReview(true);
      setMessage("");
    } catch (e) {
      setMessage((e as Error).message);
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
      setMessage(String(e));
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
      setMessage(String(e));
    }
  }
  const native = nativeValues(
    draft.slots[active].id,
    Number(draft.rarity),
    Number(draft.level),
  );
  function replaceEffect(effect: { id: string; name: string }) {
    const identity = (
      data.editorIdentities as Record<
        string,
        { prefix: number; category: number }
      >
    )[effect.id];
    setDraft({
      ...draft,
      slots: draft.slots.map((slot, i) =>
        i === active
          ? {
              ...slot,
              id: effect.id,
              name: effect.name,
              ...(identity
                ? {
                    prefix: String(identity.prefix),
                    metadata: String(
                      ((Number(slot.metadata) & 0xffff00ff) |
                        ((((Number(slot.metadata) >>> 8) & 0xc0) |
                          identity.category) <<
                          8)) >>>
                        0,
                    ),
                  }
                : {}),
            }
          : slot,
      ),
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
  ];
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
      setMessage("临时修改已开启。停止修改后重新打开绘卷，或退出游戏即可恢复。");
    } catch (error) {
      setMessage(String(error));
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
      setMessage(String(error));
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
      setMessage(
        receipt.commit_status.startsWith("committed")
          ? "修改已写入存档。"
          : "写入结果尚未确认，请核对操作回执。",
      );
      if (receipt.commit_status.startsWith("committed"))
        await saveSession!.refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBackendBusy(false);
    }
  }
  async function prepareDelete() {
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
      setMessage(String(error));
    } finally {
      setBackendBusy(false);
    }
  }
  async function readBackups() {
    try {
      const save = saveSession!.getSnapshot().selected;
      if (!save) throw Error("请先选择存档。");
      const result = await saveObserver!.run(() =>
        window.operations.execute({
          method: "save.backups",
          params: { save_id: save.save_id },
        }),
      );
      if (result && "backups" in result) setBackups(result.backups);
    } catch (error) {
      setMessage(String(error));
    }
  }
  async function prepareRestore(backupId: string) {
    try {
      const plan = await saveSession!.prepareRestore(backupId);
      setReviewedPlan(plan.plan_id);
      setReview(true);
      setSaveStateConfirmed(false);
      setMessage("将恢复选中的备份，请核对后确认。");
    } catch (error) {
      setMessage(String(error));
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
            : "读取后可编辑词条、查看备份和修改副本内容。"}
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
        <input
          aria-label="查找已有绘卷"
          placeholder="查找绘卷 ID 或词条"
          value={find}
          onChange={(e) => setFind(e.target.value)}
        />
        <div className="inventory-list">
          {inventory
            .filter((s) =>
              (s.seed + s.effects.map((e) => e.name).join()).includes(find),
            )
            .map((s, i) => (
              <div
                className="inventory-item"
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
                    栏位 {i + 1} · R{s.rarity}
                  </small>
                  <b>{s.effects[0]?.name || "空词条绘卷"}</b>
                  <span>绘卷 ID {s.seed}</span>
                </button>
              </div>
            ))}
        </div>
        <button
          disabled={backendBusy || (desktop && !current.saveEntry)}
          onClick={() =>
            desktop
              ? void prepareDelete()
              : setMessage("此预览暂不删除游戏存档中的绘卷。")
          }
        >
          删除选中绘卷{deleteSlots.length ? `（${deleteSlots.length}）` : ""}
        </button>
        <details>
          <summary>备份与恢复</summary>
          <p>修改前自动备份，可在这里恢复。</p>
          <button
            onClick={() =>
              desktop ? void readBackups() : setMessage("尚未连接备份目录。")
            }
          >
            查看备份
          </button>
          {backups.map((backup) => (
            <button
              key={backup.backup_id}
              onClick={() => void prepareRestore(backup.backup_id)}
            >
              {backup.timestamp} · 恢复
            </button>
          ))}
        </details>
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
        <p className="editor-info">
          主副词条修改可以长期保存，重启游戏仍保留。传给其他玩家后，词条会按种子重新生成。
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
          <summary>基础信息 · 长期保存</summary>
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
          </div>
        </details>
        <details
          className="module sand editor-section editor-effects"
          name="editor-section"
        >
          <summary>主副词条 · 长期保存</summary>
          <div className="editor-section-body">
            <p>选择一行，在下方更换词条。</p>
            <div className="slot-list">
              {draft.slots.map((s, i) => (
                <button
                  key={i}
                  className={active === i ? "selected" : ""}
                  onClick={() => setActive(i)}
                >
                  <span>{i + 1}</span>
                  <b>{s.name}</b>
                  <small>{s.raw}</small>
                </button>
              ))}
            </div>
            <div className="slot-edit">
              <label>
                词条 ID
                <input
                  aria-label="编辑词条ID"
                  value={draft.slots[active].id}
                  onChange={(e) => {
                    setDraft({
                      ...draft,
                      slots: draft.slots.map((s, i) =>
                        i === active
                          ? {
                              ...s,
                              id: e.target.value,
                              name:
                                data.editorEffects.find(
                                  (v) =>
                                    v.id === String(Number(e.target.value)),
                                )?.name || "未知词条",
                            }
                          : s,
                      ),
                    });
                  }}
                />
              </label>
              <label>
                原始数值
                <input
                  aria-label="编辑词条原始数值"
                  value={draft.slots[active].raw}
                  onChange={(e) => patch("raw", e.target.value)}
                />
              </label>
              <button
                onClick={() =>
                  setDraft({
                    ...draft,
                    slots: draft.slots.map((s, i) =>
                      i === active
                        ? { ...s, id: "4294967295", name: "空槽", raw: "0" }
                        : s,
                    ),
                  })
                }
              >
                清空此槽
              </button>
            </div>
            <p className="native-value-hint">
              {draft.slots[active].id === "4294967295" ? (
                "当前为空槽"
              ) : native ? (
                <>
                  原生数值：
                  {native.length === 1
                    ? native[0]
                    : `${native[0]}–${native[native.length - 1]}（${native.length} 个可用值）`}{" "}
                  ·{" "}
                  {native.includes(Number(draft.slots[active].raw))
                    ? "当前值在原生集合内"
                    : "当前值不在原生集合内，仍可自由修改"}
                </>
              ) : (
                "此词条暂无可验证的原生数值范围"
              )}
            </p>
            {native && (
              <select
                aria-label="选择原生数值"
                value={
                  native.includes(Number(draft.slots[active].raw))
                    ? draft.slots[active].raw
                    : ""
                }
                onChange={(e) => patch("raw", e.target.value)}
              >
                <option value="" disabled>
                  选择一个原生数值
                </option>
                {native.map((value) => (
                  <option key={value} value={value}>
                    {value}
                  </option>
                ))}
              </select>
            )}
            <input
              aria-label="编辑词条目录搜索"
              placeholder="搜索词条名称或 ID"
              value={catalogFind}
              onChange={(e) => setCatalogFind(e.target.value)}
            />
            <div className="editor-catalog">
              {data.editorEffects
                .filter((e) => (e.name + e.id).includes(catalogFind))
                .slice(0, 100)
                .map((e) => (
                  <button key={e.id} onClick={() => replaceEffect(e)}>
                    {e.name}
                    <span>替换</span>
                  </button>
                ))}
            </div>
            <details className="advanced">
              <summary>高级原始字段</summary>
              <p>
                不检查词条冲突或组合合法性。修改这些字段前，请确认它们的含义。
              </p>
              <div className="editor-fields">
                {(["prefix", "metadata", "tail0", "tail1"] as const).map(
                  (k) => (
                    <label key={k}>
                      {k}
                      <input
                        value={draft.slots[active][k]}
                        onChange={(e) => patch(k, e.target.value)}
                      />
                    </label>
                  ),
                )}
              </div>
            </details>
          </div>
        </details>
        {desktop && current.saveEntry && <CountEditor sample={current} />}
        <details className="module blue editor-section" name="editor-section">
          <summary>副本内容 · 临时修改</summary>
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
                      第 {i + 1} 组
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
                    第 {i + 1} 条
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
            disabled={!changed || backendBusy}
            onClick={() => void validate()}
          >
            核对修改
          </button>
        </div>
        <p role="status">{message}</p>
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
              <strong title="原始数值">{s.raw}</strong>
            </div>
          ))}
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
          <div className="editor-prompt">
            <h3>确认修改</h3>
            <p>{desktop ? message : `本次将应用 ${changes.length} 项变化。`}</p>
            {desktop ? (
              <>
                <label>
                  <input
                    type="checkbox"
                    checked={saveStateConfirmed}
                    onChange={(e) => setSaveStateConfirmed(e.target.checked)}
                  />
                  游戏已回到标题界面或已关闭
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
          {desktop ? "当前存档绘卷" : "示例绘卷 · 不写入游戏存档"}
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
