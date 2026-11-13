# Changelog

## 0.12.2 (13/nov/2026)
- **Correção grave:** travamento total do serviço (painel sem resposta, healthcheck estourando, arquivo do NUT congelado) por **dois impasses de lock** introduzidos na 0.12.1:
  - o coletor pegava `quedas` duas vezes na mesma thread — `Mutex` do `std` não é reentrante, então a segunda aquisição esperava para sempre. Era o efeito colateral de tirar o cálculo da autonomia de dentro do bloco de notificações;
  - o handler do `/api/bateria` pegava `quedas` e, dentro dos argumentos, `estado` — ordem inversa do coletor (`estado` → `quedas`), o que fecha o ciclo com uma requisição do painel.

  Agora o código usa o guard já adquirido e a ordem de lock está documentada no handler. O `HEALTHCHECK` do container pegou o travamento em 2 minutos — e o watchdog do host reinicia depois de 3 falhas seguidas.

## 0.12.1 (13/nov/2026)
- **Correção:** `battery.runtime` não aparecia quando as notificações estavam desligadas — o cálculo da autonomia estava dentro do bloco que monta as notificações. No Moon (sem ntfy/webhook) o NUT ficava sem o número mais importante para o cliente decidir desligar.
- **Correção:** `ups.alarm` com valor vazio **não** limpava nada: o `dummy-ups` repetia o token da linha anterior e o alarme aparecia igual ao `ups.load` (`ups.alarm: 33`). Agora, sem alarme, o valor é `-`.

## 0.12.0 (13/nov/2026)
Pensado para quem tem uma máquina grande atrás do mesmo nobreak.
- **`battery.runtime` no arquivo do NUT**: autonomia estimada agora (projeção ao vivo na queda; autonomia medida reescalada pela carga quando há rede). É o número que clientes NUT usam para "desligar quando faltar X de autonomia".
- **`LB` derivado** (`NOBREAK_LB_PROJETADO_MIN`, padrão 5 min): o serviço acrescenta `LB` ao `ups.status` quando a autonomia projetada acaba antes de o nobreak sinalizar bateria fraca. Sem isso, um cliente NUT só desligaria no fim da bateria.
- **Religamento por Wake-on-LAN** (`acoes.rs`): depois de uma queda registrada, com rede estável por `NOBREAK_LIGAR_ESTAVEL_S` e bateria ≥ `NOBREAK_LIGAR_BATERIA_PCT`, o serviço grava um pedido; o host envia o magic packet. Serve para deixar o "ligar ao voltar a energia" desligado no firmware e não subir a máquina com a bateria baixa.
- **Correção no arquivo do NUT**: `ups.alarm` passa a ser escrito **sempre** (vazio quando não há alarme) porque o `dummy-ups` mantém variável que desaparece do arquivo — no Moon um alarme de 30/out ficou preso por duas semanas; e o texto do alarme vai em ASCII, porque o driver embaralha byte não-ASCII.

## 0.11.0 (13/nov/2026)
A autonomia passa a ser mostrada com a carga que está ligada **agora**.
- **`/api/bateria`**: novos campos `carga_atual_pct` e `autonomia_na_carga_atual_min`, que reescalam a autonomia medida na última queda pela razão entre a carga daquela queda e a carga atual. Os avisos de autonomia (`autonomia_critica`/`autonomia_curta`) passaram a usar esse número — antes um teste com 3 % de carga rendia "50 min" mesmo com 35 % de carga ligada, o que dava falsa segurança.
- **Painel**: a seção de bateria mostra "Carga ligada agora" e "Autonomia com a carga de agora" ao lado da autonomia medida.
- **Métricas**: `nobreak_autonomia_medida_min` e `nobreak_autonomia_carga_atual_min`.

## 0.10.0 (13/nov/2026)
A conta do desligamento passou a usar a **carga atual**, não uma queda antiga.
- **Autonomia projetada pela taxa real de queda** (`politica::projetar`): com pelo menos `NOBREAK_DESLIGAR_DADOS_MINIMOS_S` de queda, o serviço calcula `% por minuto` e estima quantos segundos restam até o piso da bateria. O gatilho é "autonomia projetada abaixo de `NOBREAK_DESLIGAR_MARGEM_S`", então ele se ajusta sozinho a qualquer carga.
- **Motivo real:** em 30/out a medição foi com **3 % de carga (~36 VA)**; com o equipamento atual a carga é **35 % (~434 VA)**, cerca de 12× mais potência. Um limiar fixo de porcentagem calculado naquele teste daria minutos de folga que hoje não existem.
- **Aviso encolhe com o tempo disponível:** a contagem é no máximo um terço da autonomia projetada, e o gatilho por projeção não espera a janela de confirmação (os dados de queda já são a confirmação). Com bateria em fim de vida, isso é a diferença entre desligar limpo e perder a máquina.
- **Piso absoluto** (`NOBREAK_DESLIGAR_BATERIA_PCT`, padrão 40) cobre o início da queda, quando ainda não há taxa para projetar.
- Painel: mostra a autonomia estimada durante a queda.

