import React from "react";
import { localize } from "./presentation";
/** Translate only rendered DOM text. Query values, keys and callbacks stay intact. */
export function localizedElement(
  type: React.ElementType,
  props: any,
  ...children: any[]
): React.ReactElement {
  if (typeof type !== "string")
    return React.createElement(type, props, ...children);
  const translateChild = (value: any): any =>
    typeof value === "string"
      ? localize(value)
      : Array.isArray(value)
        ? value.map(translateChild)
        : value;
  const rendered = { ...props };
  for (const name of ["aria-label", "title", "placeholder", "alt"])
    if (typeof rendered[name] === "string")
      rendered[name] = localize(rendered[name]);
  // A translated option must not change its implicit form value.
  if (
    type === "option" &&
    rendered.value === undefined &&
    children.every((v) => typeof v === "string" || typeof v === "number")
  )
    rendered.value = children.join("");
  return React.createElement(type, rendered, ...children.map(translateChild));
}
