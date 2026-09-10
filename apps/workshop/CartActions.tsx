import {collectionKey} from "./collections";
import React, { useState, useSyncExternalStore, useEffect } from "react";
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
  ProtectedJob,
  SaveReference,
} from "../../packages/contracts/protected-responses";
export function SavePicker() {
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
    <div className="save-picker">
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
              账户 {s.account_id} · 存档 {s.save_slot + 1}
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
      {(error || state.error) && <p role="alert">{error || state.error}</p>}
    </div>
  );
}
export function DesktopCartActions({
  samples,
  mode,
  query,
  onAdded,
}: {
  samples: Sample[];
  mode: string;
  query: Query;
  onAdded?: (keys: string[]) => void;
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
  const references = samples.map((s) => s.backend?.referenceId || "");
  const signature = JSON.stringify([
    mode,
    references,
    query.recommended,
    query.transfers,
    state.selected?.save_id,
    state.inventory?.snapshot_id,
  ]);
  async function prepare() {
    setBusy(true);
    setMessage("");
    try {
      if (uncertain) throw Error("请先核对上次添加结果。");
      if (mode === "live" && samples.some((s) => (s.playthrough || 3) > 3))
        throw Error("四、五周目的研究候选不能添加。");
      const inventory = state.inventory;
      if (!inventory) throw Error("请先选择并读取存档。");
      if (references.some((r) => !r))
        throw Error("购物车存在失效的候选，请重新搜索后添加。");
      if (plan?.batch?.state === "prepared")
        await runtimeObserver!.run(() =>
          window.operations.execute({
            method: "runtime.live_batch_cancel",
            params: { batch_id: plan.batch!.batch_id },
          }),
        );
      const resolution = await window.nioh.resolveRecommendedLevel(
        query.recommended,
      );
      if (
        resolution.status !== "exact" ||
        resolution.selected_internal_level === null
      )
        throw Error("推荐等级无法转换。");
      const params = {
        mode: mode as "save" | "live",
        save_id: inventory.save_id,
        snapshot_id: inventory.snapshot_id,
        references,
        recommended_level: resolution.selected_internal_level,
        transfer_count: toRecordTransferCount(query.transfers),
      };
      if (mode === "save") {
        const prepared = await saveSession!.prepareCart(
          async () =>
            (await saveObserver!.run(() =>
              window.review.prepareCart(params),
            )) as NonNullable<ProtectedJob["result"]>,
        );
        setPlan({
          signature,
          savePlan: prepared.plan_id,
          count: samples.length,
        });
      } else {
        const result = await runtimeObserver!.run(() =>
          window.review.prepareCart(params),
        );
        if (!result || !("live_batch" in result))
          throw Error("未收到添加计划。");
        setPlan({ signature, batch: result.live_batch, count: samples.length });
        localStorage.setItem(
          "nioh3-review-live-batch",
          JSON.stringify({
            batch_id: result.live_batch.batch_id,
            plan_digest: result.live_batch.plan_digest,
            keys:samples.map(collectionKey),
          }),
        );
      }
      setTitleConfirmed(false);
    } catch (error) {
      setMessage(String(error));
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
        setMessage(
          receipt.commit_status === "committed" ||
            receipt.commit_status === "committed_with_warning"
            ? "已添加到存档。"
            : "写入尚未确认，请核对操作结果。",
        );
        if (receipt.commit_status.startsWith("committed")) {
          onAdded?.(samples.map(collectionKey));
          await saveSession!.refresh();
        }
      } else if (plan.batch) {
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
          `已验证添加 ${result.live_batch.verified_count} / ${result.live_batch.count} 张。${result.live_batch.state === "complete" ? "请在游戏中正常保存。" : "其余操作尚未全部确认，请先核对结果，不要重复添加。"}`,
        );
      }
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }
  async function inspect() {
    setBusy(true);
    try {
      const ref = JSON.parse(
        localStorage.getItem("nioh3-review-live-batch") || "null",
      );
      if (!ref) throw Error("没有待核对的实时添加。");
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
      setUncertain(true);
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
      <button
        disabled={busy || !samples.length || !state.inventory || uncertain}
        onClick={() => void prepare()}
      >
        核对添加
      </button>
      {(mode === "live" || uncertain) && (
        <button disabled={busy} onClick={() => void inspect()}>
          核对上次实时添加
        </button>
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
      {message && <p role="status">{message}</p>}
    </section>
  );
}
