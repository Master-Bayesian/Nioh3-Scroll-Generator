import React, { useMemo, useState, useSyncExternalStore } from "react";
import type {
  CharacterEquipment,
  LiveCharacter,
  SaveCharacter,
} from "../../packages/contracts/protected-responses";
import { data } from "./model";
import { desktop } from "./desktop-bridge";
import { fillTemplateSlots, plainGameText } from "./game-text";
import { LocalCatalogImport, type ActiveLocalCatalog } from "./LocalCatalogImport";
import { Notice } from "./Notice";
import { SavePicker } from "./CartActions";
import { runtimeObserver, saveObserver, saveSession } from "./save-workspace";

type Mode = "live" | "save";
type Character = (LiveCharacter | SaveCharacter) & { mode: Mode };
type Currency = "amrita" | "gold";
const CURRENCIES: Currency[] = ["amrita", "gold"];
const EMPTY_EFFECT = 0xffffffff;

const effectNames = new Map<number, string>();
for (const row of data.editorEffects as { id: string; name: string }[]) {
  const id = Number(row.id);
  if (Number.isInteger(id) && row.name && !effectNames.has(id))
    effectNames.set(id, fillTemplateSlots(plainGameText(row.name), "增益效果", "异常状态"));
}
const effectOptions = [...effectNames].map(([id, name]) => hex(id) + " " + name);

function hex(value: number) {
  return "0x" + value.toString(16).toUpperCase().padStart(4, "0");
}
function effectLabel(id: number) {
  if (id === EMPTY_EFFECT) return "（空）";
  return effectNames.get(id) ?? hex(id);
}
/** An effect field accepts `0xA166`, `41318`, or a datalist entry that starts with either. */
function parseEffectId(text: string): number | null {
  const token = text.trim().split(/\s+/)[0] ?? "";
  if (!token) return EMPTY_EFFECT;
  const value = /^0x[0-9a-f]+$/i.test(token) ? Number.parseInt(token, 16) : /^\d+$/.test(token) ? Number(token) : NaN;
  return Number.isInteger(value) && value >= 0 && value <= EMPTY_EFFECT ? value : null;
}
function parseAmount(text: string, max: number): number | null {
  if (!/^\d+$/.test(text.trim())) return null;
  const value = Number(text.trim());
  return Number.isSafeInteger(value) && value <= max ? value : null;
}

interface Draft {
  level: string;
  level_before_forge: string;
  plus: string;
  rarity: string;
  familiarity: string;
  effects: { id: string; value: string }[];
}
function draftOf(row: CharacterEquipment): Draft {
  return {
    level: String(row.level),
    level_before_forge: String(row.level_before_forge),
    plus: String(row.plus),
    rarity: String(row.rarity),
    familiarity: String(row.familiarity),
    effects: row.effects.map(effect => ({
      id: effect.effect_id === EMPTY_EFFECT ? "" : hex(effect.effect_id),
      value: String(effect.value),
    })),
  };
}

/** The changed fields of one record, or an error message when a field is invalid. */
function patchOf(row: CharacterEquipment, draft: Draft) {
  const patch: Record<string, unknown> = {};
  const fields: [keyof Draft & keyof CharacterEquipment, number, number][] = [
    ["level", 1, 65535],
    ["level_before_forge", 1, 65535],
    ["plus", 0, 65535],
    ["rarity", 0, 255],
    ["familiarity", 0, 4294967295],
  ];
  for (const [key, min, max] of fields) {
    const value = parseAmount(draft[key] as string, max);
    if (value === null || value < min) return { error: "请输入有效的数值。" };
    if (value !== row[key]) patch[key] = value;
  }
  const effects = [];
  for (const [index, effect] of draft.effects.entries()) {
    const id = parseEffectId(effect.id);
    const value = id === EMPTY_EFFECT ? 0 : parseAmount(effect.value, 4294967295);
    if (id === null || value === null) return { error: "请输入有效的词条 ID 和数值。" };
    const before = row.effects[index];
    if (!before || before.effect_id !== id || (id !== EMPTY_EFFECT && before.value !== value))
      effects.push({ index, effect_id: id, value });
  }
  if (effects.length) patch.effects = effects;
  return { patch };
}

const CURRENCY_LABEL: Record<string, string> = { amrita: "精华", gold: "持有金钱" };
const FIELD_LABEL: [keyof CharacterEquipment, string][] = [
  ["level", "等级"],
  ["level_before_forge", "锻造前等级"],
  ["plus", "+值"],
  ["rarity", "稀有度"],
  ["familiarity", "爱用度"],
];

