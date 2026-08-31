# Segurança

## Segredos

`connections` no SQLite contém host, usuário e preferências, nunca senha/passphrase/token. Segredos usam `keyring`, que integra Windows Credential Manager e o serviço de segredos do desktop Linux disponível.

Chaves privadas permanecem no caminho escolhido pelo usuário; o conteúdo da chave não é copiado para SQLite.

## SSH

- `known_hosts` é respeitado.
- mismatch bloqueia a conexão.
- host desconhecido exibe SHA-256 e exige confiança explícita.
- não existe `StrictHostKeyChecking=no` oculto.

## TLS

FTPES/FTPS e HTTPS usam validação de certificado. Não há opção padrão para aceitar certificado inválido.

## OAuth

- PKCE + state;
- callback somente em loopback;
- refresh/access tokens nunca são exibidos na UI;
- access token não é persistido.

## Logs

A rotina `security::redact` remove campos conhecidos de senha, passphrase, OAuth token, authorization header, client secret e private key antes de uso em diagnósticos persistentes.
