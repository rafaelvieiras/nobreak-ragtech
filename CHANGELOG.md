# Changelog

## 0.7.0 (30/out/2026)
Revisão de UX/UI e de internacionalização do painel.
- **Celular:** some a rolagem horizontal da página (a tabela de Quedas passou a rolar dentro do cartão); Quedas e Eventos viram lista empilhada abaixo de 480 px; cartões em 2 colunas; gráficos desenhados na largura real, com texto de 11 px.
- **Estados:** dados antigos ficam esmaecidos e a faixa de erro mostra a hora da última leitura; novo estado "Carregando…"; o erro só aparece na 2ª falha seguida; mensagens sem erro técnico cru. Durante a queda de energia o topo mostra há quanto tempo o nobreak está na bateria e a autonomia projetada.
- **Acessibilidade:** `role` de `status`/`alert`/`progressbar` nos elementos certos, descrição resumida em cada gráfico, áreas de rolagem acessíveis por teclado, `<caption>` e `<th scope>` nas tabelas, foco visível, `prefers-reduced-motion`, favicon e `<title>` que mostra o estado.
- **Leitura:** barras com cor por limiar (bateria < 30 % âmbar, < 15 % vermelha; saída > 80 % âmbar), aviso de bateria no topo com link, alternância 24 h / 30 dias, ticks redondos, eixo X com 3 a 5 marcas, linha tracejada para quem não distingue cor, legenda da faixa mín–máx, ponto isolado vira círculo.
- **Diagnóstico avançado:** Flags, Eventos e Registradores ficam num `<details>` fechado; botão "Copiar" nos registradores; o estado do NUT é traduzido (`OB LB → OL`).
- **i18n:** português e inglês, sem dependência (`frontend/src/i18n.svelte.ts`, cerca de 2 KB a mais com gzip), troca por link no rodapé e memória em `localStorage`. Números, datas, unidades e plurais usam `Intl`; vírgula decimal em português.
- **API:** `avisos[]` de `/api/bateria` ganhou `params` (valores usados no texto, ex.: `{"anos": 4.8}`), para o cliente traduzir pelo `codigo`. `texto` continua em português, igual ao webhook e ao ntfy.

## 0.6.0 (30/out/2026)
- Notificações por ntfy (`NOBREAK_NTFY_URL`, `NOBREAK_NTFY_TOPICO`, `NOBREAK_NTFY_TOKEN`), com prioridade pela gravidade e tags por evento. Webhook e ntfy podem rodar juntos, cada um com fila e thread próprias.
- `--testar-webhook` virou `--testar-notificacao` (o nome antigo ainda funciona) e testa todos os destinos ligados.

## 0.5.0 (30/out/2026)
- Notificações por webhook HTTP (`backend/src/notificar.rs`): queda, volta da rede, alarme, comunicação perdida e restabelecida, aviso de saúde da bateria. Variáveis `NOBREAK_WEBHOOK_URL`, `NOBREAK_WEBHOOK_CABECALHOS`, `NOBREAK_NOTIFICAR_NOME` e `NOBREAK_NOTIFICAR_EVENTOS`.
- Novo comando `--testar-webhook`.

## 0.4.0 (30/out/2026)
- Histórico de 30 dias (`/api/historico/longo`, `backend/src/longa.rs`): baldes de 15 min com média, mínimo e máximo, gravados em `NOBREAK_ARQUIVO_SERIE`. O painel mostra os gráficos com faixa mín–máx e corta a linha onde o serviço ficou parado.
- Novas variáveis `NOBREAK_ARQUIVO_SERIE`, `NOBREAK_SERIE_DIAS` e `NOBREAK_SERIE_INTERVALO_S`.

## 0.3.2 (30/out/2026)
- Se o serviço reinicia durante a recarga, o tempo de recarga fica desconhecido em vez de contar a partir do reinício.

## 0.3.1 (30/out/2026)
- Saúde da bateria (`/api/bateria`, painel e métricas `nobreak_bateria_*`): tensão em repouso, autonomia projetada pela queda, afundamento de tensão, tempo de recarga até 99%, temperatura e idade. Os limites estão em `backend/src/bateria.rs`.
- Nova variável `NOBREAK_BATERIA_INSTALADA`.
- O arquivo de quedas guarda tensão mínima da bateria, carga máxima e recarga (linhas antigas continuam válidas).
- Ao aparar o log de eventos (limite de 500), saem primeiro as mudanças de registrador sem nome. Estado, conexão e bits com nome ficam.

## 0.3.0 (30/out/2026)
- Alarmes com nome a partir dos bits de `EVENTOS` e `FALHAS`; aparecem no topo do painel, em `/api/estado` (`alarmes`) e em `ups.alarm` do NUT.
- Estado do NUT passa a usar o bit de ausência de rede (`OB`) e os de bateria baixa/fim de bateria (`LB`) e sobrecarga (`OVER`), em vez de só a tensão de entrada.
- Registro de quedas de energia e resumo por período: `/api/quedas`, painel e métricas `nobreak_quedas_24h`, `nobreak_segundos_em_bateria_24h`, `nobreak_em_bateria`.
- Nova variável `NOBREAK_ARQUIVO_QUEDAS`. Uma queda cortada por falta de energia do servidor aparece como "interrompida".
- Contador regressivo de desligamento (`0x98`) no painel e em `nobreak_contagem_regressiva_segundos`.
- O log de mudanças mostra o nome do bit que ligou ou desligou.

## 0.2.1
- O leitor do arquivo de eventos tolera bytes nulos deixados por queda de energia.
