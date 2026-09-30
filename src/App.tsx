import { useEffect, useState, useRef } from "react";
import { toast, Toaster } from "sonner";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { listen } from "@tauri-apps/api/event";
import { platform } from "@tauri-apps/plugin-os";
import {
  checkAccessibilityPermission,
  checkMicrophonePermission,
} from "tauri-plugin-macos-permissions-api";
import { ArrowLeft } from "lucide-react";
import { ModelStateEvent, RecordingErrorEvent } from "./lib/types/events";
import AccessibilityPermissions from "./components/AccessibilityPermissions";
import Footer from "./components/footer";
import Onboarding, { AccessibilityOnboarding } from "./components/onboarding";
import { Sidebar, SidebarSection, SECTIONS_CONFIG } from "./components/Sidebar";
import { ShellHost } from "./shell/ShellHost";
import { ECHO_MODULES, SUITE_MODULES } from "./shell/modules";
import { systemSectionIds } from "./shell/shellUx";
import { useSettings } from "./hooks/useSettings";
import { useSettingsStore } from "./stores/settingsStore";
import { commands } from "@/bindings";
import { getLanguageDirection, initializeRTL } from "@/lib/utils/rtl";

import { OnboardingWizard } from "./onboarding/OnboardingWizard";
import { isOnboardingDone } from "./onboarding/wizardLogic";

type OnboardingStep = "accessibility" | "model" | "done";

// The Settings overlay's gear icon only lists sections that don't belong to
// any suite module (advanced/postprocessing/debug/about today) — module
// sections (general/models/karaoke/...) are reached through their module
// instead (Task 2, Shell UX Polish).
const SYSTEM_SECTION_IDS = systemSectionIds(
  Object.keys(SECTIONS_CONFIG),
  [...SUITE_MODULES, ...ECHO_MODULES].map((m) => m.sections),
) as SidebarSection[];

const renderSettingsContent = (section: SidebarSection, t: TFunction) => {
  const config = SECTIONS_CONFIG[section] || SECTIONS_CONFIG.general;
  const ActiveComponent = config.component;

  if (!ActiveComponent) {
    console.error(`ActiveComponent for section "${section}" is undefined!`, {
      section,
      config,
      SECTIONS_CONFIG,
    });
    return (
      <div className="p-8 text-err bg-err/10 border border-err/20 rounded-xl">
        <h2 className="text-xl font-bold">
          {t("errors.renderingSettingsTitle")}
        </h2>
        <p>{t("errors.renderingSettingsDescription", { section })}</p>
      </div>
    );
  }

  return <ActiveComponent />;
};

