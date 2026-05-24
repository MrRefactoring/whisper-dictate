import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { MicButton } from "./components/MicButton";
import { ModelPicker } from "./components/ModelPicker";
import { TranscriptPanel } from "./components/TranscriptPanel";
import { LangSwitch } from "./components/LangSwitch";
import { Toast } from "./components/Toast";
import { useDictation } from "./hooks/useDictation";
import { useUpdater } from "./hooks/useUpdater";
import { useI18n } from "./i18n";
import type { RecordingState } from "./types";
import "./App.css";

const MEDIA_EXTS = [
  "mp3", "m4a", "aac", "wav", "flac", "ogg", "opus", "aiff", "caf",
  "mp4", "mov", "mkv", "webm", "m4v",
];

function App() {
  const { t } = useI18n();
  const d = useDictation();
  const upd = useUpdater();
  const locked = d.recordingState === "locked";
  const active = d.recordingState === "recording" || d.recordingState === "locked";
  const ready = d.modelStatus?.loaded === true;
  const fileBusy = d.fileTask !== null;
  const [dragOver, setDragOver] = useState(false);

  const STATUS_LABEL: Record<RecordingState, string> = {
    idle: t.status.idle,
    recording: t.status.recording,
    locked: t.status.locked,
    finalizing: t.status.finalizing,
  };

  const canAcceptRef = useRef(false);
  canAcceptRef.current = ready && !fileBusy && !active;
  const transcribeRef = useRef(d.transcribeFile);
  transcribeRef.current = d.transcribeFile;

  useEffect(() => {
    let effectActive = true;
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (!effectActive) return;
        const p = event.payload;
        if (p.type === "enter" || p.type === "over") {
          if (canAcceptRef.current) setDragOver(true);
        } else if (p.type === "leave") {
          setDragOver(false);
        } else if (p.type === "drop") {
          setDragOver(false);
          if (canAcceptRef.current && p.paths.length > 0) {
            transcribeRef.current(p.paths[0]);
          }
        }
      })
      .then((u) => {
        if (effectActive) {
          unlisten = u;
        } else {
          u();
        }
      });
    return () => {
      effectActive = false;
      unlisten?.();
    };
  }, []);

  const pickFile = async () => {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: t.fileDialog.mediaLabel, extensions: MEDIA_EXTS }],
    });
    if (typeof selected === "string") d.transcribeFile(selected);
  };

  return (
    <main className="app">
      <div className="app__paper" aria-hidden="true" />

      <header className="app__head">
        <h1 className="brand">ma</h1>
        <div className="app__head-right">
          <LangSwitch />
          <ModelPicker
            models={d.models}
            current={d.currentModel}
            status={d.modelStatus}
            downloads={d.downloads}
            onSelect={d.selectModel}
            onDownload={d.downloadModel}
            onCancel={d.cancelDownload}
            onDelete={d.deleteModel}
          />
        </div>
      </header>

      <TranscriptPanel
        transcript={d.transcript}
        interim={d.interim}
        active={active}
        onCopy={d.copy}
        onClear={d.clear}
        onEdit={d.editTranscript}
      />

      <section className="stage">
        {fileBusy ? (
          <div className="filecard">
            <span className="filecard__icon" aria-hidden="true">♪</span>
            <span className="filecard__name">{d.fileTask!.name}</span>
            <span className="filecard__bar">
              <span
                className={`filecard__fill ${d.fileTask!.pct === 0 ? "is-indeterminate" : ""}`}
                style={{ width: `${Math.max(2, d.fileTask!.pct)}%` }}
              />
            </span>
            <span className="filecard__meta">
              {d.fileTask!.pct === 0
                ? t.file.decoding
                : t.file.recognizing(d.fileTask!.pct)}
            </span>
            <button type="button" className="link link--accent" onClick={d.cancelFile}>
              {t.file.cancel}
            </button>
          </div>
        ) : (
          <>
            <div className={`stage__status state-${d.recordingState}`}>
              {STATUS_LABEL[d.recordingState]}
            </div>

            <MicButton
              state={d.recordingState}
              level={d.level}
              disabled={!ready}
              onStart={d.start}
              onStop={d.stop}
              onLock={() => d.setLocked(true)}
            />

            {locked ? (
              <div className="stage__lockControls">
                <button type="button" className="link" onClick={d.cancel}>
                  {t.stage.cancel}
                </button>
                <button type="button" className="link link--accent" onClick={d.stop}>
                  {t.stage.finish}
                </button>
              </div>
            ) : (
              <p className="stage__hint">
                {ready ? t.stage.hintReady : t.stage.hintNoModel}
              </p>
            )}

            {ready && (
              <button type="button" className="filebtn" onClick={pickFile}>
                {t.stage.fileBtn}
              </button>
            )}
          </>
        )}
      </section>

      {dragOver && (
        <div className="dropzone" aria-hidden="true">
          <div className="dropzone__inner">{t.dropzone}</div>
        </div>
      )}

      {upd.available && (
        <div className="update-banner" role="status">
          <span className="update-banner__text">
            {t.update.label(upd.version ?? "")}
          </span>
          <button
            type="button"
            className="link link--accent update-banner__btn"
            onClick={upd.install}
            disabled={upd.installing}
          >
            {upd.installing ? t.update.installing : t.update.install}
          </button>
          <button
            type="button"
            className="link update-banner__dismiss"
            onClick={upd.dismiss}
            aria-label={t.update.dismiss}
          >
            ✕
          </button>
        </div>
      )}

      <Toast message={d.error} onDismiss={d.dismissError} />
    </main>
  );
}

export default App;
