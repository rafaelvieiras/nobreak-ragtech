# nobreak-ragtech

**Monitor open source para nobreaks Ragtech no Linux, Raspberry Pi e Docker.**
Lê a porta USB do nobreak e mostra bateria, tensão de entrada e saída, carga, temperatura e frequência
num painel web. Sem Windows, sem Java, sem o Supervise.

Serve para quem tem um nobreak **Ragtech Easy Pro, Easy Way, Easy Jet, NEP, Toro, Innergie ou Save Digital**
ligado por USB num servidor, NAS ou Raspberry Pi, e o software oficial não roda bem (ou não roda).

> **Status: alfa.** Só leitura. O projeto nunca escreve no nobreak. Testado em um Easy Pro 1200 VA.
> Outros modelos devem funcionar, mas precisam de gente testando: veja [Ajude a testar](#ajude-a-testar).

## Recursos

- Painel web leve (Svelte, 43 KB), servido pelo próprio binário. Nada de banco de dados.
- Binário estático em Rust. No Raspberry Pi 3 usa **menos de 2 MB de RAM**.
- Imagem Docker de ~700 KB para `amd64`, `arm64`, `armv7` e `armv6`.
- API JSON (`/api/estado`, `/api/eventos`, `/api/historico`) e métricas Prometheus (`/metrics`).
- Gera o arquivo do driver `dummy-ups` do **NUT** (Network UPS Tools), então o Home Assistant e o `upsmon` continuam funcionando.
- Mostra os registradores crus, para ajudar a decodificar o que ainda falta.
- **Registro de mudanças:** cada alteração de bit em registradores de estado (EVENTOS, FALHAS, USUARIO, NVOLATIL...) vira uma linha com data, tensão da rede e bateria. É a forma mais fácil de descobrir o que cada bit significa numa queda de energia.
- **Gráficos** de rede, bateria, carga e temperatura, em SVG puro, guardados em memória (24 h por padrão).

## Instalação rápida (Docker)

```bash
docker run -d --name nobreak-ragtech --restart unless-stopped \
  -p 8088:8080 \
  --device /dev/ttyACM0:/dev/ragtech \
  -e NOBREAK_PORTA=/dev/ragtech \
  ghcr.io/rafaelvieiras/nobreak-ragtech:latest
```

Abra `http://IP-DO-SERVIDOR:8088`. Também há um [`compose.yaml`](compose.yaml) de exemplo.
Guia completo, permissões da porta e integração com NUT: [`docs/instalacao.md`](docs/instalacao.md).

Sem Docker, baixe o binário em [Releases](../../releases) e rode `./nobreak-ragtech`.

## Configuração

| Variável | Padrão | O que faz |
|---|---|---|
| `NOBREAK_PORTA` | `/dev/ragtech` | Porta serial do nobreak (`/dev/ttyACM0`, `/dev/ttyUSB0`...) |
| `NOBREAK_ESCUTA` | `0.0.0.0:8080` | Endereço do painel |
| `NOBREAK_INTERVALO_S` | `5` | Segundos entre leituras |
| `NOBREAK_ID` | `A` | Nibble de endereço do quadro (hex). O padrão serve para a maioria |
| `NOBREAK_FATOR_VBAT` | `1` | Multiplica a tensão da bateria. Use `2` se o valor aparecer pela metade do medido |
| `NOBREAK_ARQUIVO_NUT` | vazio | Caminho do arquivo para o `dummy-ups` do NUT |
| `NOBREAK_ARQUIVO_EVENTOS` | vazio | Arquivo `.jsonl` para guardar o registro de mudanças entre reinícios (pasta com escrita) |
| `NOBREAK_HISTORICO_PONTOS` | `1440` | Pontos guardados nos gráficos (memória, ~50 bytes cada) |
| `NOBREAK_HISTORICO_INTERVALO_S` | `60` | Segundos entre pontos dos gráficos |
| `NOBREAK_SIMULAR` | `0` | `1` usa dados de exemplo, com uma queda de energia simulada a cada minuto (para testar o painel) |

## Modelos

A família do protocolo que está implementada cobre os nobreaks Ragtech com USB que respondem ao quadro
`A0|id` (famílias 10, 13 e 15 do software oficial). Confirmado: **NEP/Easy Pro 1200 TI**. Veja a tabela e o
status de cada modelo em [`docs/modelos.md`](docs/modelos.md).

## Como funciona

O nobreak aparece como uma porta serial USB (CDC-ACM, `04d8:000a`) a 2400 baud, 8N1. O projeto envia pedidos
de 6 bytes com checksum e decodifica os registradores da resposta. O protocolo foi documentado por
engenharia reversa **para interoperabilidade** (Lei 9.609/98, art. 6º, V). Detalhes em
[`docs/protocolo.md`](docs/protocolo.md).

Este projeto **não contém código do software oficial** e não é afiliado nem endossado pela Ragtech.
"Ragtech", "Easy Pro" e demais nomes são marcas de seus donos, citadas só para indicar compatibilidade.

## Ajude a testar

Tem outro modelo? Abra uma issue com:

1. O modelo (etiqueta do nobreak) e a saída de `lsusb`.
2. A linha **Registradores** do painel (ou `curl localhost:8088/api/estado`).
3. Se os valores batem com o display do nobreak.

Tem também as flags `EVENTOS`, `FALHAS` e `USUARIO`, que ainda não têm significado conhecido. Se você
puder comparar o painel antes e depois de uma queda de energia, isso ajuda muito.

## Desenvolvimento

```bash
make dev-backend     # serviço com dados simulados em http://127.0.0.1:8080
make dev-frontend    # painel com hot reload (Vite)
make test            # cargo test + clippy + svelte-check
make docker          # imagem local
```

Requisitos: Rust 1.75+, Node 20+. Mais em [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Licença

[MIT](LICENSE).
