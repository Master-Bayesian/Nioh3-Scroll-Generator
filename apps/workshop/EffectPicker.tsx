import React, { useEffect, useId, useMemo, useRef, useState } from "react";
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
export function EffectPicker<C extends PickerCandidate>({ value, star, candidates, label, onPick, accessibleName, placeholder = "（空）" }: {
  value: string;
  /** The saved star marker of the current effect, when it is unchanged. */
  star?: boolean;
  candidates: C[];
  label: (candidate: C) => string;
  onPick: (candidate: C | PickerCandidate) => void;
  accessibleName: string;
  placeholder?: string;
}) {
  const [open, setOpen] = useState(false);
  const [filter, setFilter] = useState("");
  const [activeIndex, setActiveIndex] = useState<number | null>(null);
  const listId = useId();
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
  const options: (C | PickerCandidate)[] = [{ id: EMPTY_EFFECT }, ...shown];
  const activeOption = open && activeIndex !== null ? options[activeIndex] : undefined;
  const optionId = (index: number) => listId + "-option-" + index;

  function close() {
    setOpen(false);
    setActiveIndex(null);
  }

  function reveal() {
    if (open) return;
    setFilter("");
    setOpen(true);
  }

  function pick(candidate: C | PickerCandidate) {
    onPick(candidate);
    close();
  }

  useEffect(() => {
    if (open && activeIndex !== null) {
      box.current?.querySelectorAll('[role="option"]')[activeIndex]?.scrollIntoView({ block: "nearest" });
    }
  }, [open, activeIndex]);
  return (
    <div className="effect-picker" ref={box}>
      <input
        role="combobox"
        aria-label={accessibleName}
        aria-autocomplete="list"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        aria-activedescendant={activeOption && activeIndex !== null ? optionId(activeIndex) : undefined}
        value={open ? filter : localize(currentLabel)}
        placeholder={open ? "输入名称筛选" : placeholder}
        onFocus={reveal}
        onClick={reveal}
        onBlur={close}
        onChange={event => {
          setFilter(event.target.value);
          setActiveIndex(null);
          setOpen(true);
        }}
        onKeyDown={event => {
          if (event.nativeEvent.isComposing || event.nativeEvent.keyCode === 229) return;
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            const last = options.length - 1;
            const forward = event.key === "ArrowDown";
            if (!open) {
              setFilter("");
              setOpen(true);
              setActiveIndex(forward ? 0 : Math.min(candidates.length, 400));
              return;
            }
            setActiveIndex(index => forward
              ? Math.min((index ?? -1) + 1, last)
              : Math.max((index ?? last + 1) - 1, 0));
          }
          if (event.key === "Escape" && open) {
            event.preventDefault();
            event.stopPropagation();
            close();
          }
          const candidate = activeOption ?? shown[0];
          if (event.key === "Enter" && open && candidate) {
            event.preventDefault();
            pick(candidate);
          }
        }}
      />
      {open && (
        <ul id={listId} className="effect-picker-list" role="listbox" aria-label={accessibleName}>
          {options.map((candidate, index) => (
            <li key={index === 0 ? "empty" : candidate.id} role="presentation">
              <button
                id={optionId(index)}
                type="button"
                role="option"
                tabIndex={-1}
                aria-selected={activeIndex === null ? candidate.id === current : activeIndex === index}
                className={activeIndex === index ? "keyboard-active" : candidate.id === current ? "current" : ""}
                onMouseDown={event => event.preventDefault()}
                onClick={() => pick(candidate)}
              >
                {index === 0 ? "（空）" : label(candidate as C)}
              </button>
            </li>
          ))}
          {shown.length === 0 && <li className="effect-picker-empty" role="presentation">没有匹配的词条</li>}
        </ul>
      )}
    </div>
  );
}
