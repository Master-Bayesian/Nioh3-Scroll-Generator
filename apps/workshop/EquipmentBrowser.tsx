import React, { useEffect, useMemo, useRef, useState } from "react";
import { data } from "./model";
import type {
  InventorySnapshot,
  InventorySnapshotRow,
} from "../../packages/contracts/protected-responses";
import {
  desktop,
  equipmentInventorySnapshot,
} from "./desktop-bridge";
import {
  LocalCatalogImport,
  type ActiveLocalCatalog,
} from "./LocalCatalogImport";

const SLOT_SENTINEL = 0xffff;

type InventorySession = Pick<InventorySnapshot, "game_version" | "process" | "observed_slot_count">;

function sessionOf(snapshot: InventorySnapshot): InventorySession {
  return {
    game_version: snapshot.game_version,
    process: snapshot.process,
    observed_slot_count: snapshot.observed_slot_count,
  };
}

function sameSession(left: InventorySession, right: InventorySession) {
  return (
    left.game_version === right.game_version &&
    left.observed_slot_count === right.observed_slot_count &&
    left.process.pid === right.process.pid &&
    left.process.creation_filetime === right.process.creation_filetime
  );
}

/**
 * Effect names resolve by exact numeric id across the existing product
 * resources only. No base/variant normalization and no cross-rarity guessing:
 * an unknown id keeps its hex form.
 */
function collectEffectNames(): Map<number, string> {
  const names = new Map<number, string>();
  const add = (id: unknown, name: unknown) => {
    const numeric = Number(id);
    if (!Number.isInteger(numeric) || typeof name !== "string" || !name) return;
    if (!names.has(numeric)) names.set(numeric, name);
  };
  for (const row of data.editorEffects as { id: string; name: string }[])
    add(row.id, row.name);
  for (const context of Object.values(data.contexts) as unknown as {
    effects?: { id: string; name: string }[];
    graces?: { id: string; name: string }[];
  }[]) {
    for (const row of context.effects || []) add(row.id, row.name);
    for (const row of context.graces || []) add(row.id, row.name);
  }
  return names;
}
const effectNames = collectEffectNames();

/**
 * The product bundles no item-name catalog. IDs resolve only through the
 * optional exact, user-sourced local mapping; otherwise they render as an
 * honest numeric fallback and never a guess.
 */
const itemName = (id: number, catalog: ActiveLocalCatalog | null) =>
  catalog?.entries.get(id) ?? null;

function hexId(value: number) {
  const digits = Math.max(4, value > 0xffff ? 8 : 4);
  return "0x" + value.toString(16).toUpperCase().padStart(digits, "0");
}
function effectName(id: number) {
  return id === SLOT_SENTINEL ? null : effectNames.get(id) ?? null;
}

