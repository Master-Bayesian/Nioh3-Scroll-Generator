import {collectionKey} from "./collections";
import { Notice } from "./Notice";
import { PreparedLiveBatchOwner } from "./prepared-live-batch";
import React, { useState, useSyncExternalStore, useEffect, useRef } from "react";
import {
  saveSession,
  saveObserver,
  runtimeObserver,
  selectSave,
  discoverSaves,
} from "./save-workspace";
import { toRecordTransferCount, type Query, type Sample } from "./model";
import type {
  LiveBatch,
  OperationReceipt,
  ProtectedJob,
  SaveReference,
} from "../../packages/contracts/protected-responses";

function saveInstallMessage(receipt: OperationReceipt): string {
  if (receipt.commit_status.startsWith("committed")) return "已添加到存档。";
  if (receipt.warning?.includes("SAVE_SYNC_ACTIVE"))
    return "检测到游戏正在同步存档，本次没有写入。请在标题菜单停留片刻后重试。";
  if (receipt.warning?.includes("SAVE_COMMIT_ROLLED_BACK"))
    return "写入后校验失败，主存档已自动恢复；游戏备份没有改动。";
  if (receipt.warning?.includes("SAVE_COMMIT_UNCERTAIN"))
    return "写入结果无法确认。请先核对存档和操作记录，不要重复添加。";
  return receipt.warning || "本次没有写入，请回到标题界面后重试。";
}

