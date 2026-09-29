// The slot of a native sentence template, with the state word that follows an
// ailment slot in Chinese ("陷入…状态时"). The slot is empty ("{}") as the game
// stores it, or carries the argument this tool resolved ("{毒}").
const TEMPLATE_SLOT = /\^09~(BUFF|DEBUFF)~\{([^}]*)\}\^09~~(状态)?/g;

/** Whether a native string still carries an unfilled buff/ailment argument. */
export function hasTemplateSlot(text: string): boolean {
  return /\^09~(?:BUFF|DEBUFF)~|\{\}/.test(text);
}

/**
 * Fill the argument of a native sentence template. A resolved argument is the
 * game's own buff or ailment name; an unresolved one is named generically (the
 * generic ailment word already carries "状态"), so the raw control markup is
 * never shown.
 */
export function fillTemplateSlots(text: string, buff: string, ailment: string): string {
  return text.replace(TEMPLATE_SLOT, (_slot, kind: string, argument: string, state = "") =>
    argument ? argument + state : kind === "BUFF" ? buff : ailment,
  );
}

/** The template with its resolved arguments removed again ("{毒}" -> "{}"). */
export function withoutTemplateArguments(text: string): string {
  return text.replace(/(\^09~(?:BUFF|DEBUFF)~)\{[^}]*\}/g, "$1{}");
}

/** Write a resolved argument into every empty slot of a native template. */
export function withTemplateArgument(text: string, argument: string): string {
  return text.replace(/(\^09~(?:BUFF|DEBUFF)~)\{\}/g, (_slot, head: string) => `${head}{${argument}}`);
}

/** Keep the base Japanese spelling, without the game's font/ruby instructions. */
export function plainGameText(text: string): string {
  return text.replace(/\^(?:20|21)~default~|\^FE~RUBY~|\^FF~RUBY,[^~]*~/gi, "");
}