## 0.9.2 (13/nov/2026)
- Formatação (`cargo fmt`) que o CI cobra. Nada muda no comportamento em relação à 0.9.1.

## 0.9.1 (13/nov/2026)
- **Correção:** o pedido de desligamento dizia "gravado" mesmo quando a escrita falhava (ex.: permissão negada no volume). Agora o evento e a notificação carregam o erro, e o caso de `NOBREAK_ARQUIVO_DESLIGAR` vazio avisa que nenhum host vai desligar — antes parecia tudo certo e a máquina continuaria ligada até a bateria acabar.
- **Aviso de 0 s** vai direto ao pedido no mesmo ciclo, sem emitir um aviso que nunca existiu.

## 0.9.0 (13/nov/2026)
Desliga o computador antes de a bateria acabar.
- **Política de desligamento** (`backend/src/politica.rs`): age só com leitura boa e só em queda de energia. Dois gatilhos — bateria abaixo de `NOBREAK_DESLIGAR_BATERIA_PCT` **ou** `NOBREAK_DESLIGAR_TEMPO_S` segundos sem rede (proteção para quando a porcentagem mente) — exigem `NOBREAK_DESLIGAR_CONFIRMA_S` de condição contínua, depois `NOBREAK_DESLIGAR_AVISO_S` de contagem regressiva antes do pedido. Voltar a rede cancela e gera evento.
- **O padrão é desarmado** (`NOBREAK_DESLIGAR=0`): atualizar o serviço não passa a desligar máquina de ninguém sem aviso.
- **O container não desliga nada**: ele grava um JSON em `NOBREAK_ARQUIVO_DESLIGAR` (ex.: `/var/lib/nobreak-ragtech/desligar`) e quem desliga é um serviço do host (`nobreak-shutdown.path`), que reconfere idade do pedido e o estado do nobreak antes de agir. Assim o container continua `read_only`, sem privilégio e sem socket do Docker.
- **Números vindos de medição** (queda real de 30/out: 100 % → 18 % em 508 s, 9,7 %/min, autonomia total ~8,5 min): o padrão é 50 %, com 45 s de aviso — a 25 % restariam ~45 s, tempo insuficiente para desligar com folga.
- Painel: faixa vermelha com a contagem e o motivo, em português e inglês. Métricas novas: `nobreak_desligamento_armado`, `nobreak_desligamento_fase`, `nobreak_desligamento_restante_segundos`.
- Notificação de aviso e de pedido de desligamento pelos mesmos destinos (webhook/ntfy), quando configurados.

## 0.8.0 (12/nov/2026)
A serial do nobreak pode trocar de nome sozinha, e isso parava o serviço em silêncio.
- **Caminho alternativo automático** (`backend/src/fonte.rs`): quando o caminho de `NOBREAK_PORTA` não abre, o serviço tenta o último caminho que funcionou, os de `NOBREAK_PORTAS_ALTERNATIVAS` e por fim qualquer serial com o **mesmo `idVendor:idProduct`** do aparelho que já leu (lido do sysfs). Sem histórico de leitura ele não adivinha — abrir um serial alheio (coordenador Zigbee, modem) e escrever um quadro nele seria pior que ficar desconectado.
- **Mensagem de erro útil:** quando nada abre, o texto diz qual caminho falhou, que o aparelho da última leitura (USB `vid:pid`) não respondeu e quais seriais existem no sistema — é a pista para achar o nome novo.
- **`NOBREAK_PORTAS_ALTERNATIVAS`**: lista separada por vírgula de caminhos extras, para quem não tem `/dev/serial/by-id`.
- **`nobreak_segundos_sem_leitura`** nas métricas e `sem_leitura_desde` em `/api/estado`: dizem há quanto tempo o nobreak está mudo (antes só dava para deduzir pela hora da última leitura).
- **Docs:** `docs/instalacao.md` e `compose.yaml` deixam de ensinar `--device /dev/ttyACM0` (nó estático que morre na primeira reenumeração) e passam a usar `/dev` montado com `device_cgroup_rule` do major 166 + o caminho de `/dev/serial/by-id`.

Caso real que gerou a versão: no Moon, o adaptador USB reenumerou de `ttyACM0` para `ttyACM1` em 07/nov e o container ficou 5 dias com `No such device or address`, painel e NUT congelados.

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
