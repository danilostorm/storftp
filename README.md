# StorFTP

**Universal File Transfer & Cloud Manager** — um cliente desktop multiplataforma para movimentar arquivos entre computador, FTP/FTPES/FTPS, SFTP e Google Drive usando automaticamente a melhor estratégia disponível.

> Projeto oficial: `danilostorm/storftp`

## O que já está implementado

- Tauri 2 + Rust no backend e React + TypeScript + Vite na interface.
- Dois painéis independentes: qualquer lado pode representar Local, FTP/FTPES/FTPS, SFTP ou Google Drive.
- Navegação, pesquisa, criação/exclusão/rename por provider.
- Gerenciador de conexões com favoritos, grupos, tags, timeouts, keep-alive e limite por perfil.
- Cofre de credenciais do sistema operacional via `keyring`; segredos não vão para SQLite/localStorage.
- SFTP real com senha, ssh-agent ou private key + passphrase e validação de `known_hosts`/fingerprint.
- FTP, FTPES (TLS explícito) e FTPS (TLS implícito), MLSD/LIST e REST/resume quando suportado.
- Google Drive nativo com OAuth 2.0 + PKCE, múltiplas contas, paginação, busca server-side, download, upload resumível, pastas, move/rename/delete e cópia nativa `files.copy`.
- Transfer Engine persistido em SQLite com fila, pause/resume/cancel/retry, backoff exponencial, ETA, velocidade e recuperação após reiniciar o aplicativo.
- Capability resolver com `SERVER_SIDE`, `DIRECT_STREAM` e `LOCAL_RELAY`.
- Streaming com buffer limitado entre providers para não gravar arquivos gigantes temporariamente.
- Histórico e verificação de checksum quando ambos os providers expõem hashes comparáveis.
- Comparador de diretórios base do StorSync.
- Dark/Light/System e configurações de concorrência, buffer e verificação.
- CI para Linux e Windows com TypeScript, Rust fmt, Clippy, testes e build.

## Requisitos de desenvolvimento

- Node.js 20+
- Rust stable (compatível com Rust 1.77.2+)
- Dependências nativas do Tauri 2 para seu sistema operacional

Ubuntu/Zorin OS:

```bash
sudo apt update
sudo apt install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf build-essential pkg-config libssl-dev
```

## Desenvolvimento

```bash
npm install
npm run tauri dev
```

Build web/typecheck:

```bash
npm run check
npm run build
```

Testes Rust:

```bash
cd src-tauri
cargo test
cargo clippy --all-targets
```

Build desktop:

```bash
npm run tauri build
```

## Google Drive

Crie um OAuth Client do tipo **Desktop app** no Google Cloud Console com Google Drive API habilitada. Na primeira conexão, informe Client ID e, se o projeto exigir, Client Secret. O StorFTP abre o navegador, usa Authorization Code + PKCE e recebe o callback em `127.0.0.1` numa porta efêmera. Refresh token e client secret ficam no cofre do sistema operacional.

Arquivos Google Docs/Sheets/Slides nativos exigem exportação para um formato escolhido; o download binário desses itens é rejeitado explicitamente em vez de gerar arquivo incorreto.

## Segurança

- Nunca coloque senhas, chaves privadas ou tokens no código.
- A verificação SSH de host não é desligada silenciosamente.
- TLS permanece validado por padrão.
- Logs devem passar pela rotina de redaction antes de exibir conteúdo potencialmente sensível.
- SQLite guarda somente metadados não sensíveis.

Leia [docs/SECURITY.md](docs/SECURITY.md).

## Arquitetura

```text
React UI
  │ Tauri IPC
  ▼
Commands ──► ProviderFactory ──► Local / SFTP / FTP(S) / Google Drive
  │
  ├──► TransferManager ──► capability resolver ──► native copy | bounded stream
  ├──► SQLite ──► settings / queue / history / connection metadata
  └──► CredentialStore ──► Windows Credential Manager / Secret Service
```

Documentação detalhada:

- [Arquitetura](docs/ARCHITECTURE.md)
- [Providers](docs/PROVIDERS.md)
- [Transfer Engine](docs/TRANSFER_ENGINE.md)
- [Google Drive](docs/GOOGLE_DRIVE.md)
- [Segurança](docs/SECURITY.md)
- [Desenvolvimento](docs/DEVELOPMENT.md)

## Roadmap compatível com a arquitetura atual

Os módulos intencionalmente definidos como posteriores na especificação permanecem separados da base: sincronização bidirecional completa do StorSync, agendamento de bandwidth por horário, tray, auto-update assinado, ARM64 e provider opcional do rclone. A abstração `StorageProvider` foi desenhada para receber OneDrive, Dropbox, S3, B2, WebDAV, SMB e rclone sem reescrever a interface ou o motor de transferência.

## Licença

MIT.
