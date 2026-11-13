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