/** The reviewed plan in words: every changed value, before and after. */
function PlanPreview({ preview }: { preview: Record<string, unknown> }) {
  const currencies = (preview.currencies ?? []) as { currency: string; before: number; after: number }[];
  const equipment = (preview.equipment ?? []) as { slot_index: number; before: CharacterEquipment; after: CharacterEquipment }[];
  const lines: string[] = [];
  for (const change of currencies)
    lines.push((CURRENCY_LABEL[change.currency] ?? change.currency) + "：" + change.before + " → " + change.after);
  for (const change of equipment) {
    const prefix = "槽位 " + change.slot_index + "（" + hex(change.before.item_id) + "）";
    for (const [key, label] of FIELD_LABEL)
      if (change.before[key] !== change.after[key])
        lines.push(prefix + " " + label + "：" + change.before[key] + " → " + change.after[key]);
    change.after.effects.forEach((effect, index) => {
      const before = change.before.effects[index];
      if (!before || before.effect_id !== effect.effect_id || before.value !== effect.value)
        lines.push(prefix + " " + "词条" + " " + (index + 1) + "：" +
          effectLabel(before?.effect_id ?? EMPTY_EFFECT) + " " + (before?.value ?? 0) + " → " +
          effectLabel(effect.effect_id) + " " + effect.value);
    });
  }
  return <ul className="character-plan-lines">{lines.map(line => <li key={line}>{line}</li>)}</ul>;
}

