# Protocolo serial da Ragtech Quadri (Quadri 1200 VA)

Documentado por engenharia reversa de interoperabilidade, observando a comunicação entre o meu nobreak e o
software oficial (caixa-preta) e escutando a porta no Raspberry Pi. Nenhum executável do fabricante foi
descompilado ou desmontado, e nada aqui é código, configuração ou texto da interface do fabricante.
Marcado como **[confirmado]** o que foi validado contra o nobreak real, **[hipótese]** o que tem evidência mas
pouca variação e **[estimativa]** o que é ajuste com poucos pontos.

A família 10 (Easy Pro, NEP...) está em [`protocolo.md`](protocolo.md). A Quadri usa **outro quadro**: até a
0.13.1 o serviço mandava o pedido da família 10, lia o começo de um quadro da Quadri e parava com
`cabeçalho inesperado 0x55`.

## Aparelho testado

| Item | Valor |
|---|---|
| Linha | Ragtech Quadri, rack 19" 2U, 2 baterias |
| Etiqueta | Cód. Prod. 4261, modelo QDW 1200 CBU-TI, entrada 115-127/220 V automática, saída 115 V, 1200 VA, 60 Hz |
| USB | `04d8:000a` (CDC-ACM), `iSerialNumber = 0`, `/dev/serial/by-id/usb-Ragtech_Sistemas_de_Energia_USB_Serial_Port-if00` |

## Enlace

- Porta serial USB CDC-ACM, vendor/product `04d8:000a`, o mesmo da família 10. [confirmado]
- **O nobreak transmite sozinho** um quadro de 32 bytes a cada **~1,33 s**. Não existe pedido: monitorar é só
  escutar. [confirmado: 90 quadros em 120 s]
- O software oficial **nunca envia nada** para a Quadri. São zero transferências do PC para o nobreak em três
  capturas (~9 min), inclusive quando foi cancelado o desligamento automático do PC durante a queda, que é uma
  ação só local. [confirmado]
- **A velocidade da porta não importa.** Escutando a 2400, 9600 e 115200 baud, todos os quadros chegaram
  válidos. A placa USB gera os quadros e ignora o `SET_LINE_CODING`. O serviço abre a 2400 8N1, como na
  família 10. [confirmado]
- Cada leitura da porta entrega o quadro inteiro e alinhado. Em 123 quadros no Pi, nenhum byte foi descartado
  na sincronização. [confirmado]
- Um único programa por vez na porta, como na família 10.

## Quadro (32 bytes)

```
pos:  0  1  2  3  4  5  6  7  8  9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31
     55 55 ae 0e 09 89 72 63 df 6c ac 00 27 c2 30 27 10 00 00 04 03 7b c1 01 e4 00 63 0f 11 27 08 ee
     sinc. |------ constantes (2..12) -----| flags (13..19)     |Vi|Vo|Cg|Vb|  |Fq|     |Tp| soma |
```

### Soma de verificação [confirmado]

```
S = b0 + b1 + ... + b29            (soma comum, sem estouro)
(b30 << 8 | b31) == (S + (S >> 8)) & 0xFFFF
```

O sincronismo `55 55` entra na soma. Conferida em **461 de 461** quadros das capturas do Windows e em todos
os 123 quadros do Pi. Um quadro só é aceito com `55 55` no início **e** a soma certa.

### Mapa de bytes

