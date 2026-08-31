# Google Drive

## OAuth 2.0

O fluxo usa navegador do sistema, Authorization Code, PKCE, state aleatório e callback loopback `127.0.0.1`. A aplicação nunca solicita senha Google.

Escopo atual: `drive` e `userinfo.email`. O refresh token é armazenado pelo `CredentialStore`; access tokens vivem somente em memória.

## Operações

- listagem paginada;
- busca server-side;
- download com Range;
- upload resumível em chunks;
- mkdir/delete;
- rename/move;
- `files.copy` server-side;
- MD5 quando disponibilizado pela API;
- `supportsAllDrives=true` nas operações compatíveis.

Shared Drives podem ser representados por uma conexão cujo `extra.driveId` aponta para o drive. A UI específica para descobrir e cadastrar Shared Drives automaticamente é uma extensão isolada do provider.

Arquivos nativos Docs/Sheets/Slides não possuem fluxo binário comum; o provider retorna erro explícito até existir uma escolha de formato de exportação na UI.
