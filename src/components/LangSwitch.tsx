import { useI18n } from "../i18n";

export function LangSwitch() {
  const { lang, setLang } = useI18n();
  return (
    <div className="lang-switch" role="group" aria-label="Language">
      <button
        type="button"
        className={`lang-switch__btn${lang === "en" ? " is-active" : ""}`}
        onClick={() => setLang("en")}
      >
        en
      </button>
      <span className="lang-switch__sep" aria-hidden="true">·</span>
      <button
        type="button"
        className={`lang-switch__btn${lang === "ru" ? " is-active" : ""}`}
        onClick={() => setLang("ru")}
      >
        ru
      </button>
    </div>
  );
}
