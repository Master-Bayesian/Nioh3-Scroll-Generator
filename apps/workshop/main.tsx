import { useUiLocale, setUiLocale, localize } from "./presentation";
import { ScrollCard } from "./ScrollCard";
import { collectionKey, useCollections } from "./collections";
import { Updates, UpdateNotice } from "./Updates";
import { StarIcon } from "./StarIcon";
import { appIcon } from "./app-icon";
import React, {
  createContext,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import { createRoot } from "react-dom/client";
import {
  data,
  conditionKey,
  toRecordTransferCount,
  initialQuery,
  initialSample,
  matches,
  queryProblem,
  ANY_RULE_VALUE,
  ruleFamilyKeys,
  ruleFamilyValues,
  type Query,
  type Sample,
  type SelectedEffect,
} from "./model";
import "./style.css";
import { BackupManager } from "./BackupManager";
import { Editor } from "./Editor";
import { DesktopCartActions, SavePicker } from "./CartActions";
import {
  desktop,
  searchController,
  workerQuery,
  candidateSample,
  loadDesktopCatalog,
  retainSample,
  previewSeed,
  copyText,
  formQuery,
} from "./desktop-bridge";
import { searchNativePage } from "./native-search";
import { runtimeObserver, saveSession } from "./save-workspace";
import { terminal } from "../desktop/src/search-controller";
import { useConditionDrag } from "./condition-drag";
const hex = (id: string) =>
  "0x" + Number(id).toString(16).toUpperCase().padStart(4, "0");
function score(sample: Sample, mode: string) {
  const effects = sample.effects.filter(
    (e) => e.role === "主词条" || e.role === "副词条",
  );
  const selected =
    mode === "primary"
      ? effects.slice(0, 1)
      : mode === "secondary"
        ? effects.slice(1)
        : effects;
  return selected.reduce((sum, e) => sum + e.roll, 0) / (selected.length || 1);
}
const colors: Record<number, string> = { 3: "紫色", 4: "绿色", 5: "橙色" };
const modeOptions = [
  ["0", "必含"],
  ...Array.from({ length: 24 }, (_, i) => [String(i + 1), "任选组 " + (i + 1)]),
];
function Select({
  label,
  value,
  onChange,
  options,
  disabled = false,
  className,
}: {
  label: string;
  value: string | number;
  onChange: (v: string) => void;
  options: (string | number | string[])[];
  disabled?: boolean;
  className?: string;
}) {
  return (
    <select
      className={className}
      aria-label={label}
      value={value}
      disabled={disabled}
      onChange={(e) => onChange(e.target.value)}
    >
      {options.map((o) => (
        <option
          key={Array.isArray(o) ? o[0] : o}
          value={Array.isArray(o) ? o[0] : o}
        >
          {Array.isArray(o) ? o[1] : o}
        </option>
      ))}
    </select>
  );
}
const PanelContext = createContext({
  opened: {} as Record<string, string>,
  toggle: (_group: string, _title: string) => {},
});
function Panel({
  title,
  tone,
  help,
  children,
  extra,
}: {
  title: string;
  tone: string;
  help: string;
  children: React.ReactNode;
  extra?: React.ReactNode;
}) {
  const { opened, toggle } = useContext(PanelContext);
  const group = ["主副词条", "恩宠"].includes(title) ? "equipment" : "dungeon";
  const expanded = opened[group] === title;
  const [scrolled, setScrolled] = useState(false);
  const body = useRef<HTMLDivElement>(null);
  return (
    <section className={"module " + tone + (expanded ? " is-open" : "")}>
      <header>
        <button
          className="panel-toggle"
          aria-label={title}
          aria-expanded={expanded}
          onClick={() => toggle(group, title)}
        >
          <span>{expanded ? "▾" : "▸"}</span>
          <h2>{title}</h2>
        </button>
        {extra}
        <details>
          <summary aria-label={title + "说明"}>?</summary>
          <p>{help}</p>
        </details>
      </header>
      <div
        className="panel-body"
        ref={body}
        hidden={!expanded}
        onScrollCapture={(e) =>
          setScrolled((e.target as HTMLElement).scrollTop > 40)
        }
      >
        {children}
      </div>
      {expanded && scrolled && (
        <button
          className="back-top"
          style={{
            top:
              (body.current?.querySelector(".finder")?.getBoundingClientRect()
                .bottom || 0) -
              (body.current?.parentElement?.getBoundingClientRect().top || 0) +
              3,
          }}
          aria-label={title + "返回顶部"}
          onClick={() => {
            body.current
              ?.querySelectorAll(".catalog-list,.rule-tree")
              .forEach((e) => e.scrollTo({ top: 0 }));
            body.current?.scrollTo({ top: 0 });
            setScrolled(false);
          }}
        >
          ↑
        </button>
      )}
    </section>
  );
}
function Finder({
  label,
  value,
  set,
}: {
  label: string;
  value: string;
  set: (v: string) => void;
}) {
  return (
    <div className="finder">
      <span>⌕</span>
      <input
        aria-label={label}
        placeholder={label}
        value={value}
        onChange={(e) => set(e.target.value)}
      />
      {value && (
        <button
          type="button"
          aria-label={"清空" + label}
          onClick={() => set("")}
        >
          ×
        </button>
      )}
    </div>
  );
}
function ConditionGroups<T extends { id: string; mode?: number }>({
  kind,
  items,
  children,
}: {
  kind: "effects" | "enemies" | "rules";
  items: T[];
  children: (item: T) => React.ReactNode;
}) {
  const seen = new Set<number>();
  return (
    <>
      {items.map((item) => {
        if (!item.mode)
          return (
            <React.Fragment key={conditionKey(item)}>
              {children(item)}
            </React.Fragment>
          );
        if (seen.has(item.mode)) return null;
        seen.add(item.mode);
        return (
          <div
            className={
              "condition-group " +
              (kind === "effects"
                ? "effect-group"
                : kind === "enemies"
                  ? "enemy-group"
                  : "rule-group")
            }
            data-condition-group={kind + ":" + item.mode}
            key={"group:" + item.mode}
          >
            <small>
              {kind === "effects"
                ? "词条"
                : kind === "enemies"
                  ? "敌人"
                  : "特殊规则"}{" "}
              · 任选一个{" "}
              <span>{items.filter((v) => v.mode === item.mode).length} 项</span>
            </small>
            <div className="group-items">
              {items.filter((v) => v.mode === item.mode).map(children)}
            </div>
          </div>
        );
      })}
    </>
  );
}

function App() {
  const locale = useUiLocale();
  useEffect(()=>{document.documentElement.lang=locale;document.title=localize('独脚踏鞴工作室');if(desktop)void window.preferences.setLocale(locale).catch(error=>window.review.log(String(error)))},[locale]);
  const [q, setQ] = useState<Query>(initialQuery),
    [submitted, setSubmitted] = useState<Query>(initialQuery),
    [results, setResults] = useState<Sample[]>(
      desktop ? [] : data.samples.filter((s) => s.rarity === 4).slice(0, 3),
    ),
    [index, setIndex] = useState(0);
  const [effectFind, setEffectFind] = useState(""),
    [graceFind, setGraceFind] = useState(""),
    [enemyFind, setEnemyFind] = useState(""),
    [tier, setTier] = useState("全部"),
    [ruleFind, setRuleFind] = useState("");
  const [showIds, setShowIds] = useState(false),
    [modal, setModal] = useState(""),
    [status, setStatus] = useState("请选择筛选条件。"),
    [busy, setBusy] = useState(false),
    [scanned, setScanned] = useState(0),
    [direct, setDirect] = useState(""),
    [comparison, setComparison] = useState<string[]>([]),
    [expandedRule, setExpandedRule] = useState("");
  const [offset, setOffset] = useState(0);
  const [previewPicked, setPreviewPicked] = useState<string[]>([]);
  const archiveBusy = useRef(false);
  const [history, setHistory] = useState<
    { id: string; time: string; query: Query; samples: Sample[] }[]
  >([]);
  const historyRef = useRef(history);
  historyRef.current = history;
  async function archiveResults() {
    if (!results.length) return;
    const id = results
      .map((s) => s.backend?.candidateId || s.seed + ":" + s.rarity)
      .join(",");
    if (historyRef.current.some((p) => p.id === id)) return;
    const samples: Sample[] = [];
    for (const sample of results)
      samples.push(desktop ? await retainSample(sample) : sample);
    const next = [
      {
        id,
        time: new Date().toLocaleTimeString(),
        query: structuredClone(submitted),
        samples,
      },
      ...historyRef.current,
    ].slice(0, 3);

    historyRef.current = next;
    setHistory(next);
  }

  const [connected, setConnected] = useState(!desktop),
    [allowCpu, setAllowCpu] = useState(false),
    [resumeAvailable, setResumeAvailable] = useState(false);
  const pendingQuery = useRef<Query | null>(null);
  const [cache, setCache] = useState<{ id: string; key: string } | null>(null);
  const cacheKey = () =>
    JSON.stringify([
      q.ng,
      q.rarity,
      saveSession?.getSnapshot().inventory?.snapshot_id,
    ]);
  async function useMeasuredCache(capture = false) {
    const inventory = saveSession?.getSnapshot().inventory;
    if (!inventory) {
      setStatus("请先选择并读取存档。");
      return;
    }
    if (capture && !nativeConfirmed) {
      setStatus("请先确认游戏已停在标题界面。");
      return;
    }
    setBusy(true);
    nativeBusy.current = true;
    try {
      const params = {
        save_id: inventory.save_id,
        snapshot_id: inventory.snapshot_id,
        playthrough: q.ng,
        rarity: q.rarity as 4 | 5,
      };
      if (capture) {
        const level = await window.nioh.resolveRecommendedLevel(q.recommended);
        await runtimeObserver!.run(() =>
          window.operations.captureGrace({
            ...params,
            level: q.level,
            recommended_level: level.selected_internal_level!,
            title_screen_confirmed: true,
          }),
        );
      }
      const bound = await window.operations.bindCachedSearch(params);
      setCache({ id: bound.cache_id, key: cacheKey() });
      setStatus("已连接当前存档的映射缓存，可离线搜索。");
    } catch (error) {
      setStatus(String(error));
    } finally {
      setBusy(false);
      nativeBusy.current = false;
    }
  }
  const nativeBusy = useRef(false),
    nativeCursor = useRef(0),
    nativeTrial = useRef(0);
  const [nativeConfirmed, setNativeConfirmed] = useState(false);
  useEffect(() => setNativeConfirmed(false), [q.ng]);
  const observedJob = useRef("");
  const sorting = useRef("primary");
  useEffect(() => {
    if (!searchController) return;
    const controller = searchController;
    let disposed = false;
    const update = () => {
      const state = controller.getSnapshot();
      if (disposed) return;
      if (nativeBusy.current) return;
      setConnected(!!state.handshake);
      setBusy(state.busy || !!(state.job && !terminal(state.job)));
      if (state.error) {
        setStatus(state.error);
        return;
      }
      if (!state.job) {
        setStatus(
          state.handshake ? "后端已连接，请选择筛选条件。" : "正在连接后端…",
        );
        return;
      }
      const job = state.job;
      const submittedForm =
        pendingQuery.current ||
        (state.submitted ? formQuery(state.submitted) : initialQuery());
      if (observedJob.current !== job.job_id) {
        observedJob.current = job.job_id;
        setSubmitted(submittedForm);
        setIndex(0);
        if (!pendingQuery.current) setQ(submittedForm);
      }
      setResults(
        job.candidates
          .map((c) =>
            candidateSample(c, state.submitted?.query.level || 180, job.job_id),
          )
          .sort(
            (a, b) => score(b, sorting.current) - score(a, sorting.current),
          ),
      );
      setResumeAvailable(!!job.resume_token);
      setResultSource("真实搜索结果");
      setStatus(
        job.error
          ? job.error.message
          : terminal(job)
            ? job.state === "cancelled"
              ? "已取消，保留已找到的绘卷。"
              : `本批找到 ${job.candidates.length} 张绘卷，耗时 ${(job.elapsed_ms / 1000).toFixed(1)} 秒。`
            : `正在搜索，已找到 ${job.candidates.length} 张绘卷…`,
      );
    };
    const unsubscribe = controller.subscribe(update);
    void Promise.all([3, 4, 5].map(loadDesktopCatalog))
      .then(() => controller.connect())
      .catch((e) => setStatus(String(e)));
    return () => {
      disposed = true;
      unsubscribe();
      controller.dispose();
    };
  }, []);
  const [opened, setOpened] = useState<Record<string, string>>({}),
    [expandedCart, setExpandedCart] = useState(false),
    [installMode, setInstallMode] = useState("live");
  const selectionLeave = useRef<ReturnType<typeof setTimeout> | null>(null);
  const { cart, favorites, cartPending, addCart, removeCart, toggleFavorite } =
    useCollections(setStatus);
  const ownedReferences = useRef(new Set<string>());
  useEffect(() => {
    const next = new Set(
      [
        ...cart,
        ...favorites,
        ...results,
        ...history.flatMap((p) => p.samples),
      ].flatMap((s) => (s.backend?.referenceId ? [s.backend.referenceId] : [])),
    );
    if (desktop)
      for (const ref of ownedReferences.current)
        if (!next.has(ref))
          void window.review.release(ref).catch((e) => setStatus(String(e)));
    ownedReferences.current = next;
  }, [cart, favorites, results, history]);
  const favoriteButton = (sample: Sample) => (
    <button
      className="favorite-button"
      aria-label={
        favorites.some((s) => collectionKey(s) === collectionKey(sample))
          ? "取消收藏"
          : "收藏绘卷"
      }
      aria-pressed={favorites.some(
        (s) => collectionKey(s) === collectionKey(sample),
      )}
      onClick={() => void toggleFavorite(sample)}
    >
      <StarIcon filled={favorites.some((s) => collectionKey(s) === collectionKey(sample))} />
    </button>
  );
  const conditionDrag = useConditionDrag(q, setQ);
  const [cartSelected, setCartSelected] = useState<string[]>([]);
  const [addedKeys, setAddedKeys] = useState<string[]>([]);
  function recordAdded(keys: string[]) {
    setAddedKeys((old) => [...new Set([...old, ...keys])]);
    setCartSelected((old) => old.filter((key) => !keys.includes(key)));
  }
  const cartKey = collectionKey;

  const [matchedCount, setMatchedCount] = useState(0);
  const [popupPosition, setPopupPosition] = useState({ left: 140, bottom: 20 });
  const wheelTime = useRef(0);
  useEffect(() => {
    const area = document.querySelector(".search-page .result-scroll");
    if (!area) return;
    const wheel = (e: Event) => {
      const event = e as WheelEvent;
      if ((event.target as HTMLElement).closest(".number-rail")) return;
      event.preventDefault();
      if (Math.abs(event.deltaY) > 4 && Date.now() - wheelTime.current > 120) {
        wheelTime.current = Date.now();
        setIndex((i) =>
          Math.max(
            0,
            Math.min(results.length - 1, i + (event.deltaY > 0 ? 1 : -1)),
          ),
        );
      }
    };
    area.addEventListener("wheel", wheel, { passive: false });
    return () => area.removeEventListener("wheel", wheel);
  }, [results.length]);

  function stepResult(delta: number) {
    setIndex((i) => Math.max(0, Math.min(results.length - 1, i + delta)));
  }
  const [page, setPage] = useState("search"),
    [sortMode, setSortMode] = useState("primary"),
    [fontSize, setFontSize] = useState(13),
    [popup, setPopup] = useState("");
  const [appVersion, setAppVersion] = useState("");
  useEffect(() => {
    if (!desktop) return;
    void window.support
      .diagnostics()
      .then((report) => setAppVersion(report.version))
      .catch(() => {});
  }, []);
  const logs = useRef<string[]>([]);
  useEffect(() => {
    logs.current = [
      ...logs.current,
      new Date().toISOString() + " " + status.slice(0, 2000),
    ].slice(-500);
  }, [status]);
  useEffect(() => {
    const close = (e: KeyboardEvent) => {
      if (e.key === "Escape") setPopup("");
    };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, []);
  function sortResults(mode: string) {
    sorting.current = mode;
    const seed = results[index]?.seed;
    const ordered = [...results].sort(
      (a, b) => score(b, mode) - score(a, mode),
    );
    setSortMode(mode);
    setResults(ordered);
    setIndex(
      Math.max(
        0,
        ordered.findIndex((s) => s.seed === seed),
      ),
    );
  }
  const [ruleDrafts, setRuleDrafts] = useState<Record<string, string>>({});
  const qqUrl =
    "https://qm.qq.com/cgi-bin/qm/qr?k=0qS7eJtELBBcN8_ne4B7qG-c63Ze6pIo&jump_from=webapi&authKey=OMNXRYe8Ns3exbv9xiDr6HOQca3C/F+f5dVguJS7d2NFCf5URf308buzPfPXaf2G";
  const [collapsed, setCollapsed] = useState(false),
    [toast, setToast] = useState("");
  async function joinGroup() {
    try {
      await copyText(data.qq);
      setToast("群号已复制，若未自动跳转，可在 QQ 中手动加群。");
    } catch {
      setToast("请在 QQ 中搜索群号 " + data.qq + " 加入。");
    }
  }
  const [resultSource, setResultSource] = useState(desktop ? "" : "未筛选示例");
  const dialog = useRef<HTMLDialogElement>(null),
    cancel = useRef(false),
    generation = useRef(0);
  const context =
    data.contexts[`${q.ng}-${q.rarity}` as keyof typeof data.contexts];
  const change = <K extends keyof Query>(key: K, value: Query[K]) =>
    setQ((prev) => ({ ...prev, [key]: value }));
  const dirty = JSON.stringify(q) !== JSON.stringify(submitted);
  const selected = results[index];
  const patchEffect = (i: number, p: Partial<SelectedEffect>) =>
    change(
      "effects",
      q.effects.map((e, j) => (i === j ? { ...e, ...p } : e)),
    );
  function open(name: string) {
    setModal(name);
    dialog.current?.showModal();
  }
  function swapContext(ng: number, rarity: number) {
    if (ng !== 3) setInstallMode("save");
    const c = data.contexts[`${ng}-${rarity}` as keyof typeof data.contexts];
    const kept = q.effects
      .filter((e) => c.effects.some((v) => v.id === e.id))
      .map((e) => (ng === 3 ? e : { ...e, roll: 0 }));
    setQ({
      ...q,
      ng,
      rarity,
      effects: kept,
      graces: q.graces.filter((id) => c.graces.some((e) => e.id === id)),
    });
    setStatus(
      ng === 3
        ? "周目／稀有度已切换，目录已更新。"
        : "当前周目使用原生生成，支持词条组合筛选；抽取评分暂不可用，数值已设为不限。",
    );
  }
  function move(i: number, delta: number) {
    const effects = [...q.effects];
    [effects[i], effects[i + delta]] = [effects[i + delta], effects[i]];
    change("effects", effects);
  }
  async function nativeSearch(next = false, knownSeed = false) {
    if (!nativeConfirmed) {
      setStatus("请先确认游戏已停在标题界面。");
      return;
    }
    nativeBusy.current = true;
    cancel.current = false;
    setBusy(true);
    setResumeAvailable(false);
    setResultSource(knownSeed ? "种子预览" : "原生搜索结果");
    setSubmitted(structuredClone(q));
    const found: Sample[] = [];
    setResults([]);
    setIndex(0);
    try {
      const result = await searchNativePage(
        q,
        knownSeed ? Number(direct) : next ? nativeCursor.current : 0,
        () => cancel.current,
        (sample) => {
          found.push(sample);
          setResults(
            [...found].sort(
              (a, b) => score(b, sorting.current) - score(a, sorting.current),
            ),
          );
        },
        setStatus,
        knownSeed,
        next ? nativeTrial.current : 0,
      );
      nativeCursor.current = result.cursor;
      nativeTrial.current = result.trial;
      setResumeAvailable(!knownSeed && !result.exhausted);
      setStatus(
        cancel.current
          ? `已取消，保留 ${found.length} 张绘卷。`
          : `本批找到 ${found.length} 张绘卷。`,
      );
    } catch (error) {
      setStatus(String(error));
    } finally {
      nativeBusy.current = false;
      setBusy(false);
    }
  }
  async function search(next = false) {
    const problem = queryProblem(q, desktop);
    if (problem) {
      setStatus(problem);
      return;
    }
    if (archiveBusy.current) return;
    archiveBusy.current = true;
    setBusy(true);
    setStatus("正在准备搜索…");
    try {
      await archiveResults();
    } catch (error) {
      setStatus(String(error));
      setBusy(false);
      return;
    } finally {
      archiveBusy.current = false;
    }
    if (
      desktop &&
      q.ng !== 3 &&
      !(q.ng >= 4 && q.rarity === 5 && cache?.key === cacheKey())
    ) {
      await nativeSearch(next);
      setBusy(false);
      return;
    }
    if (searchController) {
      try {
        if (next) {
          await searchController.resume();
          return;
        }
        const query = workerQuery(q);
        pendingQuery.current = structuredClone(q);
        await searchController.start({
          query,
          context_digest:
            searchController.getSnapshot().handshake!.context.context_digest,
          ...(q.ng >= 4 && cache ? { cache_id: cache.id } : {}),
          result_count: q.count,
          page_trials: 1000000,
          job_trials: 10000000,
          allow_cpu_fallback: allowCpu,
          resume_token: null,
        });
      } catch (error) {
        setStatus(String(error));
        setBusy(false);
      }
      return;
    }
    const id = ++generation.current;
    cancel.current = false;
    setBusy(true);
    setScanned(0);
    const snapshot = structuredClone(q);
    const pool = data.samples.filter((s) => s.rarity === q.rarity);
    const matched: Sample[] = [];
    setStatus("正在搜索…");
    for (let i = 0; i < pool.length; i++) {
      if (cancel.current || generation.current !== id) {
        setStatus("已取消，保留上一批结果。");
        setBusy(false);
        return;
      }
      if (matches(pool[i], snapshot)) matched.push(pool[i]);
      setScanned(i + 1);
      if (i % 8 === 0) await new Promise((r) => setTimeout(r, 35));
    }
    matched.sort((a, b) => score(b, sortMode) - score(a, sortMode));
    const start = next ? offset + q.count : 0;
    setResultSource("已按条件筛选");
    setMatchedCount(matched.length);
    setResults(matched.slice(start, start + q.count));
    setIndex(0);
    setOffset(start);
    setSubmitted(snapshot);
    setComparison([]);
    setBusy(false);
    setStatus(
      `搜索完成，找到 ${matched.length} 张绘卷。${start >= matched.length && start ? "没有下一批。" : ""}`,
    );
  }
  function reset() {
    setExpandedCart(false);
    cancel.current = true;
    setQ({ ...initialQuery(), effects: [] });
    setStatus("已清空全部条件；搜索设置恢复默认。");
  }
  async function copy(text: string) {
    try {
      await copyText(text);
      setStatus("已复制。");
    } catch {
      setStatus("剪贴板不可用，请手动复制：" + text);
    }
  }
  const renderEffect = (e: SelectedEffect) => {
    const i = q.effects.indexOf(e);
    const primary = !q.unrestricted && i < q.primaryCount;
    return (
      <div
        className={"selected-row " + (primary ? "is-primary" : "")}
        key={conditionKey(e)}
        data-effect-id={e.id}
        {...conditionDrag.item("effects", conditionKey(e))}
      >
        <span className="drag-grip" title="拖到前后换序，拖到中央分组">
          ⠿
        </span>
        <span className="role">
          {q.unrestricted ? "任意槽" : primary ? "主词条备选" : "副词条"}
        </span>
        <span className="selected-name" title={hex(e.id)}>
          {e.name}
        </span>
        {primary ? (
          q.primaryCount > 1 && (
            <label className="cross" title="未当选主词条时，副词条必须包含此项">
              <input
                type="checkbox"
                checked={e.cross}
                onChange={(ev) => patchEffect(i, { cross: ev.target.checked })}
              />
              未选为主时，副词条也要有
            </label>
          )
        ) : (
          <Select
            label={e.name + "组合方式"}
            value={e.mode}
            options={modeOptions}
            onChange={(v) => patchEffect(i, { mode: Number(v) })}
          />
        )}
        <Select
          label={e.name + "数值门槛"}
          disabled={q.ng !== 3}
          value={e.roll}
          options={[
            ["0", "数值不限"],
            ["80", "抽取 ≥80%"],
            ["90", "抽取 ≥90%"],
            ["100", "抽取 100%"],
          ]}
          onChange={(v) => patchEffect(i, { roll: Number(v) })}
        />
        <div className="row-actions">
          <button
            aria-label={"移除" + e.name}
            onClick={() =>
              change(
                "effects",
                q.effects.filter((x) => conditionKey(x) !== conditionKey(e)),
              )
            }
          >
            ×
          </button>
        </div>
      </div>
    );
  };
  const count =
    q.effects.length +
    q.enemies.length +
    q.rules.length +
    q.terrains.length +
    q.capacities.length +
    q.graces.length;
  const effectRows = context.effects.filter((e) =>
    `${localize(e.name)} ${hex(e.id)}`
      .toLowerCase()
      .includes(effectFind.toLowerCase()),
  );
  const enemyRows = data.enemies.filter(
    (e) =>
      (tier === "全部" || e.tier.includes(tier.replace("手", ""))) &&
      `${localize(e.name)} ${hex(e.id)}`
        .toLowerCase()
        .includes(enemyFind.toLowerCase()),
  );
  const ruleRows = data.rules.filter((r) =>
    localize(`${r.name} ${r.variants.map((v) => v.label).join(" ")}`)
      .toLowerCase()
      .includes(ruleFind.toLowerCase()),
  );
  const categories = [...new Set(ruleRows.map((r) => r.category))];
  return (
    <div
      style={{ "--ui-font-size": fontSize + "px" } as React.CSSProperties}
      data-fontsize={fontSize}
      className={"shell " + (collapsed ? "nav-collapsed" : "")}
    >
      <aside className="nav">
        <button
          className="nav-toggle"
          aria-label={collapsed ? "展开侧栏" : "收起侧栏"}
          aria-expanded={!collapsed}
          onClick={() => setCollapsed(!collapsed)}
        >
          {collapsed ? "☰" : "‹"}
        </button>
        <div className="brand">
          仁王<span>3</span>
          <small>独脚踏鞴工作室</small>
          {appVersion && <em className="app-version">v{appVersion}</em>}
        </div>
        <nav>
          <button
            className={page === "search" ? "active" : ""}
            onClick={() => setPage("search")}
            aria-label="绘卷搜索"
            title="绘卷搜索"
          >
            ▤<span>绘卷搜索</span>
          </button>
          <button
            className={page === "editor" ? "active" : ""}
            onClick={() => setPage("editor")}
            aria-label="绘卷编辑"
            title="绘卷编辑"
          >
            ✎<span>绘卷编辑</span>
          </button>
          <button
            className={page === "backups" ? "active" : ""}
            onClick={() => setPage("backups")}
            aria-label="备份与管理"
          >
            ▣<span>备份与管理</span>
          </button>
          <button onClick={() => open("收藏夹")} aria-label="收藏夹">
            <StarIcon /><span>收藏夹（{favorites.length}）</span>
          </button>
          <button className="coming-soon" disabled>
            <span aria-hidden="true">♜</span>
            <span>敬请期待</span>
          </button>
        </nav>
        <div className="nav-bottom">
          <button
            className="language-button"
            aria-label="切换语言"
            title="切换语言"
            onClick={(e) => {
              const r = e.currentTarget.getBoundingClientRect();
              setPopupPosition({
                left: r.right + 10,
                bottom: window.innerHeight - r.bottom,
              });
              setPopup(popup === "language" ? "" : "language");
            }}
          >
            <svg viewBox="0 0 32 28" width="25" height="25" aria-hidden="true">
              <circle
                cx="14"
                cy="14"
                r="11"
                fill="none"
                stroke="currentColor"
              />
              <ellipse
                cx="14"
                cy="14"
                rx="5"
                ry="11"
                fill="none"
                stroke="currentColor"
              />
              <path
                d="M3 14h22M5 8h18M5 20h18"
                fill="none"
                stroke="currentColor"
              />
              <rect
                x="18"
                y="1"
                width="13"
                height="12"
                rx="3"
                fill="var(--language-fill,#eef1f7)"
                stroke="currentColor"
              />
              <text x="21" y="10" fill="currentColor" fontSize="9">
                A
              </text>
            </svg>
            <span>语言</span>
          </button>
          <button
            className="settings"
            aria-label="设置"
            title="设置"
            onClick={(e) => {
              const r = e.currentTarget.getBoundingClientRect();
              setPopupPosition({
                left: r.right + 10,
                bottom: window.innerHeight - r.bottom,
              });
              setPopup(popup === "settings" ? "" : "settings");
            }}
          >
            ⚙ <span>设置</span>
          </button>
          {desktop && <UpdateNotice onOpen={() => open("检查更新")} />}
        </div>
      </aside>
      <header className="topbar">
        <img className="app-icon" src={appIcon} alt="" />
        <h1>
          {page === "search"
            ? "绘卷搜索"
            : page === "backups"
              ? "备份与管理"
              : "绘卷编辑"}
        </h1>
        <div className="toplinks">
          <span className="credits">作者：MasterBayesian · Saber_Li</span>
          <button
            onClick={() => {
              void joinGroup();
              if (desktop) void window.review.openLink("qq");
              else window.open(qqUrl, "_blank", "noopener");
            }}
          >
            加入QQ群
          </button>
          <button
            onClick={() => {
              if (desktop) void window.review.openLink("github");
              else window.open(data.github, "_blank", "noopener");
            }}
          >
            GitHub
          </button>
        </div>
        {desktop && (
          <div className="title-controls">
            <button
              aria-label="最小化"
              onClick={() => void window.review.windowAction("minimize")}
            >
              −
            </button>
            <button
              aria-label="最大化或还原"
              onClick={() => void window.review.windowAction("maximize")}
            >
              □
            </button>
            <button
              className="window-close"
              aria-label="关闭应用"
              onClick={() => void window.review.windowAction("close")}
            >
              ×
            </button>
          </div>
        )}
      </header>
      <div className="search-page" hidden={page !== "search"}>
        <main>
          <div className="intro">
            装备在身上：筛选左侧词条与恩宠。刷副本：筛选右侧敌人、规则、地形与挑战次数。
            <button onClick={() => open("使用说明")}>使用说明</button>
          </div>
          <section
            onMouseEnter={() => {
              if (selectionLeave.current) clearTimeout(selectionLeave.current);
              if (count) setExpandedCart(true);
            }}
            onMouseLeave={() => {
              selectionLeave.current = setTimeout(
                () => setExpandedCart(false),
                220,
              );
            }}
            className={
              "selection " + (expandedCart ? "selection-expanded" : "")
            }
          >
            <header>
              <h2>
                已选条件 <span>{count}</span>
              </h2>
              <button onClick={reset} disabled={busy}>
                清空全部
              </button>
              <button
                aria-label={expandedCart ? "收起已选条件" : "展开已选条件"}
                aria-expanded={expandedCart}
                onClick={() => setExpandedCart(!expandedCart)}
              >
                {expandedCart ? "⌃" : "⌄"}
              </button>
            </header>
            <div className="primary-controls">
              <label>
                <input
                  type="checkbox"
                  checked={q.unrestricted}
                  onChange={(e) => change("unrestricted", e.target.checked)}
                />
                主词条不限
              </label>
              <span>主词条可选数量</span>
              <Select
                label="主词条可选数量"
                value={q.primaryCount}
                disabled={q.unrestricted}
                options={[1, 2, 3]}
                onChange={(v) => change("primaryCount", Number(v))}
              />
              <span className="primary-hint">
                前 <b>{q.primaryCount}</b> 项均可为主词条
              </span>
              <span className="drag-hint">
                拖动词条到一起分组，拖出取消分组
              </span>
            </div>
            <div className="selected-body">
              <div className="selected-effects">
                {q.effects.length ? (
                  <>
                    {(q.unrestricted
                      ? []
                      : q.effects.slice(0, q.primaryCount)
                    ).map(renderEffect)}
                    <ConditionGroups
                      kind="effects"
                      items={
                        q.unrestricted
                          ? q.effects
                          : q.effects.slice(q.primaryCount)
                      }
                    >
                      {renderEffect}
                    </ConditionGroups>
                  </>
                ) : count === 0 ? (
                  <p className="empty-selection">
                    从下方目录添加词条，也可以只选择恩宠或辅助条件。
                  </p>
                ) : null}
              </div>
              <div className="other-conditions">
                {q.graces.length > 0 && (
                  <div className="grace-group">
                    <small>恩宠 · 任选一个</small>
                    {q.graces.map((id) => (
                      <button
                        className="chip grace-chip"
                        key={id}
                        onClick={() =>
                          change(
                            "graces",
                            q.graces.filter((v) => v !== id),
                          )
                        }
                      >
                        {context.graces.find((e) => e.id === id)?.name} ×
                      </button>
                    ))}
                  </div>
                )}
                <ConditionGroups kind="enemies" items={q.enemies}>
                  {(e) => (
                    <div
                      {...conditionDrag.item("enemies", e.id)}
                      className="enemy-chip"
                      key={e.id}
                    >
                      <span className="drag-grip">⠿</span>
                      <span>敌人 · {e.name}</span>
                      <Select
                        label={e.name + "敌人组合"}
                        value={e.mode}
                        options={modeOptions}
                        onChange={(v) =>
                          change(
                            "enemies",
                            q.enemies.map((x) =>
                              x.id === e.id ? { ...x, mode: Number(v) } : x,
                            ),
                          )
                        }
                      />
                      <button
                        aria-label={"移除敌人" + e.name}
                        onClick={() =>
                          change(
                            "enemies",
                            q.enemies.filter((x) => x.id !== e.id),
                          )
                        }
                      >
                        ×
                      </button>
                    </div>
                  )}
                </ConditionGroups>
                <ConditionGroups kind="rules" items={q.rules}>
                  {(r) => (
                    <div
                      {...conditionDrag.item("rules", r.id)}
                      className="chip rule-chip"
                      key={r.id}
                    >
                      <span className="drag-grip">⠿</span>
                      <span>
                        规则 · {r.name}
                        {!r.id.startsWith("category:") && (
                          <>
                            {" · "}
                            {data.rules
                              .find((f) => f.id === r.id)
                              ?.variants.find(
                                (v) => String(v.key) === r.variant,
                              )?.label ||
                              (r.variant === ANY_RULE_VALUE ||
                              r.variant === "任意变体" ||
                              r.variant === "任意对象／数值"
                                ? "全部接受"
                                : r.variant)}
                          </>
                        )}
                      </span>
                      {r.id.startsWith("category:") && (
                        <Select
                          className="rule-value"
                          label={r.name + "统一数值"}
                          value={r.variant}
                          options={[
                            [ANY_RULE_VALUE, "任意数值"],
                            ...ruleFamilyValues(r.name).map((value) => [
                              value,
                              value,
                            ]),
                          ]}
                          onChange={(variant) =>
                            change(
                              "rules",
                              q.rules.map((item) =>
                                item.id === r.id
                                  ? {
                                      ...item,
                                      keys: ruleFamilyKeys(r.name, variant),
                                      variant,
                                    }
                                  : item,
                              ),
                            )
                          }
                        />
                      )}
                      <Select
                        className="rule-match"
                        label={r.name + "规则组合"}
                        value={r.mode || 0}
                        options={modeOptions}
                        onChange={(v) =>
                          change(
                            "rules",
                            q.rules.map((x) =>
                              x.id === r.id ? { ...x, mode: Number(v) } : x,
                            ),
                          )
                        }
                      />
                      <button
                        aria-label={"移除规则" + r.name}
                        onClick={() =>
                          change(
                            "rules",
                            q.rules.filter((x) => x.id !== r.id),
                          )
                        }
                      >
                        ×
                      </button>
                    </div>
                  )}
                </ConditionGroups>
                {q.terrains.map((id) => (
                  <button
                    className="chip"
                    key={id}
                    onClick={() =>
                      change(
                        "terrains",
                        q.terrains.filter((x) => x !== id),
                      )
                    }
                  >
                    地形任一 ·{" "}
                    {data.terrains.find((t) => t.option_id === id)?.name} ×
                  </button>
                ))}
                {q.capacities.map((n) => (
                  <button
                    className="chip"
                    key={n}
                    onClick={() =>
                      change(
                        "capacities",
                        q.capacities.filter((x) => x !== n),
                      )
                    }
                  >
                    挑战 {n} 次 ×
                  </button>
                ))}
              </div>
            </div>
          </section>
          <PanelContext.Provider
            value={{
              opened,
              toggle: (group, title) =>
                setOpened({
                  ...opened,
                  [group]: opened[group] === title ? "" : title,
                }),
            }}
          >
            <div className="catalog-columns">
              <div
                className="catalog-column"
                aria-label="装备加成筛选"
                tabIndex={0}
              >
                <header className="purpose">
                  <h2>装备加成</h2>
                  <p>选择主副词条与恩宠</p>
                </header>
                <Panel
                  title="主副词条"
                  tone="sand"
                  help="支持主词条不限或前 1–3 项主词条备选择一。上下移动已选项可改变主副角色。未当选时副词条必含可表达 A主+B副 或 B主+A副。"
                  extra={
                    <span className="count">{context.effects.length} 项</span>
                  }
                >
                  <Finder
                    label="搜索词条名称或 ID"
                    value={effectFind}
                    set={setEffectFind}
                  />
                  <div className="list-label">
                    <span>词条名称</span>
                    <span>添加到条件</span>
                  </div>
                  <div className="catalog-list effects-list">
                    {effectRows.map((e) => (
                      <div className="catalog-row" key={e.id}>
                        <span>
                          {e.name}
                          {showIds && <small>{hex(e.id)}</small>}
                        </span>
                        <button
                          aria-label={"添加词条" + e.name + " " + e.id}
                          disabled={q.effects.length >= 24}
                          onClick={() =>
                            change("effects", [
                              ...q.effects,
                              {
                                ...e,
                                choiceId: crypto.randomUUID(),
                                mode: 0,
                                roll: 0,
                                cross: false,
                              },
                            ])
                          }
                        >
                          ＋
                        </button>
                      </div>
                    ))}
                    {!effectRows.length && (
                      <p className="empty-list">没有匹配词条</p>
                    )}
                  </div>
                  <footer className="catalog-footer">
                    显示 {effectRows.length} / {context.effects.length} 项 ·
                    目录随周目与稀有度更新
                  </footer>
                </Panel>
                <Panel
                  title="恩宠"
                  tone="purple"
                  help="可以添加多个恩宠，结果有其中一个即可。不添加时不限制恩宠。"
                >
                  <Finder
                    label="搜索恩宠"
                    value={graceFind}
                    set={setGraceFind}
                  />
                  <div className="catalog-list grace-list">
                    {context.graces
                      .filter((e) =>
                        localize(e.name)
                          .toLowerCase()
                          .includes(graceFind.toLowerCase()),
                      )
                      .map((e) => (
                        <div
                          className={
                            "catalog-row " +
                            (q.graces.includes(e.id) ? "chosen" : "")
                          }
                          key={e.id}
                        >
                          <span>
                            {e.name}
                            {showIds && <small>{hex(e.id)}</small>}
                          </span>
                          <button
                            aria-label={"选择恩宠" + e.name}
                            onClick={() =>
                              change(
                                "graces",
                                q.graces.includes(e.id)
                                  ? q.graces.filter((id) => id !== e.id)
                                  : [...q.graces, e.id],
                              )
                            }
                          >
                            {q.graces.includes(e.id) ? "已选" : "＋"}
                          </button>
                        </div>
                      ))}
                    {!context.graces.length && (
                      <p className="empty-list">
                        当前周目／稀有度不开放恩宠筛选
                      </p>
                    )}
                  </div>
                </Panel>
              </div>
              <div
                className="catalog-column"
                aria-label="副本刷取筛选"
                tabIndex={0}
              >
                <header className="purpose">
                  <h2>副本刷取</h2>
                  <p>选择敌人、特殊规则、地形与挑战次数</p>
                </header>
                <Panel
                  title="敌人"
                  tone="blue"
                  help="低／中／高指生成池档位，不代表难度。选择表示至少包含，允许其他敌人出现。已选敌人可设必含或任选组。"
                  extra={
                    <button className="subtle" onClick={() => open("敌人组合")}>
                      组合说明
                    </button>
                  }
                >
                  <div className="tier-tabs">
                    {["全部", "低手", "中手", "高手"].map((t) => (
                      <button
                        className={tier === t ? "selected" : ""}
                        onClick={() => setTier(t)}
                        key={t}
                      >
                        {t}
                      </button>
                    ))}
                  </div>
                  <Finder
                    label="搜索全部敌人名称或 ID"
                    value={enemyFind}
                    set={setEnemyFind}
                  />
                  <div className="catalog-list enemy-list">
                    {enemyRows.map((e) => (
                      <div className="catalog-row" key={e.id}>
                        <span>
                          {e.name}
                          {showIds && (
                            <small>
                              {hex(e.id)} · {e.keys.length} 个变体
                            </small>
                          )}
                        </span>
                        <em>{e.tier}</em>
                        <button
                          aria-label={"添加敌人" + e.name}
                          disabled={q.enemies.some((x) => x.id === e.id)}
                          onClick={() =>
                            change("enemies", [...q.enemies, { ...e, mode: 0 }])
                          }
                        >
                          {q.enemies.some((x) => x.id === e.id) ? "已选" : "＋"}
                        </button>
                      </div>
                    ))}
                  </div>
                  <footer className="catalog-footer">
                    {enemyRows.length} / {data.enemies.length} 个敌人
                  </footer>
                </Panel>
                <Panel
                  title="特殊规则"
                  tone="rose"
                  help="按类型查找，再选择具体对象与数值。任意变体覆盖该项全部数值；选择精确变体会替换该项旧选择。最多组合三项特殊规则。"
                  extra={<span className="count">{data.rules.length} 组</span>}
                >
                  <Finder
                    label="搜索规则、部位、符咒或恩宠"
                    value={ruleFind}
                    set={setRuleFind}
                  />
                  <div className="rule-tree">
                    {categories.map((category) => (
                      <div key={category}>
                        <button
                          className="rule-category"
                          aria-expanded={
                            expandedRule === category || !!ruleFind
                          }
                          onClick={() =>
                            setExpandedRule(
                              expandedRule === category ? "" : category,
                            )
                          }
                        >
                          <span>
                            {expandedRule === category || ruleFind ? "▾" : "▸"}{" "}
                            {category}
                          </span>
                          <small>
                            {
                              ruleRows.filter((r) => r.category === category)
                                .length
                            }
                          </small>
                        </button>
                        {(expandedRule === category || !!ruleFind) && (
                          <div className="rule-picker">
                            <span>任意对象</span>
                            <Select
                              label={category + "统一数值"}
                              value={
                                ruleDrafts["category:" + category] ||
                                q.rules.find(
                                  (item) => item.id === "category:" + category,
                                )?.variant ||
                                ANY_RULE_VALUE
                              }
                              options={[
                                [ANY_RULE_VALUE, "任意数值"],
                                ...ruleFamilyValues(category).map((value) => [
                                  value,
                                  value,
                                ]),
                              ]}
                              onChange={(value) =>
                                setRuleDrafts((previous) => ({
                                  ...previous,
                                  ["category:" + category]: value,
                                }))
                              }
                            />
                            <button
                              onClick={() => {
                                const families = data.rules.filter(
                                  (r) => r.category === category,
                                );
                                const variant =
                                  ruleDrafts["category:" + category] ||
                                  q.rules.find(
                                    (item) => item.id === "category:" + category,
                                  )?.variant ||
                                  ANY_RULE_VALUE;
                                const rest = q.rules.filter(
                                  (r) =>
                                    !families.some((f) => f.id === r.id) &&
                                    r.id !== "category:" + category,
                                );
                                change("rules", [
                                  ...rest,
                                  {
                                    id: "category:" + category,
                                    name: category,
                                    keys: ruleFamilyKeys(category, variant),
                                    variant,
                                  },
                                ]);
                              }}
                            >
                              {q.rules.some(
                                (item) => item.id === "category:" + category,
                              )
                                ? "更新"
                                : "添加整类"}
                            </button>
                          </div>
                        )}
                        {(expandedRule === category || !!ruleFind) &&
                          ruleRows
                            .filter((r) => r.category === category)
                            .map((r) => (
                              <div className="rule-picker" key={r.id}>
                                <span title={r.name}>
                                  {r.name === category
                                    ? "全部对象"
                                    : r.name.replace(category, "")}
                                </span>
                                <Select
                                  label={"规则变体" + r.name}
                                  value={
                                    ruleDrafts[r.id] ||
                                    q.rules.find((x) => x.id === r.id)
                                      ?.variant ||
                                    "任意变体"
                                  }
                                  options={[
                                    ["任意变体", "全部接受"],
                                    ...[...r.variants]
                                      .sort(
                                        (a, b) =>
                                          (parseFloat(a.label) || 0) -
                                          (parseFloat(b.label) || 0),
                                      )
                                      .map((v) => [String(v.key), v.label]),
                                  ]}
                                  onChange={(v) =>
                                    setRuleDrafts((prev) => ({
                                      ...prev,
                                      [r.id]: v,
                                    }))
                                  }
                                />
                                <button
                                  aria-label={"添加规则" + r.name}
                                  onClick={() => {
                                    const v =
                                      ruleDrafts[r.id] ||
                                      q.rules.find((x) => x.id === r.id)
                                        ?.variant ||
                                      "任意变体";
                                    const rest = q.rules.filter(
                                      (x) =>
                                        x.id !== r.id &&
                                        x.id !== "category:" + category,
                                    );
                                    change("rules", [
                                      ...rest,
                                      {
                                        id: r.id,
                                        name: r.name,
                                        keys:
                                          v === "任意变体"
                                            ? r.keys
                                            : [Number(v)],
                                        variant: v,
                                      },
                                    ]);
                                  }}
                                >
                                  {q.rules.some((x) => x.id === r.id)
                                    ? "更新"
                                    : "＋"}
                                </button>
                              </div>
                            ))}
                      </div>
                    ))}
                  </div>
                </Panel>
                <Panel
                  title="地形影响与挑战次数"
                  tone="blue"
                  help="地形条件之间取任一；精确项代表完整结果，不是地图场景。挑战次数筛选种子决定的初始上限，不是剩余次数。"
                >
                  <div className="terrain-options">
                    {data.terrains
                      .filter((t) => t.aggregate)
                      .map((t) => (
                        <button
                          className={
                            "terrain-all " +
                            (data.terrains
                              .filter(
                                (v) =>
                                  !v.aggregate &&
                                  t.effect_keys.every((k) =>
                                    v.effect_keys.includes(k),
                                  ),
                              )
                              .every((v) => q.terrains.includes(v.option_id))
                              ? "selected"
                              : "")
                          }
                          key={t.option_id}
                          onClick={() => {
                            const ids = data.terrains
                              .filter(
                                (v) =>
                                  !v.aggregate &&
                                  t.effect_keys.every((k) =>
                                    v.effect_keys.includes(k),
                                  ),
                              )
                              .map((v) => v.option_id);
                            change(
                              "terrains",
                              ids.every((id) => q.terrains.includes(id))
                                ? q.terrains.filter((id) => !ids.includes(id))
                                : [...new Set([...q.terrains, ...ids])],
                            );
                          }}
                        >
                          {t.name} · 全选
                        </button>
                      ))}
                    {data.terrains
                      .filter((t) => !t.aggregate)
                      .map((t) => (
                        <button
                          className={
                            q.terrains.includes(t.option_id) ? "selected" : ""
                          }
                          onClick={() =>
                            change(
                              "terrains",
                              q.terrains.includes(t.option_id)
                                ? q.terrains.filter((x) => x !== t.option_id)
                                : [...q.terrains, t.option_id],
                            )
                          }
                          key={t.option_id}
                        >
                          {t.name}
                        </button>
                      ))}
                  </div>
                  <div className="capacity">
                    <span>挑战次数</span>
                    {[0, 4, 5, 6, 7].map((n) => (
                      <button
                        aria-pressed={
                          n ? q.capacities.includes(n) : !q.capacities.length
                        }
                        key={n}
                        onClick={() =>
                          change(
                            "capacities",
                            n
                              ? q.capacities.includes(n)
                                ? q.capacities.filter((x) => x !== n)
                                : [...q.capacities, n]
                              : [],
                          )
                        }
                      >
                        {n || "不限"}
                      </button>
                    ))}
                  </div>
                </Panel>
              </div>
            </div>
          </PanelContext.Provider>
          {desktop && q.ng !== 3 && (
            <div className="native-search-source">
              <SavePicker />
              {q.ng >= 4 && q.rarity === 5 && (
                <div>
                  <button
                    disabled={busy}
                    onClick={() => void useMeasuredCache(false)}
                  >
                    使用已有映射缓存
                  </button>
                  <button
                    disabled={busy || !nativeConfirmed}
                    onClick={() => void useMeasuredCache(true)}
                  >
                    采集并使用映射缓存
                  </button>
                </div>
              )}
              <label>
                <input
                  type="checkbox"
                  checked={nativeConfirmed}
                  onChange={(e) => setNativeConfirmed(e.target.checked)}
                />
                游戏已停在标题界面（当前周目使用游戏原生生成）
              </label>
            </div>
          )}
          <section className="search-dock">
            <div className="search-fields">
              <label>
                周目
                <Select
                  label="周目"
                  value={q.ng}
                  options={[
                    ["1", "一周目"],
                    ["2", "二周目"],
                    ["3", "三周目"],
                    ["4", "四周目（研究）"],
                    ["5", "五周目（研究）"],
                  ]}
                  onChange={(v) => swapContext(Number(v), q.rarity)}
                />
              </label>
              <label>
                稀有度
                <Select
                  label="稀有度"
                  value={q.rarity}
                  options={[
                    ["3", "R3 · 紫色"],
                    ["4", "R4 · 绿色"],
                    ["5", "R5 · 橙色"],
                  ]}
                  onChange={(v) => swapContext(q.ng, Number(v))}
                />
              </label>
              <label>
                绘卷等级
                <input
                  aria-label="绘卷等级"
                  type="number"
                  min="0"
                  max="180"
                  value={q.level}
                  onChange={(e) => change("level", e.target.valueAsNumber)}
                />
              </label>
              <label>
                推荐等级（敌人等级）
                <input
                  aria-label="推荐等级"
                  type="number"
                  min="142"
                  max="700"
                  value={q.recommended}
                  onChange={(e) =>
                    change("recommended", e.target.valueAsNumber)
                  }
                />
              </label>
              <label title="设置绘卷的转手次数">
                转手次数
                <input
                  aria-label="转手次数"
                  type="number"
                  min="-1"
                  max="4294967295"
                  value={q.transfers}
                  onChange={(e) => change("transfers", e.target.valueAsNumber)}
                />
              </label>
              <label>
                候选数量
                <input
                  aria-label="候选数量"
                  type="number"
                  min="1"
                  max="25"
                  value={q.count}
                  onChange={(e) => change("count", e.target.valueAsNumber)}
                />
              </label>
            </div>
            <div className="search-actions">
              <button
                className="primary-button"
                disabled={busy || !connected}
                onClick={() => void search()}
              >
                ⌕ 开始搜索
              </button>
              <button
                className={
                  !busy &&
                  !dirty &&
                  results.length === q.count &&
                  (desktop
                    ? resumeAvailable
                    : offset + results.length < matchedCount)
                    ? "next-batch ready"
                    : "next-batch"
                }
                disabled={
                  busy ||
                  dirty ||
                  (desktop
                    ? !resumeAvailable
                    : offset + results.length >= matchedCount ||
                      resultSource !== "已按条件筛选")
                }
                onClick={() => void search(true)}
              >
                下一批 →
              </button>
              <button
                disabled={!busy}
                onClick={() => {
                  cancel.current = true;
                  if (nativeBusy.current) void runtimeObserver!.cancel();
                  else if (searchController) void searchController.cancel();
                }}
              >
                取消
              </button>
            </div>
            <div className="direct-seed">
              <label>
                已知绘卷 ID
                <input
                  aria-label="已知绘卷ID"
                  value={direct}
                  onChange={(e) => setDirect(e.target.value)}
                  placeholder="输入种子，单点查看"
                />
              </label>
              <button
                disabled={busy}
                onClick={() => {
                  if (desktop) {
                    if (q.ng !== 3) {
                      void nativeSearch(false, true);
                      return;
                    }
                    setBusy(true);
                    void previewSeed(Number(direct), q.rarity, q.level)
                      .then((sample) => {
                        setResults([sample]);
                        setSubmitted(structuredClone(q));
                        setResultSource("种子预览");
                        setIndex(0);
                        setResumeAvailable(false);
                      })
                      .catch((e) => setStatus(String(e)))
                      .finally(() => setBusy(false));
                    return;
                  }
                  const sample = data.samples.find(
                    (s) => s.seed === direct && s.rarity === q.rarity,
                  );
                  if (sample) {
                    setResultSource("已知种子预览");
                    setResults([sample]);
                    setIndex(0);
                    setSubmitted(structuredClone(q));
                    setStatus("已找到该绘卷。");
                  } else setStatus("此预览暂不支持该绘卷 ID。");
                }}
              >
                查看
              </button>
            </div>
          </section>
          <div className="status" role="status">
            {busy && <progress aria-label="搜索进度" />}
            {status}
          </div>
        </main>
        <aside
          className="result-pane"
          tabIndex={0}
          aria-label="绘卷结果浏览"
          onKeyDown={(e) => {
            if ((e.target as HTMLElement).matches("input,select,textarea"))
              return;
            if (e.key === "ArrowDown" || e.key === "ArrowUp") {
              e.preventDefault();
              stepResult(e.key === "ArrowDown" ? 1 : -1);
            }
          }}
        >
          <header>
            <h2>
              搜索结果 <small>{results.length} 张</small>
            </h2>
            <button
              disabled={!history.length || busy}
              onClick={() => open("最近三批")}
            >
              历史
            </button>
            <Select
              label="结果排序"
              disabled={submitted.ng !== 3}
              value={sortMode}
              options={[
                ["primary", "主词条评分 ↓"],
                ["secondary", "副词条平均分 ↓"],
                ["all", "全部词条平均分 ↓"],
              ]}
              onChange={sortResults}
            />
          </header>
          <div className="result-context">
            {!resultSource
              ? ""
              : dirty
                ? "条件已修改，请重新搜索"
                : `${resultSource} · ${submitted.ng} 周目 · R${submitted.rarity} ${colors[submitted.rarity]}`}
          </div>
          {results.length > 0 && (
            <div className="result-paging">
              <button
                aria-label="上一张绘卷"
                disabled={!index}
                onClick={() => setIndex(index - 1)}
              >
                ‹
              </button>
              <label>
                第{" "}
                <input
                  aria-label="跳转到第几张"
                  type="number"
                  min="1"
                  max={results.length || 1}
                  value={index + 1}
                  onChange={(e) => {
                    const n = e.target.valueAsNumber;
                    if (Number.isInteger(n) && n >= 1 && n <= results.length)
                      setIndex(n - 1);
                  }}
                />{" "}
                / {results.length} 张
              </label>
              <button
                aria-label="下一张绘卷"
                disabled={index >= results.length - 1}
                onClick={() => setIndex(index + 1)}
              >
                ›
              </button>
            </div>
          )}
          <div className="result-scroll">
            {results.length > 0 && (
              <div
                className="result-navigation"
                style={
                  {
                    height: `calc(var(--result-row-height, 23px) * ${Math.max(1, results.length)})`,
                    "--result-count": Math.max(1, results.length),
                  } as React.CSSProperties
                }
              >
                <div className="number-rail">
                  {results.map((s, i) => (
                    <button
                      className={i === index ? "active" : ""}
                      key={s.seed}
                      aria-label={"选择绘卷" + (i + 1)}
                      onClick={() => setIndex(i)}
                    >
                      {String(i + 1).padStart(2, "0")}
                    </button>
                  ))}
                </div>
                <input
                  className="result-slider"
                  aria-label="滑动切换绘卷"
                  type="range"
                  min="1"
                  max={results.length || 1}
                  value={index + 1}
                  onChange={(e) => setIndex(Number(e.target.value) - 1)}
                />
              </div>
            )}
            {selected ? (
              <div className="result-detail">
                <ScrollCard
                  sample={selected}
                  level={submitted.recommended}
                  showIds={showIds}
                />
                <div className="result-tools">
                  <button
                    onClick={() => {
                      setResults(results.filter((_, i) => i !== index));
                      setIndex(0);
                    }}
                  >
                    移除预览
                  </button>
                  <button
                    onClick={() => {
                      setResults([]);
                      setIndex(0);
                      setResultSource("");
                    }}
                  >
                    清空预览
                  </button>
                  <button
                    onClick={() => {
                      setPreviewPicked([]);
                      open("管理预览");
                    }}
                  >
                    多选移除
                  </button>
                  {favoriteButton(selected)}
                  <button
                    className="cart-toggle"
                    aria-label="加入购物车"
                    aria-pressed={cart.some(
                      (sample) => cartKey(sample) === cartKey(selected),
                    )}
                    disabled={cartPending.includes(cartKey(selected))}
                    onClick={() => {
                      if (
                        cart.some(
                          (sample) => cartKey(sample) === cartKey(selected),
                        )
                      )
                        removeCart(selected);
                      else void addCart(selected);
                    }}
                  >
                    加入购物车
                  </button>
                </div>
                <button
                  className="compare-button"
                  disabled={!cart.length}
                  onClick={() => {
                    setCartSelected(cart.filter(s=>!addedKeys.includes(cartKey(s))).map(cartKey));
                    open("购物车");
                  }}
                >
                  查看购物车（{cart.length}）
                </button>
              </div>
            ) : resultSource && !busy ? (
              <div className="no-results">
                <h3>没有匹配结果</h3>
                <p>试试减少筛选条件。</p>
                <button onClick={reset}>清空条件</button>
              </div>
            ) : null}
          </div>
          <section className="install-mode">
            <h3>添加方式</h3>
            <label>
              <input
                type="radio"
                name="install-mode"
                checked={installMode === "live"}
                onChange={() => setInstallMode("live")}
              />
              游戏内实时添加
            </label>
            <label>
              <input
                type="radio"
                name="install-mode"
                checked={installMode === "save"}
                onChange={() => setInstallMode("save")}
              />
              回标题界面后添加到存档
            </label>
            {desktop && installMode === "save" && <SavePicker />}
            <button
              disabled={!cart.length}
              onClick={() => {
                setCartSelected(cart.filter(s=>!addedKeys.includes(cartKey(s))).map(cartKey));
                open("购物车");
              }}
            >
              选择购物车中的绘卷添加
            </button>
          </section>
        </aside>
      </div>
      <div className="editor-host" hidden={page !== "editor"}>
        <Editor cart={cart} />
      </div>
      {popup && (
        <>
          <button
            className="popup-dismiss"
            aria-label="关闭侧边菜单"
            onClick={() => setPopup("")}
          />
          <section
            className="side-popup"
            style={popupPosition}
            aria-label={popup === "settings" ? "设置菜单" : "语言菜单"}
          >
            {popup === "settings" ? (
              <>
                <h2>设置</h2>
                {desktop && (
                  <label>
                    <input
                      type="checkbox"
                      checked={allowCpu}
                      onChange={(e) => setAllowCpu(e.target.checked)}
                    />
                    允许使用 CPU 搜索
                  </label>
                )}
                <label>
                  界面字号
                  <select
                    aria-label="界面字号"
                    value={fontSize}
                    onChange={(e) => setFontSize(Number(e.target.value))}
                  >
                    {[12, 13, 14, 15, 16, 18].map((n) => (
                      <option key={n} value={n}>
                        {n} px
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  <input
                    type="checkbox"
                    checked={showIds}
                    onChange={(e) => setShowIds(e.target.checked)}
                  />
                  显示词条与敌人 ID
                </label>
                <hr />
                <button
                  onClick={() =>
                    void (desktop
                      ? window.review
                          .copyLog()
                          .then(() => setStatus("日志已复制。"))
                      : copy(logs.current.join("\n")))
                  }
                >
                  复制日志
                </button>
                <button
                  onClick={() => {
                    if (desktop) {
                      setPopup("");
                      open("检查更新");
                    } else
                      window.open(
                        data.github + "/releases/latest",
                        "_blank",
                        "noopener",
                      );
                  }}
                >
                  检查更新
                </button>
              </>
            ) : (
              <>
                <h2>界面语言</h2>
                {(
                  [
                    ["zh-CN", "简体中文"],
                    ["en-US", "English"],
                    ["ja-JP", "日本語"],
                  ] as const
                ).map(([value, label]) => (
                  <button
                    key={value}
                    className={locale === value ? "current-language" : ""}
                    onClick={() => {
                      setUiLocale(value);
                      setPopup("");
                    }}
                  >
                    {label}
                    {locale === value ? " ✓" : ""}
                  </button>
                ))}
              </>
            )}
          </section>
        </>
      )}
      {page === "backups" && <BackupManager />}
      <div className="toast" role="status" hidden={!toast}>
        {toast}
        <button aria-label="关闭提示" onClick={() => setToast("")}>
          ×
        </button>
      </div>
      <dialog ref={dialog} onClose={() => setModal("")}>
        <header>
          <h2>{modal}</h2>
          <button aria-label="关闭窗口" onClick={() => dialog.current?.close()}>
            ×
          </button>
        </header>
        {modal === "收藏夹" ? (
          <div className="favorites-review">
            <p>{favorites.length} / 50 张绘卷</p>
            {!favorites.length && <p>点击绘卷旁的 ☆ 即可收藏。</p>}
            <div className="compare-grid">
              {favorites.map((s) => (
                <div key={cartKey(s)}>
                  <ScrollCard sample={s} level={submitted.recommended} />
                  <div className="collection-actions">
                    {favoriteButton(s)}
                    <button
                      disabled={cart.some((c) => cartKey(c) === cartKey(s))}
                      onClick={() => void addCart(s)}
                    >
                      加入购物车
                    </button>
                    <button onClick={() => void copyText(s.seed)}>
                      复制 ID
                    </button>
                    <button onClick={() => void copyText("R" + s.rarity)}>
                      复制稀有度
                    </button>
                    <button
                      onClick={() => void copyText(s.seed + " · R" + s.rarity)}
                    >
                      复制 ID 和稀有度
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        ) : modal === "管理预览" ? (
          <section className="preview-management">
            {results.map((sample) => (
              <label key={sample.seed}>
                <input
                  type="checkbox"
                  checked={previewPicked.includes(sample.seed)}
                  onChange={(e) =>
                    setPreviewPicked(
                      e.target.checked
                        ? [...previewPicked, sample.seed]
                        : previewPicked.filter((s) => s !== sample.seed),
                    )
                  }
                />
                绘卷 ID {sample.seed} · {sample.effects[0]?.name}
              </label>
            ))}
            <button
              disabled={!previewPicked.length}
              onClick={() => {
                setResults(
                  results.filter((s) => !previewPicked.includes(s.seed)),
                );
                setIndex(0);
                dialog.current?.close();
              }}
            >
              移除所选 {previewPicked.length} 张预览
            </button>
          </section>
        ) : modal === "检查更新" ? (
          <Updates />
        ) : modal === "最近三批" ? (
          <div className="history-pages">
            {history.map((page) => (
              <section key={page.id}>
                <h3>
                  {page.time} · {page.query.ng} 周目 · {page.samples.length} 张
                </h3>
                <div className="compare-grid">
                  {page.samples.map((sample) => (
                    <div key={sample.seed}>
                      <ScrollCard
                        sample={sample}
                        level={page.query.recommended}
                      />
                      <button
                        disabled={cart.some(
                          (s) =>
                            s.seed === sample.seed &&
                            s.rarity === sample.rarity,
                        )}
                        onClick={() => void addCart(sample)}
                      >
                        加入购物车
                      </button>
                    </div>
                  ))}
                </div>
              </section>
            ))}
          </div>
        ) : modal === "购物车" ? (
          <div className="cart-review">
            <p>
              {cart.length} 张绘卷，已勾选{" "}
              {cart.filter((s) => cartSelected.includes(cartKey(s))).length} 张
              · {installMode === "live" ? "游戏内实时添加" : "添加到存档"}
            </p>
            <div className="compare-grid">
              {cart.map((s) => (
                <div key={cartKey(s)}>
                  <label className="cart-check">
                    {addedKeys.includes(cartKey(s)) && (
                      <strong>已添加 · </strong>
                    )}
                    <input
                      type="checkbox"
                      aria-label={"勾选绘卷" + s.seed}
                      checked={cartSelected.includes(cartKey(s))}
                      onChange={(e) =>
                        setCartSelected(
                          e.target.checked
                            ? [...cartSelected, cartKey(s)]
                            : cartSelected.filter((k) => k !== cartKey(s)),
                        )
                      }
                    />
                    选择此绘卷
                  </label>
                  <ScrollCard sample={s} level={submitted.recommended} />
                  {favoriteButton(s)}
                  <button onClick={() => removeCart(s)}>移出购物车</button>
                </div>
              ))}
            </div>
            {desktop ? (
              <DesktopCartActions
                samples={cart.filter((s) => cartSelected.includes(cartKey(s)))}
                mode={installMode}
                query={q}
                onAdded={recordAdded}
              />
            ) : (
              <button
                disabled={!cart.some((s) => cartSelected.includes(cartKey(s)))}
                onClick={() =>
                  setToast(
                    "已选择 " +
                      cart.filter((s) => cartSelected.includes(cartKey(s)))
                        .length +
                      " 张绘卷；此界面预览暂不执行游戏或存档写入。",
                  )
                }
              >
                添加所选（
                {cart.filter((s) => cartSelected.includes(cartKey(s))).length}）
              </button>
            )}
          </div>
        ) : modal === "绘卷比较" ? (
          <div className="compare-grid">
            {results
              .filter((s) => comparison.includes(s.seed))
              .map((s) => (
                <ScrollCard
                  sample={s}
                  level={submitted.recommended}
                  key={s.seed}
                />
              ))}
          </div>
        ) : (
          <div className="modal-body">
            <p>
              {modal === "敌人组合"
                ? "想刷某个敌人：添加它并选“必含”。几个敌人都可以：把它们设为同一个“任选组”，出现其中一个就算符合。"
                : "装备加成：在左边选择你想要的词条和恩宠。刷副本：在右边选择想打的敌人、特殊规则、地形和挑战次数。"}
            </p>
            <h3>怎样组合词条？</h3>
            <p>
              “必含”：每个选中的词条都要有。例如同时添加体力和幸运，结果必须同时有这两项。
            </p>
            <p>
              “任选组”：同组中有一项就可以。例如把体力和幸运放进“任选组
              1”，有体力或有幸运都可以。
            </p>
            <h3>想指定第一条词条？</h3>
            <p>
              关闭“主词条不限”。已选列表最前面的 1～3
              项作为第一条词条的备选，找到其中一项即可。拖动已选词条调整顺序。
            </p>
            <p>
              如果选了两个备选，并勾选“未选为主时，副词条也要有”，就会寻找这样的绘卷：一项在第一条，另一项在副词条。
            </p>
            <h3>规则数值怎么选？</h3>
            <p>
              选择“全部接受”，表示这条规则的任何数值都可以；也可以指定某个数值。选好后点“＋”加入条件，不需要时在上方移除。
            </p>
          </div>
        )}
      </dialog>
    </div>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
