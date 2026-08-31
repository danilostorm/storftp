import { CircleCheck, CircleX, Pause, Play, RefreshCcw, ServerCog, Zap } from "lucide-react";
import { formatBytes, formatEta, formatSpeed } from "../lib/format";
import type { TransferJob } from "../types";

interface Props {
  transfers: TransferJob[];
  onCancel: (id: string) => void;
  onRetry: (id: string) => void;
  onPause: (id: string) => void;
  onResume: (id: string) => void;
}

const strategyLabel = { server_side: "Server-Side", direct_stream: "Direct Streaming", local_relay: "Local Relay" };
const statusLabel: Record<string, string> = { queued:"Aguardando",preparing:"Preparando",connecting:"Conectando",transferring:"Transferindo",paused:"Pausada",retrying:"Tentando novamente",verifying:"Verificando",completed:"Concluída",failed:"Falhou",cancelled:"Cancelada" };

export function TransferPanel({ transfers, onCancel, onRetry, onPause, onResume }: Props) {
  const active = transfers.filter((item) => ["preparing", "connecting", "transferring", "retrying", "verifying"].includes(item.state));
  const queued = transfers.filter((item) => item.state === "queued");
  const completed = transfers.filter((item) => item.state === "completed");
  const down = active.reduce((sum, item) => sum + item.speedBps, 0);
  return <section className="transfer-panel">
    <div className="transfer-summary"><strong>Transferências</strong><span><ServerCog size={15}/>{active.length} ativas</span><span>⇄ {formatSpeed(down)}</span><span><CircleCheck size={15}/>{completed.length} concluídas</span><span><Pause size={15}/>{queued.length} aguardando</span></div>
    <div className="transfer-list">
      {transfers.length === 0 && <div className="transfer-empty">Arraste arquivos entre os painéis para iniciar uma transferência.</div>}
      {transfers.map((job) => {
        const percent=job.totalBytes>0?Math.min(100,(job.transferredBytes/job.totalBytes)*100):job.state==="completed"?100:0;
        return <article className="transfer-item" key={job.id}>
          <div className="transfer-main">
            <div className="transfer-name"><strong>{job.fileName}</strong><span>{job.source.kind} → {job.destination.kind}</span></div>
            <div className="transfer-strategy">{job.strategy==="server_side"?<Zap size={14}/>:<ServerCog size={14}/>} {strategyLabel[job.strategy]}</div>
            <div className="transfer-metric">{formatBytes(job.transferredBytes)} / {formatBytes(job.totalBytes)}</div>
            <div className="transfer-metric">{formatSpeed(job.speedBps)}</div><div className="transfer-metric">ETA {formatEta(job.etaSeconds)}</div>
            <div className={`status-pill ${job.state}`}>{statusLabel[job.state] ?? job.state}</div>
            <div className="transfer-actions">
              {job.state==="failed" && <button className="icon-button compact" title="Tentar novamente" onClick={()=>onRetry(job.id)}><RefreshCcw size={14}/></button>}
              {job.state==="paused" && <button className="icon-button compact" title="Continuar" onClick={()=>onResume(job.id)}><Play size={14}/></button>}
              {["queued","preparing","connecting","transferring","retrying","verifying"].includes(job.state) && <button className="icon-button compact" title="Pausar" onClick={()=>onPause(job.id)}><Pause size={14}/></button>}
              {!['completed','cancelled'].includes(job.state) && <button className="icon-button compact" title="Cancelar" onClick={()=>onCancel(job.id)}><CircleX size={14}/></button>}
            </div>
          </div>
          <div className="progress-track"><div className="progress-bar" style={{width:`${percent}%`}}/></div>
          {job.error && <div className="transfer-error">{job.error}</div>}
        </article>;
      })}
    </div>
  </section>;
}
