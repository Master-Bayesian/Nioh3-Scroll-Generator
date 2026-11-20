import { useEffect, useRef, useState } from "react";
import idle from "./naiwa/naiwa-idle.webp";
import grin from "./naiwa/naiwa-grin.webp";
import laugh from "./naiwa/laugh.wav";

export function NaiwaButton() {
  const audio = useRef<HTMLAudioElement>(null);
  const [playing, setPlaying] = useState(false);

  useEffect(() => {
    const clip = audio.current;
    return () => clip?.pause();
  }, []);

  function playLaugh() {
    const clip = audio.current;
    if (clip?.paused) {
      clip.currentTime = 0;
      void clip.play().catch(() => setPlaying(false));
    }
  }

  return (
    <button
      type="button"
      className={playing ? "naiwa-button is-laughing" : "naiwa-button"}
      aria-label="播放奶蛙大笑"
      title="播放奶蛙大笑"
      onClick={playLaugh}
    >
      <img src={playing ? grin : idle} alt="" draggable={false} />
      <audio
        ref={audio}
        src={laugh}
        preload="none"
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        onEnded={() => setPlaying(false)}
        onError={() => setPlaying(false)}
      />
    </button>
  );
}
