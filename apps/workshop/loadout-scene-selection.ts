import type { LoadoutBundles } from "../../packages/contracts/protected-responses";

export type LoadoutBundle = LoadoutBundles["bundles"][number];
export type LoadoutScene = NonNullable<LoadoutBundle["scene"]>;
export type SelectableLoadoutBundle = LoadoutBundle & { scene: LoadoutScene };
export type LoadoutSlot = 0 | 4 | 5 | 6 | 7 | 8 | 9 | 10;

/** A label is not weapon identity. The form and native TYPE define the pool. */
export function sceneWeaponKey(bundle: LoadoutBundle): string {
  return JSON.stringify([bundle.school, bundle.selection_weapon_class ?? bundle.weapon_class ?? bundle.weapon]);
}

/** Keep the selected attack/buff whenever that combination exists in the next group. */
export function preferredScene(
  choices: SelectableLoadoutBundle[], current?: SelectableLoadoutBundle,
): SelectableLoadoutBundle | undefined {
  if (!current) return choices[0];
  const sameAttack = (value: SelectableLoadoutBundle) => value.scene.attack_id === current.scene.attack_id;
  const sameKind = (value: SelectableLoadoutBundle) => value.scene.kind === current.scene.kind;
  const sameStance = (value: SelectableLoadoutBundle) => value.scene.stance === current.scene.stance;
  const sameBuff = (value: SelectableLoadoutBundle) => value.scene.buff === current.scene.buff;
  return choices.find(value => sameAttack(value) && sameBuff(value))
    ?? choices.find(sameAttack)
    ?? choices.find(value => sameStance(value) && sameKind(value) && sameBuff(value))
    ?? choices.find(value => sameKind(value) && sameBuff(value))
    ?? choices.find(sameBuff)
    ?? choices[0];
}

/** Mirrors protected slot_allows: unknown display names cannot drop legal owned items. */
export function nativeLoadoutSlot(bundle: LoadoutBundle, typeClass: number | null | undefined): LoadoutSlot | null {
  if (typeClass == null || !Number.isSafeInteger(typeClass) || typeClass < 0) return null;
  const requestedType = bundle.selection_weapon_class ?? bundle.weapon_class;
  if (requestedType != null && typeClass === requestedType) return 0;
  if (typeClass >= 24 && typeClass <= 38) return (Math.floor((typeClass - 24) / 3) + 4) as LoadoutSlot;
  if (typeClass === (bundle.school === "ninja" ? 40 : 39)) return 9;
  return null;
}
