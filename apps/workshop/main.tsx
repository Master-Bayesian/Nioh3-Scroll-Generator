import { useUiLocale, setUiLocale, localize } from "./presentation";
import { ScrollCard } from "./ScrollCard";
import {
  decideUpdatePrompt,
  useDialogBackdropDismiss,
} from "./use-dialog-backdrop-dismiss";
import { collectionKey, useCollections } from "./collections";
import { Updates, UpdateNotice } from "./Updates";
import { StarIcon } from "./StarIcon";
import { SectionHelp } from "./SectionHelp";
import { BrandMark } from "./BrandMark";
import React, {
  createContext,
  useContext,
  useEffect,
  useLayoutEffect,
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
  enemyCanBePossessed,
  enemyCombinationProblem,
  ANY_RULE_VALUE,
  ruleFamilyKeys,
  ruleFamilyValues,
  type Query,
  type Sample,
  type SelectedEffect,
} from "./model";
import "./style.css";
import "./arc.css";
import { BackupManager } from "./BackupManager";
import { Editor } from "./Editor";
import { CharacterEditor } from "./CharacterEditor";
import { RuntimeCompatibility } from "./RuntimeCompatibility";
import { GameInstallation, GameVersionHint } from "./GameInstallation";
import { DesktopCartActions, SavePicker } from "./CartActions";
import { FeedbackSaved, Notice } from "./Notice";
import { errorText, publicError } from "./public-errors";
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
import { searchStartParams, searchStatusText } from "./search-policy";
import { useConditionDrag } from "./condition-drag";
import { useTheme, type Theme } from "./theme";
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
function collectionSearchText(sample: Sample): string {
  const terrain = data.terrains.find(
    (candidate) =>
      !candidate.aggregate &&
      candidate.effect_keys.length === sample.terrainKeys.length &&
      candidate.effect_keys.every((key) => sample.terrainKeys.includes(key)),
  )?.name;
  const values = [
    sample.seed,
    `R${sample.rarity}`,
    sample.level || 180,
    sample.capacity,
    (sample.playthrough || 3) === 3
      ? "百境百怪绘卷 · 顿悟"
      : `${sample.playthrough} 周目战绘卷`,
    ...sample.effects.flatMap((effect) => [effect.name, effect.role]),
    ...(sample.enemyOccurrences?.map((enemy) => enemy.name) || sample.enemies),
    ...sample.rules.flatMap((rule) => [rule.name, rule.value]),
    terrain || "",
  ];
  return values
    .flatMap((value) => {
      const source = String(value);
      return [source, localize(source)];
    })
    .join(" ")
    .toLocaleLowerCase();
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
function ToggleSwitch({
  label,
  checked,
  onChange,
  disabled = false,
  compact = false,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  compact?: boolean;
}) {
  return (
    <label className={`toggle-switch${compact ? " compact" : ""}`}>
      <span>{label}</span>
      <input
        type="checkbox"
        role="switch"
        aria-label={label}
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      <i aria-hidden="true" />
    </label>
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
  const { opened } = useContext(PanelContext);
  const group = ["主副词条", "恩宠"].includes(title) ? "equipment" : "dungeon";
  const expanded = opened.catalog === title;
  const [scrolled, setScrolled] = useState(false);
  const body = useRef<HTMLDivElement>(null);
  return (
    <section className={"module " + tone + (expanded ? " is-open" : "")} hidden={!expanded} data-group={group}>
      <header>
        <h2 className="panel-title">{title}</h2>
        {extra}
        <SectionHelp key={String(expanded)} title={title + "说明"}>
          {help}
        </SectionHelp>
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
  const [feedbackState, setFeedbackState] = useState<"" | "busy" | "saved">("");
  const [resetState, setResetState] = useState<"" | "confirm" | "busy">("");
  const [resetError, setResetError] = useState("");
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
    [ruleFind, setRuleFind] = useState(""),
    [favoriteFind, setFavoriteFind] = useState("");
  const [theme, setTheme] = useTheme();
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
          state.handshake ? "请选择筛选条件。" : "正在启动…",
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
      const next = job.candidates
        .map((c) =>
          candidateSample(
            c,
            state.submitted?.query.level || 180,
            job.job_id,
            undefined,
            submittedForm.enemyVariant,
          ),
        )
        .sort(
          (a, b) => score(b, sorting.current) - score(a, sorting.current),
        );
      setResults(next);
      const viewed = next.findIndex(
        (s) => s.seed + ":" + s.rarity === viewedSeed.current,
      );
      if (viewed >= 0) setIndex(viewed);
      setResumeAvailable(!!job.resume_token);
      setResultSource("真实搜索结果");
      setStatus(searchStatusText(job));
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
  const [directAdd, setDirectAdd] = useState<{
    sample: Sample;
    mode: string;
    query: Query;
  } | null>(null);
  const [retainingCurrent, setRetainingCurrent] = useState(false);
  const ownedReferences = useRef(new Set<string>());
  useEffect(() => {
    const next = new Set(
      [
        ...cart,
        ...favorites,
        ...results,
        ...(directAdd ? [directAdd.sample] : []),
        ...history.flatMap((p) => p.samples),
      ].flatMap((s) => (s.backend?.referenceId ? [s.backend.referenceId] : [])),
    );
    if (desktop)
      for (const ref of ownedReferences.current)
        if (!next.has(ref))
          void window.review.release(ref).catch((e) => setStatus(String(e)));
    ownedReferences.current = next;
  }, [cart, favorites, results, history, directAdd]);
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
  const [popupPosition, setPopupPosition] = useState({ top: 64, right: 16 });
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
    [popup, setPopup] = useState("");
  useEffect(() => setFeedbackState(""), [popup]);
  const [appVersion, setAppVersion] = useState("");
  const [sourceCommit,setSourceCommit]=useState("");
  useEffect(() => {
    if (!desktop) return;
    void window.support
      .diagnostics()
      .then((report) => {setAppVersion(report.version);setSourceCommit(report.packageVerification?.sourceCommit?.slice(0,7)||"")})
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
  const [toast, setToast] = useState("");
  // One shared highlight glides between the navigation buttons: it follows
  // the pointer and settles back on the current page.
  const navRef = useRef<HTMLElement>(null);
  const [glide, setGlide] = useState<{ x: number; w: number } | null>(null);
  function placeGlide(target?: Element | null) {
    const button = (target ??
      navRef.current?.querySelector("button.active")) as HTMLElement | null;
    setGlide(button ? { x: button.offsetLeft, w: button.offsetWidth } : null);
  }
  useLayoutEffect(() => {
    placeGlide();
    const settle = () => placeGlide();
    window.addEventListener("resize", settle);
    void document.fonts?.ready.then(settle);
    return () => window.removeEventListener("resize", settle);
  }, [page, locale, favorites.length]);
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
    pendingUpdatePrompt = useRef(false),
    cancel = useRef(false),
    generation = useRef(0);
  const dialogBackdropDismiss = useDialogBackdropDismiss();
  const context =
    data.contexts[`${q.ng}-${q.rarity}` as keyof typeof data.contexts];
  const change = <K extends keyof Query>(key: K, value: Query[K]) =>
    setQ((prev) => ({ ...prev, [key]: value }));
  const dirty = JSON.stringify(q) !== JSON.stringify(submitted);
  const selected = results[index];
  // Results arrive while a search runs and are re-sorted each time; the scroll
  // being viewed keeps its place instead of being swapped out underneath.
  const viewedSeed = useRef<string | null>(null);
  viewedSeed.current = selected ? selected.seed + ":" + selected.rarity : null;
  const patchEffect = (i: number, p: Partial<SelectedEffect>) =>
    change(
      "effects",
      q.effects.map((e, j) => (i === j ? { ...e, ...p } : e)),
    );
  function open(name: string) {
    setModal(name);
    if (!dialog.current?.open) dialog.current?.showModal();
  }
  function openUpdatePrompt() {
    const decision = decideUpdatePrompt(
      Boolean(dialog.current?.open),
      pendingUpdatePrompt.current,
    );
    if (decision.action === "defer") {
      pendingUpdatePrompt.current = true;
      return;
    }
    pendingUpdatePrompt.current = false;
    open("检查更新");
  }
  function handleDialogClose() {
    setModal("");
    setDirectAdd(null);
    if (pendingUpdatePrompt.current) {
      const decision = decideUpdatePrompt(false, true);
      pendingUpdatePrompt.current = false;
      if (decision.action === "reopen") {
        requestAnimationFrame(() => open("检查更新"));
      }
    }
  }
  async function addCurrent() {
    if (!selected || retainingCurrent) return;
    const sample = selected;
    const mode = installMode;
    const query = { ...q };
    setRetainingCurrent(true);
    try {
      // Pin the selected candidate independently of the cart and search page.
      const retained = desktop ? await retainSample(sample) : sample;
      setDirectAdd({ sample: retained, mode, query });
      open("添加当前绘卷");
    } catch (error) {
      setStatus(String(error));
    } finally {
      setRetainingCurrent(false);
    }
  }
  function swapContext(ng: number, rarity: number) {
    if (ng !== 3) setInstallMode("save");
    const c = data.contexts[`${ng}-${rarity}` as keyof typeof data.contexts];
    const kept = q.effects
      .filter((e) => c.effects.some((v) => v.id === e.id))
      .map((e) => (ng <= 3 ? e : { ...e, roll: 0 }));
    setQ({
      ...q,
      ng,
      rarity,
      effects: kept,
      graces: q.graces.filter((id) => c.graces.some((e) => e.id === id)),
      enemyVariant: ng === 3 ? q.enemyVariant : "solo",
      enemies:
        ng === 3
          ? q.enemies
          : q.enemies.map((enemy) => ({
              ...enemy,
              state: "any",
              availability: "any",
            })),
    });
    setStatus(
      ng <= 3
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
  // The worker's own structural preflight, asked as the conditions change, so
  // an impossible combination is explained while the player is still picking
  // instead of after a search. The worker stays the only judge.
  const [conditionProblem, setConditionProblem] = useState("");
  useEffect(() => {
    if (!desktop || q.ng >= 4 || !connected) {
      setConditionProblem("");
      return;
    }
    let query: ReturnType<typeof workerQuery>;
    try {
      query = workerQuery(q);
    } catch {
      setConditionProblem("");
      return;
    }
    let stale = false;
    const timer = setTimeout(() => {
      void window.nioh
        .checkFeasibility(query)
        .then((result) => {
          if (!stale)
            setConditionProblem(
              result.checked && !result.feasible && result.reason
                ? publicError(result.reason)
                : "",
            );
        })
        .catch(() => {
          // A preflight that cannot answer must not block searching.
          if (!stale) setConditionProblem("");
        });
    }, 200);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [q, connected]);
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
    // NG1-NG3 search offline (their generation matches the native generator);
    // NG4/NG5 stay native unless a measured rarity-5 map is cached.
    if (
      desktop &&
      q.ng >= 4 &&
      !(q.rarity === 5 && cache?.key === cacheKey())
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
        await searchController.start(searchStartParams({
          query,
          contextDigest:
            searchController.getSnapshot().handshake!.context.context_digest,
          cacheId: q.ng >= 4 && cache ? cache.id : null,
          resultCount: q.count,
          allowCpuFallback: allowCpu,
        }));
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
          disabled={q.ng >= 4}
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
    `${e.name} ${localize(e.name)} ${hex(e.id)}`
      .toLowerCase()
      .includes(effectFind.toLowerCase()),
  );
  const enemyRows = data.enemies.filter(
    (e) =>
      (tier === "全部" || e.tier.includes(tier.replace("手", ""))) &&
      `${e.name} ${localize(e.name)} ${hex(e.id)}`
        .toLowerCase()
        .includes(enemyFind.toLowerCase()),
  );
  const ruleRows = data.rules.filter((r) =>
    `${r.name} ${localize(`${r.name} ${r.variants.map((v) => v.label).join(" ")}`)}`
      .toLowerCase()
      .includes(ruleFind.toLowerCase()),
  );
  const categories = [...new Set(ruleRows.map((r) => r.category))];
  const favoriteQuery = favoriteFind.trim().toLocaleLowerCase();
  const visibleFavorites = favoriteQuery
    ? favorites.filter((sample) =>
        collectionSearchText(sample).includes(favoriteQuery),
      )
    : favorites;
  return (
    <div className="shell">
      <header className="topbar">
        <div className="brand">
          <BrandMark />
          <div className="brand-text">
            <b>独脚踏鞴工作室</b>
            <small>
              <span className="credits">作者：MasterBayesian · Saber_Li</span>
              {appVersion && <em className="app-version">v{appVersion}</em>}
              {sourceCommit&&<em className="app-build" title="当前程序构建">{sourceCommit}</em>}
            </small>
          </div>
        </div>
        <h1 className="visually-hidden">
          {page === "search"
            ? "绘卷搜索"
            : page === "favorites"
              ? "收藏夹"
              : page === "backups"
              ? "存档管理"
              : page === "equipment"
                ? "装备与道具"
                : "绘卷编辑"}
        </h1>
        <nav
          className="nav"
          ref={navRef}
          onMouseOver={(e) =>
            placeGlide((e.target as Element).closest("button:not(:disabled)"))
          }
          onMouseLeave={() => placeGlide()}
        >
          <span
            className="nav-glider"
            aria-hidden="true"
            style={
              glide
                ? { transform: `translateX(${glide.x}px)`, width: glide.w }
                : { opacity: 0 }
            }
          />
          <button
            className={page === "search" ? "active" : ""}
            onClick={() => setPage("search")}
            aria-label="绘卷搜索"
            title="绘卷搜索"
          >
            <span>绘卷搜索</span>
          </button>
          <button
            className={page === "editor" ? "active" : ""}
            onClick={() => setPage("editor")}
            aria-label="绘卷编辑"
            title="绘卷编辑"
          >
            <span>绘卷编辑</span>
          </button>
          <button
            className={page === "backups" ? "active" : ""}
            onClick={() => setPage("backups")}
            aria-label="存档管理"
          >
            <span>存档管理</span>
          </button>
          <button
            className={page === "favorites" ? "active" : ""}
            onClick={() => setPage("favorites")}
            aria-label="收藏夹"
          >
            <span>收藏夹</span>
            <small className="nav-count">{favorites.length}</small>
          </button>
          <button
            className={page === "equipment" ? "active" : ""}
            onClick={() => setPage("equipment")}
            aria-label="装备与道具"
            title="装备与道具"
          >
            <span>装备与道具</span>
          </button>
          <button className="coming-soon" disabled>
            <span>敬请期待</span>
          </button>
        </nav>
        <div className="toplinks">
          {desktop && <UpdateNotice onOpen={openUpdatePrompt} />}
          <button
            className="icon-button language-button"
            aria-label="切换语言"
            title="切换语言"
            onClick={(e) => {
              const r = e.currentTarget.getBoundingClientRect();
              setPopupPosition({
                top: r.bottom + 8,
                right: window.innerWidth - r.right,
              });
              setPopup(popup === "language" ? "" : "language");
            }}
          >
            <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.7">
              <circle cx="12" cy="12" r="8.5" />
              <path d="M3.5 12h17M12 3.5c2.5 2.6 2.5 14.4 0 17M12 3.5c-2.5 2.6-2.5 14.4 0 17" />
            </svg>
          </button>
          <button
            className="icon-button settings"
            aria-label="设置"
            title="设置"
            onClick={(e) => {
              const r = e.currentTarget.getBoundingClientRect();
              setPopupPosition({
                top: r.bottom + 8,
                right: window.innerWidth - r.right,
              });
              setPopup(popup === "settings" || popup === "game" || popup === "about" ? "" : "settings");
            }}
          >
            <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.7">
              <circle cx="12" cy="12" r="3" />
              <path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1" />
            </svg>
          </button>
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
            className="github-link"
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
      {desktop&&<RuntimeCompatibility/>}
      <div className="search-page" hidden={page !== "search"}>
          <PanelContext.Provider
            value={{
              opened: { catalog: opened.catalog || "主副词条" },
              toggle: (_group, title) =>
                setOpened({ ...opened, catalog: title }),
            }}
          >
            <aside className="catalog" aria-label="条件目录">
              <header className="catalog-head">
                <h2>条件目录</h2>
                <p className="catalog-hint">先选类型，再从下方列表添加</p>
                {(
                  [
                    ["装备加成 · 装备在身上", [["主副词条", "主副词条", "sand", q.effects.length], ["恩宠", "恩宠", "purple", q.graces.length]]],
                    ["副本刷取 · 刷副本时", [["敌人", "敌人", "blue", q.enemies.length], ["特殊规则", "特殊规则", "rose", q.rules.length], ["地形影响与挑战次数", "地形 · 次数", "teal", q.terrains.length + q.capacities.length]]],
                  ] as [string, [string, string, string, number][]][]
                ).map(([group, tabs]) => (
                  <div className="catalog-tab-group" key={group}>
                    <small>{group}</small>
                    <div className="catalog-tabs" role="tablist" aria-label={group}>
                      {tabs.map(([title, label, tone, picked]) => (
                        <button
                          key={title}
                          role="tab"
                          aria-selected={(opened.catalog || "主副词条") === title}
                          className={"catalog-tab " + tone}
                          onClick={() => setOpened({ ...opened, catalog: title })}
                        >
                          <i aria-hidden="true" />
                          {label}
                          {picked > 0 && <b>{picked}</b>}
                        </button>
                      ))}
                    </div>
                  </div>
                ))}
              </header>
              <div
                className="catalog-column"
                aria-label="装备加成筛选"
                tabIndex={0}
              >
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
                          disabled={
                            q.effects.length >= 24 ||
                            q.effects.some((x) => x.id === e.id)
                          }
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
                          {q.effects.some((x) => x.id === e.id) ? "已选" : "＋"}
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
                <Panel
                  title="敌人"
                  tone="blue"
                  help="低／中／高指敌人生成池，不代表难度。添加敌人表示结果中至少包含它；游戏仍可能生成其他敌人。只有原生规则允许的低手敌人才提供地狱附身开关。"
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
                  <div className="enemy-list-frame">
                    <div className="enemy-list-caption">
                      <span>敌人列表</span>
                      <small>点击＋添加筛选条件</small>
                    </div>
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
                          <em>
                            {e.tier}
                            {!!e.possessedKeys.length && <b>可附身</b>}
                          </em>
                          <button
                            aria-label={"添加敌人" + e.name}
                            disabled={q.enemies.some((x) => x.id === e.id)}
                            onClick={() =>
                              change("enemies", [
                                ...q.enemies,
                                {
                                  ...e,
                                  mode: 0,
                                  state: "any",
                                  availability: "any",
                                },
                              ])
                            }
                          >
                            {q.enemies.some((x) => x.id === e.id) ? "已选" : "＋"}
                          </button>
                        </div>
                      ))}
                    </div>
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
            </aside>
          </PanelContext.Provider>
        <main>
          <section className="selection">
            <header>
              <h2>
                已选条件 <span>{count}</span>
              </h2>
              <p className="selection-hint">每一块都要满足；虚线框内满足其一即可</p>
              <button onClick={() => open("使用说明")}>使用说明</button>
              <button onClick={reset} disabled={busy}>
                清空全部
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
            {conditionProblem && (
              <Notice
                text={conditionProblem}
                tone="warning"
                className="condition-problem"
              />
            )}
            <div className="selected-body">
              <div className="selected-effects">
                {q.effects.length ? (
                  <>
                    {!q.unrestricted && (
                      <div className="primary-group">
                        <small>
                          主词条 · 前 {q.primaryCount} 项任一当选
                        </small>
                        {q.effects.slice(0, q.primaryCount).map(renderEffect)}
                      </div>
                    )}
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
                    从左侧条件目录添加词条，也可以只选择恩宠或辅助条件。
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
                      <span title={e.name}>敌人 · {e.name}</span>
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
                      {q.ng === 3 && enemyCanBePossessed(e) && (
                        <ToggleSwitch
                          compact
                          label="地狱附身"
                          checked={e.state === "possessed"}
                          onChange={(checked) =>
                          change(
                            "enemies",
                            q.enemies.map((item) =>
                              item.id === e.id
                                ? {
                                    ...item,
                                    state: checked ? "possessed" : "any",
                                  }
                                : item,
                            ),
                          )
                          }
                        />
                      )}
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
                {enemyCombinationProblem(q) && (
                  <p className="enemy-combination-problem" role="alert">
                    这组必含敌人不可能同时出现：{enemyCombinationProblem(q)}
                    <button className="subtle" onClick={() => open("敌人组合")}>
                      组合说明
                    </button>
                  </p>
                )}
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
          {desktop && q.ng >= 4 && (
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
                  max="356"
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
                disabled={busy || !connected || !!conditionProblem}
                title={conditionProblem ? "当前条件组合不可能出现，请先按上方提示调整" : undefined}
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
                    if (q.ng >= 4) {
                      void nativeSearch(false, true);
                      return;
                    }
                    setBusy(true);
                    void previewSeed(
                      Number(direct),
                      q.rarity,
                      q.level,
                      false,
                      q.enemyVariant,
                      q.ng,
                    )
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
          <div className="status">
            {busy && <progress aria-label="搜索进度" />}
            <Notice text={status} />
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
              disabled={submitted.ng >= 4}
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
                </div>
                <div className="result-cart-actions">
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
              </div>
            ) : resultSource && !busy ? (
              <div className="no-results">
                <h3>没有匹配结果</h3>
                <p>试试减少筛选条件。</p>
                <button onClick={reset}>清空条件</button>
              </div>
            ) : null}
          </div>
          <section className={"install-mode" + (desktop && installMode === "save" ? " install-save" : "")}>
            <div className="install-choice">
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
            </div>
            {desktop && installMode === "save" && <SavePicker compact />}
            <button
              className="install-current"
              disabled={!selected || retainingCurrent}
              onClick={() => void addCurrent()}
            >
              添加当前绘卷
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
            className={"side-popup" + (popup === "game" || popup === "about" ? " side-popup-wide" : popup === "settings" ? " settings-menu" : "")}
            style={
              {
                "--popup-top": popupPosition.top + "px",
                "--popup-right": popupPosition.right + "px",
              } as React.CSSProperties
            }
            aria-label={popup === "settings" ? "设置菜单" : popup === "game" ? "游戏版本" : popup === "about" ? "关于与安全" : "语言菜单"}
            onKeyDown={(event) => { if (event.key === "Escape") setPopup(popup === "game" || popup === "about" ? "settings" : ""); }}
          >
            {popup === "game" ? (
              <>
                <button className="menu-back" onClick={() => setPopup("settings")}>‹ 设置</button>
                <h2>游戏版本</h2>
                <GameInstallation />
              </>
            ) : popup === "about" ? (
              <>
                <button className="menu-back" onClick={() => setPopup("settings")}>‹ 设置</button>
                <h2>关于与安全</h2>
                <div className="about-safety">
                  <h3>完全免费</h3>
                  <p>本工具免费提供，作者没有授权任何人收费售卖。如果你是付费买到的，请申请退款并举报卖家。</p>
                  <h3>只从官方渠道下载</h3>
                  <p>官方渠道只有 GitHub 发布页和作者的 QQ 群。其他地方转发的文件可能被改动过：如果杀毒软件对这类文件报毒，请直接删除，再从官方渠道重新下载。</p>
                  <p>
                    <button className="about-link" onClick={() => { if (desktop) void window.review.openLink("updates"); else window.open(data.github + "/releases/latest", "_blank", "noopener"); }}>打开 GitHub 发布页</button>
                  </p>
                  <h3>它会读写什么</h3>
                  <ul>
                    <li>仁王3 的存档文件夹：修改存档前会自动备份，可以在“存档管理”里恢复。</li>
                    <li>游戏进程内存：只在你使用“游戏内实时修改”或实时添加时读写。</li>
                    <li>本工具自己的数据目录：设置、收藏、备份和日志。</li>
                  </ul>
                  <p>联网只用于检查和下载更新（连接 GitHub），不会上传存档或个人信息。“反馈问题”生成的文件保存在你自己的电脑上，由你决定是否发送。</p>
                  <h3>关于报毒</h3>
                  <p>修改游戏存档和内存的工具，常被杀毒软件按“行为可疑”拦截，被拦截不一定代表文件有害。官方下载的文件可以在 GitHub 发布页核对 SHA-256；如果仍不放心，可以把文件提交到 VirusTotal 等多引擎扫描网站自行检查。</p>
                  <p className="about-source">源码公开在 GitHub 上，欢迎查看。</p>
                </div>
              </>
            ) : popup === "settings" ? (
              <>
                <div className="theme-choice" role="radiogroup" aria-label="外观">
                  <span>外观</span>
                  {([["system", "跟随系统"], ["light", "浅色"], ["dark", "深色"]] as [Theme, string][]).map(([value, label]) => (
                    <button key={value} role="radio" aria-checked={theme === value} data-theme-choice={value}
                      className={theme === value ? "active" : ""} onClick={() => setTheme(value)}>{label}</button>
                  ))}
                </div>
                <ToggleSwitch
                  label="显示词条与敌人 ID"
                  checked={showIds}
                  onChange={setShowIds}
                />
                {desktop && (
                  <ToggleSwitch
                    label="允许使用 CPU 搜索"
                    checked={allowCpu}
                    onChange={setAllowCpu}
                  />
                )}
                <hr />
                {desktop && (
                  <button className="menu-item" data-action="open-game-installation" onClick={() => setPopup("game")}>
                    <span>游戏版本</span>
                    <GameVersionHint />
                    <i aria-hidden="true">›</i>
                  </button>
                )}
                {desktop ? (
                  <button
                    className="menu-item"
                    disabled={feedbackState === "busy"}
                    title="生成一个反馈文件（包含版本信息和最近的操作记录，不含存档内容）"
                    onClick={() => {
                      setFeedbackState("busy");
                      void window.review
                        .exportFeedback()
                        .then(() => setFeedbackState("saved"))
                        .catch((e) => {
                          setFeedbackState("");
                          setStatus(String(e));
                        });
                    }}
                  >
                    <span>反馈问题</span>
                  </button>
                ) : (
                  <button className="menu-item" onClick={() => void copy(logs.current.join("\n"))}>
                    <span>复制日志</span>
                  </button>
                )}
                {feedbackState === "saved" && <FeedbackSaved onDismiss={() => setFeedbackState("")} />}
                {desktop && window.support?.resetTool && (
                  <button className="menu-item" data-action="reset-tool" disabled={resetState === "busy"}
                    onClick={() => { setResetError(""); setResetState(resetState ? "" : "confirm"); }}>
                    <span>重置工具</span>
                  </button>
                )}
                {resetState !== "" && (
                  <div className="reset-tool">
                    <p>工具卡住、按钮没反应或一直提示先处理上一次操作时使用。请先关闭游戏。重置会把以前留下的操作记录移到数据目录的 reset-archive 文件夹，然后重新打开界面；存档备份、收藏和设置都会保留。</p>
                    <div>
                      <button className="primary" data-action="confirm-reset-tool" disabled={resetState === "busy"}
                        onClick={() => {
                          setResetState("busy");
                          setResetError("");
                          void window.support.resetTool!()
                            .then(() => {
                              try {
                                for (const key of Object.keys(localStorage)) if (key !== "nioh3-theme") localStorage.removeItem(key);
                              } catch {
                                // The records are archived; the page reload still starts clean.
                              }
                              location.reload();
                            })
                            .catch((error) => { setResetState("confirm"); setResetError(errorText(error)); });
                        }}>确认重置</button>
                      <button disabled={resetState === "busy"} onClick={() => setResetState("")}>取消</button>
                    </div>
                    <Notice text={resetError} />
                  </div>
                )}
                <button
                  className="menu-item"
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
                  <span>检查更新</span>
                </button>
                <button className="menu-item" data-action="open-about" onClick={() => setPopup("about")}>
                  <span>关于与安全</span>
                  <i aria-hidden="true">›</i>
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
      {page === "favorites" && (
        <main className="favorites-page">
          <div className="favorites-review">
            <div className="favorites-toolbar">
              <p>{favorites.length} / 50 张绘卷</p>
              {favorites.length > 0 && (
                <Finder
                  label="搜索收藏夹中的绘卷 ID、词条、恩宠或敌人"
                  value={favoriteFind}
                  set={setFavoriteFind}
                />
              )}
            </div>
            {!favorites.length && <p>点击绘卷旁的 ☆ 即可收藏。</p>}
            {favorites.length > 0 && !visibleFavorites.length && (
              <p className="empty-list">收藏夹中没有匹配的绘卷。</p>
            )}
            <div className="compare-grid">
              {visibleFavorites.map((s) => (
                <div className="collection-card-item favorite-card" key={cartKey(s)}>
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
        </main>
      )}
      {page === "backups" && <BackupManager />}
      {page === "equipment" && <CharacterEditor showIds={showIds} />}
      <div className="toast" role="status" hidden={!toast}>
        {toast}
        <button aria-label="关闭提示" onClick={() => setToast("")}>
          ×
        </button>
      </div>
      <dialog
        ref={dialog}
        {...dialogBackdropDismiss}
        onClose={handleDialogClose}
      >
        <header>
          <h2>{modal}</h2>
          <button aria-label="关闭窗口" onClick={() => dialog.current?.close()}>
            ×
          </button>
        </header>
        {modal === "管理预览" ? (
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
          <Updates onClose={() => dialog.current?.close()} />
        ) : modal === "最近三批" ? (
          <div className="history-pages">
            {history.map((page) => (
              <section key={page.id}>
                <h3>
                  {page.time} · {page.query.ng} 周目 · {page.samples.length} 张
                </h3>
                <div className="compare-grid">
                  {page.samples.map((sample) => (
                    <div className="collection-card-item" key={sample.seed}>
                      <ScrollCard
                        sample={sample}
                        level={page.query.recommended}
                      />
                      <div className="collection-actions">
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
                    </div>
                  ))}
                </div>
              </section>
            ))}
          </div>
        ) : modal === "添加当前绘卷" && directAdd ? (
          <div className="current-add-review">
            <p className="current-add-summary">
              绘卷 ID <strong>{directAdd.sample.seed}</strong> · R{directAdd.sample.rarity}
              {" · "}{directAdd.mode === "live" ? "游戏内实时添加" : "添加到存档"}
            </p>
            {desktop ? (
              <DesktopCartActions
                samples={[directAdd.sample]}
                mode={directAdd.mode}
                query={directAdd.query}
                onAdded={recordAdded}
                autoPrepare
              />
            ) : <p>此界面预览暂不执行游戏或存档写入。</p>}
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
                <div className="collection-card-item" key={cartKey(s)}>
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
                  <div className="collection-actions">
                    {favoriteButton(s)}
                    <button onClick={() => removeCart(s)}>移出购物车</button>
                  </div>
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
                <div className="collection-card-item" key={s.seed}>
                  <ScrollCard
                    sample={s}
                    level={submitted.recommended}
                  />
                </div>
              ))}
          </div>
        ) : modal === "敌人组合" ? (
          <div className="modal-body enemy-combination-help">
            <h3>必含</h3>
            <p>
              每个设为“必含”的敌人都必须出现在结果中。例如同时选择古笼火和肉瘤怪，结果必须同时包含两者。
            </p>
            <h3>任选组</h3>
            <p>
              同一任选组只需出现其中一个敌人。例如把古笼火和肉瘤怪放入“任选组 1”，出现任意一个就符合。
            </p>
            <h3>多个条件怎样计算？</h3>
            <p>
              每个必含条件和每个任选组都要分别满足；绘卷中可以同时出现没有选择的其他敌人。
            </p>
            <h3>游戏里有哪些敌人组合？</h3>
            <p>每张绘卷的敌人只会是下面三种结构之一：</p>
            <table className="enemy-structure-table">
              <thead>
                <tr><th>结构</th><th>约占</th><th>敌人构成</th></tr>
              </thead>
              <tbody>
                <tr><td>中高手</td><td>40%</td><td>只有中手和高手，共 2～3 个，其中至少 1 个高手</td></tr>
                <tr><td>低手 + 高手</td><td>40%</td><td>若干低手，外加最多 1 个高手，没有中手</td></tr>
                <tr><td>纯低手</td><td>20%</td><td>只有低手</td></tr>
              </tbody>
            </table>
            <p>所以：</p>
            <ul>
              <li>低手和中手不会出现在同一张绘卷里；</li>
              <li>有低手的绘卷最多只有 1 个高手；</li>
              <li>中手和高手合计最多 3 个（例如三个高手可以，四个不行），中手最多 2 个。</li>
            </ul>
            <p className="muted">
              低／中／高指敌人生成池，不代表难度。必含敌人违反这些规则时会直接提示，不必等搜索跑完；符合规则也只代表“有可能”，是否真有这样的绘卷仍以搜索结果为准。任选组和常世同行追加的敌人不参与这项检查。
            </p>
          </div>
        ) : (
          <div className="modal-body">
            <p>
              装备加成：在左侧条件目录的“主副词条”“恩宠”里选择你想要的词条和恩宠。刷副本：在“敌人”“特殊规则”“地形 · 次数”里选择想打的敌人、特殊规则、地形和挑战次数。
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
