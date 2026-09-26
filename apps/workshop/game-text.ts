// The slot of a native sentence template, with the state word that follows an
// ailment slot in Chinese ("陷入…状态时"), which the generic word already carries.
const TEMPLATE_SLOT = /\^09~(BUFF|DEBUFF)~\{\}\^09~~(?:状态)?/g;

/** Whether a native string still carries an unfilled buff/ailment argument. */
export function hasTemplateSlot(text: string): boolean {
  return /\^09~(?:BUFF|DEBUFF)~|\{\}/.test(text);
}

/**
 * Name the unfilled argument of a native sentence template generically. The
 * game fills it from a parameter row this tool does not resolve, so the raw
 * control markup is never shown.
 */
export function fillTemplateSlots(text: string, buff: string, ailment: string): string {
  return text.replace(TEMPLATE_SLOT, (slot) => (slot.includes("~BUFF~") ? buff : ailment));
}

/** Keep the base Japanese spelling, without the game's font/ruby instructions. */
export function plainGameText(text: string): string {
  return text.replace(/\^(?:20|21)~default~|\^FE~RUBY~|\^FF~RUBY,[^~]*~/gi, "");
}
