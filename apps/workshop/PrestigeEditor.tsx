import React, { useEffect, useRef, useState } from "react";
import type { PrestigeSnapshot, PrestigeEdit } from "../../packages/contracts/protected-responses";
import { runtimeObserver } from "./save-workspace";
import { Notice } from "./Notice";
import "./prestige-editor.css";

const storageKey = "nioh3-prestige-operation-v1";
const labels: Record<number, string> = {
  50152: "武士", 19338: "忍者", 41583: "战法", 29207: "讨伐", 60307: "强敌",
  1486: "地狱", 18917: "探索", 50326: "设施", 41813: "收集",
};
type Snapshot = PrestigeSnapshot["prestige"];
type Result = PrestigeEdit["prestige_edit"];
type Review = { snapshot: Snapshot; category: Snapshot["categories"][number]; available: number };
function pendingId() {
  try {
    const value = localStorage.getItem(storageKey);
    return value && /^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(value) ? value : null;
  } catch { return null; }
}
const terminal = (state: Result["state"]) => state !== "uncertain";

/** One category at a time; response loss retains its UUID and never replays a write. */
export function PrestigeEditor({ onBusy }: { onBusy?: (value: boolean) => void }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [drafts, setDrafts] = useState<Record<number, string>>({});
  const [review, setReview] = useState<Review | null>(null);
  const [pending, setPending] = useState<string | null>(pendingId);
  const [result, setResult] = useState<Result | null>(null);
  const [busy, setBusy] = useState(false);
  const [resourcesPending, setResourcesPending] = useState(false);
  const [message, setMessage] = useState("");
  const mounted = useRef(false), running = useRef(false), callbacks = useRef(onBusy);
  callbacks.current = onBusy;
  const reviewHeading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);
  useEffect(() => {
    if (!review) return;
    reviewHeading.current?.focus();
    const close = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !running.current) setReview(null);
    };
    document.addEventListener("keydown", close);
    return () => document.removeEventListener("keydown", close);
  }, [review]);

  async function run(task: () => Promise<void>) {
    if (running.current) return;
    running.current = true; setBusy(true); callbacks.current?.(true); setMessage("");
    try { await task(); }
    catch {
      if (mounted.current) setMessage(pendingId()
        ? "操作结果尚未确认，请先核对上次操作，不要重复写入。"
        : "未能读取或修改恩惠点数。请确认已进入游戏，使用支持的 PC v2.02 版本后重新读取。");
    } finally {
      running.current = false;
      if (mounted.current) setBusy(false);
      callbacks.current?.(false);
    }
  }
  async function read() {
    const reply = await window.operations.execute({ method: "runtime.prestige_snapshot", params: {} });
    if (!reply || !("prestige" in reply)) throw Error("PRESTIGE_SNAPSHOT_EXPECTED");
    if (!mounted.current) return;
    setSnapshot(reply.prestige); setReview(null);
    setDrafts(Object.fromEntries(reply.prestige.categories.map(row => [row.category_id, String(row.available)])));
  }
  function adopt(reply: unknown, id: string) {
    if (!reply || typeof reply !== "object" || !("prestige_edit" in reply)) throw Error("PRESTIGE_RESULT_EXPECTED");
    const next = (reply as PrestigeEdit).prestige_edit;
    if (next.operation_id !== id) throw Error("PRESTIGE_RECEIPT_MISMATCH");
    // The durable reference remains valid even if navigation hid the component.
    if (terminal(next.state)) {
      try { if (localStorage.getItem(storageKey) === id) localStorage.removeItem(storageKey); }
      catch { /* A retained terminal UUID is safe: status never replays its write. */ }
    }
    if (!mounted.current) return next;
    setResult(next); setReview(null);
    if (terminal(next.state)) setPending(null);
    setMessage(next.state === "verified"
      ? "恩惠点数已写入并核对。请在游戏中确认效果，并正常存档。"
      : next.state === "uncertain"
        ? "操作结果尚未确认，请先核对上次操作，不要重复写入。"
        : next.state === "expired"
          ? "原游戏进程已结束。请重新读取当前角色并核对点数。"
          : next.state === "not_applied"
            ? "当前仍为修改前的点数，未重新执行写入。"
            : "本次未执行新的写入，请重新读取点数后核对。");
    return next;
  }
  async function check(id: string, recover = false) {
    const execute = () => window.operations.execute({
      method: recover ? "runtime.prestige_recover" : "runtime.prestige_status",
      params: { operation_id: id },
    });
    const reply = runtimeObserver ? await runtimeObserver.run(execute) : await execute();
    const next = adopt(reply, id);
    if (mounted.current && next && terminal(next.state)) {
      setSnapshot(null); setDrafts({});
    }
  }
  useEffect(() => {
    // StrictMode re-runs effects; the running guard keeps one initial request.
    void run(async () => {
      const resources = await window.operations.execute({method:"runtime.player_status",params:{}});
      if (resources && "player_edit" in resources && resources.player_edit.state === "uncertain") {
        if (mounted.current) { setResourcesPending(true); setMessage("请先到人物资源分区核对上次操作。"); }
      }
      if (pending) return check(pending);
      const reply = await window.operations.execute({ method: "runtime.prestige_status", params: {} });
      if (!reply || !("prestige_edit" in reply)) throw Error("PRESTIGE_RESULT_EXPECTED");
      const found = reply.prestige_edit.operation_id;
      if (found) {
        localStorage.setItem(storageKey, found);
        if (mounted.current) setPending(found);
        adopt(reply, found);
      } else await read();
    });
  }, []);

  async function write() {
    if (!review || pending || resourcesPending || running.current) return;
    const selected = review;
    await run(async () => {
      const id = crypto.randomUUID();
      // Failure to retain the recovery reference aborts before any dispatch.
      localStorage.setItem(storageKey, id); setPending(id); setReview(null);
      const execute = () => window.operations.execute({
        method: "runtime.prestige_edit",
        params: {
          operation_id: id, snapshot_id: selected.snapshot.snapshot_id,
          process_id: selected.snapshot.process_id, process_birth: selected.snapshot.process_birth,
          category_id: selected.category.category_id, available: selected.available, confirmed: true,
        },
      });
      const reply = runtimeObserver ? await runtimeObserver.run(execute) : await execute();
      const next = adopt(reply, id);
      if (next?.state === "verified" && mounted.current) {
        // Keep a verified write result even when the independent refresh fails.
        try { await read(); } catch { setSnapshot(null); }
      }
    });
  }
  function prepare(row: Snapshot["categories"][number]) {
    if (!snapshot || pending || resourcesPending || busy) return;
    const text = drafts[row.category_id]?.trim() ?? "";
    if (!/^\d+$/.test(text)) { setMessage("请输入有效的非负整数。"); return; }
    const available = Number(text);
    if (!Number.isSafeInteger(available) || available + row.spent > snapshot.max_budget) {
      setMessage("目标可用点数加已分配点数不得超过 2000。"); return;
    }
    if (available === row.available) { setMessage("没有需要修改的内容。"); return; }
    setMessage(""); setResult(null);
    setReview({ snapshot, category: row, available });
  }
  return <section className="prestige-editor" aria-label="称号恩惠">
    <div className="character-toolbar">
      <h2>称号恩惠点数</h2>
      <button onClick={() => void run(read)} disabled={busy || !!pending}>重新读取点数</button>
    </div>
    <p className="character-hint">修改正在运行的游戏中的可用恩惠点数。九个类别独立计算，每类总点数最多设为 2000，已分配点数保持不变。</p>
    <Notice tone="warning" text="此功能尚未完成实机验证。写入后请在游戏内核对，保存和重载效果仍待确认。" />
    <Notice text={message} />
    {pending && <section className="character-plan prestige-recovery" aria-label="恩惠操作核对">
      <h3>有待核对的恩惠操作</h3>
      <p>重新读取操作记录或核对当前游戏，只会检查结果，不会重复写入。</p>
      <div className="character-actions">
        <button disabled={busy} onClick={() => void run(() => check(pending))}>读取操作记录</button>
        <button disabled={busy} onClick={() => void run(() => check(pending, true))}>恢复核对</button>
      </div>
    </section>}
    {review && <section className="character-plan prestige-review" aria-labelledby="prestige-review-title">
      <h3 id="prestige-review-title" ref={reviewHeading} tabIndex={-1}>确认恩惠点数修改</h3>
      <dl>
        <div><dt>类别</dt><dd>{labels[review.category.category_id]}</dd></div>
        <div><dt>可用点数</dt><dd>{review.category.available} → {review.available}</dd></div>
        <div><dt>已分配点数</dt><dd>{review.category.spent}</dd></div>
      </dl>
      <p>仅修改所选类别的点数，不改变称号、名声阶级或已分配的恩惠。</p>
      <div className="character-actions">
        <button className="primary" disabled={busy} onClick={() => void write()}>确认写入恩惠点数</button>
        <button disabled={busy} onClick={() => setReview(null)}>取消</button>
      </div>
    </section>}
    {snapshot && <div className="prestige-grid">
      {snapshot.categories.map(row => <section className="prestige-category" key={row.category_id} data-prestige-category={row.category_id}>
        <h3>{labels[row.category_id]}</h3>
        <dl>
          <div><dt>名声阶级</dt><dd>{row.rank}</dd></div>
          <div><dt>名声分数</dt><dd>{row.fame}</dd></div>
          <div><dt>已分配点数</dt><dd>{row.spent}</dd></div>
          <div><dt>可用点数</dt><dd>{row.available}</dd></div>
        </dl>
        <label><span>目标可用点数</span><input inputMode="numeric" aria-label="目标可用点数"
          value={drafts[row.category_id] ?? ""} disabled={busy || !!pending || resourcesPending || !row.editable}
          onChange={event => { setDrafts(old => ({ ...old, [row.category_id]: event.target.value })); setReview(null); }} /></label>
        <button disabled={busy || !!pending || resourcesPending || !row.editable}
          onClick={() => prepare(row)}>核对修改</button>
        {!row.editable && <p className="character-muted">该类别尚无可修改的点数记录。请先在游戏中获得对应称号。</p>}
      </section>)}
    </div>}
    {!snapshot && !busy && !pending && !result && <p className="character-empty">进入游戏后读取恩惠点数。</p>}
  </section>;
}
