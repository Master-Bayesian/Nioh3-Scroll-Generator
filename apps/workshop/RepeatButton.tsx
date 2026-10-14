import React, { useEffect, useRef } from "react";

/** Pointer holds repeat; keyboard and assistive clicks retain ordinary button behavior. */
export function RepeatButton({ step, disabled, label, title, children }: {
  step: () => void; disabled: boolean; label: string; title: string; children: React.ReactNode;
}) {
  const callback = useRef(step);
  callback.current = step;
  const delay = useRef<ReturnType<typeof setTimeout> | null>(null);
  const repeat = useRef<ReturnType<typeof setInterval> | null>(null);
  const suppressClick = useRef(false);
  const stop = () => {
    if (delay.current !== null) clearTimeout(delay.current);
    if (repeat.current !== null) clearInterval(repeat.current);
    delay.current = null; repeat.current = null;
  };
  useEffect(() => {
    const hidden = () => { if (document.hidden) stop(); };
    window.addEventListener("blur", stop);
    document.addEventListener("visibilitychange", hidden);
    return () => { stop(); window.removeEventListener("blur", stop); document.removeEventListener("visibilitychange", hidden); };
  }, []);
  useEffect(() => { if (disabled) stop(); }, [disabled]);
  return <button type="button" className="loadout-repeat-button" aria-label={label} title={title} disabled={disabled}
    onPointerDown={event => {
      if (!event.isPrimary || event.button !== 0 || disabled) return;
      stop(); suppressClick.current = true;
      event.currentTarget.setPointerCapture(event.pointerId);
      callback.current();
      delay.current = setTimeout(() => {
        callback.current();
        repeat.current = setInterval(() => callback.current(), 70);
      }, 350);
    }}
    onPointerUp={stop} onPointerCancel={stop} onLostPointerCapture={stop}
    onPointerMove={event => {
      if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
      const box = event.currentTarget.getBoundingClientRect();
      if (event.clientX < box.left || event.clientX > box.right || event.clientY < box.top || event.clientY > box.bottom) {
        stop(); event.currentTarget.releasePointerCapture(event.pointerId);
      }
    }}
    onBlur={stop} onKeyDown={event => { if (event.key === "Escape") stop(); }}
    onContextMenu={event => event.preventDefault()}
    onClick={event => {
      if (event.detail !== 0 && suppressClick.current) { suppressClick.current = false; return; }
      callback.current();
    }}>{children}</button>;
}
