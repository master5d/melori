// Settings section components
export { GeneralSettings } from "./general/GeneralSettings";
export { AdvancedSettings } from "./advanced/AdvancedSettings";
export { DebugSettings } from "./debug/DebugSettings";
export { HistorySettings } from "./history/HistorySettings";
export { CoachSettings } from "./coach/CoachSettings";
export { AssistantSettings } from "./assistant/AssistantSettings";
export { TtsSettings } from "./tts/TtsSettings";
export { AboutSettings } from "./about/AboutSettings";
export { PostProcessingSettings } from "./post-processing/PostProcessingSettings";
export { ModelsSettings } from "./models/ModelsSettings";
export { TutorSettings } from "./tutor/TutorSettings";
export { AudioSettings } from "./audio/AudioSettings";

// Studio: generic data-driven course UI + synth modules (handpan, karaoke)
export { CourseSettings } from "./course/CourseSettings";
export { HandpanSettings } from "./handpan/HandpanSettings";
export { KaraokeSettings } from "./karaoke/KaraokeSettings";

// Individual setting components
export { MicrophoneSelector } from "./sound/MicrophoneSelector";
export { ClamshellMicrophoneSelector } from "./sound/ClamshellMicrophoneSelector";
export { OutputDeviceSelector } from "./sound/OutputDeviceSelector";
export { AlwaysOnMicrophone } from "./sound/AlwaysOnMicrophone";
export { PushToTalk } from "./shortcuts/PushToTalk";
export { AudioFeedback } from "./sound/AudioFeedback";
export { ShowOverlay } from "./advanced/ShowOverlay";
export { GlobalShortcutInput } from "./shortcuts/GlobalShortcutInput";
export { EchoKeysShortcutInput } from "./shortcuts/EchoKeysShortcutInput";
export { ShortcutInput } from "./shortcuts/ShortcutInput";
export { TranslateToEnglish } from "./advanced/TranslateToEnglish";
export { CustomWords } from "./advanced/CustomWords";
export { PostProcessingToggle } from "./post-processing/PostProcessingToggle";
export {
  PostProcessingSettingsApi,
  PostProcessingSettingsPrompts,
} from "./post-processing/PostProcessingSettings";
export { AppDataDirectory } from "./about/AppDataDirectory";
export { ModelUnloadTimeoutSetting } from "./advanced/ModelUnloadTimeout";
export { StartHidden } from "./general/StartHidden";
export { HistoryLimit } from "./history/HistoryLimit";
export { RecordingRetentionPeriodSelector } from "./history/RecordingRetentionPeriod";
export { AutostartToggle } from "./general/AutostartToggle";
export { AutoPunctuate } from "./advanced/AutoPunctuate";
export { AutoCapitalize } from "./advanced/AutoCapitalize";
export { UpdateChecksToggle } from "./general/UpdateChecksToggle";
