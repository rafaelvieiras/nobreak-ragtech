# Instalação

## Descobrir a porta

```bash
lsusb | grep -i 04d8        # Microchip: é o chip do nobreak Ragtech
ls -l /dev/ttyACM* /dev/serial/by-id/
```

O nobreak costuma aparecer como `/dev/ttyACM0`. **Não use esse nome na configuração:**
o número muda quando o adaptador USB reenumera (queda de energia, replug, glitch da porta
USB) — `ttyACM0` vira `ttyACM1` e quem abria o nome antigo passa a receber
`No such device or address`.

Prefira o caminho de `/dev/serial/by-id/`, que aponta para o aparelho e não para o número:

```bash
ls -l /dev/serial/by-id/          # usb-Ragtech_Sistemas_de_Energia_USB_Serial_Port-if00
NOBREAK_PORTA=/dev/serial/by-id/usb-Ragtech_Sistemas_de_Energia_USB_Serial_Port-if00
```

Desde a 0.8.0 o serviço também se defende sozinho: se o caminho configurado não abrir, ele
procura outro serial com o **mesmo `idVendor:idProduct`** do aparelho que já leu (e nunca
abre um serial de outra pessoa, como um coordenador Zigbee). Para fixar caminhos extras
(máquina com várias seriais, sem `by-id`), use `NOBREAK_PORTAS_ALTERNATIVAS`:

```bash
NOBREAK_PORTAS_ALTERNATIVAS=/dev/ttyACM0,/dev/ttyACM1,/dev/ttyUSB0
```

Se preferir um nome curto e estável, uma regra udev também serve:

```
# /etc/udev/rules.d/99-ragtech.rules
SUBSYSTEM=="tty", ATTRS{idVendor}=="04d8", ATTRS{idProduct}=="000a", MODE="0660", GROUP="dialout", SYMLINK+="ragtech"
```

```bash
sudo udevadm control --reload && sudo udevadm trigger
```

