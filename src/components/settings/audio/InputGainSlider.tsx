import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Slider } from "../../ui/Slider";
import { commands } from "../../../bindings";

/** Formats a dB gain: "0 dB", "+6 dB", "−12 dB" (proper minus sign for cut). */
function formatDb(db: number): string {
  const rounded = Math.round(db);
  if (rounded === 0) return "0 dB";
  if (rounded > 0) return `+${rounded} dB`;
  return `−${Math.abs(rounded)} dB`;
}

/**
 * Live input-gain control for the Audio console. Reads the persisted dB value on
 * mount and writes each change straight to the backend, which updates the live
 * capture-gain atomic — the Epic-A meter above reflects it immediately.
 */
export const InputGainSlider: React.FC = () => {
  const { t } = useTranslation();
  const [db, setDb] = useState(0);

  useEffect(() => {
    let alive = true;
    void commands.getInputGain().then((res) => {
      if (alive && res.status === "ok") setDb(res.data);
    });
    return () => {
      alive = false;
    };
  }, []);

  const handleChange = (value: number) => {
    setDb(value);
    void commands.setInputGain(value);
  };

  return (
    <Slider
      value={db}
      onChange={handleChange}
      min={-24}
      max={24}
      step={1}
      label={t("settings.audio.gain.title")}
      description={t("settings.audio.gain.description")}
      descriptionMode="tooltip"
      grouped
      formatValue={formatDb}
    />
  );
};
