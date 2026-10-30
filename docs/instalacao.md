# Instalação

## Descobrir a porta

```bash
lsusb | grep -i 04d8        # Microchip: é o chip do nobreak Ragtech
ls -l /dev/ttyACM* /dev/serial/by-id/
```

O nobreak costuma aparecer como `/dev/ttyACM0`. Para um nome fixo, crie uma regra udev:

```
# /etc/udev/rules.d/99-ragtech.rules
SUBSYSTEM=="tty", ATTRS{idVendor}=="04d8", ATTRS{idProduct}=="000a", MODE="0660", GROUP="dialout", SYMLINK+="ragtech"
```

```bash
sudo udevadm control --reload && sudo udevadm trigger
```

## Docker

```bash
docker run -d --name nobreak-ragtech --restart unless-stopped \
  -p 8088:8080 --device /dev/ragtech:/dev/ragtech -e NOBREAK_PORTA=/dev/ragtech \
  --group-add "$(getent group dialout | cut -d: -f3)" \
  --memory 32m --read-only --cap-drop ALL --security-opt no-new-privileges:true \
  ghcr.io/rafaelvieiras/nobreak-ragtech:latest
```

A imagem roda como usuário sem privilégios (`65532`). Se a porta não for `0666`, use `--group-add` com o grupo dono dela.

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
NOBREAK_PORTA=/dev/ragtech ./nobreak-ragtech
```

Para iniciar no boot, use um serviço `systemd`:

```ini
[Service]
ExecStart=/usr/local/bin/nobreak-ragtech
Environment=NOBREAK_PORTA=/dev/ragtech
User=nobreak
SupplementaryGroups=dialout
Restart=on-failure
```

## Segurança

O painel **não tem autenticação**. Não publique a porta na internet. Use `100.93.251.251:8088:8080` com um proxy reverso
(Caddy/Nginx + login) ou deixe só na rede local/VPN.