> **Docker:** `--device /dev/ttyACM0:/dev/ragtech` cria um nó **estático** no container, criado
> no start. Se o aparelho reenumerar, o container fica com um nó morto para sempre (o
> `--restart` não recria) e o painel congela em silêncio. Use `/dev` montado e o `by-id` —
> veja a seção [Docker](#docker).

## Docker

```bash
docker run -d --name nobreak-ragtech --restart unless-stopped \
  -p 8088:8080 \
  -v /dev:/dev --device-cgroup-rule='c 166:* rmw' \
  -e NOBREAK_PORTA=/dev/serial/by-id/usb-Ragtech_Sistemas_de_Energia_USB_Serial_Port-if00 \
  --memory 32m --read-only --cap-drop ALL --security-opt no-new-privileges:true \
  ghcr.io/rafaelvieiras/nobreak-ragtech:latest
```

`-v /dev:/dev` é o que sobrevive à reenumeração USB; `--device-cgroup-rule='c 166:* rmw'` mantém
o container longe de qualquer coisa que não seja `ttyACM` (166 é o major desses dispositivos),
mesmo com `/dev` inteiro visível.

A imagem roda como usuário sem privilégios (`65532`). Se a porta não for `0666`, use
`--group-add` com o grupo dono dela (`"--group-add $(getent group dialout | cut -d: -f3)"`).

## Substituir o script `dummy-ups` do NUT

Se você já usa o NUT com um script que grava um arquivo para o `dummy-ups`, aponte o serviço para o mesmo arquivo.
**Pare o script antes**: dois programas na mesma porta serial se atrapalham.

```bash
docker run -d --name nobreak-ragtech ... \
  --user "$(id -u):$(id -g)" \
  -v /var/lib/nut/ragtech-ups.data:/saida/ragtech-ups.data \
  -e NOBREAK_ARQUIVO_NUT=/saida/ragtech-ups.data \
  ghcr.io/rafaelvieiras/nobreak-ragtech:latest
```

Campos gravados: `battery.charge`, `battery.voltage`, `input.voltage`, `output.voltage`, `output.current`,
`output.apower`, `ups.temperature`, `ups.load`, `ups.status`.

> `ups.status` é uma **estimativa** (`OL`, `OL CHRG` ou `OB`) enquanto os bits de EVENTOS e FALHAS não estão decodificados.
> Não use para desligar o servidor em queda de energia sem testar puxando o cabo.

## Guardar o registro de eventos entre reinícios

Os gráficos ficam só em memória. O registro de mudanças de bits pode ir para um arquivo (útil para estudar uma queda de energia
depois que o serviço reiniciou). A pasta precisa aceitar escrita pelo usuário do container:

```bash
sudo install -d -o 1000 -g 1000 /var/lib/nobreak-ragtech
docker run ... --user 1000:1000 \
  -v /var/lib/nobreak-ragtech:/dados -e NOBREAK_ARQUIVO_EVENTOS=/dados/eventos.jsonl \
  ghcr.io/rafaelvieiras/nobreak-ragtech:latest
```

Para guardar também as quedas de energia (resumo de 24 h, 7 e 30 dias), use `-e NOBREAK_ARQUIVO_QUEDAS=/dados/quedas.jsonl` na mesma pasta.

O arquivo de eventos guarda no máximo ~128 KB (os eventos mais antigos são descartados).

## Desligar o computador quando a bateria acaba

O serviço **não desliga nada sozinho**: ele grava um pedido num arquivo e quem desliga é um
serviço do sistema. Isso mantém o container `read_only`, sem privilégio e sem o socket do
Docker — quem decide desligar a máquina é o host.

```bash
docker run ... --user 1000:1000 \
  -v /var/lib/nobreak-ragtech:/dados \
  -e NOBREAK_DESLIGAR=1 \
  -e NOBREAK_ARQUIVO_DESLIGAR=/dados/desligar \
  ghcr.io/rafaelvieiras/nobreak-ragtech:latest
```

No host, um `path unit` vigia esse arquivo e executa o desligamento com as travas:

```ini
# /etc/systemd/system/nobreak-shutdown.path
[Path]
PathExists=/var/lib/nobreak-ragtech/desligar
Unit=nobreak-shutdown.service
```

O script (`nobreak-shutdown.sh`, em `moon/systemd/` do repo `homelab`) reconfere tudo antes
de desligar e cancela o pedido se: o arquivo de desarme existir
(`/etc/nobreak-desligar-desarmado`), o pedido tiver mais de 180 s, ou o nobreak responder
agora com rede presente. Para testar sem desligar, crie `/etc/nobreak-desligar-seco`.

### Escolher os limites

A porcentagem sozinha engana, por dois motivos: a bateria cai rápido no fim e a autonomia
depende da **carga** ligada nela. Por isso o serviço projeta a autonomia pela taxa de queda
que ele acabou de medir, e não por uma tabela fixa.

Duas medições do mesmo nobreak (1200 VA) mostram por que:

| Queda | Carga | Comportamento |
|---|---|---|
| 30/out/2026 | 3 % (~36 VA) | 100 % → 18 % em 508 s = **9,7 %/min** (~8,5 min no total) |
| 13/nov/2026 | 35 % (~434 VA) | mesma bateria, ~12× mais potência — a autonomia cai na mesma proporção |

Ou seja: uma bateria que entrega ~8,5 min com 36 VA está no fim da vida (o teórico seria
horas). Antes de confiar em qualquer limiar, meça a sua: ligue a carga real, corte a
energia e veja a queda.

| Variável | Padrão | O que faz |
|---|---|---|
| `NOBREAK_DESLIGAR` | `0` | `1` arma a política. Desarmado, o serviço só registra o que faria |
| `NOBREAK_DESLIGAR_MARGEM_S` | `120` | Desliga quando a autonomia **projetada** cai a este valor |
| `NOBREAK_DESLIGAR_BATERIA_PCT` | `40` | Piso absoluto: dispara mesmo sem projeção (início da queda) |
| `NOBREAK_DESLIGAR_PISO_PCT` | `15` | Bateria considerada vazia na projeção (o nobreak corta antes de 0 %) |
| `NOBREAK_DESLIGAR_DADOS_MINIMOS_S` | `20` | Tempo mínimo de queda para acreditar na taxa medida |
| `NOBREAK_DESLIGAR_CONFIRMA_S` | `10` | Confirmação dos gatilhos por piso/tempo (a projeção dispensa) |
| `NOBREAK_DESLIGAR_TEMPO_S` | `180` | Teto de tempo em bateria, caso a projeção diga que sobra carga |
| `NOBREAK_DESLIGAR_AVISO_S` | `20` | Contagem regressiva máxima; encolhe para 1/3 da autonomia projetada |
| `NOBREAK_ARQUIVO_DESLIGAR` | vazio | Onde gravar o pedido. Vazio = não grava nada (só evento e métrica) |

Se a rede voltar em qualquer momento antes do pedido, a contagem é cancelada e o evento
aparece no painel. A autonomia estimada aparece durante a queda.

## Integração com NUT (outras máquinas desligam junto)

O serviço grava o arquivo que o driver `dummy-ups` lê, então qualquer máquina pode ser
cliente NUT do host onde ele roda. O caso real é um servidor grande (Unraid) que precisa
desligar **antes** do nobreak acabar, enquanto o host pequeno segue na bateria.

No host do serviço (o que tem o cabo USB):

```ini
# /etc/nut/ups.conf
[server-room]
    driver = dummy-ups
    port = /var/lib/nut/ragtech-ups.data

# /etc/nut/upsd.users  — usuário só para escutar (slave)
[nashome]
    password = <senha>
    upsmon slave
```

`upsd` escuta em `0.0.0.0:3493` por padrão. Duas variáveis do arquivo existem para o
cliente decidir bem:

| Variável | O que é |
|---|---|
| `battery.runtime` | Autonomia estimada agora, em segundos: projeção ao vivo durante a queda, autonomia medida reescalada pela carga atual quando há rede |
| `ups.status` com `LB` derivado | `NOBREAK_LB_PROJETADO_MIN` (padrão 5): o serviço acrescenta `LB` quando a autonomia projetada cai abaixo disso, mesmo que o nobreak ainda não sinalize bateria fraca — é o que faz o cliente desligar a tempo |

Do outro lado, um `upsmon` em modo slave:

```ini
MONITOR server-room@HOST 1 nashome <senha> slave
MINSUPPLIES 1
SHUTDOWNCMD "/sbin/shutdown -h +0"
POLLFREQ 5
```

No Unraid isso é nativo: **Settings → UPS Settings → UPS type = NUT**, IP do host, nome do
UPS (`server-room`), usuário e senha. Os limiares do Unraid (bateria % ou tempo restante)
usam `battery.charge` e `battery.runtime` do serviço.

## Religar a máquina grande por Wake-on-LAN

`NOBREAK_LIGAR_NASHOME=1` faz o serviço gravar um pedido de WOL quando, **depois de uma
queda registrada**, a rede fica estável por `NOBREAK_LIGAR_ESTAVEL_S` (padrão 900 s) e a
bateria chega a `NOBREAK_LIGAR_BATERIA_PCT` (padrão 85 %). Quem envia o magic packet é o
host (`nobreak-ligar-nashome.path` → `nobreak-ligar-nashome.sh` → wolbridge). A ideia é
manter o "ligar ao voltar a energia" desligado no firmware da máquina, para ela não subir
com a bateria ainda baixa e cair de novo.

| Variável | Padrão | O que faz |
|---|---|---|
| `NOBREAK_LIGAR_NASHOME` | `0` | Liga o pedido de WOL |
| `NOBREAK_LIGAR_ESTAVEL_S` | `900` | Tempo de rede estável exigido |
| `NOBREAK_LIGAR_BATERIA_PCT` | `85` | Bateria mínima para religar |
| `NOBREAK_LIGAR_JANELA_H` | `6` | Só religa se a queda terminou nas últimas N horas |
| `NOBREAK_ARQUIVO_LIGAR_NASHOME` | vazio | Onde gravar o pedido |

## Binário sem Docker

Baixe o `.tar.gz` da sua arquitetura em [Releases](../../releases), confira o `SHA256SUMS` e rode:

```bash
NOBREAK_PORTA=/dev/serial/by-id/usb-Ragtech_Sistemas_de_Energia_USB_Serial_Port-if00 ./nobreak-ragtech
```

Para iniciar no boot, use um serviço `systemd`:

```ini
[Service]
ExecStart=/usr/local/bin/nobreak-ragtech
Environment=NOBREAK_PORTA=/dev/serial/by-id/usb-Ragtech_Sistemas_de_Energia_USB_Serial_Port-if00
User=nobreak
SupplementaryGroups=dialout
Restart=on-failure
```

## Segurança

O painel **não tem autenticação**. Não publique a porta na internet. Use `100.93.251.251:8088:8080` com um proxy reverso
(Caddy/Nginx + login) ou deixe só na rede local/VPN.
