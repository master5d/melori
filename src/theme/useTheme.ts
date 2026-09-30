import { useEffect, useState } from "react";
import type { Theme } from "./theme";

function readTheme(): Theme {
  return document.documentElement.dataset.theme === "evening"
    ? "evening"
    : "day";
}

export function useTheme(): Theme {
  const [theme, setTheme] = useState<Theme>(readTheme);

  useEffect(() => {
    const observer = new MutationObserver(() => setTheme(readTheme()));
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    return () => observer.disconnect();
  }, []);

  return theme;
}
