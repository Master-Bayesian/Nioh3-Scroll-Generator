import React, { useEffect, useState } from "react";
import { localize, useUiLocale } from "./presentation";
import "./loadout-planner.css";

import { ClanPanel, NativeSkillPanel, PlannerEquipmentRows, PlannerScrollRow, type SelectionSetter } from "./LoadoutPickers";
import type { Sample } from "./model";
import { GuardianPanel } from "./LoadoutGuardians";
import { type LoadoutSelection, type LoadoutStyle } from "./loadout-state";
import { LoadoutEffectSummary } from "./LoadoutEffectSummary";
import { LoadoutSetSummary } from "./LoadoutSetSummary";
import { SoulCorePanel } from "./LoadoutSoulCores";
import { LoadoutWarnings } from "./LoadoutWarnings";
import { LoadoutSaveConfig } from "./LoadoutSaveConfig";
import { LoadoutDraftPicker } from "./LoadoutDraftPicker";
import { detailSlots, LoadoutEquipmentDetail } from "./LoadoutEquipmentDetail";
import { setArmorRemodel } from "./loadout-remodel";
import { loadoutStats, statRangeLabel } from "./loadout-stats";
import { RepeatButton } from "./RepeatButton";
import type { SavedLoadoutImport } from "./loadout-save-config";
export type { LoadoutStyle } from "./loadout-state";

type EquipmentRow = {
  slot: string;
  localizedSlot?: string;
};
type EquipmentGroup = { title: string; rows: EquipmentRow[] };
const equipmentSlotKeys: Record<string, string[]> = {
  "近战武器": ["melee0", "melee1"],
  "远程武器": ["ranged"],
  "防具": ["head", "body", "arms", "legs", "feet"],
  "饰品": ["accessory0", "accessory1"],
  "绘卷": ["scroll"],
};
const equipmentByStyle: Record<LoadoutStyle, EquipmentGroup[]> = {
  samurai: [
    {
      title: "近战武器",
      rows: [
        { slot: "近战" },
        { slot: "近战" },
      ],
    },
    {
      title: "远程武器",
      rows: [{ slot: "远程" }],
    },
    {
      title: "防具",
      rows: [
        { slot: "头" },
        { slot: "身" },
        {
          slot: "手",
          localizedSlot: "防具手部",
        },
        { slot: "腰" },
        { slot: "脚" },
      ],
    },
    {
      title: "饰品",
      rows: [
        { slot: "饰品" },
        { slot: "饰品" },
      ],
    },
    {
      title: "绘卷",
      rows: [{ slot: "绘卷" }],
    },
  ],
  ninja: [
    {
      title: "近战武器",
      rows: [
        { slot: "近战" },
        { slot: "近战" },
      ],
    },
    {
      title: "远程武器",
      rows: [{ slot: "远程" }],
    },
    {
      title: "防具",
      rows: [
        { slot: "头" },
        { slot: "身" },
        {
          slot: "手",
          localizedSlot: "防具手部",
        },
        { slot: "腰" },
        { slot: "脚" },
      ],
    },
    {
      title: "饰品",
      rows: [
        { slot: "饰品" },
        { slot: "饰品" },
      ],
    },
    {
      title: "绘卷",
      rows: [{ slot: "绘卷" }],
    },
  ],
};

export type LoadoutAttribute = { name: string; value: number };

const ATTRIBUTE_MIN = 5;
const ATTRIBUTE_MAX = 150;
const LEVEL_OFFSET = 35;
const LEVEL_MAX = 450;

export function loadoutAttributeBounds(attributes: LoadoutAttribute[], index: number) {
  const otherPoints = attributes.reduce((total, attribute, position) =>
    total + (position === index ? 0 : attribute.value), 0);
  return {
    minimum: Math.max(ATTRIBUTE_MIN, LEVEL_OFFSET + 1 - otherPoints),
    maximum: Math.min(ATTRIBUTE_MAX, LEVEL_OFFSET + LEVEL_MAX - otherPoints),
  };
}

