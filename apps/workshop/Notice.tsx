import React, { useEffect, useState } from "react";
import { desktop } from "./desktop-bridge";
import {
  isFailureText,
  isLiveAddLockError,
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
  const [dismissed, setDismissed] = useState(false);
  const [lockReset, setLockReset] = useState<"" | "busy" | "cleared" | "held" | "failed">("");
  const [gameSelecting,setGameSelecting]=useState(false);
  const [gameMessage,setGameMessage]=useState("");
  // A new message replaces the previous one together with its feedback hint,
  // and shows again even if the player closed the previous one.
  useEffect(() => {
    setFeedback("");
    setDismissed(false);
    setLockReset("");
    setGameMessage("");
  }, [text]);
  if (!text.trim() || dismissed) return null;
  const effective =
    tone ??
    (isUserCorrectable(text) ? "warning" : isFailureText(text) ? "error" : "info");
  const display = stripErrorPrefix(text).trim();
  const technical = publicError(display) !== display ? display : "";
  const gameInstallError=desktop&&/GAME_(?:EXECUTABLE|VERSION|INSTALL_CONFIG)_/.test(text);
  async function selectGame(){
    setGameSelecting(true);
    try{const result=await window.review.gameInstallation("select");if(result?.restart_required)setGameMessage("游戏路径已记录。请重新打开工作室后使用，游戏无需关闭。");}
    catch{setGameMessage("游戏程序没有选择成功，请到设置中重新选择。")}finally{setGameSelecting(false)}
  }
  async function resetLiveAddLock() {
    setLockReset("busy");
    try {
      const result = await window.operations.execute({
        method: "runtime.reset_live_add_lock",
        params: {},
      });
      setLockReset(
        result && "state" in result && result.state === "held" ? "held" : "cleared",
      );
    } catch {
      setLockReset("failed");
    }
  }
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
      {effective === "error" && (
        <button
          type="button"
          className="notice-close"
          aria-label="关闭提示"
          title="关闭提示"
          onClick={() => setDismissed(true)}
        >
          ×
        </button>
      )}
      <p>{display}</p>
      {effective === "error" && (
        <div className="notice-actions">
          {technical && (
            <details>
              <summary>技术详情</summary>
              <Technical text={technical} />
            </details>
          )}
          {desktop && isLiveAddLockError(text) && (
            <button
              type="button"
              disabled={lockReset === "busy"}
              onClick={() => void resetLiveAddLock()}
            >
              重置实时添加状态
            </button>
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
          {gameInstallError&&<button disabled={gameSelecting} onClick={()=>void selectGame()}>选择游戏程序</button>}
        </div>
      )}
      {lockReset === "cleared" && (
        <p className="notice-hint">已重置。请重新操作一次（例如再点“核对添加”）；如果还是出现这个提示，请导出反馈文件发给开发者。</p>
      )}
      {lockReset === "held" && (
        <p className="notice-hint">还有一次实时添加正在进行。请等几秒后再点一次“重置实时添加状态”；仍不行请完全关闭本程序后重新打开，或重启电脑。</p>
      )}
      {lockReset === "failed" && (
        <p className="notice-hint">重置没有成功。请完全关闭本程序后重新打开再试；仍不行请导出反馈文件发给开发者。</p>
      )}
      {feedback === "saved" && <FeedbackSaved onDismiss={() => setFeedback("")} />}
      {gameMessage&&<p className="notice-hint">{gameMessage}</p>}
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
