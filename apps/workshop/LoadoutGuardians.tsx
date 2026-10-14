import { EffectLabel, EffectValue, effectText } from "./effect-presentation";
import React, { useEffect, useState } from "react";
import { nameLabel } from "./equipment-facets";
import { localize, localizeName } from "./presentation";
import { PickerDialog, type SelectionSetter } from "./LoadoutPickers";
import { GUARDIAN_LEVEL_MAX, guardianEffects, guardiansForStyle, setDestroyedSpikeCount, setGuardian, type LoadoutSelection, type LoadoutStyle } from "./loadout-state";

function GuardianNumber({ label, value, minimum, maximum, optional, disabled, title, change }: {
  label: string; value: number | null; minimum: number; maximum: number;
  optional?: boolean; disabled?: boolean; title?: string; change: (value: number | null) => void;
}) {
  const [draft, setDraft] = useState(value === null ? "" : String(value));
  useEffect(() => setDraft(value === null ? "" : String(value)), [value]);
  const commit = () => {
    const parsed = draft.trim() === "" ? (optional ? null : value) : Number(draft);
    const next = parsed === null ? null : Number.isFinite(parsed)
      ? Math.max(minimum, Math.min(maximum, Math.trunc(parsed))) : value;
    setDraft(next === null ? "" : String(next)); change(next);
  };
  return <label className="loadout-guardian-number"><span>{label}</span><input
    type="number" min={minimum} max={maximum} step={1} aria-label={label} title={title}
    placeholder={disabled ? "—" : optional ? "未设置" : undefined} disabled={disabled} value={draft}
    onChange={event => {
      const text = event.currentTarget.value; setDraft(text);
      if (text.trim() === "") { if (optional) change(null); return; }
      const number = Number(text);
      if (Number.isInteger(number) && number >= minimum && number <= maximum) change(number);
    }} onBlur={commit} onKeyDown={event => {
      if (event.key === "Enter") { commit(); event.currentTarget.blur(); }
      else if (event.key === "Escape") setDraft(value === null ? "" : String(value));
    }} /></label>;
}

function GuardianEffectList({ effects }: { effects: ReturnType<typeof guardianEffects> }) {
  return <ul className="loadout-guardian-effects">{effects.map(effect => <li
    key={effect.slot} data-guardian-effect={effect.slot} data-activation={effect.activation}
    title={effectText(effect.description_zh)}>
    <span>{<EffectLabel text={effect.label_zh} />}</span><EffectValue display={effect.display} />
    {effect.activation_threshold > 0 && <small>{localize("所需楔数")} {effect.activation_threshold} · {localize(
      effect.activation === "active" ? "已启用" : effect.activation === "locked" ? "未启用" : "待确认")}</small>}
  </li>)}</ul>;
}

export function GuardianPanel({ style, selection, setSelection }: {
  style: LoadoutStyle; selection: LoadoutSelection; setSelection: SelectionSetter;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const choice = selection.guardians[style];
  const roster = guardiansForStyle(style);
  const chosen = choice && roster.find(row => row.guardian_id === choice.guardianId);
  const effects = guardianEffects(selection, style);
  const matching = roster.filter(row => [row.name_zh, localizeName(row.name_zh) ?? "",
    ...row.slots.flatMap(slot => [slot.label_zh, effectText(slot.label_zh)])]
    .join(" ").toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));
  const choose = (guardianId: number | null) => {
    setSelection(current => setGuardian(current, style, guardianId === null ? null : {
      guardianId, level: current.guardians[style]?.level ?? 1,
    })); setOpen(false);
  };
  return <section className="loadout-subsection loadout-guardian-section" aria-labelledby="loadout-guardian-heading">
    <h3 id="loadout-guardian-heading">守护灵 <span className="loadout-section-meta">{style === "samurai" ? "武士" : "忍者"}</span></h3>
    <div className="loadout-guardian-choice" data-empty={!chosen}>
      <button type="button" data-guardian-select aria-label="选择守护灵"
        title={chosen ? localizeName(chosen.name_zh) ?? chosen.name_zh : localize("选择守护灵")}
        onClick={() => { setQuery(""); setOpen(true); }}>
        <span className="loadout-guardian-name">{!chosen && <span className="loadout-guardian-add" aria-hidden="true">+</span>}{chosen ? nameLabel(chosen.name_zh) : "选择守护灵"}</span>
        <span className="loadout-guardian-action" aria-hidden="true">{chosen && "更换"}<span>›</span></span>
      </button>
    </div>
    <div className="loadout-guardian-metrics">
      <GuardianNumber label="守护灵等级" value={choice?.level ?? null} minimum={1} maximum={GUARDIAN_LEVEL_MAX} disabled={!choice}
        title="游戏内等级范围为 1–50。"
        change={level => { if (level !== null) setSelection(current => {
          const active = current.guardians[style];
          return active ? setGuardian(current, style, { ...active, level }) : current;
        }); }} />
      <GuardianNumber label="地狱之楔破坏数" value={selection.destroyedSpikeCount} minimum={0} maximum={65535} optional
        title="按角色实际进度填写，两个风格共用；未填写时仅计入无条件效果。"
        change={count => setSelection(current => setDestroyedSpikeCount(current, count))} />
    </div>
    {!!effects.length && <details className="loadout-guardian-details"><summary>特殊效果</summary><GuardianEffectList effects={effects} /></details>}
    {open && <PickerDialog title="选择守护灵" close={() => setOpen(false)}>
      <input aria-label="搜索守护灵或词条" placeholder="搜索守护灵或词条" value={query} onChange={event => setQuery(event.target.value)} />
      <p className="loadout-deferred-note">仅显示当前风格的守护灵；同名形态按词条区分。</p>
      <button type="button" onClick={() => choose(null)}>清空栏位</button>
      <div className="loadout-picker-list loadout-guardian-list">{matching.map(row => <button type="button" key={row.guardian_id} data-guardian-id={row.guardian_id}
        className={choice?.guardianId === row.guardian_id ? "selected" : ""} onClick={() => choose(row.guardian_id)}>
        <strong title={`0x${row.guardian_id.toString(16).padStart(4, "0")}`}>{nameLabel(row.name_zh)}</strong>
        <GuardianEffectList effects={guardianEffects(setGuardian(selection, style, {
          guardianId: row.guardian_id, level: choice?.level ?? 1,
        }), style)} />
      </button>)}</div>
      {!matching.length && <p role="status">没有匹配的守护灵</p>}
    </PickerDialog>}
  </section>;
}