export const initialLoadoutAttributes: LoadoutAttribute[] = [
  { name: "体", value: 40 },
  { name: "心", value: 35 },
  { name: "刚", value: 40 },
  { name: "武", value: 45 },
  { name: "技", value: 40 },
  { name: "智", value: 35 },
  { name: "咒", value: 45 },
];

function AttributeInput({
  attribute,
  index,
  minimum,
  maximum,
  onChange,
}: {
  attribute: LoadoutAttribute;
  index: number;
  minimum: number;
  maximum: number;
  onChange: (value: number) => void;
}) {
  const [draft, setDraft] = useState(String(attribute.value));
  useEffect(() => setDraft(String(attribute.value)), [attribute.value]);
  const id = `loadout-attribute-${index}`;
  const normalize = () => {
    const number = draft.trim() === "" ? attribute.value : Number(draft);
    return Number.isFinite(number)
      ? Math.min(maximum, Math.max(minimum, Math.trunc(number)))
      : attribute.value;
  };
  const commit = (value = normalize()) => {
    setDraft(String(value));
    onChange(value);
  };
  const step = (delta: number) => commit(Math.min(maximum, Math.max(minimum, normalize() + delta)));

  return (
    <div>
      <dt id={`${id}-label`}>{attribute.name}</dt>
      <dd className="loadout-attribute-control" role="group" aria-labelledby={`${id}-label`}>
        <RepeatButton
          label="减少属性点"
          title={normalize() === minimum && minimum > ATTRIBUTE_MIN ? "已达到等级下限" : "减少属性点"}
          disabled={normalize() === minimum}
          step={() => step(-1)}
        >
          <span aria-hidden="true">−</span>
        </RepeatButton>
        <input
          id={id}
          type="number"
          min={minimum}
          max={maximum}
          step={1}
          inputMode="numeric"
          aria-labelledby={`${id}-label`}
          value={draft}
          onChange={(event) => {
            const value = event.currentTarget.value;
            setDraft(value);
            if (value.trim() === "") return;
            const number = Number(value);
            if (Number.isInteger(number) && number >= minimum && number <= maximum) onChange(number);
          }}
          onBlur={() => commit()}
          onKeyDown={(event) => {
            if (event.key === "ArrowUp" || event.key === "ArrowDown") {
              event.preventDefault();
              step(event.key === "ArrowUp" ? 1 : -1);
            } else if (event.key === "Enter") {
              commit();
              event.currentTarget.blur();
            } else if (event.key === "Escape") {
              setDraft(String(attribute.value));
            }
          }}
        />
        <RepeatButton
          label="增加属性点"
          title={normalize() === maximum && maximum < ATTRIBUTE_MAX ? "已达到等级上限" : "增加属性点"}
          disabled={normalize() === maximum}
          step={() => step(1)}
        >
          <span aria-hidden="true">+</span>
        </RepeatButton>
      </dd>
    </div>
  );
}