function App() {
  const { t, i18n } = useTranslation();
  const [onboardingStep, setOnboardingStep] = useState<OnboardingStep | null>(
    null,
  );
  // Track if this is a returning user who just needs to grant permissions
  // (vs a new user who needs full onboarding including model selection)
  const [isReturningUser, setIsReturningUser] = useState(false);
  const [currentSection, setCurrentSection] = useState<SidebarSection>(
    SYSTEM_SECTION_IDS[0] ?? "general",
  );
  // Suite-shell content (ShellHost/RailShell) is the default main-window view;
  // the gear icon in the shell switches to the legacy full Sidebar+section
  // settings screen (unchanged), which also still hosts Advanced/Debug/About/
  // post-processing (the "system" sections that aren't part of any module) —
  // module sections are filtered out (Task 2), reachable only via their
  // module's workspace instead.
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [showWizard, setShowWizard] = useState(false);

  useEffect(() => {
    if (!isOnboardingDone(window.localStorage)) {
      setShowWizard(true);
    }
  }, []);

  // The Settings overlay covers the still-mounted shell (Task 1). Mark the
  // shell subtree `inert` while Settings or Wizard is open so its focusable controls
  // (module cards, palette-trigger, gear) leave the Tab order + a11y tree
  // instead of being reachable behind the opaque overlay.
  const shellWrapperRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = shellWrapperRef.current;
    if (el) el.inert = settingsOpen || showWizard;
  }, [settingsOpen, showWizard]);
  const { settings, updateSetting } = useSettings();
  const direction = getLanguageDirection(i18n.language);
  const refreshAudioDevices = useSettingsStore(
    (state) => state.refreshAudioDevices,
  );
  const refreshOutputDevices = useSettingsStore(
    (state) => state.refreshOutputDevices,
  );
  const hasCompletedPostOnboardingInit = useRef(false);
  useEffect(() => {
    checkOnboardingStatus();
  }, []);

  // Initialize RTL direction when language changes
  useEffect(() => {
    initializeRTL(i18n.language);
  }, [i18n.language]);

  // Initialize Enigo, shortcuts, and refresh audio devices when main app loads
  useEffect(() => {
    if (onboardingStep === "done" && !hasCompletedPostOnboardingInit.current) {
      hasCompletedPostOnboardingInit.current = true;
      Promise.all([
        commands.initializeEnigo(),
        commands.initializeShortcuts(),
      ]).catch((e) => {
        console.warn("Failed to initialize:", e);
      });
      refreshAudioDevices();
      refreshOutputDevices();
    }
  }, [onboardingStep, refreshAudioDevices, refreshOutputDevices]);

  // Handle keyboard shortcuts for debug mode toggle
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Check for Ctrl+Shift+D (Windows/Linux) or Cmd+Shift+D (macOS)
      const isDebugShortcut =
        event.shiftKey &&
        event.key.toLowerCase() === "d" &&
        (event.ctrlKey || event.metaKey);

      if (isDebugShortcut) {
        event.preventDefault();
        const currentDebugMode = settings?.debug_mode ?? false;
        updateSetting("debug_mode", !currentDebugMode);
      }
    };

    // Add event listener when component mounts
    document.addEventListener("keydown", handleKeyDown);

    // Cleanup event listener when component unmounts
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [settings?.debug_mode, updateSetting]);

  // Listen for recording errors from the backend and show a toast
  useEffect(() => {
    const unlisten = listen<RecordingErrorEvent>("recording-error", (event) => {
      const { error_type, detail } = event.payload;

      if (error_type === "microphone_permission_denied") {
        const currentPlatform = platform();
        const platformKey = `errors.micPermissionDenied.${currentPlatform}`;
        const description = t(platformKey, {
          defaultValue: t("errors.micPermissionDenied.generic"),
        });
        toast.error(t("errors.micPermissionDeniedTitle"), { description });
      } else if (error_type === "no_input_device") {
        toast.error(t("errors.noInputDeviceTitle"), {
          description: t("errors.noInputDevice"),
        });
      } else {
        toast.error(
          t("errors.recordingFailed", { error: detail ?? "Unknown error" }),
        );
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  // Listen for paste failures and show a toast.
  // The technical error detail is logged to echo.log on the Rust side
  // (see actions.rs `error!("Failed to paste transcription: ...")`),
  // so we show a localized, user-friendly message here instead of the raw error.
  useEffect(() => {
    const unlisten = listen("paste-error", () => {
      toast.error(t("errors.pasteFailedTitle"), {
        description: t("errors.pasteFailed"),
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  // Listen for voice capture results and show a toast
  useEffect(() => {
    const un = listen<{ ok: boolean; path?: string; error?: string }>(
      "voice-capture",
      (e) => {
        if (e.payload.ok) toast.success(t("settings.capture.captureSuccess"));
        else toast.error(t("settings.capture.captureError"));
      },
    );
    return () => {
      un.then((f) => f());
    };
  }, [t]);

  // Listen for model loading failures and show a toast
  useEffect(() => {
    const unlisten = listen<ModelStateEvent>("model-state-changed", (event) => {
      if (event.payload.event_type === "loading_failed") {
        toast.error(
          t("errors.modelLoadFailed", {
            model:
              event.payload.model_name || t("errors.modelLoadFailedUnknown"),
          }),
          {
            description: event.payload.error,
          },
        );
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  const revealMainWindowForPermissions = async () => {
    try {
      await commands.showMainWindowCommand();
    } catch (e) {
      console.warn("Failed to show main window for permission onboarding:", e);
    }
  };

  const checkOnboardingStatus = async () => {
    console.log("Checking onboarding status...");
    try {
      // Check if they have any models available
      const result = await commands.hasAnyModelsAvailable();
      console.log("hasAnyModelsAvailable result:", result);
      const hasModels = result.status === "ok" && result.data;
      const currentPlatform = platform();

      if (hasModels) {
        // Returning user - check if they need to grant permissions first
        setIsReturningUser(true);

        if (currentPlatform === "macos") {
          try {
            const [hasAccessibility, hasMicrophone] = await Promise.all([
              checkAccessibilityPermission(),
              checkMicrophonePermission(),
            ]);
            if (!hasAccessibility || !hasMicrophone) {
              await revealMainWindowForPermissions();
              setOnboardingStep("accessibility");
              return;
            }
          } catch (e) {
            console.warn("Failed to check macOS permissions:", e);
            // If we can't check, proceed to main app and let them fix it there
          }
        }

        if (currentPlatform === "windows") {
          try {
            const microphoneStatus =
              await commands.getWindowsMicrophonePermissionStatus();
            if (
              microphoneStatus.supported &&
              microphoneStatus.overall_access === "denied"
            ) {
              await revealMainWindowForPermissions();
              setOnboardingStep("accessibility");
              return;
            }
          } catch (e) {
            console.warn("Failed to check Windows microphone permissions:", e);
            // If we can't check, proceed to main app and let them fix it there
          }
        }

        setOnboardingStep("done");
      } else {
        // New user - start full onboarding
        setIsReturningUser(false);
        setOnboardingStep("accessibility");
      }
    } catch (error) {
      console.error("Failed to check onboarding status:", error);
      setOnboardingStep("accessibility");
    }
  };

  const handleAccessibilityComplete = () => {
    // Returning users already have models, skip to main app
    // New users need to select a model
    setOnboardingStep(isReturningUser ? "done" : "model");
  };

  const handleModelSelected = () => {
    // Transition to main app - user has started a download
    setOnboardingStep("done");
  };

  // Still checking onboarding status
  if (onboardingStep === null) {
    return (
      <div className="flex h-screen items-center justify-center bg-ground text-white">
        <div className="flex flex-col items-center gap-4">
          <div className="w-8 h-8 border-4 border-accent border-t-transparent rounded-full animate-spin"></div>
          <p className="text-sm font-medium tracking-wide animate-pulse">
            Initializing Echo...
          </p>
        </div>
      </div>
    );
  }

  if (onboardingStep === "accessibility") {
    return <AccessibilityOnboarding onComplete={handleAccessibilityComplete} />;
  }

  if (onboardingStep === "model") {
    return <Onboarding onModelSelected={handleModelSelected} />;
  }

  return (
    <div
      dir={direction}
      className="h-screen flex flex-col select-none cursor-default bg-background"
    >
      <Toaster
        theme="system"
        toastOptions={{
          unstyled: true,
          classNames: {
            toast:
              "bg-background border border-mid-gray/20 rounded-lg shadow-lg px-4 py-3 flex items-center gap-3 text-sm",
            title: "font-medium",
            description: "text-mid-gray",
          },
        }}
      />
      {settingsOpen && (
        <div className="flex items-center px-4 py-2 border-b border-rule">
          <button
            type="button"
            className="flex items-center gap-1.5 rounded-md border border-edge bg-surface-raised px-2.5 py-1 text-xs text-primary hover:bg-edge transition-colors"
            onClick={() => setSettingsOpen(false)}
          >
            <ArrowLeft width={14} height={14} />
            {t("shell.rail.backToSuite")}
          </button>
        </div>
      )}
      {/* Main content area that takes remaining space. ShellHost renders
          ALWAYS here (Task 1) — Settings is a sibling overlay layer drawn on
          top of it instead of a hard swap, so ShellHost never unmounts (its
          global Cmd+K listener and activeModuleId/isModuleOpen state
          survive opening/closing Settings). */}
      <div className="flex-1 flex overflow-hidden relative">
        <div
          ref={shellWrapperRef}
          aria-hidden={settingsOpen || undefined}
          className="flex-1 flex flex-col overflow-hidden"
        >
          <AccessibilityPermissions />
          <div className="flex-1 min-h-0">
            <ShellHost
              onOpenSettings={() => setSettingsOpen(true)}
              settingsOpen={settingsOpen}
              onCloseSettings={() => setSettingsOpen(false)}
            />
          </div>
        </div>
        {settingsOpen && (
          <div className="absolute inset-0 z-10 flex bg-background">
            <Sidebar
              activeSection={currentSection}
              onSectionChange={setCurrentSection}
              sectionIds={SYSTEM_SECTION_IDS}
            />
            {/* Scrollable content area */}
            <div className="flex-1 overflow-y-auto custom-scrollbar">
              <div className="flex flex-col items-center p-4 gap-4">
                <AccessibilityPermissions />
                {renderSettingsContent(currentSection, t)}
              </div>
            </div>
          </div>
        )}
      </div>
      {/* Fixed footer at bottom */}
      <Footer />
      {showWizard && (
        <OnboardingWizard
          onClose={() => setShowWizard(false)}
          onOpenSettings={(section) => {
            if (section) setCurrentSection(section as SidebarSection);
            setSettingsOpen(true);
          }}
        />
      )}
    </div>
  );
}

export default App;
