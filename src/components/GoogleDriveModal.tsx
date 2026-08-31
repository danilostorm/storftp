import { Cloud, LogIn, X } from "lucide-react";
import { useState } from "react";
import { api } from "../lib/api";

export function GoogleDriveModal({ onClose, onConnected }: { onClose: () => void; onConnected: () => Promise<void> }) {
  const [name, setName] = useState("Google Drive");
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const login = async () => {
    setBusy(true); setMessage("Abrindo o Google no navegador…");
    try {
      await api.googleOAuth(name, clientId, clientSecret);
      await onConnected();
      onClose();
    } catch (e) { setMessage(String(e)); } finally { setBusy(false); }
  };
  return <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}><div className="modal google-modal">
    <header className="modal-header"><div><span className="eyebrow">Cloud nativo</span><h2><Cloud size={20} /> Conectar Google Drive</h2></div><button className="icon-button" onClick={onClose}><X size={18} /></button></header>
    <div className="google-body">
      <p>O StorFTP usa OAuth 2.0 oficial e nunca solicita a senha da sua conta Google. Crie um OAuth Client do tipo <strong>Desktop app</strong> no Google Cloud Console e informe as credenciais abaixo.</p>
      <label><span>Nome da conexão</span><input value={name} onChange={(e) => setName(e.target.value)} /></label>
      <label><span>OAuth Client ID</span><input value={clientId} onChange={(e) => setClientId(e.target.value)} placeholder="xxxxxxxx.apps.googleusercontent.com" /></label>
      <label><span>OAuth Client Secret</span><input type="password" value={clientSecret} onChange={(e) => setClientSecret(e.target.value)} /></label>
      <div className="security-note">O refresh token e o client secret são armazenados no cofre seguro do sistema. O access token permanece somente em memória e nunca é escrito em logs.</div>
      {message && <div className="inline-message">{message}</div>}
    </div>
    <div className="modal-actions"><span /><button className="secondary-button" onClick={onClose}>Cancelar</button><button className="primary-button" disabled={busy || !clientId.trim()} onClick={() => void login()}><LogIn size={15} /> Entrar com Google</button></div>
  </div></div>;
}
