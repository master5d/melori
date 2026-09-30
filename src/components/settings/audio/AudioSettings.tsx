import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { MicrophoneSelector } from "../sound/MicrophoneSelector";
import { MuteWhileRecording } from "../sound/MuteWhileRecording";
import { AudioFeedback } from "../sound/AudioFeedback";
import { OutputDeviceSelector } from "../sound/OutputDeviceSelector";
import { VolumeSlider } from "../sound/VolumeSlider";
import { InputMeter } from "./InputMeter";
import { SpectrumPanel } from "./SpectrumPanel";
import { InputGainSlider } from "./InputGainSlider";
import { PassthroughToggle } from "./PassthroughToggle";
import { MonitorDeviceSelect } from "./MonitorDeviceSelect";
import { MonitorVolumeSlider } from "./MonitorVolumeSlider";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { useSettings } from "../../../hooks/useSettings";
import { DeviceHealthCard } from "./DeviceHealthCard";
import { useDeviceHealth } from "./useDeviceHealth";
import { signalLost } from "./deviceHealthMath";
import { AutoStopSilence } from "./AutoStopSilence";

export const AudioSettings: React.FC = () => {
  const { t } = useTranslation();
  const { audioFeedbackEnabled } = useSettings();
  const [passthrough, setPassthrough] = useState(false);
  // Called exactly ONCE for the whole console; the result is passed down. The
  // meter and the spectrum need this to stop painting a frozen last frame as
  // live audio when the device dies.
  const { health, key: healthKey } = useDeviceHealth();
  const lost = signalLost(healthKey);

  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.audio.title")}>
        <MicrophoneSelector descriptionMode="tooltip" grouped={true} />
        <InputMeter signalLost={lost} />
        <SpectrumPanel signalLost={lost} />
        <DeviceHealthCard health={health} statusKey={healthKey} />
        <AutoStopSilence />
        <InputGainSlider />
        <PassthroughToggle
          enabled={passthrough}
          onEnabledChange={setPassthrough}
          healthKey={healthKey}
        />
        <MonitorDeviceSelect disabled={!passthrough} />
        <MonitorVolumeSlider disabled={!passthrough} />
        <MuteWhileRecording descriptionMode="tooltip" grouped={true} />
        <AudioFeedback descriptionMode="tooltip" grouped={true} />
        <OutputDeviceSelector
          descriptionMode="tooltip"
          grouped={true}
          disabled={!audioFeedbackEnabled}
        />
        <VolumeSlider disabled={!audioFeedbackEnabled} />
      </SettingsGroup>
    </div>
  );
};
