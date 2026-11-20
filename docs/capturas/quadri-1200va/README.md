# Capturas da Ragtech Quadri 1200 VA (20/nov/2026)

Base da análise em [`../../quadri1200va.md`](../../quadri1200va.md). Os três `.pcapng` são capturas USBPcap
(Windows) **filtradas**: só o nobreak (dispositivo USB `1.1`, `04d8:000a`), sem os outros aparelhos do PC e sem
os metadados de sistema da captura. Abrem no Wireshark; o filtro `usbcom` mostra os quadros.

| Arquivo | Quadros | O que aconteceu |
|---|---|---|
| `1-rede-normal.pcapng` | 21 | rede normal, carga 3 % |
| `2-carga.pcapng` | 211 | um servidor ligado no nobreak (carga 3 % → 8–10 %) e depois desligado |
| `3-queda-de-energia.pcapng` | 229 | nobreak tirado da tomada por ~100 s (rel. 111 → 211 s) e recarga em seguida |
| `4-pi-escuta-2400-120s.txt` | 90 | escuta no Raspberry Pi a 2400 baud, 120 s, sem transmitir (quadro em hex + decodificação) |

Nenhuma captura tem transferência do PC para o nobreak: o software oficial só escuta.

## Valores de referência

Lidos na tela do software oficial durante a captura (gravação de tela alinhada pela coluna de tempo do
Wireshark; tolerância de ±1 quadro). "rel." é o tempo relativo ao primeiro pacote do arquivo. Entrada e
saída em V, bateria em V e %, carga em %.

### `2-carga.pcapng` (rel. 0 = 18:56:17,7, UTC−3)

| rel. (s) | Entrada | Saída | Bateria | Bateria % | Temp. °C | Carga % | Freq. Hz |
|---|---|---|---|---|---|---|---|
| 60 | 131 | 113,7 | 14,1 | 110 | 39 | 3 | 59,9 |
| 63 | 132 | 113,7 | 14,2 | 110 | 38 | 3 | 59,9 |
| 66 | 131 | 113,7 | 14,2 | 110 | 38 | 10 | 59,9 |
| 70 | 132 | 113,7 | 14,2 | 110 | 39 | 8 | 59,9 |
| 100 | 131 | 113,7 | 14,2 | 110 | 39 | 10 | 59,9 |
| 150 | 131 | 113,7 | 14,2 | 110 | 39 | 10 | 60,3 |
| 210 | 131 | 113,7 | 14,2 | 111 | 39 | 8 | 59,9 |
| 252 | 132 | 113,7 | 14,2 | 111 | 38 | 8 | 59,9 |
| 255 | 131 | 113,7 | 14,2 | 111 | 39 | 8 | 59,9 |
| 258 | 131 | 113,7 | 14,2 | 110 | 39 | 3 | 59,9 |

### `3-queda-de-energia.pcapng` (rel. 0 = 19:21:23,9, UTC−3)

| rel. (s) | Rede | Entrada | Saída | Bateria | Bateria % | Temp. °C | Carga % | Freq. Hz |
|---|---|---|---|---|---|---|---|---|
| 16 | sim | 131 | 113,7 | 13,7 | 98 | 41 | 10 | 59,9 |
| 56 | sim | 131 | 113,7 | 13,8 | 100 | 42 | 8 | 59,9 |
| 108 | sim | 131 | 113,7 | 13,7 | 98 | 42 | 8 | 59,9 |
| 112 | **não** | 0 | 116,1 | 13,3 | 87 | 42 | 3 | 59,9 |
| 116 | não | 0 | 116,1 | 13,1 | 82 | 42 | 3 | 59,9 |
| 127 | não | 0 | 114,9 | 12,8 | 74 | 42 | 8 | 59,9 |
| 159 | não | 0 | 114,9 | 12,8 | 74 | 42 | 3 | 59,9 |
| 191 | não | 0 | 114,9 | 12,7 | 72 | 43 | 3 | 59,9 |
| 210 | não | 0 | 114,3 | 12,7 | 72 | 44 | 3 | 59,9 |
| 214 | **sim** | 131 | 113,7 | 13,1 | 80 | 44 | 10 | 59,9 |
| 222 | sim | 131 | 113,7 | 13,2 | 85 | 44 | 8 | 59,9 |
| 236 | sim | 131 | 113,7 | 13,3 | 87 | 43 | 8 | 59,9 |
| 256 | sim | 131 | 113,7 | 13,4 | 90 | 42 | 8 | 59,9 |
| 289 | sim | 131 | 114,3 | 13,5 | 92 | 43 | 3 | 59,9 |

Na bateria, o software oficial mostra a entrada como 0, embora o byte 21 traga 7–9.

## Como ler sem Wireshark

Cada transferência `URB_BULK in` do endpoint `0x81` traz um quadro de 32 bytes depois do cabeçalho USBPcap
(27 bytes). Validação: começa com `55 55` e `BE16(b30, b31) == (S + (S >> 8)) & 0xFFFF`, com `S = soma(b0..b29)`.