| Byte | Nome | Conversão | Status |
|---|---|---|---|
| 0–1 | sincronismo | sempre `55 55` | confirmado |
| 2–12 | constantes | não mudaram em nenhuma condição. **Não** contêm o número de série (veja Metodologia) | desconhecido |
| 13–19 | flags | veja abaixo | parcial |
| 21 | tensão de entrada | `×1,065` → V (123 → 131 V, 124 → 132 V). Sem rede, o byte cai para 7–9 | hipótese forte |
| 22 | tensão de saída | `×0,5892` → V (193 → 113,7; 194 → 114,3; 195 → 114,9; 197 → 116,1) | confirmado |
| 23 | carga na saída | `×2,5` → %, arredondando 0,5 para cima (1 → 3 %, 3 → 8 %, 4 → 10 %) | confirmado |
| 24 | tensão da bateria | `×0,0618` → V (206 → 12,7 … 230 → 14,2) | confirmado |
| 25, 27, 28 | constantes | `00`, `0f`, `11` | desconhecido |
| 26 | frequência | `79,7 − 0,2 × b26` → Hz (99 → 59,9; 97 → 60,3). Não mudou na bateria | estimativa |
| 29 | temperatura | °C direto (38 a 45 nos testes) | confirmado |
| 30–31 | soma de verificação | veja acima | confirmado |

A escala da entrada é próxima da `×1,06` do registrador `0x8B` da família 10. Com a rede quase constante
(122–125), não dá para separar escala de deslocamento.

### Carga da bateria (calculada) [hipótese]

O quadro **não** traz porcentagem de bateria. O software oficial mostra um valor que acompanha exatamente a
tensão (byte 24). O serviço usa:

```
carga_% = (b24 − 161,7) × 1,625      limitada a 0..100
```

Isso dá ≈ 0 % em 10,0 V e 100 % em 13,8 V. Bate com o software oficial em ±1 ponto de 72 % a 100 % (descarga e
recarga). Carregando a 14,2 V, o software mostra 110–111 %; o serviço limita a 100 %. Como na família 10
(`0x87`), é uma estimativa por tensão: no começo de uma queda ela afunda pela resistência interna, e é por isso
que existe o assentamento da política de desligamento.

### Corrente, potência e firmware

O quadro não traz corrente nem versão de firmware. A API devolve `corrente_saida_a: null` e `firmware: null`, e
o arquivo do NUT omite `output.current` e `output.apower`, para não publicar um "0 A" falso.

## Bits de status

| Byte | Bit | Com rede | Sem rede | Significado | Status |
|---|---|---|---|---|---|
| **18** | `0x10` | 0 | 1 | **ausência de rede** (nobreak na bateria). Liga no 1º quadro da queda e apaga no 1º quadro com rede | confirmado (1 queda de ~100 s) |
| 16 | `0x10`/`0x01` | `10` | `01`/`11` | acompanha a rede no mesmo quadro do byte 18 | observação |
| 19 | `0x08` | 0 | 1 | apaga 1 quadro **depois** do byte 18 (inversor?) | hipótese |
| 14 | — | `30` (às vezes `31`) | `28` | apaga 1 quadro depois do byte 18 | observação |
| 13 | `0x02` | 1 | 0 | some na bateria | observação |
| 13, 15 | `0x40` | alterna | alterna | alterna sozinho a cada poucos quadros, sem relação com a rede | ruído/pulso |
| 15 | `0x03` | `3` | `0` | some na bateria | observação |
| 15 | `0x80` | 1 com bateria em flutuação (13,7 V) | — | apagou depois da queda e não acendeu na recarga (13,1 → 13,5 V) nem com 14,2 V | hipótese: "bateria carregada/flutuação" |

**Não observados:** bateria baixa, fim de bateria e sobrecarga. A queda mais funda chegou a 72 %. Sem eles, o
`LB` do NUT vem só do `LB` derivado da autonomia projetada (`NOBREAK_LB_PROJETADO_MIN`), que já existe para a
família 10.

## Como o serviço usa

- `NOBREAK_PROTOCOLO=auto` (padrão): a cada abertura da porta, o serviço **escuta por até 3 s sem enviar nada**.
  Se chegar um quadro `55 55` válido, fica no protocolo da Quadri. Se não chegar, segue com a família 10 como
  antes. Assim a Quadri nunca recebe o pedido da família 10, e a família 10 só ganha 3 s na primeira leitura.
