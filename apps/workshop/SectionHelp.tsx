import React, { useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

/** Render outside the accordion so collapsed and scrollable sections cannot clip help. */
export function SectionHelp({ title, children }: {
  title: string;
  children: React.ReactNode;
}) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<{ left: number; top: number } | null>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const popup = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    if (!open) return;
    const updatePosition = () => {
      const button = trigger.current;
      const content = popup.current;
      if (!button || !content) return;
      const anchor = button.getBoundingClientRect();
      if (!anchor.width || !anchor.height) {
        setOpen(false);
        return;
      }
      const margin = 12;
      const width = content.offsetWidth;
      const height = content.offsetHeight;
      const below = anchor.bottom + 7;
      const top = below + height <= window.innerHeight - margin
        ? below : Math.max(margin, anchor.top - height - 7);
      const left = Math.max(margin, Math.min(anchor.right - width, window.innerWidth - width - margin));
      setPosition({ left, top });
    };
    const closeOutside = (event: Event) => {
      const target = event.target;
      if (target instanceof Node && !trigger.current?.contains(target) && !popup.current?.contains(target)) {
        setOpen(false);
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        setOpen(false);
      }
    };
    updatePosition();
    document.addEventListener("pointerdown", closeOutside, true);
    document.addEventListener("focusin", closeOutside, true);
    document.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("resize", updatePosition);
    document.addEventListener("scroll", updatePosition, true);
    return () => {
      document.removeEventListener("pointerdown", closeOutside, true);
      document.removeEventListener("focusin", closeOutside, true);
      document.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("resize", updatePosition);
      document.removeEventListener("scroll", updatePosition, true);
    };
  }, [open]);

  return <>
    <button
      ref={trigger}
      type="button"
      className="section-help-trigger"
      aria-label={title}
      aria-expanded={open}
      aria-controls={open ? id : undefined}
      aria-describedby={open ? id : undefined}
      onClick={() => {
        setPosition(null);
        setOpen(!open);
      }}
    >?</button>
    {open && createPortal(
      <div
        ref={popup}
        id={id}
        role="tooltip"
        className="section-help-popover"
        style={position || { left: 0, top: 0, visibility: "hidden" }}
      >{children}</div>,
      document.body,
    )}
  </>;
}
