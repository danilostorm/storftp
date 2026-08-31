# Transfer Engine

Estados persistidos: Queued, Preparing, Connecting, Transferring, Paused, Retrying, Verifying, Completed, Failed e Cancelled.

## Resolução de estratégia

1. `SERVER_SIDE`: mesma instância de provider e capability explícita de cópia nativa.
2. `DIRECT_STREAM`: origem fornece download stream e destino aceita upload stream.
3. `LOCAL_RELAY`: reservado para uma integração futura que realmente exija staging local.

O caminho normal entre SFTP/FTP/Local e Google Drive é um pipe bounded em memória. O tamanho total do buffer é configurável e não cresce com o tamanho do arquivo.

## Persistência e recuperação

Cada job é gravado em SQLite. Ao iniciar, jobs incompletos voltam para `Queued`. Resume usa offset quando o destino suporta recuperação independente. Sessões resumíveis do Google Drive ainda não são serializadas entre processos; por segurança, uma sessão interrompida é reiniciada em vez de assumir offset inválido.

## Retry

Erros temporários passam por backoff exponencial até `max_attempts`. HTTP 429 do Drive preserva `Retry-After` na mensagem de diagnóstico.

## Verificação

Checksums só são comparados quando os dois lados retornam hashes do mesmo algoritmo. Isso evita comparar, por exemplo, SHA-256 local com MD5 do Drive.
