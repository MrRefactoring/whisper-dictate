import { useRef, useState } from "react";
import type { DownloadState, ModelId, ModelInfo, ModelStatus } from "../types";
import { useI18n } from "../i18n";

interface ModelPickerProps {
  models: ModelInfo[];
  current: ModelId;
  status: ModelStatus | null;
  downloads: Record<string, DownloadState>;
  onSelect: (model: ModelId) => void;
  onDownload: (model: ModelId) => void;
  onCancel: (model: ModelId) => void;
  onDelete: (model: ModelId) => void;
}

export function ModelPicker({
  models,
  current,
  status,
  downloads,
  onSelect,
  onDownload,
  onCancel,
  onDelete,
}: ModelPickerProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  const [confirmId, setConfirmId] = useState<ModelId | null>(null);
  const confirmTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const askDelete = (model: ModelId) => {
    setConfirmId(model);
    if (confirmTimer.current) clearTimeout(confirmTimer.current);
    confirmTimer.current = setTimeout(() => setConfirmId(null), 3000);
  };
  const active = models.find((m) => m.id === current);
  const loading = status != null && !status.loaded && status.error === "loading";
  const failed = status != null && !status.loaded && status.error != null && status.error !== "loading";

  return (
    <div className={`model ${open ? "is-open" : ""}`}>
      <button type="button" className="model__trigger" onClick={() => setOpen((v) => !v)}>
        <span className={`model__dot ${loading ? "is-loading" : failed ? "is-error" : "is-ok"}`} />
        <span className="model__name">{active?.label ?? current}</span>
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2">
          <path d="m6 9 6 6 6-6" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>

      {open && (
        <ul className="model__menu" role="listbox">
          {models.map((m) => {
            const dl = downloads[m.id];
            const downloading = dl?.status === "downloading";
            const pct = dl?.pct ?? 0;
            return (
              <li key={m.id} className={`model__row ${m.id === current && m.available ? "is-current" : ""}`}>
                <button
                  type="button"
                  className="model__pick"
                  disabled={!m.available}
                  onClick={() => {
                    if (m.available && m.id !== current) {
                      setOpen(false);
                      onSelect(m.id);
                    }
                  }}
                >
                  <span className="model__nameRow">
                    {m.label}
                    {m.recommended && <span className="tag">{t.model.recommended}</span>}
                  </span>
                  <span className="model__size">{m.size}</span>
                </button>

                <div className="model__action">
                  {m.available ? (
                    <span className="model__avail">
                      {m.id === current && <span className="model__check">✓</span>}
                      {confirmId === m.id ? (
                        <button
                          type="button"
                          className="model__confirmDel"
                          onClick={() => {
                            setConfirmId(null);
                            onDelete(m.id);
                          }}
                        >
                          {t.model.deleteConfirm}
                        </button>
                      ) : (
                        <button
                          type="button"
                          className="model__trash"
                          title={t.model.deleteTitle}
                          aria-label={t.model.deleteTitle}
                          onClick={() => askDelete(m.id)}
                        >
                          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="1.8">
                            <path d="M4 7h16M9 7V5h6v2M6 7l1 13h10l1-13" strokeLinecap="round" strokeLinejoin="round" />
                          </svg>
                        </button>
                      )}
                    </span>
                  ) : downloading ? (
                    <span className="model__progress" title={t.model.downloading}>
                      <span className="model__bar">
                        <span className="model__barFill" style={{ width: `${Math.round(pct * 100)}%` }} />
                      </span>
                      <span className="model__pct">
                        {dl?.pct != null ? `${Math.round(pct * 100)}%` : "…"}
                      </span>
                      <button
                        type="button"
                        className="model__cancel"
                        title={t.model.cancelDownload}
                        aria-label={t.model.cancelDownload}
                        onClick={() => onCancel(m.id)}
                      >
                        <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" strokeWidth="2.2">
                          <path d="M6 6l12 12M18 6L6 18" strokeLinecap="round" />
                        </svg>
                      </button>
                    </span>
                  ) : (
                    <button type="button" className="model__dl" onClick={() => onDownload(m.id)}>
                      {dl?.status === "error" ? t.model.retry : t.model.download}
                    </button>
                  )}
                </div>
              </li>
            );
          })}
          <li className="model__hint">{t.model.hint}</li>
        </ul>
      )}

      {failed && <p className="model__error">{status?.error}</p>}
    </div>
  );
}
