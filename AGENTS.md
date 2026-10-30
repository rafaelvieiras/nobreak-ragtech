# AGENTS.md

Repositório **open source em português** (decisão de 29/out/2026: o público é brasileiro e pesquisa em PT-BR).
Código de domínio, commits, issues e docs em PT-BR.

## O que é
Monitor de nobreaks Ragtech: backend Rust (`backend/`) lê a serial e serve a API + o painel Svelte/Vite (`frontend/`),
embutido no binário com `rust-embed`. Imagem Docker `FROM scratch` multi-arquitetura.

## Regras que não mudam
- **Somente leitura.** O binário nunca escreve no nobreak. Escrita exige opção explícita e teste em hardware real.
- **Nada do software oficial** neste repo: nem código, nem decompilado, nem `upscfg*.cfg`, nem strings da UI.
- Checksum do pedido = soma dos bytes 2..=4 (veja `docs/protocolo.md`). Nunca envie quadro sem checksum válido.
- Toda fórmula nova: teste com captura real. Marque “confirmado” só o que foi visto num nobreak.
- Memória no Pi 3 é escassa: sem dependências pesadas, sem banco de dados, binário < 1 MB.

## Comandos
`make test` · `make dev-backend` (simulado) · `make docker` · `make arm`

## Release
Tag `vX.Y.Z` → `.github/workflows/release.yml` publica `ghcr.io/<dono>/nobreak-ragtech` (amd64, arm64, armv7, armv6) e anexa
binários estáticos + `SHA256SUMS`. Detalhes de engenharia reversa e estado do hardware ficam no vault do Rafael
(`ai-workspace/.agents/servidor-moon/ragtech-protocolo-re.md`), não aqui.
