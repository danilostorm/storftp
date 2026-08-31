import { Cloud, Fingerprint, KeyRound, Plus, Save, Server, Star, Trash2, X } from "lucide-react";
import { useMemo, useState } from "react";
import { api } from "../lib/api";
import type { ConnectionProfile, ProviderKind } from "../types";

interface Props {
  connections: ConnectionProfile[];
  onClose: () => void;
  onChanged: () => Promise<void>;
}

const emptyProfile = (): ConnectionProfile => ({
  id: crypto.randomUUID(),
  name: "Nova conexão",
  kind: "sftp",
  host: "",
  port: 22,
  username: "",
  initialPath: "/",
  timeoutSeconds: 30,
  keepAliveSeconds: 30,
  maxConnections: 4,
  groupName: null,
  tags: [],
  favorite: false,
  extra: {}
});

export function ConnectionManager({ connections, onClose, onChanged }: Props) {
  const [selectedId, setSelectedId] = useState<string | null>(connections[0]?.id ?? null);
  const source = useMemo(() => connections.find((item) => item.id === selectedId) ?? null, [connections, selectedId]);
  const [draft, setDraft] = useState<ConnectionProfile>(source ? structuredClone(source) : emptyProfile());
  const [secret, setSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const select = (profile: ConnectionProfile) => {
    setSelectedId(profile.id);
    setDraft(structuredClone(profile));
    setSecret("");
    setMessage(null);
  };

  const changeKind = (kind: ProviderKind) => {
    const defaultPort = kind === "sftp" ? 22 : kind === "ftps" ? 990 : 21;
    setDraft((old) => ({ ...old, kind, port: defaultPort }));
  };

  const save = async () => {
    setBusy(true); setMessage(null);
    try {
      const saved = await api.saveConnection(draft, secret || null);
      setSelectedId(saved.id);
      await onChanged();
      setMessage("Conexão salva com credenciais protegidas pelo sistema operacional.");
      setSecret("");
    } catch (error) {
      setMessage(String(error));
    } finally { setBusy(false); }
  };

  return (
    <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className="modal connection-modal">
        <header className="modal-header"><div><span className="eyebrow">StorFTP</span><h2>Gerenciador de conexões</h2></div><button className="icon-button" onClick={onClose}><X size={18} /></button></header>
        <div className="connection-layout">
          <aside className="connection-sidebar">
            <button className="primary-button full" onClick={() => { const next = emptyProfile(); setSelectedId(null); setDraft(next); setSecret(""); }}><Plus size={16} /> Nova conexão</button>
            <div className="connection-list">
              {connections.map((item) => <button key={item.id} className={`connection-card ${selectedId === item.id ? "active" : ""}`} onClick={() => select(item)}>
                {item.kind === "google_drive" ? <Cloud size={17} /> : <Server size={17} />}
                <span><strong>{item.name}</strong><small>{item.kind.toUpperCase()} {item.host ? `· ${item.host}` : ""}</small></span>
                {item.favorite && <Star size={14} />}
              </button>)}
            </div>
          </aside>
          <main className="connection-form">
            <div className="form-grid two">
              <label><span>Nome</span><input value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} /></label>
              <label><span>Protocolo</span><select value={draft.kind} onChange={(e) => changeKind(e.target.value as ProviderKind)}>
                <option value="sftp">SFTP</option><option value="ftp">FTP</option><option value="ftpes">FTPES (TLS explícito)</option><option value="ftps">FTPS (TLS implícito)</option>
              </select></label>
              <label><span>Host</span><input placeholder="servidor.exemplo.com" value={draft.host ?? ""} onChange={(e) => setDraft({ ...draft, host: e.target.value })} /></label>
              <label><span>Porta</span><input type="number" value={draft.port ?? 22} onChange={(e) => setDraft({ ...draft, port: Number(e.target.value) })} /></label>
              <label><span>Usuário</span><input value={draft.username ?? ""} onChange={(e) => setDraft({ ...draft, username: e.target.value })} /></label>
              <label><span>Senha / passphrase</span><div className="input-with-icon"><KeyRound size={15} /><input type="password" placeholder={selectedId ? "Deixe vazio para manter" : "Credencial"} value={secret} onChange={(e) => setSecret(e.target.value)} /></div></label>
              {draft.kind === "sftp" && <label><span>Chave SSH privada</span><input placeholder="~/.ssh/id_ed25519" value={String(draft.extra.keyPath ?? "")} onChange={(e) => setDraft({ ...draft, extra: { ...draft.extra, keyPath: e.target.value } })} /></label>}
              <label><span>Pasta inicial</span><input value={draft.initialPath ?? "/"} onChange={(e) => setDraft({ ...draft, initialPath: e.target.value })} /></label>
              <label><span>Grupo</span><input value={draft.groupName ?? ""} onChange={(e) => setDraft({ ...draft, groupName: e.target.value || null })} /></label>
              <label><span>Timeout (s)</span><input type="number" min={5} value={draft.timeoutSeconds} onChange={(e) => setDraft({ ...draft, timeoutSeconds: Number(e.target.value) })} /></label>
              <label><span>Keep-alive (s)</span><input type="number" min={0} value={draft.keepAliveSeconds} onChange={(e) => setDraft({ ...draft, keepAliveSeconds: Number(e.target.value) })} /></label>
              <label><span>Conexões máximas</span><input type="number" min={1} max={32} value={draft.maxConnections} onChange={(e) => setDraft({ ...draft, maxConnections: Number(e.target.value) })} /></label>
              <label><span>Tags</span><input value={draft.tags.join(", ")} onChange={(e) => setDraft({ ...draft, tags: e.target.value.split(",").map((x) => x.trim()).filter(Boolean) })} /></label>
            </div>
            <label className="check-row"><input type="checkbox" checked={draft.favorite} onChange={(e) => setDraft({ ...draft, favorite: e.target.checked })} /> Favorito</label>
            <div className="security-note">Senhas e tokens não são gravados no SQLite nem no localStorage. O StorFTP usa o cofre de credenciais do Windows ou Secret Service/Keyring no Linux.</div>
            {selectedId && draft.kind === "sftp" && <button className="secondary-button host-verify" disabled={busy} onClick={async () => {
              setBusy(true); setMessage(null);
              try { const probe = await api.probeSftpHost(selectedId); const approved = window.confirm(`Servidor desconhecido\n\n${probe.host}:${probe.port}\n${probe.fingerprint}\n\nConfirme este fingerprint com o administrador do servidor. Confiar sempre neste host?`); if (approved) { await api.trustSftpHost(selectedId, probe.fingerprint); await onChanged(); setMessage(`Host confiável salvo: ${probe.fingerprint}`); } } catch (error) { setMessage(String(error)); } finally { setBusy(false); }
            }}><Fingerprint size={15} /> Verificar fingerprint SFTP</button>}
            {message && <div className="inline-message">{message}</div>}
            <div className="modal-actions">
              {selectedId && <button className="danger-button" onClick={async () => { if (!selectedId) return; await api.deleteConnection(selectedId); await onChanged(); const next = emptyProfile(); setSelectedId(null); setDraft(next); }}><Trash2 size={15} /> Excluir</button>}
              <span />
              <button className="secondary-button" onClick={onClose}>Cancelar</button>
              <button className="primary-button" disabled={busy || !draft.name.trim() || !draft.host?.trim()} onClick={() => void save()}><Save size={15} /> Salvar</button>
            </div>
          </main>
        </div>
      </div>
    </div>
  );
}
