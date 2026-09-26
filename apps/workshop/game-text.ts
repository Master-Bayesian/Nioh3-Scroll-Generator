const TEMPLATE_SLOT = /\^09~(BUFF|DEBUFF)~\{\}\^09~~/g;
const SLOT_WORDS: Record<string, [string, string]> = {
  "zh-CN": ["增益效果", "异常状态"],
  "en-US": ["Buff", "Ailment"],
  "ja-JP": ["強化効果", "状態異常"],
};

/** Whether a native string still carries an unfilled buff/ailment argument. */
export function hasTemplateSlot(text: string): boolean {
  return /\^09~(?:BUFF|DEBUFF)~|\{\}/.test(text);
}

/**
 * Name the unfilled argument of a native sentence template generically. The
 * game fills it from a parameter row this tool does not resolve, so the raw
 * control markup is never shown.
 */
export function fillTemplateSlots(text: string, locale: string): string {
  const [buff, ailment] = SLOT_WORDS[locale] ?? SLOT_WORDS["zh-CN"];
  return text
    .replace(TEMPLATE_SLOT, (_, kind) => (kind === "BUFF" ? buff : ailment))
    // "陷入^09~DEBUFF~{}^09~~状态时" already names the state after the slot.
    .replaceAll("异常状态状态", "异常状态");
}

/** Keep the base Japanese spelling, without the game's font/ruby instructions. */
export function plainGameText(text: string): string {
  return text.replace(/\^(?:20|21)~default~|\^FE~RUBY~|\^FF~RUBY,[^~]*~/gi, "");
}
