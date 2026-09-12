import { useRef, type PointerEvent as ReactPointerEvent } from "react";

function isBackdropPointer(event: ReactPointerEvent<HTMLDialogElement>) {
  if (event.target !== event.currentTarget) return false;
  const rectangle = event.currentTarget.getBoundingClientRect();
  return (
    event.clientX < rectangle.left ||
    event.clientX > rectangle.right ||
    event.clientY < rectangle.top ||
    event.clientY > rectangle.bottom
  );
}

/**
 * Close a native modal only after a complete pointer press on its backdrop.
 * A press that starts in dialog content and ends outside must never dismiss it.
 */
export function useDialogBackdropDismiss() {
  const backdropPointer = useRef<number | null>(null);
  return {
    onPointerDown(event: ReactPointerEvent<HTMLDialogElement>) {
      backdropPointer.current = isBackdropPointer(event)
        ? event.pointerId
        : null;
    },
    onPointerUp(event: ReactPointerEvent<HTMLDialogElement>) {
      const shouldDismiss =
        backdropPointer.current === event.pointerId && isBackdropPointer(event);
      backdropPointer.current = null;
      if (shouldDismiss) event.currentTarget.close();
    },
    onPointerCancel(event: ReactPointerEvent<HTMLDialogElement>) {
      if (backdropPointer.current === event.pointerId)
        backdropPointer.current = null;
    },
  };
}
