// The slot of a native sentence template, with the state word that follows an
// ailment slot in Chinese ("陷入…状态时"). The slot is empty ("{}") as the game
// stores it, or carries the argument this tool resolved ("{毒}").
const TEMPLATE_SLOT = /\^09~(BUFF|DEBUFF)~\{([^}]*)\}\^09~~(状态)?/g;
// A bare slot names a ninjutsu, onmyo magic or martial skill ("{}计量槽增加量");
// only a resolved one ("{怪风}") is filled here.
const RESOLVED_BARE_SLOT = /(?<!~)\{([^{}":]+)\}/g;

/** Whether a native string still carries an unfilled buff/ailment argument. */
export function hasTemplateSlot(text: string): boolean {
  return /\^09~(?:BUFF|DEBUFF)~|\{\}/.test(text);
}

/**
 * Fill the arguments of a native sentence template. A resolved argument is the
 * game's own name; an unresolved buff or ailment is named generically (the
 * generic ailment word already carries "状态"), so the raw control markup is
 * never shown. Unresolved bare slots stay "{}" for the caller to word.
 */
export function fillTemplateSlots(text: string, buff: string, ailment: string): string {
  return text
    .replace(TEMPLATE_SLOT, (_slot, kind: string, argument: string, state = "") =>
      argument ? argument + state : kind === "BUFF" ? buff : ailment,
    )
    .replace(RESOLVED_BARE_SLOT, "$1");
}

/** The template with its resolved arguments removed again ("{毒}" -> "{}"). */
export function withoutTemplateArguments(text: string): string {
  return text.replace(/(\^09~(?:BUFF|DEBUFF)~)\{[^}]*\}/g, "$1{}").replace(RESOLVED_BARE_SLOT, "{}");
}

/** Write a resolved argument into every empty slot of a native template. */
export function withTemplateArgument(text: string, argument: string): string {
  return text.replace(/\{\}/g, `{${argument}}`);
}

/** Keep the base Japanese spelling, without the game's font/ruby instructions. */
export function plainGameText(text: string): string {
  return text.replace(/\^(?:20|21)~default~|\^FE~RUBY~|\^FF~RUBY,[^~]*~/gi, "");
}
