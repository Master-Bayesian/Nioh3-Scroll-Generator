import {
  useRef,
  useState,
  type Dispatch,
  type SetStateAction,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { conditionKey, type Query } from "./model";
type Kind = "effects" | "enemies" | "rules";
type Drag = { kind: Kind; id: string; target: string | null; group: boolean };
export function useConditionDrag(
  q: Query,
  setQ: Dispatch<SetStateAction<Query>>,
) {
  const latest = useRef(q);
  latest.current = q;
  const [drag, setDrag] = useState<Drag | null>(null);
  function start(kind: Kind, id: string, event: ReactPointerEvent) {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement).closest("button,input,select,a,label"))
      return;
    event.preventDefault();
    const source = event.currentTarget.closest<HTMLElement>(
      "[data-condition-id]",
    );
    if (!source) return;
    const rect = source.getBoundingClientRect(),
      offsetX = event.clientX - rect.x,
      offsetY = event.clientY - rect.y;
    const ghost = source.cloneNode(true) as HTMLElement;
    ghost.removeAttribute("data-condition-id");
    ghost.classList.add("drag-preview");
    Object.assign(ghost.style, {
      position: "fixed",
      width: rect.width + "px",
      height: rect.height + "px",
      left: rect.x + "px",
      top: rect.y + "px",
      margin: "0",
      transform: "none",
      opacity: "1",
    });
    ghost.setAttribute("aria-hidden", "true");
    document.body.appendChild(ghost);
    const origin = latest.current;
    let state: Drag = { kind, id, target: null, group: false };
    setDrag(state);
    let frame = 0;
    function reorder(target: string, after: boolean) {
      const current = latest.current;
      const items = [...current[kind]];
      const from = items.findIndex((v) => conditionKey(v) === id);
      const to = items.findIndex((v) => conditionKey(v) === target);
      if (from < 0 || to < 0) return;
      const [item] = items.splice(from, 1);
      const targetItem = items.find((v) => conditionKey(v) === target)!;
      const targetMode = targetItem.mode;
      const members =
        targetMode && targetMode !== item.mode
          ? items
              .map((v, i) => (v.mode === targetMode ? i : -1))
              .filter((i) => i >= 0)
          : [items.findIndex((v) => conditionKey(v) === target)];
      items.splice(
        after ? Math.max(...members) + 1 : Math.min(...members),
        0,
        item,
      );
      if (
        items.every(
          (v, i) => conditionKey(v) === conditionKey(current[kind][i]),
        )
      )
        return;
      const positions = new Map(
        Array.from(
          document.querySelectorAll<HTMLElement>("[data-condition-id]"),
        ).map((el) => [el.dataset.conditionId!, el.getBoundingClientRect()]),
      );
      const next = { ...current, [kind]: items };
      latest.current = next;
      setQ(next);
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        document
          .querySelectorAll<HTMLElement>("[data-condition-id]")
          .forEach((el) => {
            const old = positions.get(el.dataset.conditionId!);
            if (!old || el.dataset.conditionId === kind + ":" + id) return;
            const now = el.getBoundingClientRect();
            if (old.x !== now.x || old.y !== now.y)
              el.animate(
                [
                  {
                    transform: `translate(${old.x - now.x}px,${old.y - now.y}px)`,
                  },
                  { transform: "translate(0,0)" },
                ],
                { duration: 190, easing: "ease-out" },
              );
          });
      });
    }
    function move(e: PointerEvent) {
      ghost.style.left = e.clientX - offsetX + "px";
      ghost.style.top = e.clientY - offsetY + "px";
      const hit = document.elementFromPoint(e.clientX, e.clientY);
      const directTarget = hit?.closest<HTMLElement>("[data-condition-id]");
      const groupTarget = hit?.closest<HTMLElement>("[data-condition-group]");
      const target =
        directTarget ||
        groupTarget?.querySelector<HTMLElement>("[data-condition-id]");
      const key = target?.dataset.conditionId || "",
        prefix = kind + ":";
      if (!target || !key.startsWith(prefix) || key === prefix + id) {
        state = { kind, id, target: null, group: false };
        setDrag(state);
        return;
      }
      const targetId = key.slice(prefix.length),
        box = target.getBoundingClientRect();
      const x = (e.clientX - box.x) / box.width,
        y = (e.clientY - box.y) / box.height;
      const center =
        !!(!directTarget && groupTarget) ||
        kind !== "effects" ||
        (x > 0.1 && x < 0.9 && y > 0.1 && y < 0.9) ||
        !!hit?.closest(".drag-grip");
      state = { kind, id, target: targetId, group: center };
      setDrag(state);
      if (!center && kind === "effects") reorder(targetId, x >= 0.5);
      const viewport = target.closest(".selected-body");
      if (viewport) {
        const bounds = viewport.getBoundingClientRect();
        if (e.clientY > bounds.bottom - 18) viewport.scrollTop += 12;
        if (e.clientY < bounds.top + 18) viewport.scrollTop -= 12;
      }
    }
    function normalize(items: { mode?: number }[]) {
      const counts = new Map<number, number>();
      items.forEach((v) => {
        if (v.mode) counts.set(v.mode, (counts.get(v.mode) || 0) + 1);
      });
      items.forEach((v) => {
        if (v.mode && counts.get(v.mode) === 1) v.mode = 0;
      });
    }
    function finish(e: PointerEvent) {
      if (e.type !== "pointercancel" && state.group && state.target) {
        const current = origin;
        const items = current[kind].map((v) => ({ ...v }));
        const from = items.findIndex((v) => conditionKey(v) === id),
          to = items.findIndex((v) => conditionKey(v) === state.target);
        if (from >= 0 && to >= 0) {
          if (
            kind === "effects" &&
            !current.unrestricted &&
            (from < current.primaryCount || to < current.primaryCount)
          ) {
            if (from < current.primaryCount && to < current.primaryCount) {
              /* Already primary alternatives. */
            } else if (current.primaryCount < 3) {
              const secondaryIndex = from < current.primaryCount ? to : from;
              const [item] = items.splice(secondaryIndex, 1);
              items.splice(current.primaryCount, 0, item);
              items
                .slice(0, current.primaryCount + 1)
                .forEach((v) => (v.mode = 0));
              normalize(items);
              setQ({
                ...current,
                effects: items as Query["effects"],
                primaryCount: current.primaryCount + 1,
              });
            }
          } else {
            const mode =
              items[to].mode ||
              Math.max(0, ...items.map((v) => v.mode || 0)) + 1;
            items[from].mode = mode;
            items[to].mode = mode;

            if (kind === "effects") {
              const [item] = items.splice(from, 1);
              items.splice(
                items.findIndex((v) => conditionKey(v) === state.target) + 1,
                0,
                item,
              );
            }
            normalize(items);
            setQ({ ...current, [kind]: items });
          }
        }
      }
      if (e.type !== "pointercancel" && !state.group) {
        const sourceItem = origin[kind].find((v) => conditionKey(v) === id);
        const destination = document
          .elementFromPoint(e.clientX, e.clientY)
          ?.closest<HTMLElement>("[data-condition-group]");
        const sameGroup =
          sourceItem?.mode &&
          destination?.dataset.conditionGroup === kind + ":" + sourceItem.mode;
        if (sourceItem?.mode && !sameGroup) {
          const current = latest.current;
          const items = current[kind].map((v) => ({
            ...v,
            mode: conditionKey(v) === id ? 0 : v.mode,
          }));
          normalize(items);
          setQ({ ...current, [kind]: items });
        }
      }
      if (e.type === "pointercancel") setQ(origin);
      ghost.remove();
      setDrag(null);
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", finish);
      window.removeEventListener("pointercancel", finish);
    }
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", finish);
    window.addEventListener("pointercancel", finish);
  }
  function item(kind: Kind, id: string) {
    return {
      onPointerDown: (event: ReactPointerEvent) => start(kind, id, event),
      "data-condition-id": kind + ":" + id,
      "data-drag-source": (drag?.kind === kind && drag.id === id) || undefined,
      "data-group-target":
        (drag?.kind === kind && drag.target === id && drag.group) || undefined,
    };
  }
  return { start, item };
}
