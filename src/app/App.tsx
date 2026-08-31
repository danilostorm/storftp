import { Cloud, Link2, Moon, Settings, Sun, Wifi, Zap } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { ConnectionManager } from "../components/ConnectionManager";
import { FilePane } from "../components/FilePane";
import { GoogleDriveModal } from "../components/GoogleDriveModal";
import { SettingsModal } from "../components/SettingsModal";
import { TransferPanel } from "../components/TransferPanel";
import { api } from "../lib/api";
import type { AppSettings, ConnectionProfile, FileEntry, ProviderRef, TransferJob } from "../types";

const defaultSettings: AppSettings = { theme: "dark", concurrentTransfers: 4, bufferSizeMiB: 4, uploadLimitBps: null, downloadLimitBps: null, verifyAfterTransfer: true, developerMode: false };
const local: ProviderRef = { kind: "local", connectionId: null };

function basename(path: string): string { return path.replace(/[\\/]+$/, "").split(/[\\/]/).at(-1) || "arquivo"; }
function join(base: string, name: string): string { if (/^[A-Za-z]:\\/.test(base)) return `${base.replace(/[\\/]+$/, "")}\\${name}`; return base === "/" ? `/${name}` : `${base.replace(/[\\/]+$/, "")}/${name}`; }

export function App() {
  const [connections, setConnections] = useState<ConnectionProfile[]>([]);
  const [leftProvider, setLeftProvider] = useState<ProviderRef>(local);
  const [rightProvider, setRightProvider] = useState<ProviderRef>(local);
  const [transfers, setTransfers] = useState<TransferJob[]>([]);
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [connectionOpen, setConnectionOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [googleOpen, setGoogleOpen] = useState(false);
  const [homePath, setHomePath] = useState("/");

  const loadConnections = useCallback(async () => { try { setConnections(await api.connections()); } catch (e) { console.error(e); } }, []);
  const loadTransfers = useCallback(async () => { try { setTransfers(await api.transfers()); } catch (e) { console.error(e); } }, []);

  useEffect(() => {
    void loadConnections(); void loadTransfers();
    void api.settings().then(setSettings).catch(() => undefined);
    void api.homeDirectory().then(setHomePath).catch(() => undefined);
    const timer = window.setInterval(() => void loadTransfers(), 900);
    return () => window.clearInterval(timer);
  }, [loadConnections, loadTransfers]);

  useEffect(() => {
    const root = document.documentElement;
    const resolved = settings.theme === "system" ? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light") : settings.theme;
    root.dataset.theme = resolved;
  }, [settings.theme]);

  const onLeftDrop = async (entry: FileEntry, sourceProvider: ProviderRef, sourcePath: string, destinationPath: string) => {
    if (entry.isDir) return;
    try { await api.enqueueTransfer(sourceProvider, leftProvider, sourcePath, join(destinationPath, basename(sourcePath))); await loadTransfers(); } catch (e) { console.error(e); }
  };
  const onRightDrop = async (entry: FileEntry, sourceProvider: ProviderRef, sourcePath: string, destinationPath: string) => {
    if (entry.isDir) return;
    try { await api.enqueueTransfer(sourceProvider, rightProvider, sourcePath, join(destinationPath, basename(sourcePath))); await loadTransfers(); } catch (e) { console.error(e); }
  };

  return <div className="app-shell">
    <header className="topbar">
      <div className="brand"><div className="brand-mark"><Zap size={20} /></div><div><strong>StorFTP</strong><span>Universal File Transfer & Cloud Manager</span></div></div>
      <div className="connection-health"><span className="health-dot" /><Wifi size={15} /><span>Transfer Engine pronto</span></div>
      <div className="top-actions">
        <button className="toolbar-button" onClick={() => setGoogleOpen(true)}><Cloud size={16} /> Google Drive</button>
        <button className="toolbar-button" onClick={() => setConnectionOpen(true)}><Link2 size={16} /> Conexões</button>
        <button className="icon-button" title="Alternar tema" onClick={() => setSettings((old) => ({ ...old, theme: old.theme === "dark" ? "light" : "dark" }))}>{settings.theme === "dark" ? <Sun size={17} /> : <Moon size={17} />}</button>
        <button className="icon-button" title="Configurações" onClick={() => setSettingsOpen(true)}><Settings size={17} /></button>
      </div>
    </header>

    <main className="workspace">
      <div className="panes">
        <FilePane title="ORIGEM / PAINEL A" provider={leftProvider} connections={connections} initialPath={homePath} onProviderChange={setLeftProvider} onDropEntry={onLeftDrop} />
        <div className="pane-divider"><Zap size={14} /></div>
        <FilePane title="DESTINO / PAINEL B" provider={rightProvider} connections={connections} initialPath={homePath} onProviderChange={setRightProvider} onDropEntry={onRightDrop} />
      </div>
      <TransferPanel transfers={transfers} onCancel={(id) => void api.cancelTransfer(id).then(loadTransfers)} onRetry={(id) => void api.retryTransfer(id).then(loadTransfers)} onPause={(id) => void api.pauseTransfer(id).then(loadTransfers)} onResume={(id) => void api.resumeTransfer(id).then(loadTransfers)} />
    </main>

    {connectionOpen && <ConnectionManager connections={connections} onClose={() => setConnectionOpen(false)} onChanged={loadConnections} />}
    {settingsOpen && <SettingsModal settings={settings} onClose={() => setSettingsOpen(false)} onSaved={setSettings} />}
    {googleOpen && <GoogleDriveModal onClose={() => setGoogleOpen(false)} onConnected={loadConnections} />}
  </div>;
}
