import { ArrowLeft, ArrowUp, Folder, HardDrive, RefreshCw, Search, UploadCloud } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api } from "../lib/api";
import { formatBytes } from "../lib/format";
import type { ConnectionProfile, FileEntry, ProviderRef } from "../types";
import { ProviderPicker } from "./ProviderPicker";

interface Props {
  title: string;
  provider: ProviderRef;
  connections: ConnectionProfile[];
  initialPath: string;
  onProviderChange: (provider: ProviderRef) => void;
  onDropEntry: (entry: FileEntry, sourceProvider: ProviderRef, sourcePath: string, destinationPath: string) => void;
}

function parentPath(path: string): string {
  const normalized = path.replace(/\\/g, "/");
  if (normalized === "/" || /^[A-Za-z]:\/?$/.test(normalized)) return path;
  const parent = normalized.replace(/\/+$/, "").split("/").slice(0, -1).join("/") || "/";
  return parent;
}

function joinPath(base: string, name: string): string {
  if (/^[A-Za-z]:\\/.test(base)) return `${base.replace(/[\\/]+$/, "")}\\${name}`;
  if (base === "/") return `/${name}`;
  return `${base.replace(/\/+$/, "")}/${name}`;
}

export function FilePane({ title, provider, connections, initialPath, onProviderChange, onDropEntry }: Props) {
  const [path, setPath] = useState(initialPath);
  const [entries, setEntries] = useState<FileEntry[]>([]);
  const [query, setQuery] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [history, setHistory] = useState<string[]>([]);

  const refresh = async (target = path) => {
    setLoading(true);
    setError(null);
    try {
      const result = await api.list(provider, target || "/");
      setEntries(result);
      setPath(target || "/");
    } catch (err) {
      const value = err as { message?: string }; setError(value?.message ?? String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    const profile = connections.find((item) => item.id === provider.connectionId);
    const nextPath = provider.kind === "local" ? initialPath : (profile?.initialPath || "/");
    setPath(nextPath);
    setHistory([]);
    void refresh(nextPath);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [provider.kind, provider.connectionId]);

  const visibleEntries = useMemo(() => {
    const term = query.trim().toLowerCase();
    return entries
      .filter((entry) => !term || entry.name.toLowerCase().includes(term))
      .sort((a, b) => Number(b.isDir) - Number(a.isDir) || a.name.localeCompare(b.name));
  }, [entries, query]);

  const navigate = (next: string) => {
    setHistory((items) => [...items, path]);
    void refresh(next);
  };

  return (
    <section className="file-pane" onDragOver={(event) => event.preventDefault()} onDrop={(event) => {
      event.preventDefault();
      const raw = event.dataTransfer.getData("application/storftp-entry");
      if (!raw) return;
      try {
        const payload = JSON.parse(raw) as { entry: FileEntry; provider: ProviderRef; sourcePath: string };
        onDropEntry(payload.entry, payload.provider, payload.sourcePath, path);
      } catch {
        // Ignore malformed external drag data.
      }
    }}>
      <div className="pane-heading">
        <div>
          <span className="eyebrow">{title}</span>
          <ProviderPicker value={provider} connections={connections} onChange={onProviderChange} />
        </div>
        <button className="icon-button" title="Atualizar" onClick={() => void refresh()} disabled={loading}>
          <RefreshCw size={16} className={loading ? "spin" : ""} />
        </button>
      </div>

      <div className="path-bar">
        <button className="icon-button compact" title="Voltar" disabled={history.length === 0} onClick={() => {
          const previous = history.at(-1);
          if (!previous) return;
          setHistory((items) => items.slice(0, -1));
          void refresh(previous);
        }}><ArrowLeft size={15} /></button>
        <button className="icon-button compact" title="Pasta acima" onClick={() => void refresh(parentPath(path))}><ArrowUp size={15} /></button>
        <HardDrive size={15} className="muted-icon" />
        <input value={path} onChange={(event) => setPath(event.target.value)} onKeyDown={(event) => {
          if (event.key === "Enter") void refresh(path);
        }} aria-label="Caminho atual" />
      </div>

      <div className="filter-bar">
        <Search size={15} />
        <input placeholder="Pesquisar nesta pasta" value={query} onChange={(event) => setQuery(event.target.value)} />
        <span>{visibleEntries.length} itens</span>
      </div>

      <div className="file-table" role="table">
        <div className="file-row header" role="row">
          <span>Nome</span><span>Tamanho</span><span>Modificado</span>
        </div>
        {error && <div className="pane-message error-message">{error}</div>}
        {!error && !loading && visibleEntries.length === 0 && <div className="pane-message">Pasta vazia</div>}
        {visibleEntries.map((entry) => (
          <div
            className="file-row"
            role="row"
            key={`${entry.path}-${entry.name}`}
            draggable
            onDragStart={(event) => event.dataTransfer.setData("application/storftp-entry", JSON.stringify({ entry, provider, sourcePath: entry.path }))}
            onDoubleClick={() => entry.isDir && navigate(entry.path || joinPath(path, entry.name))}
          >
            <span className="file-name">{entry.isDir ? <Folder size={17} /> : <UploadCloud size={16} />}<span>{entry.name}</span></span>
            <span>{entry.isDir ? "—" : formatBytes(entry.size)}</span>
            <span>{entry.modifiedAt ? new Date(entry.modifiedAt * 1000).toLocaleString() : "—"}</span>
          </div>
        ))}
      </div>
    </section>
  );
}