export function SavePicker({ compact = false }: { compact?: boolean }) {
  const state = useSyncExternalStore(
    saveSession!.subscribe,
    saveSession!.getSnapshot,
  );
  const [error, setError] = useState(""),
    [saves, setSaves] = useState<SaveReference[]>([]),
    [loading, setLoading] = useState(true);
  async function scan(refresh = false) {
    setLoading(true);
    try {
      setSaves(await discoverSaves(refresh));
      setError("");
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }
  useEffect(() => {
    void scan();
  }, []);
  return (
    <div className={"save-picker" + (compact ? " save-picker-compact" : "")}>
      <label>
        目标存档
        <select
          aria-label="自动检测的存档"
          disabled={state.busy || loading}
          value={state.selected?.save_id || ""}
          onChange={(e) => {
            const chosen = saves.find((s) => s.save_id === e.target.value);
            if (chosen)
              void saveSession!
                .select(chosen)
                .catch((e) => setError(String(e)));
          }}
        >
          <option value="">
            {loading
              ? "正在检测存档…"
              : saves.length
                ? "请选择角色存档"
                : "未检测到存档"}
          </option>
          {[
            ...saves,
            ...(state.selected &&
            !saves.some((s) => s.save_id === state.selected?.save_id)
              ? [state.selected]
              : []),
          ].map((s) => (
            <option key={s.save_id} value={s.save_id}>
              存档 {s.save_slot + 1}（SAVEDATA{String(s.save_slot).padStart(2, "0")}）· 账户 {s.account_id}
            </option>
          ))}
        </select>
      </label>
      <button disabled={state.busy || loading} onClick={() => void scan(true)}>
        重新检测
      </button>
      <button
        disabled={state.busy || loading}
        onClick={() => void selectSave().catch((e) => setError(String(e)))}
      >
        手动定位
      </button>
      {state.selected && (
        <button
          disabled={state.busy}
          onClick={() =>
            void saveSession!.refresh().catch((e) => setError(String(e)))
          }
        >
          重新读取
        </button>
      )}
      {state.uncertainOperationId && (
        <button
          onClick={() =>
            void saveSession!.recoverReceipt().catch((e) => setError(String(e)))
          }
        >
          核对上次写入结果
        </button>
      )}
      {!loading && !state.selected && saves.length > 1 && (
        <p className="save-picker-hint">
          检测到 {saves.length} 个存档，请在上面选择你正在玩的那一个；选过一次后会自动记住。
        </p>
      )}
      {!loading && !saves.length && !state.selected && !(error || state.error) && (
        <p className="save-picker-hint">
          没有找到存档。请先启动一次游戏并进入角色，或点“手动定位”选择 SAVEDATA.BIN。
        </p>
      )}
      <Notice text={error || state.error || ""} tone="error" />
    </div>
  );
}
export function DesktopCartActions({
  samples,
  mode,
  query,
  onAdded,
  autoPrepare = false,
}: {
  samples: Sample[];
  mode: string;
  query: Query;
  onAdded?: (keys: string[]) => void;
  autoPrepare?: boolean;
}) {
  const runtime = useSyncExternalStore(
    runtimeObserver!.subscribe,
    runtimeObserver!.getSnapshot,
  );
  const state = useSyncExternalStore(
    saveSession!.subscribe,
    saveSession!.getSnapshot,
  );
  const [busy, setBusy] = useState(false),
    [message, setMessage] = useState(""),
    [titleConfirmed, setTitleConfirmed] = useState(false);
  const [plan, setPlan] = useState<{
    signature: string;
    savePlan?: string;
    batch?: LiveBatch;
    count: number;
  } | null>(null);
  const [uncertain, setUncertain] = useState(
    () => !!localStorage.getItem("nioh3-review-live-batch"),
  );
  const [inspectFailed, setInspectFailed] = useState(false);
  const preparedOwner = useRef<PreparedLiveBatchOwner | null>(null);
  if (!preparedOwner.current) {
    preparedOwner.current = new PreparedLiveBatchOwner(async batch => {
      // Never replace the observer of an executing or interrupted protected operation.
      if (!runtimeObserver!.canStart()) throw Error("LIVE_BATCH_CLEANUP_DEFERRED_BUSY");
      const result = await runtimeObserver!.run(() => window.operations.execute({
        method: "runtime.live_batch_cancel",
        params: { batch_id: batch.batch_id },
      }));
      if (!result || !("live_batch" in result)) throw Error("LIVE_BATCH_CANCEL_RECEIPT_REQUIRED");
      return result.live_batch;
    }, batch => {
      const marker = JSON.parse(localStorage.getItem("nioh3-review-live-batch") || "null");
      if (marker?.batch_id === batch.batch_id && marker?.plan_digest === batch.plan_digest) {
        localStorage.removeItem("nioh3-review-live-batch");
        window.dispatchEvent(new Event("nioh3-live-batch-cancelled"));
      }
    });
  }
  useEffect(() => () => {
    void preparedOwner.current!.close().catch(error => {
      // Keep the matching recovery marker when cancellation cannot be confirmed.
      void window.review.log(`Prepared live batch cleanup: ${String(error)}`).catch(() => {});
    });
  }, []);
  useEffect(() => {
    // A newly opened view may overlap the previous view's cancellation response.
    const cancelled = () => {
      if (!localStorage.getItem("nioh3-review-live-batch")) setUncertain(false);
    };
    window.addEventListener("nioh3-live-batch-cancelled", cancelled);
    return () => window.removeEventListener("nioh3-live-batch-cancelled", cancelled);
  }, []);
  const references = samples.map((s) => s.backend?.referenceId || "");
  const signatureFor = (snapshotId: string | undefined) =>
    JSON.stringify([
      mode,
      references,
      query.recommended,
      query.transfers,
      state.selected?.save_id,
      snapshotId,
    ]);
  const signature = signatureFor(state.inventory?.snapshot_id);
  const initialPreparation = useRef(false);
  useEffect(() => {
    if (!autoPrepare || initialPreparation.current || !state.inventory || state.busy || busy || uncertain)
      return;
    // Only prepare once on entry. Writing always requires the confirmation button.
    initialPreparation.current = true;
    void prepare();
  }, [autoPrepare, state.inventory, state.busy, busy, uncertain]);
  async function prepare() {
    setBusy(true);
    setMessage("");
    try {
      if (uncertain) throw Error("请先核对上次添加结果。");
      if (mode === "live" && samples.some((s) => (s.playthrough || 3) > 3))
        throw Error("四、五周目的研究候选不能添加。");
      let inventory = state.inventory;
      if (!inventory) throw Error("请先选择并读取存档。");
      if (references.some((r) => !r))
        throw Error("候选已失效，请重新搜索后添加。");
      await preparedOwner.current!.discard();
      const resolution = await window.nioh.resolveRecommendedLevel(
        query.recommended,
      );
      if (
        resolution.status !== "exact" ||
        resolution.selected_internal_level === null
      )
        throw Error("推荐等级无法转换。");
      const level = resolution.selected_internal_level;
      const paramsFor = (snapshot: NonNullable<typeof inventory>) => ({
        mode: mode as "save" | "live",
        save_id: snapshot.save_id,
        snapshot_id: snapshot.snapshot_id,
        references,
        recommended_level: level,
        transfer_count: toRecordTransferCount(query.transfers),
      });
      if (preparedOwner.current!.closed) return;
      // An in-game save or a restarted save worker retires the snapshot this
      // view read. That is not a failure the player can act on: read the save
      // again and prepare once more against the fresh snapshot.
      const prepareFor = (params: ReturnType<typeof paramsFor>) =>
        mode === "save"
          ? saveSession!.prepareCart(
              async () =>
                (await saveObserver!.run(() =>
                  window.review.prepareCart(params),
                )) as NonNullable<ProtectedJob["result"]>,
            )
          : runtimeObserver!.run(() => window.review.prepareCart(params));
      let prepared;
      try {
        prepared = await prepareFor(paramsFor(inventory));
      } catch (error) {
        if (!/Snapshot expired/.test(String(error))) throw error;
        inventory = await saveSession!.refresh();
        if (!inventory || preparedOwner.current!.closed) return;
        prepared = await prepareFor(paramsFor(inventory));
      }
      const preparedSignature = signatureFor(inventory.snapshot_id);
      if (mode === "save") {
        if (!prepared || !("plan_id" in prepared)) throw Error("未收到添加计划。");
        setPlan({
          signature: preparedSignature,
          savePlan: prepared.plan_id,
          count: samples.length,
        });
      } else {
        const result = prepared;
        if (!result || !("live_batch" in result))
          throw Error("未收到添加计划。");
        // A prepared batch has dispatched nothing and the backend never treats
        // it as unresolved, so no recovery marker is written until execution
        // starts; an abandoned preview must not block later additions.
        if (!await preparedOwner.current!.adopt(result.live_batch)) return;
        setPlan({ signature: preparedSignature, batch: result.live_batch, count: samples.length });
      }
      setTitleConfirmed(false);
    } catch (error) {
      if (preparedOwner.current!.closed)
        void window.review.log(`Closed addition view preparation: ${String(error)}`).catch(() => {});
      else setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }
  async function execute() {
    if (!plan || plan.signature !== signature || busy) return;
    setBusy(true);
    try {
      if (plan.savePlan) {
        if (!titleConfirmed) throw Error("请确认游戏已回到标题界面。");
        const receipt = await saveSession!.commit(plan.savePlan);
        setPlan(null);
        setMessage(saveInstallMessage(receipt));
        if (receipt.commit_status.startsWith("committed")) {
          onAdded?.(samples.map(collectionKey));
          await saveSession!.refresh();
        }
      } else if (plan.batch) {
        preparedOwner.current!.beginExecution(plan.batch.batch_id);
        // From here the outcome must be confirmed from receipts before another
        // addition, even if this view closes mid-way.
        localStorage.setItem(
          "nioh3-review-live-batch",
          JSON.stringify({
            batch_id: plan.batch.batch_id,
            plan_digest: plan.batch.plan_digest,
            keys: samples.map(collectionKey),
          }),
        );
        setUncertain(true);
        const result = await runtimeObserver!.run(() =>
          window.operations.execute({
            method: "runtime.live_batch_execute",
            params: {
              batch_id: plan.batch!.batch_id,
              plan_digest: plan.batch!.plan_digest,
            },
          }),
        );
        if (
          !result ||
          !("live_batch" in result) ||
          result.live_batch.batch_id !== plan.batch.batch_id
        )
          throw Error("添加回执不匹配。");
        setPlan(result.live_batch.state === "complete" ? null : { ...plan, batch: result.live_batch });
        setUncertain(result.live_batch.state === "uncertain");
        onAdded?.(samples.slice(0, result.live_batch.verified_count).map(collectionKey));
        if (result.live_batch.state !== "uncertain")
          localStorage.removeItem("nioh3-review-live-batch");
        setMessage(
          `已验证添加 ${result.live_batch.verified_count} / ${result.live_batch.count} 张。${result.live_batch.state === "complete" ? "游戏背包里已经能看到；之后在游戏里正常存档（例如在神社休息），它才会写进存档文件。" : "其余操作尚未全部确认，请先核对结果，不要重复添加。"}`,
        );
      }
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }
  async function inspect() {
    const ref = JSON.parse(
      localStorage.getItem("nioh3-review-live-batch") || "null",
    );
    if (!ref) {
      // Nothing is waiting: that is an answer, not a failure.
      setUncertain(false);
      setMessage("没有待核对的实时添加。");
      return;
    }
    setBusy(true);
    try {
      await runtimeObserver!.recover();
      const result = await runtimeObserver!.run(() =>
        window.operations.execute({
          method: "runtime.live_batch_status",
          params: { batch_id: ref.batch_id },
        }),
      );
      if (!result || !("live_batch" in result)) throw Error("未取得添加回执。");
      let batch = result.live_batch;
      for (const operationId of batch.child_operation_ids) {
        const child = await runtimeObserver!.run(() =>
          window.operations.execute({
            method: "runtime.live_add_status",
            params: { operation_id: operationId },
          }),
        );
        if (
          child &&
          "live_add" in child &&
          child.live_add.state === "uncertain"
        )
          await runtimeObserver!.run(() =>
            window.operations.execute({
              method: "runtime.live_add_recover",
              params: { operation_id: operationId },
            }),
          );
      }
      const refreshed = await runtimeObserver!.run(() =>
        window.operations.execute({
          method: "runtime.live_batch_status",
          params: { batch_id: ref.batch_id },
        }),
      );
      if (!refreshed || !("live_batch" in refreshed))
        throw Error("未取得核对后的添加回执。");
      batch = refreshed.live_batch;
      if(Array.isArray(ref.keys))onAdded?.(ref.keys.slice(0,batch.verified_count));
      if (batch.state === "prepared") {
        await runtimeObserver!.run(() =>
          window.operations.execute({
            method: "runtime.live_batch_cancel",
            params: { batch_id: batch.batch_id },
          }),
        );
        setMessage("上次计划尚未执行，已取消，可以重新核对添加。");
      } else
        setMessage(
          `已验证添加 ${batch.verified_count} / ${batch.count} 张；${batch.state === "complete" ? "已完成" : batch.state === "cancelled" ? "已取消" : "请核对剩余操作，避免重复添加"}`,
        );
      setUncertain(batch.state === "uncertain");
      if (batch.state !== "uncertain") {
        localStorage.removeItem("nioh3-review-live-batch");
        setPlan(null);
      }
    } catch (error) {
      // Only an unsettled marker keeps new additions closed; a check that found
      // nothing to settle must not lock the view until it is reopened.
      setUncertain(!!localStorage.getItem("nioh3-review-live-batch"));
      setInspectFailed(true);
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }
  return (
    <section className="cart-execution">
      <SavePicker />
      {busy && runtime.job?.kind === "runtime.live_batch_execute" && (
        <div>
          <progress
            max={Number(runtime.job.progress?.total) || samples.length}
            value={Number(runtime.job.progress?.completed) || 0}
          />
          <p>
            已添加 {Number(runtime.job.progress?.completed) || 0}{" "}
            张，当前一张完成后可停止。
          </p>
          <button
            disabled={runtime.job.state === "cancel_requested"}
            onClick={() => void runtimeObserver!.cancel()}
          >
            停止后续添加
          </button>
        </div>
      )}
      {samples.some(
        (s) => (s.playthrough === 1 || s.playthrough === 2) && s.rarity === 4,
      ) && (
        <p className="temporary-warning">
          一、二周目的绿色绘卷属于自定义配置，不代表游戏自然掉落。
        </p>
      )}
      <p>
        本次选择 {samples.length} 张 · 推荐等级 {query.recommended} · 转手次数{" "}
        {query.transfers}
      </p>
      {mode === "save" && (
        <p className="temporary-warning">
          请先回到标题界面。检测到游戏正在同步存档时，程序会无损中止并提示重试。
        </p>
      )}
      {!state.inventory && (
        <p className="cart-hint">
          {state.busy ? "正在读取存档…" : "先在上面选好目标存档，才能核对添加。"}
        </p>
      )}
      {uncertain && (
        <p className="cart-hint">
          上次实时添加的结果还没有确认。请先点“核对上次实时添加”，确认后才能继续添加，避免重复。
        </p>
      )}
      <button
        disabled={busy || !samples.length || !state.inventory || uncertain}
        onClick={() => void prepare()}
      >
        核对添加
      </button>
      {uncertain && (
        <button disabled={busy} onClick={() => void inspect()}>
          核对上次实时添加
        </button>
      )}
      {uncertain && inspectFailed && !busy && (
        <div className="cart-forget">
          <p>
            如果游戏已经重启过，上次的结果可能无法再核对。可以先在游戏背包里看那张绘卷在不在，再决定是否继续。
          </p>
          <button
            onClick={() => {
              // The durable record stays in the backend for later recovery;
              // only this view's reminder is dropped, at the player's request.
              localStorage.removeItem("nioh3-review-live-batch");
              setUncertain(false);
              setInspectFailed(false);
              setMessage("已不再提醒上次的实时添加。继续添加前，请先在游戏背包里确认上次那张是否已经加进去，避免重复。");
            }}
          >
            不再核对，继续添加
          </button>
        </div>
      )}
      {plan && (
        <div className="prepared-cart">
          <p>
            {plan.signature === signature
              ? `已准备 ${plan.count} 张绘卷的添加计划。`
              : "选择已改变，请重新核对添加。"}
          </p>
          {plan.savePlan && (
            <label>
              <input
                type="checkbox"
                checked={titleConfirmed}
                onChange={(e) => setTitleConfirmed(e.target.checked)}
              />
              游戏已回到标题界面
            </label>
          )}
          <button
            disabled={
              busy ||
              uncertain ||
              plan.signature !== signature ||
              (!!plan.savePlan && !titleConfirmed) ||
              (!!plan.batch && plan.batch.state !== "prepared")
            }
            onClick={() => void execute()}
          >
            确认添加所选 {plan.count} 张
          </button>
        </div>
      )}
      <Notice text={message} />
    </section>
  );
}
