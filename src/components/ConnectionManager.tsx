import {
  Activity,
  CheckCircle2,
  Cloud,
  Download,
  Fingerprint,
  KeyRound,
  Link2,
  Plus,
  Save,
  Server,
  ShieldAlert,
  Star,
  Trash2,
  X
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api, errorMessage, type SftpHostProbe } from "../lib/api";
import type { ConnectionProfile, ProviderKind } from "../types";

type ConnectTarget = "left" | "right";

interface Props {
  connections: ConnectionProfile[];
  onClose: () => void;
  onChanged: () => Promise<void>;
  onConnect: (profile: ConnectionProfile, target: ConnectTarget) => void;
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

function trustedFingerprint(profile: ConnectionProfile): string {
  return String(profile.extra.hostFingerprint ?? "");
}

export function ConnectionManager({ connections, onClose, onChanged, onConnect }: Props) {
  const [selectedId, setSelectedId] = useState<string | null>(connections[0]?.id ?? null);
  const source = useMemo(
    () => connections.find((item) => item.id === selectedId) ?? null,
    [connections, selectedId]
  );
  const [draft, setDraft] = useState<ConnectionProfile>(source ? structuredClone(source) : emptyProfile());
  const [secret, setSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [observed, setObserved] = useState<SftpHostProbe | null>(null);
  const [probeError, setProbeError] = useState<string | null>(null);
  const [target, setTarget] = useState<ConnectTarget>("right");

  const authMethod = String(draft.extra.authMethod ?? "auto");
  const savedFingerprint = trustedFingerprint(draft);
  const fingerprintMatches = Boolean(observed && savedFingerprint && observed.fingerprint === savedFingerprint);
  const fingerprintChanged = Boolean(observed && savedFingerprint && observed.fingerprint !== savedFingerprint);
  const fingerprintNeedsTrust = Boolean(observed && !savedFingerprint);

  useEffect(() => {
    if (draft.kind !== "sftp" || !draft.host?.trim()) {
      setObserved(null);
      setProbeError(null);
      return;
    }

    let cancelled = false;
    const timer = window.setTimeout(() => {
      void api.probeSftpProfile(draft)
        .then((probe) => {
          if (cancelled) return;
          setObserved(probe);
          setProbeError(null);
        })
        .catch((error) => {
          if (cancelled) return;
          setObserved(null);
          setProbeError(errorMessage(error));
        });
    }, 180);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
    // Somente alterações de endpoint devem disparar uma nova sondagem.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [draft.id, draft.kind, draft.host, draft.port]);

  const select = (profile: ConnectionProfile) => {
    setSelectedId(profile.id);
    setDraft(structuredClone(profile));
    setSecret("");
    setMessage(null);
    setObserved(null);
    setProbeError(null);
  };

  const newConnection = () => {
    const next = emptyProfile();
    setSelectedId(null);
    setDraft(next);
    setSecret("");
    setMessage(null);
    setObserved(null);
    setProbeError(null);
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

  const save = async (): Promise<ConnectionProfile | null> => {
    setBusy(true);
    setMessage(null);
    try {
      const saved = await api.saveConnection(draft, secret || null);
      setSelectedId(saved.id);
      setDraft(structuredClone(saved));
      await onChanged();
      setMessage(
        secret
          ? "Conexão e credencial salvas no cofre seguro do sistema operacional."
          : "Conexão salva. Se já havia uma credencial segura, ela foi mantida."
      );
      setSecret("");
      return saved;
    } catch (error) {
      setMessage(errorMessage(error));
      return null;
    } finally {
      setBusy(false);
    }
  };

  const probe = async (profile: ConnectionProfile): Promise<SftpHostProbe | null> => {
    try {
      const result = await api.probeSftpProfile(profile);
      setObserved(result);
      setProbeError(null);
      return result;
    } catch (error) {
      const text = errorMessage(error);
      setProbeError(text);
      setMessage(`Não foi possível alcançar o servidor SFTP: ${text}`);
      return null;
    }
  };

  const prepareSftp = async (profile: ConnectionProfile): Promise<ConnectionProfile | null> => {
    if (profile.kind !== "sftp") return profile;
    const current = await probe(profile);
    if (!current) return null;

    const trusted = trustedFingerprint(profile);
    if (!trusted) {
      setMessage("Servidor novo. Confira o fingerprint mostrado abaixo e clique em “Confiar fingerprint” antes de conectar.");
      return null;
    }
    if (trusted !== current.fingerprint) {
      setMessage("ATENÇÃO: o fingerprint atual é diferente do fingerprint confiado. A conexão foi bloqueada.");
      return null;
    }
    return profile;
  };

  const runTest = async (profile: ConnectionProfile, enteredSecret: string): Promise<boolean> => {
    const prepared = await prepareSftp(profile);
    if (!prepared) return false;
    try {
      const result = await api.testConnection(prepared, enteredSecret || null);
      setMessage(`✓ ${result.message} (${result.latencyMs} ms).`);
      return true;
    } catch (error) {
      setMessage(`Falha no teste: ${errorMessage(error)}`);
      return false;
    }
  };

  const test = async () => {
    setBusy(true);
    setMessage(null);
    try {
      await runTest(draft, secret);
    } finally {
      setBusy(false);
    }
  };

  const connectProfile = async (profile: ConnectionProfile, enteredSecret: string) => {
    setBusy(true);
    setMessage(null);
    try {
      const prepared = await prepareSftp(profile);
      if (!prepared) return;

      const result = await api.testConnection(prepared, enteredSecret || null);
      const saved = await api.saveConnection(prepared, enteredSecret || null);
      await onChanged();
      setSelectedId(saved.id);
      setDraft(structuredClone(saved));
      setSecret("");
      setMessage(`✓ ${result.message} Abrindo no ${target === "left" ? "Painel A" : "Painel B"}...`);
      onConnect(saved, target);
      onClose();
    } catch (error) {
      setMessage(`Falha ao conectar: ${errorMessage(error)}`);
    } finally {
      setBusy(false);
    }
  };

  const trustObservedFingerprint = async () => {
    if (!observed) return;
    setBusy(true);
    setMessage(null);
    try {
      if (selectedId) {
        const saved = await api.trustSftpHost(selectedId, observed.fingerprint);
        setDraft(structuredClone(saved));
        await onChanged();
        setMessage(`Fingerprint confiado e salvo: ${observed.fingerprint}`);
      } else {
        setDraft((old) => ({
          ...old,
          extra: { ...old.extra, hostFingerprint: observed.fingerprint }
        }));
        setMessage(`Fingerprint aceito para esta nova conexão: ${observed.fingerprint}. Clique em Salvar ou Conectar.`);
      }
    } catch (error) {
      setMessage(`Não foi possível confiar no fingerprint: ${errorMessage(error)}`);
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
          `Importação concluída: ${report.imported.length} conexão(ões) adicionada(s), ${report.skipped} duplicada(s) ignorada(s). Senhas protegidas pelo cofre próprio do Remmina podem precisar ser digitadas uma vez no StorFTP.${warning}`
        );
      }
    } catch (error) {
      setMessage(`Falha ao importar conexões: ${errorMessage(error)}`);
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

            <div className="connect-target">
              <span>Duplo clique conecta em:</span>
              <div>
                <button className={target === "left" ? "active" : ""} onClick={() => setTarget("left")}>Painel A</button>
                <button className={target === "right" ? "active" : ""} onClick={() => setTarget("right")}>Painel B</button>
              </div>
            </div>

            <div className="connection-list">
              {connections.map((item) => (
                <button
                  key={item.id}
                  className={`connection-card ${selectedId === item.id ? "active" : ""}`}
                  onClick={() => select(item)}
                  onDoubleClick={() => {
                    select(item);
                    void connectProfile(structuredClone(item), "");
                  }}
                  title={`Duplo clique para conectar no ${target === "left" ? "Painel A" : "Painel B"}`}
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
                <input placeholder="servidor.exemplo.com" value={draft.host ?? ""} onChange={(e) => setDraft({ ...draft, host: e.target.value })} />
              </label>
              <label>
                <span>Porta</span>
                <input type="number" value={draft.port ?? 22} onChange={(e) => setDraft({ ...draft, port: Number(e.target.value) })} />
              </label>
              <label>
                <span>Usuário</span>
                <input value={draft.username ?? ""} onChange={(e) => setDraft({ ...draft, username: e.target.value })} />
              </label>

              {draft.kind === "sftp" && (
                <label>
                  <span>Autenticação SFTP</span>
                  <select value={authMethod} onChange={(e) => setDraft({ ...draft, extra: { ...draft.extra, authMethod: e.target.value } })}>
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
                      placeholder={selectedId ? "Digite para substituir; vazio mantém a salva" : authMethod === "key" ? "Opcional se a chave não tiver passphrase" : "Senha"}
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
                    onChange={(e) => setDraft({ ...draft, extra: { ...draft.extra, keyPath: e.target.value } })}
                  />
                </label>
              )}

              <label>
                <span>Pasta inicial</span>
                <input value={draft.initialPath ?? "/"} onChange={(e) => setDraft({ ...draft, initialPath: e.target.value })} />
              </label>
              <label>
                <span>Grupo</span>
                <input value={draft.groupName ?? ""} onChange={(e) => setDraft({ ...draft, groupName: e.target.value || null })} />
              </label>
              <label>
                <span>Timeout (s)</span>
                <input type="number" min={5} value={draft.timeoutSeconds} onChange={(e) => setDraft({ ...draft, timeoutSeconds: Number(e.target.value) })} />
              </label>
              <label>
                <span>Keep-alive (s)</span>
                <input type="number" min={0} value={draft.keepAliveSeconds} onChange={(e) => setDraft({ ...draft, keepAliveSeconds: Number(e.target.value) })} />
              </label>
              <label>
                <span>Conexões máximas</span>
                <input type="number" min={1} max={32} value={draft.maxConnections} onChange={(e) => setDraft({ ...draft, maxConnections: Number(e.target.value) })} />
              </label>
              <label>
                <span>Tags</span>
                <input value={draft.tags.join(", ")} onChange={(e) => setDraft({ ...draft, tags: e.target.value.split(",").map((x) => x.trim()).filter(Boolean) })} />
              </label>
            </div>

            <label className="check-row">
              <input type="checkbox" checked={draft.favorite} onChange={(e) => setDraft({ ...draft, favorite: e.target.checked })} />
              Favorito
            </label>

            {draft.kind === "sftp" && (
              <div className={`fingerprint-card ${fingerprintChanged ? "danger" : fingerprintMatches ? "ok" : fingerprintNeedsTrust ? "warning" : ""}`}>
                <div className="fingerprint-title">
                  {fingerprintChanged ? <ShieldAlert size={17} /> : fingerprintMatches ? <CheckCircle2 size={17} /> : <Fingerprint size={17} />}
                  <strong>Fingerprint SFTP</strong>
                </div>
                <div className="fingerprint-lines">
                  <span><b>Confiado:</b> <code>{savedFingerprint || "ainda não confiado"}</code></span>
                  <span><b>Atual:</b> <code>{observed?.fingerprint || (probeError ? "não disponível" : "verificando...")}</code></span>
                  {observed && <span><b>Servidor:</b> {observed.host}:{observed.port}</span>}
                  {probeError && <span className="fingerprint-error">{probeError}</span>}
                </div>
                {observed && !fingerprintMatches && !fingerprintChanged && (
                  <button className="secondary-button" disabled={busy} onClick={() => void trustObservedFingerprint()}>
                    <Fingerprint size={15} /> Confiar fingerprint
                  </button>
                )}
                {fingerprintChanged && <strong className="fingerprint-danger">Não confie sem confirmar a troca da chave no servidor.</strong>}
              </div>
            )}

            <div className="security-note">
              Senhas digitadas são gravadas somente no cofre seguro do Windows ou Secret Service/Keyring no Linux. Ao clicar em <b>Conectar</b>, o StorFTP testa primeiro e só salva a senha se a autenticação funcionar. Conexões importadas do Remmina podem exigir que a senha seja digitada uma vez, pois o Remmina pode guardá-la no próprio cofre.
            </div>

            <div className="connection-tools">
              <button className="secondary-button" disabled={busy || !draft.host?.trim() || !draft.username?.trim()} onClick={() => void test()}>
                <Activity size={15} /> {busy ? "Aguarde..." : "Testar conexão"}
              </button>
              <button className="primary-button" disabled={busy || !draft.host?.trim() || !draft.username?.trim()} onClick={() => void connectProfile(draft, secret)}>
                <Link2 size={15} /> Conectar no {target === "left" ? "Painel A" : "Painel B"}
              </button>
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
                      setMessage(errorMessage(error));
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
