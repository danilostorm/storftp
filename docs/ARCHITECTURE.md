# Arquitetura

O StorFTP separa UI, persistência, credenciais, providers e execução de transferências. A UI nunca implementa protocolo de rede.

## Camadas

1. **React/TypeScript**: dois painéis genéricos, Site Manager, Google OAuth, configurações e fila.
2. **Tauri Commands**: fronteira IPC; operações de rede/arquivo potencialmente bloqueantes são movidas para workers.
3. **ProviderFactory**: resolve `ProviderRef` em uma implementação `StorageProvider`.
4. **Providers**: Local, SFTP, FTP/FTPES/FTPS e Google Drive.
5. **TransferManager**: fila persistente, concorrência, retry, pause/cancel, streaming e capability negotiation.
6. **Database**: SQLite apenas para metadados.
7. **CredentialStore**: segredos fora do banco.

## ProviderRef

Um painel guarda `{ kind, connectionId }`. Local não precisa de conexão. Providers remotos precisam de uma entrada de `connections`.

## Extensão

Adicionar um provider exige implementar `StorageProvider`, declarar capabilities e adicioná-lo ao `ProviderFactory`. O Transfer Engine não contém regras específicas de UI para novos clouds.
