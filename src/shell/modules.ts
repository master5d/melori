import type { LucideIcon } from "lucide-react";
import {
  Mic,
  MessageCircle,
  Music2,
  Languages,
  UserRound,
  Wand2,
  UsersRound,
} from "lucide-react";
import type { SidebarSection } from "../components/Sidebar";

/** A section id from `Sidebar.SECTIONS_CONFIG` (the single source of truth for
 * mountable settings/workspace content). */
export type SectionId = SidebarSection;

export type ModuleId =
  | "dictate"
  | "assistant"
  | "vocal"
  | "language"
  | "clone"
  | "studio"
  | "consult";

export interface ModuleDef {
  id: ModuleId;
  titleKey: string;
  icon: LucideIcon;
  sections: SectionId[];
}

/** The melori shell exposes one consultation module. */
export const SUITE_MODULES: ModuleDef[] = [
  {
    id: "consult",
    titleKey: "shell.modules.consult",
    icon: UsersRound,
    sections: ["clients"],
  },
];

/** Inherited echo modules remain in the tree but are not mounted by melori. */
export const ECHO_MODULES: ModuleDef[] = [
  {
    id: "dictate",
    titleKey: "shell.modules.dictate",
    icon: Mic,
    sections: ["history", "transcribe"],
  },
  {
    id: "assistant",
    titleKey: "shell.modules.assistant",
    icon: MessageCircle,
    sections: ["assistant", "tts"],
  },
  {
    id: "vocal",
    titleKey: "shell.modules.vocal",
    icon: Music2,
    sections: ["menVoice", "womenVoice", "voiceTraining", "handpan", "karaoke"],
  },
  {
    id: "language",
    titleKey: "shell.modules.language",
    icon: Languages,
    sections: [
      "practice",
      "linguaTutor",
      "tutor",
      "coach",
      "dictionTraining",
      "onlineTraining",
      "oratoryTraining",
      "speechImprov",
      "toastTraining",
      "readingTraining",
      "rhetoricTraining",
      "cameraTraining",
    ],
  },
  {
    id: "clone",
    titleKey: "shell.modules.clone",
    icon: UserRound,
    sections: [],
  },
  {
    id: "studio",
    titleKey: "shell.modules.studio",
    icon: Wand2,
    sections: [],
  },
];

/** System/settings sections — not owned by any suite module. This is a curated
 * doc/test constant: `modules.test.ts` asserts it equals the complement of the
 * module sections (guarding that every section is categorized). The LIVE gear
 * filter is computed in `App.tsx` via `systemSectionIds(...)`, so editing this
 * list alone does NOT change the gear — the test will flag any divergence. */
export const SYSTEM_SECTIONS: SectionId[] = [
  "engine",
  "templates",
  "general",
  "models",
  "audio",
  "privacy",
  "appearance",
  "advanced",
  "debug",
  "about",
  "postprocessing",
];
