import { Fragment, useEffect, useRef, useState } from "react";
import { useI18n } from "../i18n";
import { useTheme, type Theme } from "../hooks/useTheme";
import type { Lang } from "../i18n/locales";

const THEME_OPTS: { value: Theme; label: string }[] = [
  { value: "light", label: "☼" },
  { value: "system", label: "◑" },
  { value: "dark", label: "☾" },
];

const LANG_OPTS: { value: Lang; label: string }[] = [
  { value: "en", label: "en" },
  { value: "ru", label: "ru" },
];

export function SettingsMenu() {
  const [open, setOpen] = useState(false);
  const { lang, setLang, t } = useI18n();
  const { theme, setTheme } = useTheme();
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!containerRef.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  return (
    <div ref={containerRef} className={`settings ${open ? "is-open" : ""}`}>
      <button
        type="button"
        className="settings__trigger"
        onClick={() => setOpen((v) => !v)}
        aria-label="Settings"
      >
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
          <line x1="4" y1="6" x2="20" y2="6" />
          <circle cx="9" cy="6" r="2.5" />
          <line x1="4" y1="14" x2="20" y2="14" />
          <circle cx="15" cy="14" r="2.5" />
        </svg>
      </button>

      {open && (
        <div className="settings__menu">
          <div className="settings__row">
            <span className="settings__label">{t.settings.theme}</span>
            <div className="settings__opts" role="group">
              {THEME_OPTS.map((opt, i) => (
                <Fragment key={opt.value}>
                  {i > 0 && <span className="settings__sep" aria-hidden="true">·</span>}
                  <button
                    type="button"
                    className={`settings__opt${theme === opt.value ? " is-active" : ""}`}
                    onClick={() => setTheme(opt.value)}
                  >
                    {opt.label}
                  </button>
                </Fragment>
              ))}
            </div>
          </div>

          <div className="settings__divider" aria-hidden="true" />

          <div className="settings__row">
            <span className="settings__label">{t.settings.language}</span>
            <div className="settings__opts" role="group">
              {LANG_OPTS.map((opt, i) => (
                <Fragment key={opt.value}>
                  {i > 0 && <span className="settings__sep" aria-hidden="true">·</span>}
                  <button
                    type="button"
                    className={`settings__opt${lang === opt.value ? " is-active" : ""}`}
                    onClick={() => setLang(opt.value)}
                  >
                    {opt.label}
                  </button>
                </Fragment>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
