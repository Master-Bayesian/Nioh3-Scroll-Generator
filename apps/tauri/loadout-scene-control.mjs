/** Exercise the public picker in the same order as a player. */
export async function selectLoadoutScene(page, descriptor) {
  const weapon = JSON.stringify([descriptor.school, descriptor.selection_weapon_class ?? descriptor.weapon_class ?? descriptor.weapon]);
  const choices = [['流派', descriptor.school], ['武器类型', weapon], ['段位', descriptor.scene.stance],
    ['攻击', descriptor.scene.attack_id], ['增益条件', descriptor.id]];
  for (const [label, value] of choices) {
    const control = page.getByRole('combobox', {name:label, exact:true});
    if (await control.inputValue() !== value) await control.selectOption(value);
  }
}
