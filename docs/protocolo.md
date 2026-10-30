# Protocolo serial dos nobreaks Ragtech (famílias 10, 13 e 15)

Documentado por engenharia reversa de interoperabilidade. Nada aqui é código do fabricante.
Marcado como **[confirmado]** o que foi validado contra um nobreak real e **[hipótese]** o que não foi.

## Enlace

- Porta serial USB CDC-ACM, vendor/product `04d8:000a`, **2400 baud, 8N1**. [confirmado]
- Sem controle de fluxo. Um único mestre por vez: feche o `dummy-ups`, o Supervise e qualquer outro programa antes.
- Pause ~50 ms entre o pedido e a leitura e limpe o buffer de entrada antes de cada pedido. [confirmado]

## Pedido (6 bytes)

```
[A0|id] [cmd] [end_hi] [end_lo] [valor] [cks]
```

- `id`: nibble baixo do primeiro byte. Nos exemplares testados o nobreak responde com `A` (`0xAA`). [confirmado]
- `cks = (end_hi + end_lo + valor) & 0xFF`. **Só os bytes 2 a 4 entram na soma**; o primeiro byte e o comando não. [confirmado]
- `cmd`:

| cmd | Função | Status |
|---|---|---|
| `04` | Ler `valor` bytes a partir de `end` | confirmado |
| `01` | Escrever (copiar) | hipótese |
| `02` | Escrever com AND | hipótese |
| `03` | Escrever com OR | hipótese |

Este projeto só usa `04`. Quadros com checksum errado são ignorados ou travam a resposta do nobreak:
sempre calcule o checksum.

Exemplo: ler 30 bytes em `0x0080` → `AA 04 00 80 1E 9E`.

## Resposta

`[A0|id]` seguido dos bytes pedidos. Não há checksum na resposta. Um primeiro byte com nibble alto `C` indica
que o nobreak recusou o pedido. [confirmado para o caso de sucesso; recusa é hipótese]

## Mapa de registradores (bloco `0x80`, 30 bytes)

| End. | Nome | Conversão | Status |
|---|---|---|---|
| `80` | NVOLATIL | bits (flags que sobrevivem a reinício) | bits desconhecidos |
| `81`-`82` | PPOWERLO / TPOWERLO | modo baixo consumo | hipótese |
| `83` | CAPBATREF | capacidade de referência | hipótese |
| `84`-`86` | WAKEUP | timer de 24 bits | hipótese |
| `87` | carga da bateria | `×0,393` → % | confirmado |
| `8A` | tensão da bateria | `×escala do modelo` (0,067 ou 0,134) | ver abaixo |
| `8B` | tensão de entrada | `×1,06` → V | confirmado |
| `8C` | corrente de saída | `×escala do modelo ÷ valor de 0xF3` → A | confirmado |
| `8D` | carga na saída | % | confirmado |
| `8E` | temperatura | °C | confirmado |
| `90` | EVENTOS | bits | bits desconhecidos |
| `91` | FALHAS | bits | bits desconhecidos |
| `92` | USUARIO | bits | bits desconhecidos |
| `95` | GERAIS2 | bits | bits desconhecidos |
| `97` | frequência | usa `0x202` e `0x203` | estimativa |
| `98` | contador regressivo de desligamento | s | hipótese |
| `9A` | índice do modelo | tabela em `backend/src/modelos.rs` | confirmado |
| `9B` | versão do firmware | `×0,1` | confirmado |
| `9D` | tensão de saída | `×0,555` (115 V) ou `×1,03` (220 V) | confirmado |

Registradores avulsos: `0xF3` (calibração da corrente, 1 byte), `0x136` (capacidade de bateria detectada),
`0x202`-`0x203` (constantes do oscilador).

### Frequência

```
f = (((reg97 - osc53) × 4 / (osc57 - osc53)) + 53) × 0,9806 + 1,46
```

onde `osc53` e `osc57` são os registradores `0x202` e `0x203`. Resultado típico na rede de 60 Hz: ~59,7 Hz. [estimativa]

### Tensão da bateria

`0x8A × escala do modelo` (0,067 no modelo 6) dá ≈ 13 V no Easy Pro 1200 TI. O arquivo de um script antigo registrava o dobro
(~26,8 V), mas o dono do nobreak concluiu que esse valor estava errado. Se o seu exemplar tiver bateria de 24 V e o painel mostrar
metade, use `NOBREAK_FATOR_VBAT=2` e conte na issue o modelo e a medida com multímetro.

## Pistas sobre o LED que fica piscando depois de queda de energia [hipótese]

O software oficial grava no registrador `0x80` com o quadro `AA 02 00 80 FE cks` (AND com `0xFE`, zera o bit 0).
O bit 0 do `NVOLATIL` aparece ligado em exemplares que piscam. Isso **não foi testado**: este projeto não escreve no nobreak.
Se você quiser ajudar, veja `CONTRIBUTING.md`.

## Sequência do software oficial

Ao conectar, ele marca "software presente" em `USUARIO` (OR `0x02`) e ao sair desmarca (AND `0xFD`). Isso sugere que o
firmware muda de comportamento quando há software ligado. Este projeto **não** faz isso por enquanto.

## Como descobrir o significado dos bits (somente leitura)

Deixe o serviço rodando com `NOBREAK_ARQUIVO_EVENTOS` e provoque uma queda curta (tire o nobreak da tomada por 1 minuto, com pouca carga).
A tabela **Eventos e mudanças de bits** do painel mostra, em ordem, quais bits ligaram (`+bitN`) e desligaram (`−bitN`), a tensão da rede
e a bateria naquele instante. Abra uma issue com essa tabela e o modelo do nobreak.

### Bits que oscilam com a rede normal (Easy Pro 1200 TI, 30/out/2026) [observação]

Com a rede estável e a carga em 2%, quatro registradores alternam sozinhos a cada poucos segundos, sem relação com queda de energia:

| End. | Bit que alterna | Hipótese |
|---|---|---|
| `0x93` | bit 2 (`0x82` ↔ `0x86`) | estado do carregador ou do relé (não confirmado) |
| `0x94` | bit 6 (`0x82` ↔ `0xC2`) | idem |
| `0x95` | bit 0 (`0x00` ↔ `0x01`) | GERAIS2: pulso ligado ao ciclo do carregador (não confirmado) |
| `0x99` | bit 0 (`0x76` ↔ `0x77`) | possível ruído de medição |

Já `0x80`, `0x90`, `0x91` e `0x92` ficam parados nesse estado. Por isso o painel só destaca esses últimos como candidatos a alarme
(o painel já trata como "ruidoso", só contando, o registrador que muda mais de 30 vezes em 10 minutos).
