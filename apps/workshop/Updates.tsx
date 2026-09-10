import React, { useEffect, useState } from "react";
import type { UpdateState } from "../desktop/src/portable-update";
export function UpdateNotice({ onOpen }: { onOpen: () => void }) {
  const [available, setAvailable] = useState(false);
  useEffect(() => {
    let stopped = false;
    let checked = false;
    const channel = localStorage.getItem("nioh3-update-channel") === "beta" ? "beta" : "stable";
    async function poll(initial = false) {
      try {
        let state = await window.review.update({ action: "status", channel });
        if (!checked && state.canApply && state.phase === "idle") {
          checked = true;
          state = await window.review.update({ action: "check", channel });
        }
        if (!stopped) setAvailable(["available", "ready"].includes(state.phase));
      } catch { /* Background network failures must not interrupt editing. */ }
    }
    void poll(true);
    const timer = setInterval(() => void poll(), 3000);
    return () => { stopped = true; clearInterval(timer); };
  }, []);
  return available ? <button className="update-notice" onClick={onOpen}>发现新版本</button> : null;
}
export function Updates() {
  const [channel, setChannel] = useState<"stable" | "beta">(() =>
    localStorage.getItem("nioh3-update-channel") === "beta" ? "beta" : "stable",
  );
  const [state, setState] = useState<UpdateState & { canApply: boolean }>({
    phase: "idle",
    canApply: false,
  });
  const [error, setError] = useState("");
  async function action(action: "status" | "check" | "download" | "apply") {
    try {
      setState(await window.review.update({ action, channel }));
      setError("");
    } catch (e) {
      setError(String(e));
    }
  }
  useEffect(() => {
    void action("status");
    const timer = setInterval(() => void action("status"), 1000);
    return () => clearInterval(timer);
  }, [channel]);
  const busy = ["checking", "downloading"].includes(state.phase);
  return (
    <section className="update-panel">
      <label>
        更新通道
        <select
          value={channel}
          disabled={busy}
          onChange={(e) => {
            const next = e.target.value as "stable" | "beta";
            setChannel(next);
            localStorage.setItem("nioh3-update-channel", next);
          }}
        >
          <option value="stable">正式版</option>
          <option value="beta">测试版</option>
        </select>
      </label>
      <p>
        {
          {
            idle: "检查是否有新版本。",
            checking: "正在检查更新…",
            current: "当前没有可用的新版本。",
            available: "发现新版本 " + state.version,
            downloading: "正在下载并校验更新…",
            ready: "更新已准备好，重启后完成安装。",
            failed: "更新未完成，请重试。",
          }[state.phase]
        }
      </p>
      {state.notes && <p className="release-notes">{state.notes}</p>}
      {busy && (
        <progress
          max={state.total || 1}
          value={state.phase === "downloading" ? state.downloaded : undefined}
        />
      )}
      <div>
        <button disabled={busy} onClick={() => void action("check")}>
          检查更新
        </button>
        {state.phase === "available" && (
          <button onClick={() => void action("download")}>下载更新</button>
        )}
        {state.phase === "ready" && (
          <button
            disabled={!state.canApply}
            onClick={() => void action("apply")}
          >
            安全退出并安装更新
          </button>
        )}
      </div>
      {state.phase === "ready" && !state.canApply && (
        <p>源码启动版请使用发行包测试安装更新。</p>
      )}
      {(error || state.error) && <p role="alert">{error || state.error}</p>}
    </section>
  );
}