export function CharacterEditor() {
  const [mode, setMode] = useState<Mode>("live");
  const [character, setCharacter] = useState<Character | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [currencyDraft, setCurrencyDraft] = useState<Record<Currency, string>>({ amrita: "", gold: "" });
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<number | null>(null);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [modded, setModded] = useState(false);
  const [plan, setPlan] = useState<{ plan_id: string; preview: Record<string, unknown> } | null>(null);
  const [catalog, setCatalog] = useState<ActiveLocalCatalog | null>(null);
  const save = useSyncExternalStore(
    saveSession ? saveSession.subscribe : () => () => {},
    saveSession ? saveSession.getSnapshot : () => null,
  );

  const row = character?.equipment.find(entry => entry.slot_index === selected) ?? null;
  const rows = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const all = character?.equipment ?? [];
    if (!needle) return all.slice(0, 300);
    return all
      .filter(entry => {
        const name = catalog?.entries.get(entry.item_id) ?? "";
        const text = [hex(entry.item_id), name, ...entry.effects.map(effect => effectLabel(effect.effect_id))].join(" ");
        return text.toLowerCase().includes(needle);
      })
      .slice(0, 300);
  }, [character, query, catalog]);

  function adopt(next: Character) {
    setCharacter(next);
    setCurrencyDraft({
      amrita: next.currencies.amrita == null ? "" : String(next.currencies.amrita),
      gold: next.currencies.gold == null ? "" : String(next.currencies.gold),
    });
    const kept = next.equipment.find(entry => entry.slot_index === selected);
    setDraft(kept ? draftOf(kept) : null);
    if (!kept) setSelected(null);
  }

  async function run(task: () => Promise<void>) {
    setBusy(true);
    setMessage("");
    try {
      await task();
    } catch (error) {
      setMessage(String(error instanceof Error ? error.message : error));
    } finally {
      setBusy(false);
    }
  }

  function load(target: Mode = mode) {
    return run(async () => {
      setPlan(null);
      if (target === "live") {
        const result = await window.operations.execute({ method: "runtime.character_snapshot", params: {} });
        if (!result || !("source" in result) || result.source !== "runtime") throw new Error("UNEXPECTED_CHARACTER_SNAPSHOT");
        adopt({ ...result, mode: "live" });
      } else {
        const selectedSave = saveSession?.getSnapshot().selected;
        if (!selectedSave) throw new Error("请先选择存档。");
        const result = await saveObserver!.run(() =>
          window.operations.execute({ method: "save.character", params: { save_id: selectedSave.save_id } }),
        );
        if (!result || !("equipment_slots" in result) || !("save_id" in result)) throw new Error("UNEXPECTED_CHARACTER_SNAPSHOT");
        adopt({ ...(result as SaveCharacter), mode: "save" });
      }
    });
  }

  function switchMode(next: Mode) {
    setMode(next);
    setCharacter(null);
    setSelected(null);
    setDraft(null);
    setPlan(null);
    setMessage("");
  }

  async function submit(edit: { currencies?: Record<string, number>; equipment?: { slot_index: number; patch: Record<string, unknown> }[] }) {
    if (!character) return;
    if (character.mode === "live") {
      const live = character as LiveCharacter & { mode: Mode };
      const equipment = (edit.equipment ?? []).map(item => ({
        ...item,
        expected_record_sha256: live.equipment.find(entry => entry.slot_index === item.slot_index)?.record_sha256 ?? "",
      }));
      const expected: Record<string, number> = {};
      for (const key of Object.keys(edit.currencies ?? {})) expected[key] = Number(live.currencies[key as Currency]);
      const result = await runtimeObserver!.run(() =>
        window.operations.execute({
          method: "runtime.character_edit",
          params: {
            process_id: live.process_id,
            ...(edit.currencies ? { currencies: edit.currencies, expected_currencies: expected } : {}),
            ...(equipment.length ? { equipment } : {}),
          },
        }),
      );
      const outcome = result && "character_edit" in result ? result.character_edit : null;
      if (!outcome) throw new Error("UNEXPECTED_CHARACTER_EDIT");
      if (outcome.state === "verified") setMessage("已写入游戏。到神社存档即可保存到存档文件。");
      else if (outcome.state === "rejected") setMessage("没有写入：" + (outcome.error ?? ""));
      else setMessage("写入结果不确定，请重新读取后核对：" + (outcome.error ?? ""));
      const refreshed = await window.operations.execute({ method: "runtime.character_snapshot", params: {} });
      if (refreshed && "source" in refreshed && refreshed.source === "runtime") adopt({ ...refreshed, mode: "live" });
    } else {
      const result = await saveSession!.prepareCharacterEdit(edit);
      if (!("plan_id" in result)) throw new Error("UNEXPECTED_CHARACTER_PLAN");
      setPlan({ plan_id: result.plan_id, preview: result.preview as Record<string, unknown> });
      setMessage("已生成修改计划。核对下方内容后点击“写入存档”。游戏必须关闭。");
    }
  }

  function applyCurrencies() {
    return run(async () => {
      if (!character) return;
      const currencies: Record<string, number> = {};
      for (const key of CURRENCIES) {
        const value = parseAmount(currencyDraft[key], 9007199254740991);
        if (value === null) throw new Error("请输入有效的数值。");
        if (value !== character.currencies[key]) currencies[key] = value;
      }
      if (!Object.keys(currencies).length) throw new Error("没有需要修改的内容。");
      await submit({ currencies });
    });
  }

  function applyEquipment() {
    return run(async () => {
      if (!row || !draft) return;
      if (!modded) throw new Error("请先勾选确认这是魔改修改。");
      const result = patchOf(row, draft);
      if ("error" in result) throw new Error(result.error);
      if (!Object.keys(result.patch).length) throw new Error("没有需要修改的内容。");
      await submit({ equipment: [{ slot_index: row.slot_index, patch: result.patch }] });
    });
  }

  function commitPlan() {
    return run(async () => {
      if (!plan) return;
      const receipt = await saveSession!.commit(plan.plan_id);
      setPlan(null);
      setMessage(
        receipt.commit_status === "committed"
          ? "已写入存档，并已自动备份原存档。"
          : "写入结果不确定，请在备份与管理中核对操作结果。",
      );
      await saveSession!.refresh();
      await load("save");
    });
  }

  if (!desktop) return <main className="equipment-page"><Notice text="请在桌面版中使用此功能。" /></main>;
  return (
    <main className="equipment-page character-page">
      <div className="character-modes" role="tablist">
        <button className={mode === "live" ? "active" : ""} onClick={() => switchMode("live")} disabled={busy}>游戏内实时修改</button>
        <button className={mode === "save" ? "active" : ""} onClick={() => switchMode("save")} disabled={busy}>修改存档文件</button>
      </div>
      <p className="equipment-description">
        {mode === "live"
          ? "直接修改正在运行的游戏，需要先读档进入游戏。修改后到神社存档即可保存。"
          : "修改存档文件，游戏必须关闭。写入前会自动备份原存档。"}
      </p>
      {mode === "save" && <SavePicker compact />}
      <div className="equipment-toolbar">
        <button onClick={() => load()} disabled={busy || (mode === "save" && !save?.selected)}>
          {character ? "重新读取" : "读取角色"}
        </button>
      </div>
      <Notice text={message} />
      {character && (
        <>
          <section className="character-currencies">
            <h3>货币</h3>
            <label>
              <span>精华</span>
              <input inputMode="numeric" value={currencyDraft.amrita} onChange={event => setCurrencyDraft({ ...currencyDraft, amrita: event.target.value })} />
            </label>
            <label>
              <span>持有金钱</span>
              <input inputMode="numeric" value={currencyDraft.gold} onChange={event => setCurrencyDraft({ ...currencyDraft, gold: event.target.value })} />
            </label>
            <button onClick={applyCurrencies} disabled={busy}>修改货币</button>
          </section>
          <section className="character-equipment">
            <h3>装备</h3>
            <div className="equipment-toolbar">
              <label className="equipment-search">
                <span>搜索</span>
                <input value={query} onChange={event => setQuery(event.target.value)} placeholder="物品 ID、名称或词条" />
              </label>
              <span className="equipment-range">
                {character.equipment.length} / {character.equipment_slots}
              </span>
            </div>
            <LocalCatalogImport onCatalogChange={setCatalog} />
            <table className="equipment-table">
              <thead>
                <tr><th>槽位</th><th>物品</th><th>等级</th><th>+值</th><th>稀有度</th><th>词条</th></tr>
              </thead>
              <tbody>
                {rows.map(entry => (
                  <tr key={entry.slot_index} className={entry.slot_index === selected ? "selected" : ""}>
                    <td><button onClick={() => { setSelected(entry.slot_index); setDraft(draftOf(entry)); setModded(false); }}>{entry.slot_index}</button></td>
                    <td><code>{hex(entry.item_id)}</code>{catalog?.entries.get(entry.item_id) && <span className="equipment-item-name">{catalog.entries.get(entry.item_id)}</span>}</td>
                    <td>{entry.level}</td>
                    <td>{entry.plus}</td>
                    <td>{entry.rarity}</td>
                    <td>{entry.effects.filter(effect => effect.effect_id !== EMPTY_EFFECT).map(effect => effectLabel(effect.effect_id)).join("、")}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            {row && draft && (
              <div className="equipment-detail character-detail">
                <h3>编辑装备</h3>
                <div className="character-fields">
                  {([
                    ["level", "等级"],
                    ["level_before_forge", "锻造前等级"],
                    ["plus", "+值"],
                    ["rarity", "稀有度"],
                    ["familiarity", "爱用度"],
                  ] as const).map(([key, label]) => (
                    <label key={key}>
                      <span>{label}</span>
                      <input inputMode="numeric" value={draft[key]} onChange={event => setDraft({ ...draft, [key]: event.target.value })} />
                    </label>
                  ))}
                </div>
                <datalist id="character-effect-names">
                  {effectOptions.map(option => <option key={option} value={option} />)}
                </datalist>
                <table className="equipment-effects">
                  <thead><tr><th>#</th><th>词条</th><th>原始数值</th><th>当前</th></tr></thead>
                  <tbody>
                    {draft.effects.map((effect, index) => (
                      <tr key={index}>
                        <td>{index + 1}</td>
                        <td><input list="character-effect-names" value={effect.id} placeholder="留空表示空槽" onChange={event => {
                          const effects = draft.effects.slice();
                          effects[index] = { ...effect, id: event.target.value };
                          setDraft({ ...draft, effects });
                        }} /></td>
                        <td><input inputMode="numeric" value={effect.value} onChange={event => {
                          const effects = draft.effects.slice();
                          effects[index] = { ...effect, value: event.target.value };
                          setDraft({ ...draft, effects });
                        }} /></td>
                        <td>{effectLabel(row.effects[index]?.effect_id ?? EMPTY_EFFECT)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <p className="equipment-notes">
                  原始数值按游戏内部单位填写，例如百分比词条 15 表示 1.5%。
                </p>
                <label className="character-modded">
                  <input type="checkbox" checked={modded} onChange={event => setModded(event.target.checked)} />
                  <span>我了解这是魔改修改：结果不受游戏生成规则约束，可能无法自然获得。</span>
                </label>
                <button onClick={applyEquipment} disabled={busy || !modded}>修改装备</button>
              </div>
            )}
          </section>
          {plan && (
            <section className="character-plan">
              <h3>修改计划</h3>
              <PlanPreview preview={plan.preview} />
              <button onClick={commitPlan} disabled={busy}>写入存档</button>
              <button onClick={() => { setPlan(null); void saveSession!.discard(); }} disabled={busy}>放弃</button>
            </section>
          )}
        </>
      )}
    </main>
  );
}
