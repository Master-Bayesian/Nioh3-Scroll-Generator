import React from "react";

export function StarIcon({ filled = false }: { filled?: boolean }) {
  return <svg className="star-icon" viewBox="0 0 24 24" width="22" height="22" aria-hidden="true" focusable="false">
    <path d="m12 3 2.78 5.63L21 9.54l-4.5 4.39 1.06 6.2L12 17.2l-5.56 2.93 1.06-6.2L3 9.54l6.22-.91Z"
      fill={filled ? "currentColor" : "none"} stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
  </svg>;
}

/** The game's protection mark, shared by the scroll and equipment lists (left of the star, as in game). */
export function LockMark() {
  return <span className="lock-mark" title="游戏里已保护：不能卖掉或丢弃" aria-label="已保护">
    <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinejoin="round">
      <rect x="5" y="11" width="14" height="10" rx="2" />
      <path d="M8 11V8a4 4 0 0 1 8 0v3" />
    </svg>
  </span>;
}
