# Providers

`StorageProvider` define list/stat/download/upload/delete/mkdir/rename/copy/server-side-copy/checksum/search e `capabilities()`.

## Local

Usa `std::fs`, streaming e resume por seek. Hash SHA-256 disponível.

## SFTP

Usa `ssh2`. Autenticação por senha, ssh-agent ou chave privada (`extra.keyPath`) com passphrase no credential store. Antes da autenticação, a chave do host é conferida em `~/.ssh/known_hosts`; se não existir, o usuário deve confirmar o fingerprint antes de persistir confiança.

## FTP / FTPES / FTPS

Usa `suppaftp`. FTPES aplica TLS explícito; FTPS usa TLS implícito. Certificados TLS continuam validados. MLSD é preferido e LIST é fallback. REST é usado para resume quando o servidor suporta.

## Google Drive

Provider nativo pela API Drive v3. A listagem é paginada; busca usa query do servidor. Upload usa sessão resumível e chunks. `files.copy` é exposto como server-side copy somente dentro da mesma conexão/provider compatível.

## Capabilities

O motor nunca deduz server-side copy apenas porque dois lados são “cloud”. A operação só é selecionada quando origem e destino representam a mesma instância lógica e o provider declara suporte.
