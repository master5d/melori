import { describe, expect, it, vi } from "vitest";
import {
  paletteCommands,
  filterPaletteCommands,
  type PaletteActions,
} from "./CommandPalette";

function makeActions(): PaletteActions {
  return {
    onOpenClients: vi.fn(),
    onNewClient: vi.fn(),
    onStartMeeting: vi.fn(),
    onOpenSettings: vi.fn(),
    onChangeTheme: vi.fn(),
  };
}

describe("paletteCommands", () => {
  it("contains only the consultation command set", () => {
    const commands = paletteCommands(makeActions());
    expect(commands.map((command) => command.id)).toEqual([
      "open-clients",
      "new-client",
      "start-meeting",
      "open-settings",
      "theme:system",
      "theme:day",
      "theme:evening",
    ]);
    expect(commands.map((command) => command.titleKey)).toEqual([
      "shell.palette.clients",
      "shell.palette.newClient",
      "shell.palette.startMeeting",
      "shell.palette.settings",
      "shell.palette.theme.system",
      "shell.palette.theme.day",
      "shell.palette.theme.evening",
    ]);
  });

  it("dispatches consultation and theme actions", () => {
    const actions = makeActions();
    const commands = paletteCommands(actions);
    commands.find((command) => command.id === "new-client")!.run();
    commands.find((command) => command.id === "start-meeting")!.run();
    commands.find((command) => command.id === "theme:evening")!.run();
    expect(actions.onNewClient).toHaveBeenCalledOnce();
    expect(actions.onStartMeeting).toHaveBeenCalledOnce();
    expect(actions.onChangeTheme).toHaveBeenCalledWith("evening");
  });

  it("every command carries a titleKey and at least one keyword", () => {
    for (const command of paletteCommands(makeActions())) {
      expect(command.titleKey).toMatch(/^shell\.palette\./);
      expect(command.keywords.length).toBeGreaterThan(0);
    }
  });

  it("returns all commands for an empty/whitespace query", () => {
    const commands = paletteCommands(makeActions());
    expect(filterPaletteCommands(commands, "")).toEqual(commands);
    expect(filterPaletteCommands(commands, "   ")).toEqual(commands);
  });

  it("returns an empty array when nothing matches", () => {
    expect(
      filterPaletteCommands(
        paletteCommands(makeActions()),
        "zzzz-no-such-command",
      ),
    ).toEqual([]);
  });

  it("finds a theme by its Russian name", () => {
    const commands = paletteCommands(makeActions());
    expect(filterPaletteCommands(commands, "вечерн").map((c) => c.id)).toEqual([
      "theme:evening",
    ]);
    expect(filterPaletteCommands(commands, "Дневная").map((c) => c.id)).toEqual(
      ["theme:day"],
    );
  });

  it("filters by English and Russian command terms", () => {
    const commands = paletteCommands(makeActions());
    expect(
      filterPaletteCommands(commands, "клиент").map((c) => c.id),
    ).toContain("new-client");
    expect(
      filterPaletteCommands(commands, "meeting").map((c) => c.id),
    ).toContain("start-meeting");
  });
});
