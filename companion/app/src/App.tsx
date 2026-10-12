import { useEffect, useState } from "react";
import {
  getSettings,
  getStatus,
  onStatus,
  reconnect,
  setAutoConnect,
  setStartWithWindows,
  type Link,
  type Settings,
  type Status,
} from "./api";

const LINK_TEXT: Record<Link, string> = {
  stopped: "Not connecting",
  scanning: "Searching…",
  connecting: "Connecting…",
  ready: "Connected",
  waiting: "Disconnected",
  mismatch: "Protocol mismatch",
};

function linkText(status: Status): string {
  if (status.link === "waiting" && status.retryInS !== null) {
    return `Disconnected · retry in ${status.retryInS} s`;
  }
  return LINK_TEXT[status.link];
}

function Toggle(props: { label: string; checked: boolean; onChange: (value: boolean) => void }) {
  return (
    <label className="row toggle">
      <span>{props.label}</span>
      <input type="checkbox" checked={props.checked} onChange={(e) => props.onChange(e.target.checked)} />
    </label>
  );
}

export default function App() {
  const [status, setStatus] = useState<Status | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = onStatus(setStatus);
    getStatus().then(setStatus).catch((e) => setError(String(e)));
    getSettings().then(setSettings).catch((e) => setError(String(e)));
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  const update = (request: Promise<Settings>) => {
    request.then(setSettings).catch((e) => setError(String(e)));
  };

  if (!status || !settings) {
    return <main className="app">{error ?? "Loading…"}</main>;
  }

  const ready = status.link === "ready";
  return (
    <main className="app">
      <header>Terra</header>

      <section>
        <h2>Device</h2>
        <div className="device">
          <span className={`dot ${status.link}`} aria-hidden />
          <span>{status.device ? `Terra ${status.device}` : "Terra"}</span>
        </div>
        <div className="secondary">{linkText(status)}</div>
        <button type="button" onClick={() => reconnect().catch((e) => setError(String(e)))}>
          Reconnect
        </button>
      </section>

      <section>
        <h2>Now Playing</h2>
        {ready && status.media ? (
          <>
            <div className="title">{status.media.title || "Unknown title"}</div>
            <div className="secondary">{status.media.artist}</div>
            <div className="meta">
              {status.media.source} · {status.media.playing ? "Playing" : "Paused"}
            </div>
          </>
        ) : (
          <div className="secondary">Nothing Playing</div>
        )}
      </section>

      <section>
        <h2>Volume</h2>
        <div className="title">
          {ready && status.volume ? (status.volume.muted ? "Muted" : `${status.volume.percent}%`) : "--"}
        </div>
      </section>

      <section>
        <Toggle
          label="Auto Connect"
          checked={settings.autoConnect}
          onChange={(value) => update(setAutoConnect(value))}
        />
        <Toggle
          label="Start with Windows"
          checked={settings.startWithWindows}
          onChange={(value) => update(setStartWithWindows(value))}
        />
      </section>

      <section className="info">
        <div className="row">
          <span>Firmware</span>
          <span>{status.firmware ?? "--"}</span>
        </div>
        <div className="row">
          <span>Protocol</span>
          <span>{status.protocol ?? "--"}</span>
        </div>
        <div className="row">
          <span>Companion</span>
          <span>{status.companion}</span>
        </div>
      </section>

      {(error ?? status.lastMessage) && <footer title="Last companion message">{error ?? status.lastMessage}</footer>}
    </main>
  );
}