- `NOBREAK_PROTOCOLO=quadri` ou `familia10` pula a detecção.
- Na Quadri, a cada ciclo o serviço limpa o buffer de entrada e espera o próximo quadro completo (no máximo 3 s).
  **Nenhum byte é escrito na porta.**
- `ups.status`: `OB` com o bit 4 do byte 18 (ou entrada abaixo de 80 V, como reforço), `CHRG` com rede e carga
  abaixo de 99 %. O alarme `sem_rede` usa o mesmo bit.
- No painel, a linha de bytes crus mostra o quadro inteiro, e os eventos de mudança de bits usam a posição do
  byte (`byte 18`) em vez de um registrador.
- `NOBREAK_SIMULAR=1` com `NOBREAK_PROTOCOLO=quadri` simula a Quadri com quadros reais (com uma queda a cada
  60 ciclos).

## Metodologia de testes

### Regras seguidas

1. **Somente leitura.** No Pi, o script de escuta abre a porta e substitui a função de escrita por uma que dá
   erro. Nenhum comando foi enviado à Quadri durante a investigação.
2. **Caixa-preta.** O software oficial só rodou normalmente enquanto o tráfego USB era gravado. Nenhum executável
   foi aberto, descompilado ou desmontado.
3. **Um programa por vez na porta.** O serviço ficou parado durante as capturas, e a porta foi conferida livre
   (`fuser`) antes de cada escuta.
4. **DTR e RTS em nível baixo** no script de escuta, por causa do aviso do driver `ragtech` do NUT sobre
   desligamento remoto. O Linux pode pulsar as linhas ao abrir a porta. As versões anteriores do serviço já
   abriam essa porta sem efeito nenhum.
5. **Queda de energia só com autorização do dono**, com carga leve e o plugue retirado da tomada, sem mexer no
   disjuntor.

### 1. Sintoma no serviço (0.13.1)

Com a porta liberada para o container, `/api/estado` mostrava `cabeçalho inesperado 0x55` e `leituras_ok: 0`.
O nobreak respondia, mas com um primeiro byte que não é o `0xA?` da família 10.

### 2. Captura USB no Windows

- PC x86_64 com Windows 11, o software oficial (Supervise Personal 6.3) e Wireshark 4.6 com **USBPcap 1.5.4**.
  O instalador do Wireshark 4.6 não ofereceu o USBPcap, que foi instalado separado. O `USBPcapCMD.exe` também
  precisou do Visual C++ Redistributable x64.
- Interface `USBPcap1`, com a opção de injetar os descritores dos aparelhos já ligados. O nobreak aparece como
  dispositivo `1.1` (endpoint `0x81` para dados do nobreak ao PC).
- O filtro `usb.capdata` fica vazio, porque o Wireshark reconhece a porta como serial e põe os dados no
  dissector `usbcom`. Os pcaps foram lidos por um leitor próprio de pcapng/USBPcap em Python, sem `tshark`.
- O `SET_LINE_CODING` **não apareceu** em nenhuma captura, nem reabrindo o software. Aparentemente um serviço
  dele mantém a porta aberta. A velocidade foi resolvida depois, no Pi.

### 3. Correlação com o software oficial

- Três capturas: rede normal (21 quadros), **teste de carga** (211 quadros, ligando e desligando um servidor no
  nobreak: carga de 3 % → 10 % → 3 %) e **queda de energia** (229 quadros, ~100 s sem rede, recarga em seguida).
- Durante as duas últimas, **gravação de tela** com o software oficial, o Wireshark e um relógio com segundos
  visíveis. O alinhamento entre vídeo e pcap usa a coluna de tempo relativo do Wireshark, que aparece no próprio
  vídeo (por exemplo, `rel = t_vídeo + 30,1 s`, conferido em dois pontos de cada vídeo).
