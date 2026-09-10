import React from "react";

export function StarIcon({ filled = false }: { filled?: boolean }) {
  return <svg className="star-icon" viewBox="0 0 24 24" width="22" height="22" aria-hidden="true" focusable="false">
    <path d="m12 3 2.78 5.63L21 9.54l-4.5 4.39 1.06 6.2L12 17.2l-5.56 2.93 1.06-6.2L3 9.54l6.22-.91Z"
      fill={filled ? "currentColor" : "none"} stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" />
  </svg>;
}
