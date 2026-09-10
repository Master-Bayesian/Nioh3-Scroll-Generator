/** Keep the base Japanese spelling, without the game's font/ruby instructions. */
export function plainGameText(text: string): string {
  return text.replace(/\^(?:20|21)~default~|\^FE~RUBY~|\^FF~RUBY,[^~]*~/gi, "");
}
