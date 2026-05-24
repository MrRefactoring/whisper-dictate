import { useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n";

interface TranscriptPanelProps {
  transcript: string;
  interim: string;
  active: boolean;
  onCopy: () => Promise<boolean>;
  onClear: () => void;
  onEdit: (text: string) => void;
}

export function TranscriptPanel({
  transcript,
  interim,
  active,
  onCopy,
  onClear,
  onEdit,
}: TranscriptPanelProps) {
  const { t } = useI18n();
  const [copied, setCopied] = useState(false);
  const flowRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!active) return;
    const el = flowRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [transcript, interim, active]);

  const handleCopy = async () => {
    const ok = await onCopy();
    if (ok) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
    }
  };

  const empty = transcript.length === 0;

  return (
    <section className="transcript">
      {active ? (
        <div className="transcript__flow" ref={flowRef}>
          {transcript && <span>{transcript} </span>}
          {interim && <span className="transcript__interim">{interim}</span>}
          <span className="transcript__caret" />
        </div>
      ) : (
        <textarea
          className="transcript__edit"
          value={transcript}
          onChange={(e) => onEdit(e.target.value)}
          placeholder={t.transcript.placeholder}
          spellCheck={false}
          autoComplete="off"
        />
      )}

      <footer className="transcript__actions">
        <button type="button" className="link" onClick={onClear} disabled={empty}>
          {t.transcript.clear}
        </button>
        <button type="button" className="link link--accent" onClick={handleCopy} disabled={empty}>
          {copied ? t.transcript.copied : t.transcript.copy}
        </button>
      </footer>
    </section>
  );
}
