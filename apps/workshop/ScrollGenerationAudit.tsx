import React, { useEffect, useRef, useState } from "react";
import type { ProtectedResult } from "../desktop/src/operations-api";
import type {
  ScrollAudit,
  ScrollAuditRow,
} from "../../packages/contracts/protected-responses";
import { saveObserver, saveSession } from "./save-workspace";
type ContextProof = ScrollAudit["context"];

interface AuditCommand {
  method: "save.audit_scrolls";
  params: { save_id: string; snapshot_id: string };
}

function isScrollAudit(value: unknown): value is ScrollAudit {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<ScrollAudit>;
  return (
    typeof candidate.save_id === "string" &&
    typeof candidate.snapshot_id === "string" &&
    typeof candidate.source_sha256 === "string" &&
    candidate.coverage_scope === "generated_effect_projection" &&
    typeof candidate.context === "object" &&
    candidate.context !== null &&
    Array.isArray(candidate.rows)
  );
}

function display(value: unknown): string {
  if (value === null || value === undefined || value === "")
    return "未提供";
  return String(value);
}

function replayLabel(row: ScrollAuditRow): string {
  const evidence = row.replay_evidence;
  if (!evidence?.attempted)
    return "不支持";
  return evidence.matched
    ? "生成核对匹配"
    : "生成核对未匹配";
}

const reasonLabels: Record<string, string> = {
  normal_input_domain_unproven: "正常取得条件尚未验证",
  unsupported_record_type: "记录类型暂不支持",
  unsupported_playthrough: "周目暂不支持",
  unsupported_rarity: "稀有度暂不支持",
  unsupported_game_file_version: "游戏文件版本不支持",
  context_not_production: "当前上下文不是生产环境",
  context_unavailable_for_replay: "缺少重放所需上下文",
  replay_match: "重放匹配",
  replay_mismatch: "重放不匹配",
};

function reasonLabel(code: string): string {
  const translated = reasonLabels[code];
  if (translated) return translated;
  if (code.startsWith("replay_error:"))
    return `重放错误：${code.slice("replay_error:".length)}`;
  return code;
}

function contextEntries(context: ContextProof): [string, unknown][] {
  return [
    ["product_version", context.product_version],
    ["game_profile", context.game_profile],
    ["game_file_version", context.game_file_version],
    ["versioned_resource_dir", context.versioned_resource_dir],
    ["bundle_digest", context.bundle_digest],
    ["versioned_digest", context.versioned_digest],
    ["resources_digest", context.resources_digest],
    ["algorithm_version", context.algorithm_version],
    ["policy_version", context.policy_version],
    ["context_digest", context.context_digest],
    ["legacy_context_digest", context.legacy_context_digest],
    ["production_authority", context.production_authority ? "是" : "否"],
    ["seed_accelerator_abi", context.seed_accelerator_abi],
    ["seed_accelerator_build_id", context.seed_accelerator_build_id],
  ];
}

