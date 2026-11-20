# Modelos

| Família do protocolo | Linhas Ragtech | Estado |
|---|---|---|
| 10 | NEP / Toro / Innergie, Easy Pro (600 a 2200 VA) | **implementada**; testado: modelo 6 (1200 VA, 115 V) |
| 13 | Save Digital | mesmo quadro; tabela de modelos a conferir |
| 15 | Easy Jet | mesmo quadro; registradores diferentes, a mapear |
| Quadri | Quadri (rack 19") | **implementada, só escuta** (quadro `55 55` que o nobreak transmite sozinho); testado: Quadri 1200 VA, 115 V. Ver [`quadri1200va.md`](quadri1200va.md) |
| 0 a 9, 12, 14 | Micron, Senium, Infinium, Precision... | **não suportadas** (outro formato) |

Na família 10, o índice do modelo vem do registrador `0x9A` (a Quadri não manda índice).
`NOBREAK_PROTOCOLO=auto` (padrão) escolhe entre a família 10 e a Quadri sem enviar nada para a Quadri. Se o painel mostrar “modelo N ainda não está na tabela”, as escalas de
tensão da bateria e corrente podem estar erradas. Abra uma issue com os registradores.
