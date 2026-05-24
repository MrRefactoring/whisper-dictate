import { useCallback, useEffect, useRef, useState } from "react";
import type { RecordingState } from "../types";
import { InkWave } from "./InkWave";
import { useI18n } from "../i18n";

interface MicButtonProps {
  state: RecordingState;
  level: number;
  disabled?: boolean;
  onStart: () => void;
  onStop: () => void;
  onLock: () => void;
}

const LOCK_THRESHOLD = 72;

export function MicButton({ state, level, disabled, onStart, onStop, onLock }: MicButtonProps) {
  const { t } = useI18n();
  const [dragUp, setDragUp] = useState(0);
  const holdingRef = useRef(false);
  const startYRef = useRef(0);

  const active = state === "recording" || state === "locked";
  const locked = state === "locked";
  const finalizing = state === "finalizing";

  const begin = useCallback(
    (clientY: number) => {
      if (disabled || finalizing || holdingRef.current || locked) return;
      holdingRef.current = true;
      startYRef.current = clientY;
      setDragUp(0);
      onStart();
    },
    [disabled, finalizing, locked, onStart],
  );

  const end = useCallback(() => {
    if (!holdingRef.current) return;
    holdingRef.current = false;
    const lockedNow = dragUp >= LOCK_THRESHOLD;
    setDragUp(0);
    if (!lockedNow) onStop();
  }, [dragUp, onStop]);

  const onPointerDown = (e: React.PointerEvent) => {
    (e.target as HTMLElement).setPointerCapture?.(e.pointerId);
    begin(e.clientY);
  };

  const onPointerMove = (e: React.PointerEvent) => {
    if (!holdingRef.current || locked) return;
    const up = Math.max(0, startYRef.current - e.clientY);
    setDragUp(up);
    if (up >= LOCK_THRESHOLD) {
      holdingRef.current = false;
      setDragUp(0);
      onLock();
    }
  };

  useEffect(() => {
    const isTyping = () => {
      const el = document.activeElement;
      return el instanceof HTMLElement && (el.isContentEditable || /INPUT|TEXTAREA/.test(el.tagName));
    };
    const down = (e: KeyboardEvent) => {
      if (e.code !== "Space" || e.repeat || isTyping()) return;
      e.preventDefault();
      begin(Number.MAX_SAFE_INTEGER);
    };
    const up = (e: KeyboardEvent) => {
      if (e.code !== "Space" || isTyping()) return;
      e.preventDefault();
      end();
    };
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
    };
  }, [begin, end]);

  const lockProgress = Math.min(1, dragUp / LOCK_THRESHOLD);
  const dragging = holdingRef.current && dragUp > 6;

  return (
    <div className="ink">
      <span
        className={`ink__stroke ${dragging ? "is-visible" : ""}`}
        style={{ ["--p" as string]: lockProgress.toFixed(3) }}
      />

      {locked && (
        <span className="ink__seal" title={t.mic.sealTitle}>
          録
        </span>
      )}

      <InkWave level={level} active={active} />

      <button
        type="button"
        className={`ink__dot state-${state}`}
        style={{ ["--level" as string]: level.toFixed(3) }}
        disabled={disabled || finalizing}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={end}
        onPointerLeave={end}
        onContextMenu={(e) => e.preventDefault()}
        aria-label={active ? t.mic.ariaActive : t.mic.ariaIdle}
      >
        {finalizing && <span className="ink__spinner" />}
      </button>
    </div>
  );
}
