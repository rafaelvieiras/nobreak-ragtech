# Changelog

## 0.3.0 (30/out/2026)
- Alarmes com nome a partir dos bits de `EVENTOS` e `FALHAS`; aparecem no topo do painel, em `/api/estado` (`alarmes`) e em `ups.alarm` do NUT.
- Estado do NUT passa a usar o bit de ausência de rede (`OB`) e os de bateria baixa/fim de bateria (`LB`) e sobrecarga (`OVER`), em vez de só a tensão de entrada.
- Registro de quedas de energia e resumo por período: `/api/quedas`, painel e métricas `nobreak_quedas_24h`, `nobreak_segundos_em_bateria_24h`, `nobreak_em_bateria`.
- Nova variável `NOBREAK_ARQUIVO_QUEDAS`. Uma queda cortada por falta de energia do servidor aparece como "interrompida".
- Contador regressivo de desligamento (`0x98`) no painel e em `nobreak_contagem_regressiva_segundos`.
- O log de mudanças mostra o nome do bit que ligou ou desligou.

## 0.2.1
- O leitor do arquivo de eventos tolera bytes nulos deixados por queda de energia.
