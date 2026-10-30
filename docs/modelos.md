# Modelos

| Família do protocolo | Linhas Ragtech | Estado |
|---|---|---|
| 10 | NEP / Toro / Innergie, Easy Pro (600 a 2200 VA) | **implementada**; testado: modelo 6 (1200 VA, 115 V) |
| 13 | Save Digital | mesmo quadro; tabela de modelos a conferir |
| 15 | Easy Jet | mesmo quadro; registradores diferentes, a mapear |
| 0 a 9, 12, 14 | Micron, Senium, Infinium, Precision... | **não suportadas** (outro formato) |

O índice do modelo vem do registrador `0x9A`. Se o painel mostrar “modelo N ainda não está na tabela”, as escalas de
tensão da bateria e corrente podem estar erradas. Abra uma issue com os registradores.
