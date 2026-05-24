import { useEffect, useState } from "react";

export type Theme = "light" | "dark" | "system";

function applyTheme(theme: Theme) {
  if (theme === "system") {
    document.documentElement.removeAttribute("data-theme");
  } else {
    document.documentElement.setAttribute("data-theme", theme);
  }
}

export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(() => {
    const stored = localStorage.getItem("theme");
    const t: Theme =
      stored === "light" || stored === "dark" || stored === "system"
        ? stored
        : "system";
    applyTheme(t);
    return t;
  });

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  const setTheme = (t: Theme) => {
    localStorage.setItem("theme", t);
    setThemeState(t);
  };

  return { theme, setTheme };
}
