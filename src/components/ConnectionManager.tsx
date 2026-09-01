import {
  Activity,
  Cloud,
  Download,
  Fingerprint,
  KeyRound,
  Plus,
  Save,
  Server,
  Star,
  Trash2,
  X
} from "lucide-react";
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
  extra: { authMethod: "auto" }
});

export function ConnectionManager({ connections, onClose, onChanged }: Props) {
  const [selectedId, setSelectedId] = useState<string | null>(connections[0]?.id ?? null);
  const source = useMemo(
    () => connections.find((item) => item.id === selectedId) ?? null,
    [connections, selectedId]
  );
  const [draft, setDraft] = useState<ConnectionProfile>(source ? structuredClone(source) : emptyProfile());
  const [secret, setSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const authMethod = String(draft.extra.authMethod ?? "auto");

  const select = (profile: ConnectionProfile) => {
    setSelectedId(profile.id);
    setDraft(structuredClone(profile));
    setSecret("");
    setMessage(null);
  };

  const newConnection = () => {
    const next = emptyProfile();
    setSelectedId(null);
    setDraft(next);
    setSecret("");
    setMessage(null);
  };

  const changeKind = (kind: ProviderKind) => {
    const defaultPort = kind === "sftp" ? 22 : kind === "ftps" ? 990 : 21;
    setDraft((old) => ({
      ...old,
      kind,
      port: defaultPort,
      extra: kind === "sftp" ? { ...old.extra, authMethod: old.extra.authMethod ?? "auto" } : old.extra
    }));
  };

  const save = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const saved = await api.saveConnection(draft, secret || null);
      setSelectedId(saved.id);
      setDraft(structuredClone(saved));
      await onChanged();
      setMessage("Conexão salva. A credencial ficou protegida pelo cofre do sistema operacional.");
      setSecret("");
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  };

  const test = async () => {
    setBusy(true);
    setMessage(null);
    try {
      let testProfile = structuredClone(draft);

      if (testProfile.kind === "sftp") {
        const probe = await api.probeSftpProfile(testProfile);
        const trusted = String(testProfile.extra.hostFingerprint ?? "");
        if (trusted !== probe.fingerprint) {
          const approved = window.confirm(
            `Fingerprint do servidor SFTP\n\n${probe.host}:${probe.port}\n${probe.fingerprint}\n\n` +
              "Confira esse fingerprint com o administrador. Deseja confiar nele para este perfil e continuar o teste?"
          );
          if (!approved) {
            setMessage("Teste cancelado antes da autenticação. O fingerprint não foi aceito.");
            return;
          }
          testProfile = {
            ...testProfile,
            extra: { ...testProfile.extra, hostFingerprint: probe.fingerprint }
          };
          setDraft(testProfile);
        }
      }

      const result = await api.testConnection(testProfile, secret || null);
      const saveHint = testProfile.kind === "sftp" ? " Clique em Salvar para guardar qualquer alteração/fingerprint." : "";
      setMessage(`✓ ${result.message} (${result.latencyMs} ms).${saveHint}`);
    } catch (error) {
      setMessage(`Falha no teste: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  };

  const importConnections = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const report = await api.importConnections();
      await onChanged();
      if (report.imported[0]) {
        setSelectedId(report.imported[0].id);
        setDraft(structuredClone(report.imported[0]));
        setSecret("");
      }
      const warning = report.warnings.length ? ` Avisos: ${report.warnings.join(" | ")}` : "";
      if (report.imported.length === 0) {
        setMessage(
          `Nenhuma conexão nova encontrada. ${report.skipped} já existente(s) foram ignoradas. O StorFTP procura FileZilla, Remmina e ~/.ssh/config automaticamente.${warning}`
        );
      } else {
        setMessage(
          `Importação concluída: ${report.imported.length} conexão(ões) adicionada(s), ${report.skipped} duplicada(s) ignorada(s).${warning}`
        );
      }
    } catch (error) {
      setMessage(`Falha ao importar conexões: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  };

  const saveDisabled =
    busy ||
    !draft.name.trim() ||
    !draft.host?.trim() ||
    (draft.kind === "sftp" && authMethod === "password" && !selectedId && !secret.trim());

  return (
    <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
      <div className="modal connection-modal">
        <header className="modal-header">
          <div><span className="eyebrow">StorFTP</span><h2>Gerenciador de conexões</h2></div>
          <button className="icon-button" onClick={onClose}><X size={18} /></button>
        </header>
        <div className="connection-layout">
          <aside className="connection-sidebar">
            <div className="connection-sidebar-actions">
              <button className="primary-button full" disabled={busy} onClick={newConnection}>
                <Plus size={16} /> Nova conexão
              </button>
              <button className="secondary-button full" disabled={busy} onClick={() => void importConnections()}>
                <Download size={15} /> Importar conexões
              </button>
            </div>
            <div className="connection-list">
              {connections.map((item) => (
                <button
                  key={item.id}
                  className={`connection-card ${selectedId === item.id ? "active" : ""}`}
                  onClick={() => select(item)}
                >
                  {item.kind === "google_drive" ? <Cloud size={17} /> : <Server size={17} />}
                  <span>
                    <strong>{item.name}</strong>
                    <small>{item.kind.toUpperCase()} {item.host ? `· ${item.host}` : ""}</small>
                  </span>
                  {item.favorite && <Star size={14} />}
                </button>
              ))}
            </div>
          </aside>
          <main className="connection-form">
            <div className="form-grid two">
              <label>
                <span>Nome</span>
                <input value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
              </label>
              <label>
                <span>Protocolo</span>
                <select value={draft.kind} onChange={(e) => changeKind(e.target.value as ProviderKind)}>
                  <option value="sftp">SFTP</option>
                  <option value="ftp">FTP</option>
                  <option value="ftpes">FTPES (TLS explícito)</option>
                  <option value="ftps">FTPS (TLS implícito)</option>
                </select>
              </label>
              <label>
                <span>Host</span>
                <input
                  placeholder="servidor.exemplo.com"
                  value={draft.host ?? ""}
                  onChange={(e) => setDraft({ ...draft, host: e.target.value })}
                />
              </label>
              <label>
                <span>Porta</span>
                <input
                  type="number"
                  value={draft.port ?? 22}
                  onChange={(e) => setDraft({ ...draft, port: Number(e.target.value) })}
                />
              </label>
              <label>
                <span>Usuário</span>
                <input
                  value={draft.username ?? ""}
                  onChange={(e) => setDraft({ ...draft, username: e.target.value })}
                />
              </label>

              {draft.kind === "sftp" && (
                <label>
                  <span>Autenticação SFTP</span>
                  <select
                    value={authMethod}
                    onChange={(e) => setDraft({
                      ...draft,
                      extra: { ...draft.extra, authMethod: e.target.value }
                    })}
                  >
                    <option value="auto">Automática (senha → chave → SSH Agent)</option>
                    <option value="password">Usuário + senha</option>
                    <option value="key">Chave SSH privada</option>
                    <option value="agent">SSH Agent</option>
                  </select>
                </label>
              )}

              {authMethod !== "agent" && (
                <label>
                  <span>{draft.kind === "sftp" && authMethod === "key" ? "Passphrase da chave" : "Senha / passphrase"}</span>
                  <div className="input-with-icon">
                    <KeyRound size={15} />
                    <input
                      type="password"
                      placeholder={selectedId ? "Deixe vazio para manter a credencial salva" : authMethod === "key" ? "Opcional se a chave não tiver passphrase" : "Senha"}
                      value={secret}
                      onChange={(e) => setSecret(e.target.value)}
                    />
                  </div>
                </label>
              )}

              {draft.kind === "sftp" && (authMethod === "auto" || authMethod === "key") && (
                <label>
                  <span>Chave SSH privada</span>
                  <input
                    placeholder="~/.ssh/id_ed25519"
                    value={String(draft.extra.keyPath ?? "")}
                    onChange={(e) => setDraft({
                      ...draft,
                      extra: { ...draft.extra, keyPath: e.target.value }
                    })}
                  />
                </label>
              )}

              <label>
                <span>Pasta inicial</span>
                <input
                  value={draft.initialPath ?? "/"}
                  onChange={(e) => setDraft({ ...draft, initialPath: e.target.value })}
                />
              </label>
              <label>
                <span>Grupo</span>
                <input
                  value={draft.groupName ?? ""}
                  onChange={(e) => setDraft({ ...draft, groupName: e.target.value || null })}
                />
              </label>
              <label>
                <span>Timeout (s)</span>
                <input
                  type="number"
                  min={5}
                  value={draft.timeoutSeconds}
                  onChange={(e) => setDraft({ ...draft, timeoutSeconds: Number(e.target.value) })}
                />
              </label>
              <label>
                <span>Keep-alive (s)</span>
                <input
                  type="number"
                  min={0}
                  value={draft.keepAliveSeconds}
                  onChange={(e) => setDraft({ ...draft, keepAliveSeconds: Number(e.target.value) })}
                />
              </label>
              <label>
                <span>Conexões máximas</span>
                <input
                  type="number"
                  min={1}
                  max={32}
                  value={draft.maxConnections}
                  onChange={(e) => setDraft({ ...draft, maxConnections: Number(e.target.value) })}
                />
              </label>
              <label>
                <span>Tags</span>
                <input
                  value={draft.tags.join(", ")}
                  onChange={(e) => setDraft({
                    ...draft,
                    tags: e.target.value.split(",").map((x) => x.trim()).filter(Boolean)
                  })}
                />
              </label>
            </div>

            <label className="check-row">
              <input
                type="checkbox"
                checked={draft.favorite}
                onChange={(e) => setDraft({ ...draft, favorite: e.target.checked })}
              />
              Favorito
            </label>

            <div className="security-note">
              Senhas importadas ou digitadas não são gravadas no SQLite nem no localStorage. O StorFTP usa o cofre de credenciais do Windows ou Secret Service/Keyring no Linux. FileZilla, Remmina e OpenSSH são detectados nos locais padrão do sistema.
            </div>

            <div className="connection-tools">
              <button
                className="secondary-button"
                disabled={busy || !draft.host?.trim() || !draft.username?.trim()}
                onClick={() => void test()}
              >
                <Activity size={15} /> {busy ? "Testando..." : "Testar conexão"}
              </button>

              {selectedId && draft.kind === "sftp" && (
                <button
                  className="secondary-button host-verify"
                  disabled={busy}
                  onClick={async () => {
                    setBusy(true);
                    setMessage(null);
                    try {
                      const probe = await api.probeSftpHost(selectedId);
                      const approved = window.confirm(
                        `Servidor SFTP\n\n${probe.host}:${probe.port}\n${probe.fingerprint}\n\nConfirme este fingerprint com o administrador do servidor. Confiar sempre neste host?`
                      );
                      if (approved) {
                        const saved = await api.trustSftpHost(selectedId, probe.fingerprint);
                        setDraft(structuredClone(saved));
                        await onChanged();
                        setMessage(`Host confiável salvo: ${probe.fingerprint}`);
                      }
                    } catch (error) {
                      setMessage(String(error));
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  <Fingerprint size={15} /> Verificar fingerprint SFTP
                </button>
              )}
            </div>

            {message && <div className="inline-message">{message}</div>}

            <div className="modal-actions">
              {selectedId && (
                <button
                  className="danger-button"
                  disabled={busy}
                  onClick={async () => {
                    if (!selectedId) return;
                    setBusy(true);
                    try {
                      await api.deleteConnection(selectedId);
                      await onChanged();
                      newConnection();
                    } catch (error) {
                      setMessage(String(error));
                    } finally {
                      setBusy(false);
                    }
                  }}
                >
                  <Trash2 size={15} /> Excluir
                </button>
              )}
              <span />
              <button className="secondary-button" onClick={onClose}>Cancelar</button>
              <button className="primary-button" disabled={saveDisabled} onClick={() => void save()}>
                <Save size={15} /> Salvar
              </button>
            </div>
          </main>
        </div>
      </div>
    </div>
  );
}
