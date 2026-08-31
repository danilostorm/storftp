import { Save, X } from "lucide-react";
import { useState } from "react";
import { api } from "../lib/api";
import type { AppSettings } from "../types";

export function SettingsModal({ settings, onClose, onSaved }: { settings: AppSettings; onClose: () => void; onSaved: (settings: AppSettings) => void }) {
  const [draft, setDraft] = useState(settings);
  const [busy, setBusy] = useState(false);
  const save = async () => {
    setBusy(true);
    try { onSaved(await api.saveSettings(draft)); onClose(); } finally { setBusy(false); }
  };
  return <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
    <div className="modal settings-modal">
      <header className="modal-header"><div><span className="eyebrow">Preferências</span><h2>Configurações</h2></div><button className="icon-button" onClick={onClose}><X size={18} /></button></header>
      <div className="settings-body">
        <nav className="settings-nav"><button className="active">Geral</button><button>Aparência</button><button>Transferências</button><button>FTP</button><button>SFTP</button><button>Google Drive</button><button>Segurança</button><button>Rede</button><button>Logs</button><button>Avançado</button><button>Sobre</button></nav>
        <div className="settings-content">
          <h3>Geral</h3>
          <div className="form-grid two">
            <label><span>Tema</span><select value={draft.theme} onChange={(e) => setDraft({ ...draft, theme: e.target.value as AppSettings["theme"] })}><option value="dark">Dark</option><option value="light">Light</option><option value="system">System</option></select></label>
            <label><span>Transferências simultâneas</span><input type="number" min={1} max={32} value={draft.concurrentTransfers} onChange={(e) => setDraft({ ...draft, concurrentTransfers: Number(e.target.value) })} /></label>
            <label><span>Buffer de streaming (MiB)</span><input type="number" min={1} max={128} value={draft.bufferSizeMiB} onChange={(e) => setDraft({ ...draft, bufferSizeMiB: Number(e.target.value) })} /></label>
            <label><span>Limite upload (bytes/s, 0 = ilimitado)</span><input type="number" min={0} value={draft.uploadLimitBps ?? 0} onChange={(e) => setDraft({ ...draft, uploadLimitBps: Number(e.target.value) || null })} /></label>
            <label><span>Limite download (bytes/s, 0 = ilimitado)</span><input type="number" min={0} value={draft.downloadLimitBps ?? 0} onChange={(e) => setDraft({ ...draft, downloadLimitBps: Number(e.target.value) || null })} /></label>
          </div>
          <label className="check-row"><input type="checkbox" checked={draft.verifyAfterTransfer} onChange={(e) => setDraft({ ...draft, verifyAfterTransfer: e.target.checked })} /> Verificar transferência quando o provider oferece checksum confiável</label>
          <label className="check-row"><input type="checkbox" checked={draft.developerMode} onChange={(e) => setDraft({ ...draft, developerMode: e.target.checked })} /> Modo desenvolvedor (logs técnicos com segredos sempre redigidos)</label>
          <div className="about-card"><strong>StorFTP 0.1.0</strong><span>Universal File Transfer & Cloud Manager</span><code>github.com/danilostorm/storftp</code></div>
        </div>
      </div>
      <div className="modal-actions"><span /><button className="secondary-button" onClick={onClose}>Cancelar</button><button className="primary-button" onClick={() => void save()} disabled={busy}><Save size={15} /> Salvar</button></div>
    </div>
  </div>;
}
