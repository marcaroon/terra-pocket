// Thin bindings to the Rust commands/events in src-tauri/src/main.rs.
// All Bluetooth, media and volume logic stays in Rust (spec §5, §27).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Link = "stopped" | "scanning" | "connecting" | "ready" | "waiting" | "mismatch";

export interface NowPlaying {
  source: string;
  title: string;
  artist: string;
  playing: boolean;
}

export interface Volume {
  percent: number;
  muted: boolean;
}

export interface Status {
  link: Link;
  device: string | null;
  firmware: string | null;
  protocol: number | null;
  retryInS: number | null;
  media: NowPlaying | null;
  volume: Volume | null;
  lastMessage: string | null;
  companion: string;
}

export interface Settings {
  autoConnect: boolean;
  startWithWindows: boolean;
}

export const getStatus = () => invoke<Status>("get_status");
export const getSettings = () => invoke<Settings>("get_settings");
export const setAutoConnect = (enabled: boolean) => invoke<Settings>("set_auto_connect", { enabled });
export const setStartWithWindows = (enabled: boolean) => invoke<Settings>("set_start_with_windows", { enabled });
export const reconnect = () => invoke<void>("reconnect");

export const onStatus = (handler: (status: Status) => void): Promise<UnlistenFn> =>
  listen<Status>("status", (event) => handler(event.payload));
