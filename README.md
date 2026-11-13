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
- API JSON (`/api/estado`, `/api/eventos`, `/api/historico`, `/api/quedas`, `/api/bateria`, `/api/historico/longo`) e métricas Prometheus (`/metrics`).
- **Alarmes com nome** (ausência de rede, fim de bateria, sobrecarga, sobretemperatura, falha do inversor...) e estado correto para o NUT (`OL`, `OB`, `LB`, `OVER`, `CHRG`).
- **Resumo de quedas de energia** (24 h, 7 e 30 dias): quantidade, tempo na bateria, maior queda e bateria mínima. O começo de cada queda vai para o disco na hora, então uma queda que derruba o servidor não se perde.
- **Notificações por webhook e ntfy**: avisa em queda de energia, volta da rede, alarme novo, perda de comunicação e aviso de saúde da bateria. Envio em segundo plano, com novas tentativas, sem travar a leitura.
- **Histórico de 30 dias**: baldes de 15 min com média, mínimo e máximo de rede, bateria, carga e temperatura, gravados em arquivo. Sobrevive a reinício. O mínimo mostra quedas curtas que a média esconde.
- **Saúde da bateria**: tensão em repouso, autonomia projetada e tensão afundando a cada queda, tempo de recarga, temperatura e idade. Gera avisos no painel e em `/api/bateria`.
- **Contador regressivo de desligamento** do nobreak, quando ativo.
- Gera o arquivo do driver `dummy-ups` do **NUT** (Network UPS Tools), então o Home Assistant e o `upsmon` continuam funcionando.
- Mostra os registradores crus, para ajudar a decodificar o que ainda falta.
- **Registro de mudanças:** cada alteração de bit em registradores de estado (EVENTOS, FALHAS, USUARIO, NVOLATIL...) vira uma linha com data, tensão da rede e bateria. É a forma mais fácil de descobrir o que cada bit significa numa queda de energia.
- **Gráficos** de rede, bateria, carga e temperatura, em SVG puro, guardados em memória (24 h por padrão).

## Instalação rápida (Docker)

```bash
docker run -d --name nobreak-ragtech --restart unless-stopped \
  -p 8088:8080 \
  -v /dev:/dev --device-cgroup-rule='c 166:* rmw' \
  -e NOBREAK_PORTA=/dev/serial/by-id/usb-Ragtech_Sistemas_de_Energia_USB_Serial_Port-if00 \
  ghcr.io/rafaelvieiras/nobreak-ragtech:latest
```

O `by-id` não muda de nome quando o adaptador USB reenumera (`ttyACM0` → `ttyACM1`); o
`by-id` também não. Não use `--device /dev/ttyACM0`: é um nó estático que morre na
primeira reenumeração.

Abra `http://IP-DO-SERVIDOR:8088`. Também há um [`compose.yaml`](compose.yaml) de exemplo.
Guia completo, permissões da porta e integração com NUT: [`docs/instalacao.md`](docs/instalacao.md).

Sem Docker, baixe o binário em [Releases](../../releases) e rode `./nobreak-ragtech`.

## Configuração