- Quadros do vídeo extraídos com `ffmpeg` a cada poucos segundos e nos instantes de mudança (início e fim da
  queda, ligar a carga). Para cada quadro do vídeo, o valor mostrado pelo software foi comparado com o último
  quadro USB recebido antes dele. O software mostra o quadro recebido até ~0,5 s antes, então a tolerância é de
  ±1 quadro.
- Cada fórmula da tabela foi conferida em todos os pontos disponíveis (10 a 40 por medida).

### 4. Análise dos bytes

- Bytes que variam × constantes, por captura e entre capturas.
- A soma de verificação não era soma de 8 ou 16 bits, CRC-16 comum (ARC, MODBUS, XMODEM, CCITT, KERMIT, X25,
  BUYPASS, DNP) nem soma de palavras. A diferença entre os bytes 30–31 e a soma comum dava 7, 8 ou 9 e
  acompanhava quantas vezes a soma passava de múltiplos de 256. Daí a fórmula `S + (S >> 8)`, conferida em
  todos os quadros.
- O bit de rede saiu do teste de queda: comparando o último quadro com rede, o primeiro sem rede, o último sem
  rede e o primeiro com rede de volta.

### 5. Validação no Raspberry Pi (só escuta)

Raspberry Pi 5 com Debian 13, `pyserial` 3.5 num ambiente isolado, serviço parado, 8N1 sem controle de fluxo.

| Baud | Duração | Quadros válidos | Inválidos | Bytes descartados |
|---|---|---|---|---|
| 2400 | 15 s (execução manual, interrompida) | 8 | 0 | — |
| 2400 | 15 s | 12 | 0 | 0 |
| 9600 | 10 s | 7 | 0 | 0 |
| 115200 | 8 s | 6 | 0 | 0 |
| 2400 | 120 s | 90 | 0 | 0 |

Os quadros do Pi são iguais aos do Windows: mesmas constantes, mesma soma e valores coerentes (saída
113,7–114,9 V, bateria 13,84–13,90 V, 39–40 °C).

### 6. Número de série

O número de série da etiqueta foi procurado nos quadros e nos arquivos brutos: em texto, como inteiro de 2 a
8 bytes (big e little-endian) e em BCD, inteiro e sem o prefixo do código do produto. **Não aparece em nenhum
lugar.** O descritor USB também não tem número de série. Os quadros podem ser publicados.

### 7. O serviço no hardware

O binário estático arm64 desta versão (gerado pelo `Dockerfile`) rodou 40 s no Pi com `NOBREAK_PROTOCOLO=auto`. A detecção escolheu a Quadri
na primeira leitura, sem enviar nada. O `/api/estado` ficou conectado e `OL`, com valores coerentes (entrada
132 V, saída 114,3 V, bateria 13,78 V, carga 3 %, 45 °C), corrente e firmware `null`. O arquivo do NUT saiu sem
`output.current`.

### 8. Testes automatizados

`backend/src/quadri.rs` usa seis quadros reais como fixtures: rede normal, carga de 10 %, primeiro quadro sem
rede, meio da queda, primeiro quadro com rede de volta e recarga. Cada um é conferido contra o valor que o
software oficial mostrava no mesmo instante. Também há testes de soma, quadro corrompido, sincronização no fluxo
(lixo antes, quadro partido), alarme, log de eventos e arquivo do NUT.

## Capturas

As capturas filtradas (só o nobreak) e os valores de referência alinhados no tempo estão em
[`capturas/quadri-1200va/`](capturas/quadri-1200va/).

## Pendências

- Bits de bateria baixa, fim de bateria e sobrecarga: precisam de uma descarga mais longa (com carga de teste e
  nada importante no nobreak) ou de sobrecarga controlada.
- Escala × deslocamento da entrada: só com outra tensão de rede (ou em 220 V).
- Frequência: mais pontos para trocar a estimativa linear por uma fórmula.
- Bytes constantes 2–12, 25, 27 e 28: provavelmente modelo e configuração. Precisa de outro exemplar da Quadri
  (outra potência) para comparar.
