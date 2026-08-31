import { Cloud, FolderOpen, HardDrive, Server } from "lucide-react";
import type { ConnectionProfile, ProviderRef } from "../types";

interface Props {
  value: ProviderRef;
  connections: ConnectionProfile[];
  onChange: (value: ProviderRef) => void;
}

function iconFor(kind: string) {
  if (kind === "local") return <HardDrive size={15} />;
  if (kind === "google_drive") return <Cloud size={15} />;
  if (kind === "sftp" || kind === "ftp" || kind === "ftpes" || kind === "ftps") return <Server size={15} />;
  return <FolderOpen size={15} />;
}

export function ProviderPicker({ value, connections, onChange }: Props) {
  const key = value.kind === "local" ? "local" : value.connectionId ?? "local";
  return (
    <div className="provider-picker-wrap">
      <span className="provider-picker-icon">{iconFor(value.kind)}</span>
      <select
        className="provider-picker"
        value={key}
        onChange={(event) => {
          if (event.target.value === "local") onChange({ kind: "local", connectionId: null });
          else {
            const connection = connections.find((item) => item.id === event.target.value);
            if (connection) onChange({ kind: connection.kind, connectionId: connection.id });
          }
        }}
      >
        <option value="local">Este computador</option>
        {connections.map((connection) => (
          <option value={connection.id} key={connection.id}>
            {connection.name} · {connection.kind.toUpperCase()}
          </option>
        ))}
      </select>
    </div>
  );
}
