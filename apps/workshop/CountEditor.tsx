import React, { useEffect, useState, useSyncExternalStore } from "react";
import type { CountEdit } from "../../packages/contracts/protected-responses";
import type { Sample } from "./model";
import { runtimeObserver, saveSession } from "./save-workspace";

const storageKey = "nioh3-count-edit-review";
type Reference = { operation_id: string; plan_digest: string; scope: string };
const stored = (): Reference | null => {
  try {
    const value = JSON.parse(localStorage.getItem(storageKey) || "null");
    return value &&
      typeof value.operation_id === "string" &&
      /^[0-9a-f-]{36}$/.test(value.operation_id) &&
      /^[0-9a-f]{64}$/.test(value.plan_digest) &&
      typeof value.scope === "string"
      ? value
      : null;
  } catch {
    return null;
  }
};

export function CountEditor({ sample }: { sample: Sample }) {
  const save = useSyncExternalStore(
    saveSession!.subscribe,
    saveSession!.getSnapshot,
  );
  const [value, setValue] = useState(
    sample.saveEntry?.derived.remaining_challenge_attempts ?? 0,
  );
  const [plan, setPlan] = useState<CountEdit | null>(null),
    [ref, setRef] = useState<Reference | null>(stored);
  const [busy, setBusy] = useState(false),
    [message, setMessage] = useState("");
  const scope = JSON.stringify([
    save.inventory?.save_id,
    save.inventory?.snapshot_id,
    sample.saveEntry?.slot_index,
    value,
  ]);
  useEffect(
    () => setValue(sample.saveEntry?.derived.remaining_challenge_attempts ?? 0),
    [sample.saveEntry],
  );
  const finish = (result: unknown, expected?: Reference) => {
    if (!result || typeof result !== "object" || !("count_edit" in result))
      throw Error("COUNT_RESULT_EXPECTED");
    const next = (result as { count_edit: CountEdit }).count_edit;
    if (
      expected &&
      (next.operation_id !== expected.operation_id ||
        next.plan_digest !== expected.plan_digest)
    )
      throw Error("COUNT_RECEIPT_MISMATCH");
    setPlan(next);
    if (next.state === "verified") {
      localStorage.removeItem(storageKey);
      setRef(null);
      setMessage("当前次数已修改，请在游戏中正常保存。");
    } else if (next.state === "uncertain")
      setMessage("修改结果尚未确认，请核对上次操作，不要重复修改。");
    else if (next.state === "rejected") {
      localStorage.removeItem(storageKey);
      setRef(null);
      setMessage(next.error || "本次没有写入，请重新核对。");
    }
    return next;
  };
  async function prepare() {
    if (!save.inventory || !sample.saveEntry) return;
    setBusy(true);
    setMessage("");
    try {
      await runtimeObserver!.recover();
      const next = finish(
        await runtimeObserver!.run(() =>
          window.operations.prepareCount({
            save_id: save.inventory!.save_id,
            snapshot_id: save.inventory!.snapshot_id,
            slot_index: sample.saveEntry!.slot_index,
            new_count: value,
          }),
        ),
      );
      const reference = {
        operation_id: next.operation_id,
        plan_digest: next.plan_digest,
        scope,
      };
      localStorage.setItem(storageKey, JSON.stringify(reference));
      setRef(reference);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }
  async function execute() {
    if (!ref || scope !== ref.scope || plan?.state !== "prepared") return;
    setBusy(true);
    setMessage("");
    try {
      finish(
        await runtimeObserver!.run(() =>
          window.operations.execute({
            method: "runtime.count_execute",
            params: {
              operation_id: ref.operation_id,
              plan_digest: ref.plan_digest,
            },
          }),
        ),
        ref,
      );
    } catch (error) {
      setPlan(null);
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }
  async function inspect() {
    if (!ref) return;
    setBusy(true);
    try {
      await runtimeObserver!.recover();
      finish(
        await runtimeObserver!.run(() =>
          window.operations.execute({
            method: "runtime.count_recover",
            params: { operation_id: ref.operation_id },
          }),
        ),
        ref,
      );
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }
  return (
    <details className="module sand editor-section" name="editor-section">
      <summary>当前可挑战次数</summary>
      <div className="editor-section-body">
        <p>
          修改游戏内这张绘卷的剩余次数，修改后请在游戏中正常保存。挑战次数上限在“副本内容”中临时修改。
        </p>
        <label>
          设为{" "}
          <input
            type="number"
            min="0"
            max="7"
            aria-label="当前可挑战次数"
            value={Number.isNaN(value) ? "" : value}
            onChange={(e) => setValue(e.target.valueAsNumber)}
          />
        </label>
        <p>允许范围：0–7。设为0将无法继续挑战。</p>
        <button
          disabled={
            busy ||
            !!ref ||
            !save.inventory ||
            !Number.isInteger(value) ||
            value < 0 ||
            value > 7
          }
          onClick={() => void prepare()}
        >
          核对次数修改
        </button>
        {ref && (
          <button disabled={busy} onClick={() => void inspect()}>
            核对上次次数修改
          </button>
        )}
        {plan?.state === "prepared" && (
          <div className="prepared-cart">
            <p>
              绘卷 ID {plan.seed} · R{plan.rarity}：{plan.old_count} →{" "}
              {plan.new_count}
            </p>
            <p>自动备份已完成。</p>
            <button
              disabled={busy || scope !== ref?.scope}
              onClick={() => void execute()}
            >
              确认修改当前次数
            </button>
            <button
              disabled={busy}
              onClick={() => {
                localStorage.removeItem(storageKey);
                setRef(null);
                setPlan(null);
              }}
            >
              放弃本次核对
            </button>
          </div>
        )}
        {message && <p role="status">{message}</p>}
      </div>
    </details>
  );
}