export function EquipmentBrowser() {
  const [catalog, setCatalog] = useState<ActiveLocalCatalog | null>(null);
  const [rows, setRows] = useState<InventorySnapshotRow[] | null>(null);
  const [snapshot, setSnapshot] = useState<InventorySnapshot | null>(null);
  const [session, setSession] = useState<InventorySession | null>(null);
  const [history, setHistory] = useState<number[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<number | null>(null);
  // A monotonic token plus an unmount guard discards any response that arrives
  // after a newer request, a page change, or unmount.
  const request = useRef(0);
  const live = useRef(true);
  useEffect(
    () => () => {
      live.current = false;
    },
    [],
  );

  async function load(start: number, establishSession = false) {
    const token = ++request.current;
    setLoading(true);
    setError("");
    // Do not leave an older page or selected row looking current while a new
    // read is in flight. An explicit refresh is the only operation that may
    // establish a different process session.
    setRows(null);
    setSnapshot(null);
    setSelected(null);
    if (establishSession) setSession(null);
    try {
      const next = await equipmentInventorySnapshot(start);
      if (token !== request.current || !live.current) return;
      const nextSession = sessionOf(next);
      if (session && !establishSession && !sameSession(session, nextSession)) {
        setRows(null);
        setSnapshot(null);
        setSelected(null);
        setSession(null);
        setHistory([]);
        setError("游戏会话或槽位范围已变化，请点击加载装备按钮重新加载装备。");
        return;
      }
      setRows(next.rows);
      setSnapshot(next);
      setSession(nextSession);
      setSelected(null);
    } catch (reason) {
      if (token !== request.current || !live.current) return;
      setRows(null);
      setSnapshot(null);
      setSelected(null);
      setSession(null);
      setHistory([]);
      setError(String(reason));
    } finally {
      if (token === request.current && live.current) setLoading(false);
    }
  }

  function reload() {
    setHistory([]);
    void load(0, true);
  }
  function nextPage() {
    if (!snapshot || snapshot.next_start === null || loading) return;
    setHistory((stack) => [...stack, snapshot.start]);
    void load(snapshot.next_start);
  }
  function previousPage() {
    if (!history.length || loading) return;
    const target = history[history.length - 1];
    setHistory((stack) => stack.slice(0, -1));
    void load(target);
  }

  const visible = useMemo(() => {
    if (!rows) return [];
    const needle = query.trim().toLowerCase();
    if (!needle) return rows;
    return rows.filter((row) => {
      const name = itemName(row.item_id, catalog) || "";
      const hex = hexId(row.item_id).toLowerCase();
      return (
        String(row.slot).includes(needle) ||
        hex.includes(needle) ||
        hex.slice(2).includes(needle) ||
        name.toLowerCase().includes(needle)
      );
    });
  }, [catalog, rows, query]);

  const active = rows?.find((row) => row.slot === selected) ?? null;
  const start = snapshot?.start ?? 0;
  const end = snapshot && snapshot.rows.length ? start + snapshot.rows.length - 1 : start;

  return (
    <section className="equipment-page" aria-busy={loading}>
      <header className="equipment-toolbar">
        <button
          className="equipment-load"
          onClick={reload}
          disabled={loading || !desktop}
        >
          {snapshot ? "刷新" : "加载装备"}
        </button>
        <label className="equipment-search">
          本页搜索（名称或十六进制 ID）
          <input
            value={query}
            placeholder="例如 0xF6E8"
            onChange={(event) => setQuery(event.target.value)}
          />
        </label>
      </header>
      <LocalCatalogImport onCatalogChange={setCatalog} />
      <p className="equipment-description">
        只读实验功能：读取运行中的游戏内存，不写入存档。
      </p>
      <p className="equipment-status" role="status">
        {error
          ? "读取失败："
          : loading
            ? "正在读取…"
            : snapshot
              ? "已加载"
              : "尚未加载。请点击加载装备按钮读取当前背包。"}
      </p>
      {error && <p className="equipment-error">{error}</p>}
      {snapshot && (
        <>
          <dl className="equipment-meta">
            <div>
              <dt>游戏版本</dt>
              <dd>{snapshot.game_version}</dd>
            </div>
            <div>
              <dt>观测时间</dt>
              <dd>{snapshot.observed_at}</dd>
            </div>
            <div>
              <dt>原始槽位总数</dt>
              <dd>{snapshot.observed_slot_count}</dd>
            </div>
            <div>
              <dt>本页行数</dt>
              <dd>{snapshot.rows.length}</dd>
            </div>
            <div>
              <dt>一致性</dt>
              <dd>二次读取一致</dd>
            </div>
            <div>
              <dt>只读</dt>
              <dd>是</dd>
            </div>
          </dl>
          <div className="equipment-pager">
            <button onClick={previousPage} disabled={!history.length || loading}>
              上一页
            </button>
            <span className="equipment-range">
              槽位范围 {start}-{end}
            </span>
            <button
              onClick={nextPage}
              disabled={snapshot.next_start === null || loading}
            >
              下一页
            </button>
          </div>
        </>
      )}
      {snapshot && (
        <table className="equipment-table">
          <thead>
            <tr>
              <th scope="col">槽位</th>
              <th scope="col">物品 ID</th>
              <th scope="col">等级（原始）</th>
              <th scope="col">强化（原始）</th>
              <th scope="col">数量（原始）</th>
              <th scope="col">稀有度（原始）</th>
              <th scope="col">记录摘要</th>
            </tr>
          </thead>
          <tbody>
            {visible.map((row) => (
              <tr key={row.slot} className={row.slot === selected ? "selected" : ""}>
                <td>
                  <button
                    aria-pressed={row.slot === selected}
                    onClick={() => setSelected(row.slot)}
                  >
                    {row.slot}
                  </button>
                </td>
                <td>
                  <code>{hexId(row.item_id)}</code>
                  {itemName(row.item_id, catalog) !== null && (
                    React.createElement(
                      "span",
                      { className: "equipment-item-name" },
                      itemName(row.item_id, catalog),
                    )
                  )}
                  {itemName(row.item_id, catalog) === null && (
                    <span className="equipment-unknown">名称未收录</span>
                  )}
                </td>
                <td>{row.level_raw}</td>
                <td>{row.plus_raw}</td>
                <td>{row.quantity_raw}</td>
                <td>{row.rarity_raw}</td>
                <td className="equipment-digest">{row.record_sha256.slice(0, 8)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {snapshot && !visible.length && (
        <p className="equipment-status" role="status">
          {query.trim() ? "本页没有匹配的行。" : "本页没有可显示的槽位。"}
        </p>
      )}
      <section className="equipment-detail" aria-label="原始字段">
        <h2>原始字段</h2>
        {active ? (
          <>
            <dl className="equipment-raw">
              <div>
                <dt>槽位</dt>
                <dd>{active.slot}</dd>
              </div>
              <div>
                <dt>物品 ID</dt>
                <dd>
                  <code>{hexId(active.item_id)}</code>
                  {itemName(active.item_id, catalog) !== null && (
                    React.createElement(
                      "span",
                      { className: "equipment-item-name" },
                      itemName(active.item_id, catalog),
                    )
                  )}
                  {itemName(active.item_id, catalog) === null && (
                        <span className="equipment-unknown">名称未收录</span>
                  )}
                </dd>
              </div>
              <div>
                <dt>等级（原始）</dt>
                <dd>{active.level_raw}</dd>
              </div>
              <div>
                <dt>强化（原始）</dt>
                <dd>{active.plus_raw}</dd>
              </div>
              <div>
                <dt>数量（原始）</dt>
                <dd>{active.quantity_raw}</dd>
              </div>
              <div>
                <dt>稀有度（原始）</dt>
                <dd>{active.rarity_raw}</dd>
              </div>
              <div>
                <dt>记录摘要</dt>
                <dd className="equipment-digest">{active.record_sha256}</dd>
              </div>
            </dl>
            <h3>词条槽位</h3>
            <table className="equipment-effects">
              <thead>
                <tr>
                  <th scope="col">槽位</th>
                  <th scope="col">ID</th>
                  <th scope="col">原始值</th>
                </tr>
              </thead>
              <tbody>
                {active.effects.map((effect) => (
                  <tr key={effect.slot}>
                    <td>{effect.slot}</td>
                    <td>
                      <code>
                        {effect.id === SLOT_SENTINEL ? "—" : hexId(effect.id)}
                      </code>
                      {effect.id !== SLOT_SENTINEL &&
                        effectName(effect.id) === null && (
                          <span className="equipment-unknown">未知词条</span>
                        )}
                      {effectName(effect.id) !== null && (
                        <span className="equipment-effect-name">
                          {effectName(effect.id)}
                        </span>
                      )}
                    </td>
                    <td>
                      {effect.id === SLOT_SENTINEL ? "—" : effect.raw_value}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        ) : (
          <p className="equipment-status">选择一行查看原始字段。</p>
        )}
      </section>
      {snapshot && (
        <ul className="equipment-notes">
          <li>
            {catalog
              ? "本地目录只为精确匹配的物品 ID 提供名称；未匹配项仍显示十六进制 ID。"
              : "物品名称尚未收录，暂以十六进制 ID 显示。"}
          </li>
          <li>以下数值为内存原始值，未按游戏内显示换算。</li>
          <li>
            槽位序号为读取到的原始槽位，不代表玩家背包已占用数量或认证容量。
          </li>
        </ul>
      )}
    </section>
  );
}
