import type { EngineClient } from "./api";

export type EngineState = "starting" | "ready" | "down";
export type DiskEncryption = "on" | "off" | "unknown" | null;
export interface ClientsPageStateInput {
  engine: EngineState;
  diskEncryption: DiskEncryption;
  clients: EngineClient[] | null;
  purged: string[];
}
export interface ClientsPageState {
  banners: Array<"engine-down" | "disk-off" | "disk-unknown" | "purged">;
  canCreate: boolean;
  view: "list" | "empty" | "loading" | "unavailable";
}

export function normalizeEngineState(
  value: string | null | undefined,
): EngineState {
  if (value === "starting" || value === "ready") return value;
  return "down";
}

export function clientsPageState(
  input: ClientsPageStateInput,
): ClientsPageState {
  const banners: ClientsPageState["banners"] = [];
  if (input.engine === "down") banners.push("engine-down");
  if (input.diskEncryption === "off") banners.push("disk-off");
  if (input.diskEncryption === "unknown") banners.push("disk-unknown");
  if (input.purged.length > 0) banners.push("purged");
  // список неизвестен: пока движок поднимается — «загрузка»; движок лёг — «недоступно»,
  // а не «журнал пуст» (пустоту утверждаем только по ответу движка)
  const view =
    input.clients === null
      ? input.engine === "down"
        ? "unavailable"
        : "loading"
      : input.clients.length > 0
        ? "list"
        : "empty";
  return {
    banners,
    // диск — предупреждение (баннер), не запрет: блокирует только недоступный движок
    canCreate: input.engine === "ready",
    view,
  };
}
