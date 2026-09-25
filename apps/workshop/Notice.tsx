import React, { useEffect, useState } from "react";
import { desktop } from "./desktop-bridge";
import {
  isFailureText,
  isUserCorrectable,
  publicError,
  stripErrorPrefix,
} from "./public-errors";

export type NoticeTone = "info" | "success" | "warning" | "error";

/** Raw technical text, rendered without interface translation. */
function Technical({ text }: { text: string }) {
  return React.createElement("code", null, text);
}

/**
 * One status or failure message.
 *
 * A failure says what happened in plain words, keeps the technical text one
 * click away, and offers a feedback file the player can send as it is.
 */
export function Notice({
  text,
  tone,
  className = "",
}: {
  text: string;
  tone?: NoticeTone;
  className?: string;
}) {
  const [feedback, setFeedback] = useState<"" | "busy" | "saved" | "failed">("");
  // A new message replaces the previous one together with its feedback hint.
  useEffect(() => setFeedback(""), [text]);
  if (!text.trim()) return null;
  const effective =
    tone ??
    (isUserCorrectable(text) ? "warning" : isFailureText(text) ? "error" : "info");
  const display = stripErrorPrefix(text).trim();
  const technical = publicError(display) !== display ? display : "";
  async function exportFeedback() {
    setFeedback("busy");
    try {
      await window.review.exportFeedback();
      setFeedback("saved");
    } catch {
      setFeedback("failed");
    }
  }
  return (
    <div
      className={`notice notice-${effective} ${className}`.trim()}
      role={effective === "error" ? "alert" : "status"}
    >
      <p>{display}</p>
      {effective === "error" && (
        <div className="notice-actions">
          {technical && (
            <details>
              <summary>技术详情</summary>
              <Technical text={technical} />
            </details>
          )}
          {desktop && (
            <button
              type="button"
              disabled={feedback === "busy"}
              onClick={() => void exportFeedback()}
            >
              导出反馈文件
            </button>
          )}
        </div>
      )}
      {feedback === "saved" && <FeedbackSaved onDismiss={() => setFeedback("")} />}
      {feedback === "failed" && (
        <p className="notice-hint">反馈文件没有导出成功，请在“设置”里再试一次。</p>
      )}
    </div>
  );
}

/** Where the feedback file went and where to send it. */
export function FeedbackSaved({ onDismiss }: { onDismiss?: () => void }) {
  return (
    <div className="notice-hint">
      <p>反馈文件已保存，所在文件夹已经打开。把这个文件和出问题前的操作步骤一起发到 QQ 群或 GitHub 即可。</p>
      <div className="notice-links">
        <button type="button" onClick={() => void window.review.openLink("qq")}>
          加入QQ群
        </button>
        <button type="button" onClick={() => void window.review.openLink("github")}>
          GitHub
        </button>
        {onDismiss && (
          <button type="button" onClick={onDismiss}>
            知道了
          </button>
        )}
      </div>
    </div>
  );
}
