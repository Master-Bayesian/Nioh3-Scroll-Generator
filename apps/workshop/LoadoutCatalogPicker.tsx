import React, { useState } from "react";
import type { LoadoutCatalog, LoadoutCatalogSelection, LoadoutBundles } from "../../packages/contracts/protected-responses";
import { saveObserver } from "./save-workspace";
import { errorText, publicError } from "./public-errors";

const MISSING_FACT: Record<string, string> = {
  runtime_landed_multiplicity: "实际命中次数",
  simultaneous_contact_order: "同时命中的结算次序",
  source_object_incarnation: "攻击来源对象的身份",
  source_rebuild_epoch: "来源状态更新的时点",
  runtime_extension_membership: "本次攻击实际包含的追加命中",
  projectile_descriptor_binding: "飞行道具与攻击参数的对应关系",
  stop_or_charge_decision: "持续攻击的停止或蓄力条件",
  non_projectile_dispatch_effects: "非飞行道具事件的实际效果",
  current_role_source_binding: "当前角色与攻击来源的对应关系",
  installed_evaluation_plan: "已安装的计算场景",
};

function attackName(selection: LoadoutCatalogSelection): string {
  return selection.name || `未命名攻击 ${selection.action}`;
}

/** The entire catalog remains discoverable without presenting static rows as runnable attacks. */
export function LoadoutCatalogPicker({ catalog, disabled, onSelect }: {
  catalog?: LoadoutCatalog;
  disabled: boolean;
  onSelect: (ids: string[]) => void;
}) {
  const [page, setPage] = useState(catalog);
  const [query, setQuery] = useState(catalog?.query ?? "");
  const [reading, setReading] = useState(false);
  const [error, setError] = useState("");
  if (!page) return null;
  const blocked = disabled || reading;
  const queryChanged = query.trim() !== page.query;
  async function read(offset: number) {
    if (!saveObserver?.canStart()) return;
    setReading(true); setError("");
    try {
      const response = await saveObserver.run(() => window.operations.execute({ method: "save.loadout_bundles",
        params: { catalog_query: query.trim(), catalog_offset: offset, catalog_limit: page?.limit ?? 50 } }));
      if (!response || !("bundles" in response) || !(response as LoadoutBundles).catalog)
        throw Error("LOADOUT_CATALOG_UNAVAILABLE");
      setPage((response as LoadoutBundles).catalog);
    } catch (failure) { setError(publicError(errorText(failure))); }
    finally { setReading(false); }
  }
  return <details className="loadout-catalog character-hint" data-testid="loadout-catalog">
    <summary>完整攻击目录</summary>
    {page.source_status !== "ready" ? <p>完整攻击目录暂不可用。上方仍可选择已安装的计算场景。</p> : <>
      <p><span>已编目攻击：</span> {page.total.toLocaleString()} · <span>匹配：</span> {page.matched.toLocaleString()}</p>
      <p>只有已具备完整计算条件的攻击可以搜索；其余攻击保留在目录中，并显示缺少的信息。</p>
      <form onSubmit={event => { event.preventDefault(); void read(0); }} className="loadout-form">
        <label className="loadout-field"><span>查找攻击</span>
          <input aria-label="查找攻击" value={query} maxLength={256} disabled={blocked}
            onChange={event => setQuery(event.target.value)} placeholder="武器、攻击名称或编号" />
        </label>
        <button type="submit" disabled={blocked}>查找</button>
      </form>
      <div className="loadout-catalog-list">
        {page.selections.map((selection, index) => <article key={`${selection.resource}:${selection.action}:${selection.parameter_sha256}:${index}`}>
          <strong>{attackName(selection)}</strong>
          <span className="loadout-count">{selection.weapon || "武器未确定"}</span>
          {selection.school && <span className="loadout-count">{selection.school === "ninja" ? "忍者" : "武士"}</span>}
          {selection.plan_ready && selection.scene_ids.length > 0 ?
            <button type="button" disabled={blocked} onClick={() => onSelect(selection.scene_ids)}>选择此攻击</button> :
            <span className="loadout-count">暂不可搜索</span>}
          {(!selection.plan_ready || !selection.static_complete) && <details>
            <summary>缺少的信息</summary>
            <ul>
              {!selection.static_complete && <li>完整的攻击参数记录</li>}
              {selection.missing_facts.map(fact => <li key={fact}>{MISSING_FACT[fact] ?? fact}</li>)}
              {!selection.plan_ready && selection.missing_facts.length === 0 && <li>尚未建立可执行的攻击计算场景</li>}
            </ul>
          </details>}
          <small className="equipment-notes">{selection.resource} · {selection.action}</small>
        </article>)}
        {page.matched === 0 && <p>没有匹配的攻击。</p>}
      </div>
      <nav aria-label="攻击目录分页">
        <button type="button" disabled={blocked || queryChanged || page.offset === 0}
          onClick={() => { void read(Math.max(0, page.offset - page.limit)); }}>上一页</button>
        <span> {Math.min(page.offset + 1, page.matched)}–{Math.min(page.offset + page.limit, page.matched)} / {page.matched} </span>
        <button type="button" disabled={blocked || queryChanged || page.offset + page.limit >= page.matched}
          onClick={() => { void read(page.offset + page.limit); }}>下一页</button>
      </nav>
    </>}
    {error && <p role="alert">{error}</p>}
  </details>;
}
