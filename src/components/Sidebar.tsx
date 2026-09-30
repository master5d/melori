import React from "react";
import { useTranslation } from "react-i18next";
import {
  Cog,
  FlaskConical,
  History,
  Info,
  Sparkles,
  Cpu,
  LineChart,
  MessageCircle,
  FileAudio,
  Volume2,
  GraduationCap,
  Activity,
  Heart,
  Award,
  Video,
  Presentation,
  Zap,
  GlassWater,
  BookOpenCheck,
  Smile,
  Disc,
  Music,
  Gauge,
  Flame,
  UsersRound,
} from "lucide-react";
import { ClientsPage } from "@/consult/ClientsPage";
import { LinguaTutor } from "@/lingua/LinguaTutor";
import { PracticeWorkspace } from "@/practice/PracticeWorkspace";
import MeloriWordmark from "./icons/MeloriWordmark";
import EchoHand from "./icons/EchoHand";
import { useSettings } from "../hooks/useSettings";
import {
  GeneralSettings,
  AdvancedSettings,
  HistorySettings,
  CoachSettings,
  AssistantSettings,
  TtsSettings,
  TutorSettings,
  DebugSettings,
  AboutSettings,
  PostProcessingSettings,
  ModelsSettings,
  CourseSettings,
  HandpanSettings,
  KaraokeSettings,
  AudioSettings,
} from "./settings";
import {
  EngineSettings,
  PrivacySettings,
  AppearanceSettings,
} from "./settings/melori/ConsultSettings";
import { TranscribeFile } from "./settings/transcribe/TranscribeFile";
import TemplatesSettings from "./settings/melori/TemplatesSettings";

export type SidebarSection = keyof typeof SECTIONS_CONFIG;

interface IconProps {
  width?: number | string;
  height?: number | string;
  size?: number | string;
  className?: string;
  [key: string]: any;
}

interface SectionConfig {
  labelKey: string;
  icon: React.ComponentType<IconProps>;
  component: React.ComponentType;
  enabled: (settings: any) => boolean;
}

export const SECTIONS_CONFIG = {
  engine: {
    labelKey: "settings.melori.engine.title",
    icon: Cpu,
    component: EngineSettings,
    enabled: () => true,
  },
  templates: {
    labelKey: "settings.melori.templates.title",
    icon: BookOpenCheck,
    component: TemplatesSettings,
    enabled: () => true,
  },
  privacy: {
    labelKey: "settings.melori.privacy.title",
    icon: Heart,
    component: PrivacySettings,
    enabled: () => true,
  },
  appearance: {
    labelKey: "settings.melori.appearance.title",
    icon: Sparkles,
    component: AppearanceSettings,
    enabled: () => true,
  },
  general: {
    labelKey: "sidebar.general",
    icon: EchoHand,
    component: GeneralSettings,
    enabled: () => true,
  },
  audio: {
    labelKey: "sidebar.audio",
    icon: Gauge,
    component: AudioSettings,
    enabled: () => true,
  },
  models: {
    labelKey: "sidebar.models",
    icon: Cpu,
    component: ModelsSettings,
    enabled: () => true,
  },
  advanced: {
    labelKey: "sidebar.advanced",
    icon: Cog,
    component: AdvancedSettings,
    enabled: () => true,
  },
  history: {
    labelKey: "sidebar.history",
    icon: History,
    component: HistorySettings,
    enabled: () => true,
  },
  transcribe: {
    labelKey: "sidebar.transcribe",
    icon: FileAudio,
    component: TranscribeFile,
    enabled: () => true,
  },
  coach: {
    labelKey: "sidebar.coach",
    icon: LineChart,
    component: CoachSettings,
    enabled: () => true,
  },
  assistant: {
    labelKey: "sidebar.assistant",
    icon: MessageCircle,
    component: AssistantSettings,
    enabled: () => true,
  },
  tts: {
    labelKey: "sidebar.tts",
    icon: Volume2,
    component: TtsSettings,
    enabled: () => true,
  },
  tutor: {
    labelKey: "sidebar.tutor",
    icon: GraduationCap,
    component: TutorSettings,
    enabled: () => true,
  },
  postprocessing: {
    labelKey: "sidebar.postProcessing",
    icon: Sparkles,
    component: PostProcessingSettings,
    enabled: (settings) => settings?.post_process_enabled ?? false,
  },
  debug: {
    labelKey: "sidebar.debug",
    icon: FlaskConical,
    component: DebugSettings,
    enabled: (settings) => settings?.debug_mode ?? false,
  },
  about: {
    labelKey: "sidebar.about",
    icon: Info,
    component: AboutSettings,
    enabled: () => true,
  },
  linguaTutor: {
    labelKey: "sidebar.linguaTutor",
    icon: Sparkles,
    component: LinguaTutor,
    enabled: () => true,
  },
  practice: {
    labelKey: "sidebar.practice",
    icon: Flame,
    component: PracticeWorkspace,
    enabled: () => true,
  },
  dictionTraining: {
    labelKey: "sidebar.dictionTraining",
    icon: Volume2,
    component: () => <CourseSettings courseId="dictionTraining" />,
    enabled: () => true,
  },
  menVoice: {
    labelKey: "sidebar.menVoice",
    icon: Activity,
    component: () => <CourseSettings courseId="menVoice" />,
    enabled: () => true,
  },
  womenVoice: {
    labelKey: "sidebar.womenVoice",
    icon: Heart,
    component: () => <CourseSettings courseId="womenVoice" />,
    enabled: () => true,
  },
  voiceTraining: {
    labelKey: "sidebar.voiceTraining",
    icon: Award,
    component: () => <CourseSettings courseId="voiceTraining" />,
    enabled: () => true,
  },
  onlineTraining: {
    labelKey: "sidebar.onlineTraining",
    icon: Video,
    component: () => <CourseSettings courseId="onlineTraining" />,
    enabled: () => true,
  },
  oratoryTraining: {
    labelKey: "sidebar.oratoryTraining",
    icon: Presentation,
    component: () => <CourseSettings courseId="oratoryTraining" />,
    enabled: () => true,
  },
  speechImprov: {
    labelKey: "sidebar.speechImprov",
    icon: Zap,
    component: () => <CourseSettings courseId="speechImprov" />,
    enabled: () => true,
  },
  toastTraining: {
    labelKey: "sidebar.toastTraining",
    icon: GlassWater,
    component: () => <CourseSettings courseId="toastTraining" />,
    enabled: () => true,
  },
  readingTraining: {
    labelKey: "sidebar.readingTraining",
    icon: BookOpenCheck,
    component: () => <CourseSettings courseId="readingTraining" />,
    enabled: () => true,
  },
  rhetoricTraining: {
    labelKey: "sidebar.rhetoricTraining",
    icon: Smile,
    component: () => <CourseSettings courseId="rhetoricTraining" />,
    enabled: () => true,
  },
  cameraTraining: {
    labelKey: "sidebar.cameraTraining",
    icon: Video,
    component: () => <CourseSettings courseId="cameraTraining" />,
    enabled: () => true,
  },
  handpan: {
    labelKey: "sidebar.handpan",
    icon: Disc,
    component: HandpanSettings,
    enabled: () => true,
  },
  karaoke: {
    labelKey: "sidebar.karaoke",
    icon: Music,
    component: KaraokeSettings,
    enabled: () => true,
  },
  clients: {
    labelKey: "sidebar.clients",
    icon: UsersRound,
    component: ClientsPage,
    enabled: () => true,
  },
} as const satisfies Record<string, SectionConfig>;