export function ScrollGenerationAudit({ active = true }: { active?: boolean }) {
  const [saveState, setSaveState] = useState(() => saveSession?.getSnapshot());
  const [, refreshObserver] = useState(0);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<ScrollAudit | null>(null);
  const [error, setError] = useState("");
  const requestToken = useRef(0);
  const identity = saveState?.inventory
    ? `${saveState.inventory.save_id}:${saveState.inventory.snapshot_id}`
    : "";

  useEffect(() => {
    const session = saveSession;
    return session?.subscribe(() => setSaveState(session.getSnapshot()));
  }, []);
  useEffect(() => saveObserver?.subscribe(() => refreshObserver((value) => value + 1)), []);
  useEffect(() => {
    requestToken.current += 1;
    setResult(null);
    setError("");
    setBusy(false);
  }, [identity, active]);
  useEffect(() => () => {
    requestToken.current += 1;
  }, []);

  const inventory = saveState?.inventory;
  const canAudit =
    !!inventory &&
    !!saveState?.selected &&
    saveState.selected.save_id === inventory.save_id &&
    !saveState.busy &&
    !busy &&
    !!saveObserver &&
    saveObserver.canStart();

  async function runAudit() {
    if (!inventory || !saveState?.selected || !saveObserver || busy) return;
    const token = ++requestToken.current;
    const requestIdentity = `${inventory.save_id}:${inventory.snapshot_id}`;
    setBusy(true);
    setResult(null);
    setError("");
    const command: AuditCommand = {
      method: "save.audit_scrolls",
      params: {
        save_id: inventory.save_id,
        snapshot_id: inventory.snapshot_id,
      },
    };
    try {
      const raw = await saveObserver.run(() =>
        (window.operations.execute as unknown as (
          value: AuditCommand,
        ) => Promise<ProtectedResult>)(command),
      );
      const current = saveSession?.getSnapshot().inventory;
      if (
        token !== requestToken.current ||
        requestIdentity !==
          (current ? `${current.save_id}:${current.snapshot_id}` : "")
      )
        return;
      if (!isScrollAudit(raw)) throw new Error("UNEXPECTED_SCROLL_AUDIT_RESULT");
      setResult(raw);
    } catch (value) {
      if (token === requestToken.current)
        setError(String(value));
    } finally {
      if (token === requestToken.current) setBusy(false);
    }
  }

  return (
    <section
      className="scroll-audit module"
      data-testid="scroll-generation-audit"
      aria-label="绘卷生成核对（实验）"
    >
      <header>
        <div>
          <h2>绘卷生成核对（实验）</h2>
          <p className="scroll-audit-subtitle">
            用于核对 PC v2.02 三周目 R3/R4 绘卷词条是否与当前生成器一致。
          </p>
        </div>
        <button
          type="button"
          className="scroll-audit-run"
          disabled={!canAudit}
          onClick={() => void runAudit()}
        >
          生成核对
        </button>
      </header>
      <p className="scroll-audit-disclaimer">
        匹配不代表完整规则判定，且不会修改存档。
      </p>
      {!inventory && (
        <p className="scroll-audit-status" role="status">
          尚未加载存档。
        </p>
      )}
      {busy && (
        <p className="scroll-audit-status" role="status">
          正在核对…
        </p>
      )}
      {error && (
        <div className="scroll-audit-error" role="alert">
          <strong>读取失败：</strong> {error}
          <p>重新读取存档后重试。</p>
        </div>
      )}
      {result && (
        <div className="scroll-audit-result">
          <div className="scroll-audit-summary">
            <span>状态：{result.status}</span>
            <span>存档：{result.save_id}</span>
            <span>快照：{result.snapshot_id}</span>
          </div>
          <details className="scroll-audit-technical">
            <summary>技术范围与摘要</summary>
            <dl>
              <div><dt>coverage_scope</dt><dd>{result.coverage_scope}</dd></div>
              <div><dt>source_sha256</dt><dd>{result.source_sha256}</dd></div>
            </dl>
          </details>
          <details className="scroll-audit-context">
            <summary>上下文证据</summary>
            <dl>
              {contextEntries(result.context).map(([key, value]) => (
                <div key={key}>
                  <dt>{key}</dt>
                  <dd>{display(value)}</dd>
                </div>
              ))}
            </dl>
          </details>
          <div className="scroll-audit-rows">
            {result.rows.map((row) => (
              <article className="scroll-audit-row" data-testid="scroll-audit-row" key={`${row.slot_index}:${row.record_sha256}`}>
                <header>
                  <strong>槽位 {row.slot_index}</strong>
                  <span className="scroll-audit-replay">{replayLabel(row)}</span>
                </header>
                <dl>
                  <div><dt>种子</dt><dd>{row.seed}</dd></div>
                  <div><dt>稀有度</dt><dd>R{row.rarity}</dd></div>
                  <div><dt>等级</dt><dd>{row.level}</dd></div>
                  <div><dt>周目</dt><dd>{display(row.playthrough)}</dd></div>
                  <div><dt>重放阶段</dt><dd>{display(row.replay_evidence?.matched_phase)}</dd></div>
                </dl>
                <p><b>原因：</b>{" "}{row.reasons.length ? row.reasons.map((reason, index) => <React.Fragment key={`${reason}:${index}`}>{index ? ", " : ""}<span>{reasonLabel(reason)}</span></React.Fragment>) : "无"}</p>
                {row.reasons.length > 0 && (
                  <details>
                    <summary>原始原因代码</summary>
                    <code className="scroll-audit-reasons-raw">{row.reasons.join(", ")}</code>
                  </details>
                )}
                <details className="scroll-audit-technical">
                  <summary>技术记录字段</summary>
                  <dl>
                    <div><dt>record_type</dt><dd>{row.record_type}</dd></div>
                    <div><dt>record_offset</dt><dd>{row.record_offset}</dd></div>
                    <div><dt>record_sha256</dt><dd>{row.record_sha256}</dd></div>
                  </dl>
                </details>
                {row.replay_evidence?.compared_fields.length > 0 && (
                  <p><b>比对字段：</b> {row.replay_evidence.compared_fields.join(", ")}</p>
                )}
                {row.replay_evidence?.phase_results.length > 0 && (
                  <details>
                    <summary>阶段结果</summary>
                    <ul>
                      {row.replay_evidence.phase_results.map((phase) => (
                        <li key={`${phase.phase}:${phase.matched}`}>
                          {phase.phase} · {phase.matched ? "匹配" : "未匹配"}
                          {phase.mismatches.length ? ` · ${phase.mismatches.join(", ")}` : ""}
                        </li>
                      ))}
                    </ul>
                  </details>
                )}
              </article>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}