export function LoadoutPlanner({
  style,
  onStyleChange,
  attributes,
  onAttributeChange,
  selection,
  setSelection,
  scrollFavorites,
  onImport,
  drafts,
  activeDraftId,
  onDraftChange,
  onCreateDraft,
  onRenameDraft,
  storageError, storageBlocked, onRetrySave,
}: {
  style: LoadoutStyle;
  onStyleChange: (style: LoadoutStyle) => void;
  attributes: LoadoutAttribute[];
  onAttributeChange: (index: number, value: number) => void;
  selection: LoadoutSelection;
  setSelection: SelectionSetter;
  scrollFavorites: Sample[];
  onImport: (value: SavedLoadoutImport) => void;
  drafts: { id: number; name: string }[];
  activeDraftId: number;
  onDraftChange: (id: number) => void;
  onCreateDraft: (name: string) => void;
  onRenameDraft: (name: string) => void;
  storageError: string; storageBlocked: boolean; onRetrySave: () => void;
}) {
  const locale = useUiLocale();
  const [detailSlot, setDetailSlot] = useState<string | null>(null);
  useEffect(() => setDetailSlot(null), [style]);
  const inspectedSlot = detailSlot ?? detailSlots.find(([key]) => key === "scroll" ? selection.scrolls[style] : selection.equipment[style][key])?.[0] ?? "melee0";
  const equipment = equipmentByStyle[style];
  const summary = loadoutStats(selection, style, attributes);
  const resourceTitle = ["常态计算；未计等级同步、未导入的固定加成和临时战斗修正。",
    ...summary.resources.notes, ...summary.resources.reasons].join("；");
  const percentage = summary.ratio ? `${summary.ratio.minimum.toFixed(1)}${summary.ratio.maximum === summary.ratio.minimum ? "" : `–${summary.ratio.maximum.toFixed(1)}`}%` : "待确认";
  const attributeTotal = attributes.reduce((total, attribute) => total + attribute.value, 0);
  const characterLevel = attributeTotal - LEVEL_OFFSET;

  return (
    <main className="loadout-page" lang={locale} aria-label="配装器">
      <div className="loadout-toolbar">
        <div
          className="loadout-style-switch"
          role="group"
          aria-label="角色风格"
        >
          {(
            [
              ["samurai", "武士"],
              ["ninja", "忍者"],
            ] as const
          ).map(([value, label]) => (
            <button
              key={value}
              type="button"
              className={style === value ? "selected" : ""}
              aria-pressed={style === value}
              onClick={() => onStyleChange(value)}
            >
              {label}
            </button>
          ))}
        </div>
        <div className="loadout-toolbar-actions">
          <LoadoutDraftPicker drafts={drafts} activeId={activeDraftId} onSelect={onDraftChange} onCreate={onCreateDraft} onRename={onRenameDraft} />
          <LoadoutSaveConfig onImport={onImport} />
        </div>

      </div>

      {storageError && <div className="loadout-storage-error" role="alert"><span>{storageError}</span><button type="button" onClick={onRetrySave}>{storageBlocked ? "保留原数据并保存当前配装" : "重试保存"}</button></div>}
      <div className="loadout-columns">
        <section
          className="loadout-column loadout-equipment-column"
          aria-labelledby="loadout-equipment-heading"
        >
          <header className="loadout-column-heading">
            <h2 id="loadout-equipment-heading">装备</h2>
            <span>{style === "samurai" ? "武士" : "忍者"}</span>
          </header>
          <div className="loadout-column-content">
            <div className="loadout-equipment-head"><span>装备名称</span><span>套装/恩宠</span></div>
            {equipment.map((group) => (
              <section className="loadout-subsection" key={`${style}-${group.title}`}>
                <h3>{group.title}</h3>
                {group.title === "绘卷" ? <PlannerScrollRow style={style} selection={selection} setSelection={setSelection} favorites={scrollFavorites} onInspect={() => setDetailSlot("scroll")} /> : <PlannerEquipmentRows
                  rows={group.rows.map((row, index) => ({
                    slot: `${localize(locale === "zh-CN" ? row.slot : row.localizedSlot ?? row.slot)}${["近战武器", "饰品"].includes(group.title) ? index + 1 : ""}`,
                    key: equipmentSlotKeys[group.title][index],
                  }))}
                  style={style}
                  selection={selection}
                  setSelection={setSelection}
                  onInspect={setDetailSlot}
                  inspectedSlot={inspectedSlot}
                />}
              </section>
            ))}
            <p className="loadout-deferred-note">点击装备查看详情，点击“更换”选择其他装备。</p>
            <LoadoutEquipmentDetail selection={selection} style={style} slot={inspectedSlot}
              onRemodelChange={mode => setSelection(previous => setArmorRemodel(previous, style, inspectedSlot, mode))} />
          </div>
        </section>

        <div className="loadout-character-column">
          <div className="loadout-column-content">
            <GuardianPanel key={`guardian-${style}`} style={style} selection={selection} setSelection={setSelection} />
            <ClanPanel key={`clans-${style}`} style={style} selection={selection} setSelection={setSelection} />
            <SoulCorePanel style={style} selection={selection} setSelection={setSelection} />
            <section className="loadout-subsection loadout-skills-section" aria-labelledby="loadout-skills-heading">
              <h3 id="loadout-skills-heading">技能</h3>
              <div className="loadout-skill-grid">
                <NativeSkillPanel category="common" style={style} selection={selection} setSelection={setSelection} />
                <NativeSkillPanel key={style} category={style} style={style} selection={selection} setSelection={setSelection} />
              </div>
              <p className="loadout-deferred-note">
                容量与精髓冲突会显示在警告区。
              </p>
            </section>
          </div>
        </div>

        <section
          className="loadout-column loadout-summary-column"
          aria-labelledby="loadout-summary-heading"
        >
          <header className="loadout-column-heading">
            <h2 id="loadout-summary-heading">
              当前配装汇总
            </h2>
            <span>{style === "samurai" ? "武士" : "忍者"}</span>
          </header>
          <div className="loadout-column-content">
            <section className="loadout-subsection loadout-character-status">
              <section aria-labelledby="loadout-attributes-heading">
                <h3 id="loadout-attributes-heading">角色属性</h3>
                <dl className="loadout-attributes">
                  {attributes.map((attribute, index) => (
                    <AttributeInput
                      key={`${activeDraftId}-${style}-${attribute.name}`}
                      attribute={attribute}
                      index={index}
                      {...loadoutAttributeBounds(attributes, index)}
                      onChange={(value) => onAttributeChange(index, value)}
                    />
                  ))}
                </dl>
              </section>
              <section aria-labelledby="loadout-status-heading">
                <h3 id="loadout-status-heading">当前状态</h3>
                <dl className="loadout-summary-metrics">
                  <div>
                    <dt>等级</dt>
                    <dd>{`${characterLevel} / ${LEVEL_MAX}`}</dd>
                  </div>
                  <div>
                    <dt>体力</dt>
                    <dd data-loadout-hp title={resourceTitle}>{statRangeLabel(summary.resources.hp)}</dd>
                  </div>
                  <div>
                    <dt>精力</dt>
                    <dd data-loadout-ki title={resourceTitle}>{statRangeLabel(summary.resources.ki)}</dd>
                  </div>
                  <div>
                    <dt>负重</dt>
                    <dd className="loadout-weight-value" data-loadout-weight title={summary.reasons.join("；")}>
                      <span>{`${statRangeLabel(summary.weight, 10)} / ${statRangeLabel(summary.capacity, 10)}`}</span>
                      <span className="loadout-weight-percent">
                        {`(${percentage})`}
                      </span>
                    </dd>
                  </div>
                  <div>
                    <dt>敏捷</dt>
                    <dd data-loadout-agility title={summary.reasons.join("；")}>{summary.agility ?? "待确认"}</dd>
                  </div>
                  <div>
                    <dt>韧性</dt>
                    <dd data-loadout-toughness title={summary.reasons.join("；")}>{`${summary.toughnessRank ?? "待确认"}（${statRangeLabel(summary.toughness)}）`}</dd>
                  </div>
                </dl>
                {summary.resources.usesAmritaGauge && <label className="loadout-resource-gauge">精华槽 (%)
                  <input type="number" aria-label="精华槽百分比" min={0} max={100} step={0.1} placeholder="未设置"
                    value={selection.amritaGaugePercent ?? ""} onChange={event => {
                      const text = event.currentTarget.value, value = text === "" ? null : Number(text);
                      if (value === null || Number.isFinite(value) && value >= 0 && value <= 100)
                        setSelection(current => ({ ...current, amritaGaugePercent: value }));
                    }} />
                </label>}
              </section>
            </section>
            <LoadoutEffectSummary selection={selection} style={style} attributes={attributes} />
            <LoadoutSetSummary selection={selection} style={style} />
            <LoadoutWarnings selection={selection} style={style} attributes={attributes} />
          </div>
        </section>
      </div>
    </main>
  );
}