interface SidebarProps {
  activeSection: SidebarSection;
  onSectionChange: (section: SidebarSection) => void;
  /** Optional allow-list of section ids to render, e.g. the Settings
   * overlay's gear icon restricting the legacy Sidebar to system-only
   * sections (Task 2, Shell UX Polish). Intersected with the existing
   * `.enabled(settings)` filter and `SECTIONS_CONFIG` order; when omitted
   * (Onboarding's use of Sidebar), all enabled sections render as before. */
  sectionIds?: SidebarSection[];
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
  sectionIds,
}) => {
  const { t } = useTranslation();
  const { settings } = useSettings();

  const allowedIds = sectionIds ? new Set(sectionIds) : null;
  const availableSections = Object.entries(SECTIONS_CONFIG)
    .filter(
      ([id, config]) =>
        config.enabled(settings) &&
        (!allowedIds || allowedIds.has(id as SidebarSection)),
    )
    .map(([id, config]) => ({ id: id as SidebarSection, ...config }));

  return (
    <div className="flex flex-col w-52 h-full border-e border-rule items-center px-4 bg-surface">
      <div className="py-8">
        <MeloriWordmark size={34} />
      </div>
      <div className="flex flex-col w-full items-center gap-2 pt-6 pb-4 border-t border-surface-raised/50 flex-1 min-h-0 overflow-y-auto">
        {availableSections.map((section) => {
          const Icon = section.icon;
          const isActive = activeSection === section.id;

          return (
            <button
              key={section.id}
              type="button"
              aria-current={isActive ? "page" : undefined}
              className={`
                flex flex-row items-center w-full px-4 py-3 rounded-xl cursor-pointer transition-[colors,transform] duration-300 group text-left
                focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-surface
                ${
                  isActive
                    ? "bg-accent/90 text-white shadow-lg shadow-accent/20 translate-x-1"
                    : "text-secondary hover:bg-surface-raised/50 hover:text-primary"
                }
              `}
              onClick={() => onSectionChange(section.id)}
            >
              <Icon
                width={20}
                height={20}
                className={`shrink-0 transition-colors duration-300 ${
                  isActive
                    ? "text-white"
                    : "text-secondary group-hover:text-accent"
                }`}
              />
              <p
                className="text-[13px] font-bold tracking-tight ml-3 truncate"
                title={t(section.labelKey)}
              >
                {t(section.labelKey)}
              </p>
            </button>
          );
        })}
      </div>
    </div>
  );
};
