import React, { useEffect, useMemo, useRef, useState } from "react";
import { localize } from "./presentation";

/** The effect id that marks an empty slot. */
export const EMPTY_EFFECT = 0xffffffff;
export interface PickerCandidate {
  id: number;
  star?: boolean;
}
const searchable = (text: string) => text + " " + localize(text);
export function hex(value: number) {
  return "0x" + value.toString(16).toUpperCase().padStart(4, "0");
}
/** An effect field accepts `0xA166`, `41318`, or a picker entry that starts with either. */
export function parseEffectId(text: string): number | null {
  const token = text.trim().split(/\s+/)[0] ?? "";
  if (!token) return EMPTY_EFFECT;
  const value = /^0x[0-9a-f]+$/i.test(token) ? Number.parseInt(token, 16) : /^\d+$/.test(token) ? Number(token) : NaN;
  return Number.isInteger(value) && value >= 0 && value <= EMPTY_EFFECT ? value : null;
}

/**
 * Effect picker: opening it always lists every candidate; only text typed after
 * opening filters the list, so the current effect never hides the others.
 */
export function EffectPicker<C extends PickerCandidate>({ value, star, candidates, label, onPick, placeholder = "（空）" }: {
  value: string;
  /** The saved star marker of the current effect, when it is unchanged. */
  star?: boolean;
  candidates: C[];
  label: (candidate: C) => string;
  onPick: (candidate: C | PickerCandidate) => void;
  placeholder?: string;
}) {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState("");
  const box = useRef<HTMLDivElement>(null);
  const current = parseEffectId(value);
  const currentLabel =
    current === null
      ? value
      : current === EMPTY_EFFECT
        ? ""
        : label({ ...(candidates.find(candidate => candidate.id === current) ?? { id: current }), ...(star === undefined ? {} : { star }) } as C);
  const shown = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    const list = needle
      ? candidates.filter(candidate => searchable(label(candidate) + " " + hex(candidate.id)).toLowerCase().includes(needle))
      : candidates;
    return list.slice(0, 400);
  }, [candidates, filter, label]);
  useEffect(() => {
    if (!open) return;
    const close = (event: MouseEvent) => {
      if (box.current && !box.current.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);
  return (
    <div className="effect-picker" ref={box}>
      <input
        value={open ? filter : localize(currentLabel)}
        placeholder={open ? "输入名称筛选" : placeholder}
        onFocus={() => { setFilter(""); setOpen(true); }}
        onChange={event => setFilter(event.target.value)}
        onKeyDown={event => {
          if (event.key === "Escape") setOpen(false);
          if (event.key === "Enter" && shown[0]) { onPick(shown[0]); setOpen(false); }
        }}
      />
      {open && (
        <ul className="effect-picker-list" role="listbox">
          <li><button type="button" onMouseDown={() => { onPick({ id: EMPTY_EFFECT }); setOpen(false); }}>（空）</button></li>
          {shown.map(candidate => (
            <li key={candidate.id}>
              <button type="button" className={candidate.id === current ? "current" : ""}
                onMouseDown={() => { onPick(candidate); setOpen(false); }}>
                {label(candidate)}
              </button>
            </li>
          ))}
          {shown.length === 0 && <li className="effect-picker-empty">没有匹配的词条</li>}
        </ul>
      )}
    </div>
  );
}
