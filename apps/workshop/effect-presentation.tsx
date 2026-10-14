import React from "react";
import { localize } from "./presentation";
import { equipmentEffectDetail } from "./loadout-effect-summary";

/** Effects carry native sentence arguments; catalog-name rendering must not bypass them. */
export function effectText(text: string): string {
  return localize(text).replaceAll("{}", localize("（对象待确认）"));
}

/** Keep already localized effect text out of the JSX substring translator. */
export function EffectLabel({ text }: { text: string }) {
  return React.createElement("span", null, effectText(text));
}

/** Empty values represent mechanisms, while unknown values remain explicit. */
export function EffectValue({ display, raw }: { display: string | null; raw?: number }) {
  if (display === "") return null;
  return React.createElement("strong", {
    title: display === null && raw !== undefined ? localize(`原始数值：${raw}；单位尚未解析`) : undefined,
  }, display ?? localize("数值待确认"));
}

export function equipmentEffectPreview(effectId: number, raw: number): string {
  const effect = equipmentEffectDetail(effectId, raw);
  return [effectText(effect.label), effect.display ?? localize("数值待确认")].filter(Boolean).join(" ");
}

export function EffectPreview({ effects }: { effects: { effectId: number; raw: number }[] }) {
  const text = effects.filter(effect => effect.effectId !== 0xffffffff)
    .map(effect => equipmentEffectPreview(effect.effectId, effect.raw)).join(" / ");
  return React.createElement("span", { className: "loadout-option-effects", title: text }, text);
}
