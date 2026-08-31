# Desenvolvimento

## Estrutura

```text
src/                 React/TypeScript
src-tauri/src/
  commands/          IPC
  credentials/       keyring
  database/          SQLite
  providers/         local, sftp, ftp, google_drive
  transfer/          fila e streaming
  sync/              comparação/StorSync
  security/          redaction
.github/workflows/   CI
```

## Checks locais

```bash
npm install
npm run check
npm run build
cd src-tauri
cargo fmt
cargo clippy --all-targets
cargo test
```

## Princípios

- nenhuma operação de rede no React;
- sem segredos em fixtures/logs;
- provider novo precisa declarar capabilities honestamente;
- não usar `unwrap()` em caminhos de produção para mascarar erro recuperável;
- adicionar teste sempre que alterar capability resolver, path handling, queue/retry ou redaction.