| Variável | Padrão | O que faz |
|---|---|---|
| `NOBREAK_PORTA` | `/dev/ragtech` | Porta serial do nobreak. Prefira `/dev/serial/by-id/usb-Ragtech_...` a `/dev/ttyACM0` (o número muda quando o aparelho reenumera) |
| `NOBREAK_DESLIGAR` | `0` | `1` liga o desligamento do host por bateria baixa (o serviço grava um pedido, o host executa; veja `docs/instalacao.md`) |
| `NOBREAK_DESLIGAR_MARGEM_S` | `120` | Desliga quando a autonomia projetada (pela taxa de queda medida) cai a este valor |
| `NOBREAK_DESLIGAR_BATERIA_PCT` | `40` | Piso absoluto de bateria, para o início da queda |
| `NOBREAK_DESLIGAR_PISO_PCT` | `15` | Bateria considerada vazia na projeção |
| `NOBREAK_DESLIGAR_DADOS_MINIMOS_S` | `20` | Tempo de queda necessário para projetar |
| `NOBREAK_DESLIGAR_TEMPO_S` | `180` | Teto de tempo em bateria |
| `NOBREAK_DESLIGAR_CONFIRMA_S` | `10` | Confirmação dos gatilhos por piso/tempo |
| `NOBREAK_DESLIGAR_AVISO_S` | `20` | Contagem regressiva máxima (encolhe com pouca autonomia) |
| `NOBREAK_ARQUIVO_DESLIGAR` | vazio | Arquivo do pedido de desligamento (vazio = não grava) |
| `NOBREAK_LB_PROJETADO_MIN` | `5` | Minutos de autonomia projetada que fazem o NUT reportar `LB` (para outras máquinas desligarem a tempo). `0` desliga |
| `NOBREAK_LIGAR_NASHOME` | `0` | `1` pede o religamento da máquina grande por Wake-on-LAN depois da queda |
| `NOBREAK_LIGAR_ESTAVEL_S` | `900` | Rede estável exigida antes de religar |
| `NOBREAK_LIGAR_BATERIA_PCT` | `85` | Bateria mínima para religar |
| `NOBREAK_ARQUIVO_LIGAR_NASHOME` | vazio | Arquivo do pedido de WOL |
| `NOBREAK_PORTAS_ALTERNATIVAS` | vazio | Caminhos extras de serial, separados por vírgula. Desde a 0.8.0 o serviço já procura sozinho o mesmo aparelho (`idVendor:idProduct`) quando o caminho configurado não abre |
| `NOBREAK_ESCUTA` | `0.0.0.0:8080` | Endereço do painel |
| `NOBREAK_INTERVALO_S` | `5` | Segundos entre leituras |
| `NOBREAK_ID` | `A` | Nibble de endereço do quadro (hex). O padrão serve para a maioria |
| `NOBREAK_FATOR_VBAT` | `1` | Multiplica a tensão da bateria. Use `2` se o valor aparecer pela metade do medido |
| `NOBREAK_ARQUIVO_NUT` | vazio | Caminho do arquivo para o `dummy-ups` do NUT |
| `NOBREAK_ARQUIVO_EVENTOS` | vazio | Arquivo `.jsonl` para guardar o registro de mudanças entre reinícios (pasta com escrita) |
| `NOBREAK_ARQUIVO_QUEDAS` | vazio | Arquivo `.jsonl` das quedas de energia (2 linhas por queda; pasta com escrita) |
| `NOBREAK_BATERIA_INSTALADA` | vazio | Data da troca da bateria (`AAAA-MM-DD` ou `AAAA-MM`). Liga o aviso de idade (3 e 4 anos) |
| `NOBREAK_ARQUIVO_SERIE` | vazio | Arquivo `.jsonl` da série longa (cerca de 190 bytes por balde, uns 550 KB em 30 dias). Sem ele, a série longa fica só na memória |
| `NOBREAK_SERIE_DIAS` | `30` | Dias guardados na série longa |
| `NOBREAK_SERIE_INTERVALO_S` | `900` | Segundos por balde da série longa |
| `NOBREAK_WEBHOOK_URL` | vazio | Liga as notificações. Só `http://` (o cliente não tem TLS, para o binário continuar leve). Para `https`, use um proxy local |
| `NOBREAK_WEBHOOK_CABECALHOS` | vazio | Cabeçalhos extras, separados por `\|` (ex.: `Authorization: Bearer x\|X-Outro: 1`). Trate como segredo |
| `NOBREAK_NTFY_URL` | vazio | Liga o ntfy. Endereço do servidor, só `http://` (ex.: `http://host:2586`) |
| `NOBREAK_NTFY_TOPICO` | `nobreak` | Tópico do ntfy (sem `/` nem espaços) |
| `NOBREAK_NTFY_TOKEN` | vazio | Token de acesso do ntfy (enviado como `Authorization: Bearer`). Trate como segredo |
| `NOBREAK_NOTIFICAR_NOME` | `Nobreak` | Prefixo do título das mensagens |
| `NOBREAK_NOTIFICAR_EVENTOS` | vazio (todos) | Lista separada por vírgula: `queda`, `rede_voltou`, `alarme`, `desconectado`, `reconectado`, `saude` |
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

Os bits de `EVENTOS`, `FALHAS` e `USUARIO` já têm nome (veja `docs/protocolo.md`), mas alguns ainda são lacunas. Se você
puder comparar o painel antes e depois de uma queda de energia, isso ajuda muito, sobretudo uma queda que esgote a bateria.

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

## Notificações

Com `NOBREAK_WEBHOOK_URL` definida, o serviço faz um `POST` JSON a cada evento:

```json
{"evento":"queda","gravidade":"aviso","titulo":"Nobreak: falta de energia","texto":"O nobreak passou para a bateria (100%).","t":1793389130}
```

`gravidade` é `info`, `aviso` ou `erro`; `t` são segundos Unix. Para testar o destino sem esperar uma queda:

```bash
NOBREAK_WEBHOOK_URL=http://host:porta/caminho ./nobreak-ragtech --testar-notificacao
```

Regras do envio:
- Duas tentativas extras (após 2 s e 10 s). Se falhar, o erro vai para o log e a mensagem é descartada.
- Fila de 32 mensagens. Se o destino ficar fora do ar por muito tempo, as mais novas são descartadas.
- O mesmo alarme só reaparece depois de 10 min, e o mesmo aviso de saúde depois de 6 h.
- A falta de rede gera só o aviso de queda, não um alarme a mais.
- URL ou cabeçalho inválido desliga as notificações e escreve o motivo no log. A leitura do nobreak continua.

### ntfy

Com `NOBREAK_NTFY_URL`, o serviço publica no tópico `NOBREAK_NTFY_TOPICO` em JSON (`POST /` com `topic`, `title`, `message`, `priority`, `tags`). A prioridade segue a gravidade: `info` = 3, `aviso` = 4 (alta), `erro` = 5 (urgente). Cada evento leva uma tag que vira emoji no app (queda = ⚡, alarme = 🚨, bateria = 🔋).

```bash
NOBREAK_NTFY_URL=http://host:2586 NOBREAK_NTFY_TOPICO=nobreak NOBREAK_NTFY_TOKEN=tk_xxx ./nobreak-ragtech --testar-notificacao
```

Os dois destinos podem ficar ligados juntos. Cada um tem fila e thread próprias, então um destino fora do ar não atrasa o outro. O servidor `ntfy.sh` público usa `https` e por isso exige um proxy local; o normal é rodar o seu próprio ntfy na rede.
