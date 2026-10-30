// Dicionário leve (sem dependência) e formatadores `Intl`. O idioma vem de `localStorage` ou do navegador.
// Os textos dos alarmes e dos avisos da bateria chegam do serviço com `codigo` estável; o `texto` em
// português serve de reserva para códigos que este dicionário ainda não conhece.

type Dic = Record<string, string>

const pt: Dic = {
  'app.titulo': 'Nobreak Ragtech',
  'sel.carregando': 'Carregando…',
  'sel.sem_resposta': 'Sem resposta do serviço',
  'sel.desconectado': 'Nobreak desconectado',
  'sel.bateria': 'Na bateria (falta de energia)',
  'sel.carregando_bat': 'Rede normal, carregando a bateria',
  'sel.normal': 'Rede normal',
  'chip.avisos_one': '{n} aviso da bateria',
  'chip.avisos_other': '{n} avisos da bateria',
  'err.api': 'Sem comunicação com o serviço. Tentando de novo…',
  'err.dados_de': 'Os valores abaixo são da última leitura, às {hora} (há {idade}).',
  'err.nobreak': 'Sem leitura do nobreak. Último erro: {erro}',
  'err.aguardando': 'aguardando a primeira leitura',
  'err.detalhe': 'Detalhe técnico: {erro}',
  'grav.erro': 'Erro',
  'grav.aviso': 'Aviso',
  'grav.info': 'Informação',
  'contagem': 'Contagem regressiva para desligar: {dur}',
  'modelo.desconhecido': 'O modelo {n} ainda não está na tabela. Os valores podem estar em escala errada. Abra uma issue com a linha “Registradores” (em Diagnóstico avançado).',
  'queda.titulo': 'Falta de energia',
  'queda.agora': 'Na bateria há {dur}. Carga: {pct}.',
  'queda.agora_sem_tempo': 'Na bateria. Carga: {pct}.',
  'queda.autonomia': 'Na última queda medida, a autonomia projetada foi de {min}.',
  'sec.bateria': 'Bateria',
  'sec.rede': 'Rede e saída',
  'sec.saude': 'Saúde da bateria',
  'sec.quedas': 'Quedas de energia',
  'sec.graficos': 'Gráficos',
  'sec.diag': 'Diagnóstico avançado',
  'sec.flags': 'Flags (bytes crus)',
  'sec.eventos': 'Eventos e mudanças de bits',
  'sec.regs': 'Registradores',
  'diag.nota': 'Dados técnicos para quem investiga o protocolo. Não precisa abrir no dia a dia.',
  'c.carga_bat': 'Carga da bateria',
  'c.tensao_bat': 'Tensão da bateria',
  'c.temp': 'Temperatura',
  'c.ent': 'Tensão de entrada',
  'c.sai': 'Tensão de saída',
  'c.freq': 'Frequência',
  'c.corr': 'Corrente de saída',
  'c.carga_saida': 'Carga na saída',
  'c.pot': 'Potência aparente',
  'c.estimada': 'estimada',
  'c.sem_cal': 'sem calibração',
  'c.calculada': 'calculada (V × A)',
  'c.ajuda_vbat': 'Não bate com o multímetro? Veja NOBREAK_FATOR_VBAT no README.',
  'ajuda': 'ajuda',
  'v.24h': '24 horas',
  'v.30d': '30 dias',
  'v.periodo': 'Período dos gráficos',
  'g.rede': 'Tensão da rede',
  'g.bateria': 'Bateria',
  'g.vbat': 'Tensão da bateria',
  'g.carga': 'Carga na saída',
  'g.temp': 'Temperatura',
  's.entrada': 'entrada',
  's.saida': 'saída',
  's.carga': 'carga',
  's.bateria': 'bateria',
  's.inversor': 'inversor',
  's.media': 'média',
  'g.faixa': 'faixa mín–máx',
  'g.agora': 'agora',
  'g.minimo': 'mínimo',
  'g.maximo': 'máximo',
  'g.vazio': 'Juntando dados… (um ponto a cada poucos minutos)',
  'g.legenda30': 'Linha = média. Faixa = mínimo e máximo de cada intervalo de {int}. A linha é cortada onde o serviço ficou parado.',
  'g.legenda24': 'Cada ponto é uma leitura. A linha é cortada onde o serviço ficou parado.',
  'q.periodo': 'Período',
  'q.quedas': 'Quedas',
  'q.tempo': 'Na bateria',
  'q.maior': 'Maior',
  'q.bat_min': 'Mín.',
  'q.resumo': 'Resumo das quedas por período',
  'q.lista': 'Lista das quedas',
  'q.inicio': 'Início',
  'q.duracao': 'Duração',
  'q.bateria': 'Bateria',
  'q.rede_min': 'Rede mínima',
  'q.interrompida': 'interrompida (serviço parou)',
  'q.em_curso': 'em curso, {dur}',
  'q.nenhuma': 'Nenhuma queda registrada.',
  'b.repouso': 'Tensão em repouso',
  'b.medindo': 'medindo (cerca de 5 min com a bateria cheia)',
  'b.nominal': '{v} (bateria de {n})',
  'b.temp': 'Temperatura do inversor',
  'b.idade': 'Idade da bateria',
  'b.nao_informada': 'não informada',
  'b.autonomia': 'Autonomia projetada (última queda)',
  'b.anterior': ', média anterior {v}',
  'b.afundou': 'Tensão caiu na queda',
  'b.cargamax': 'Carga máxima na queda',
  'b.recarga': 'Recarga até 99%',
  'b.em_andamento': 'em andamento',
  'b.ultima': 'Última queda analisável',
  'b.nenhuma': 'nenhuma ainda (precisa de 30 s ou mais com perda de carga)',
  'b.sem_aviso': 'Nenhum aviso: a bateria não mostra sinal de desgaste.',
  'ev.quando': 'Quando',
  'ev.oque': 'O que',
  'ev.mudanca': 'Mudança',
  'ev.rede': 'Rede',
  'ev.bateria': 'Bateria',
  'ev.inicio': 'início',
  'ev.estado': 'estado',
  'ev.conexao': 'conexão',
  'ev.nenhum': 'Nenhum evento ainda.',
  'ev.ruido': 'Registradores que mudam o tempo todo (só contados, sem evento): {lista}.',
  'ev.nota': 'Medidas que variam sempre (tensões, corrente, temperatura) não entram aqui; veja os gráficos. Os eventos ficam guardados enquanto o serviço roda.',
  'ev.cortado': 'Mostrando os {n} mais recentes de {total}.',
  'ev.restabelecida': 'comunicação restabelecida',
  'ev.sem_comunicacao': 'sem comunicação: {motivo}',
  'ev.lista': 'Eventos recentes',
  'st.OL': 'Rede normal',
  'st.OB': 'Na bateria',
  'st.LB': 'bateria baixa',
  'st.CHRG': 'carregando',
  'st.OVER': 'sobrecarga',
  'f.reg': 'Registrador',
  'f.end': 'End.',
  'f.hex': 'Hex',
  'f.bits': 'Bits (7→0)',
  'f.nota': 'Bit azul e sublinhado = ligado. Bits com nome aparecem como alarmes no topo; os demais ainda não têm significado conhecido.',
  'f.tabela': 'Bytes crus dos registradores de estado',
  'r.copiar': 'Copiar',
  'r.copiado': 'Copiado',
  'rod.ultima': 'Última leitura: {hora} (há {idade})',
  'rod.nenhuma': 'Nenhuma leitura ainda',
  'rod.dev': 'Para desenvolvedores',
  'rod.leituras': '{n} leituras com sucesso',
  'rod.idioma': 'English',
  'reg.0x80': 'NVOLÁTIL',
  'reg.0x90': 'EVENTOS',
  'reg.0x91': 'FALHAS',
  'reg.0x92': 'USUÁRIO',
  'reg.0x95': 'GERAIS2',
  'alarme.bateria_baixa': 'Bateria baixa',
  'alarme.sobretemperatura': 'Sobretemperatura',
  'alarme.fim_de_bateria': 'Fim de bateria',
  'alarme.sobrecarga': 'Sobrecarga na saída',
  'alarme.tensao_saida_anormal': 'Tensão de saída anormal',
  'alarme.tensao_bateria_anormal': 'Tensão de bateria anormal',
  'alarme.falha_inversor': 'Falha do inversor',
  'alarme.descarga_completa': 'Descarga completa da bateria ativada',
  'alarme.sem_rede': 'Ausência de rede elétrica',
  'alarme.rede_baixa': 'Tensão de rede baixa',
  'alarme.rede_alta': 'Tensão de rede alta',
  'alarme.software_presente': 'Software de monitoramento presente',
  'aviso.repouso_muito_baixo': 'Tensão em repouso muito baixa ({v}): bateria provavelmente no fim da vida',
  'aviso.repouso_baixo': 'Tensão em repouso baixa ({v}) com a rede ligada e a carga completa',
  'aviso.repouso_alto': 'Tensão em repouso alta ({v}): o carregador pode estar com defeito',
  'aviso.autonomia_critica': 'Autonomia projetada de {m} na última queda: bateria muito fraca',
  'aviso.autonomia_curta': 'Autonomia projetada de {m} na última queda',
  'aviso.autonomia_caindo': 'Autonomia caiu: {m} contra média de {media} nas quedas anteriores',
  'aviso.tensao_afunda': 'A tensão caiu {d} durante a última queda: bateria com resistência interna alta',
  'aviso.recarga_lenta': 'A última recarga levou {h} até 99%',
  'aviso.temperatura_critica': 'Temperatura do inversor em {t}',
  'aviso.temperatura_alta': 'Temperatura do inversor em {t}',
  'aviso.bateria_velha': 'Bateria com {anos}: troque (a vida típica é de 3 a 5 anos)',
  'aviso.bateria_envelhecendo': 'Bateria com {anos}: planeje a troca',
}

const en: Dic = {
  'app.titulo': 'Ragtech UPS',
  'sel.carregando': 'Loading…',
  'sel.sem_resposta': 'No response from the service',
  'sel.desconectado': 'UPS disconnected',
  'sel.bateria': 'On battery (power failure)',
  'sel.carregando_bat': 'Mains OK, charging the battery',
  'sel.normal': 'Mains OK',
  'chip.avisos_one': '{n} battery warning',
  'chip.avisos_other': '{n} battery warnings',
  'err.api': 'Cannot reach the service. Trying again…',
  'err.dados_de': 'The values below come from the last reading, at {hora} ({idade} ago).',
  'err.nobreak': 'No reading from the UPS. Last error: {erro}',
  'err.aguardando': 'waiting for the first reading',
  'err.detalhe': 'Technical detail: {erro}',
  'grav.erro': 'Error',
  'grav.aviso': 'Warning',
  'grav.info': 'Information',
  'contagem': 'Shutdown countdown: {dur}',
  'modelo.desconhecido': 'Model {n} is not in the table yet. Values may be scaled wrong. Open an issue with the “Registers” line (under Advanced diagnostics).',
  'queda.titulo': 'Power failure',
  'queda.agora': 'On battery for {dur}. Charge: {pct}.',
  'queda.agora_sem_tempo': 'On battery. Charge: {pct}.',
  'queda.autonomia': 'In the last measured outage the projected runtime was {min}.',
  'sec.bateria': 'Battery',
  'sec.rede': 'Mains and output',
  'sec.saude': 'Battery health',
  'sec.quedas': 'Power outages',
  'sec.graficos': 'Charts',
  'sec.diag': 'Advanced diagnostics',
  'sec.flags': 'Flags (raw bytes)',
  'sec.eventos': 'Events and bit changes',
  'sec.regs': 'Registers',
  'diag.nota': 'Technical data for people investigating the protocol. You do not need to open it day to day.',
  'c.carga_bat': 'Battery charge',
  'c.tensao_bat': 'Battery voltage',
  'c.temp': 'Temperature',
  'c.ent': 'Input voltage',
  'c.sai': 'Output voltage',
  'c.freq': 'Frequency',
  'c.corr': 'Output current',
  'c.carga_saida': 'Output load',
  'c.pot': 'Apparent power',
  'c.estimada': 'estimated',
  'c.sem_cal': 'not calibrated',
  'c.calculada': 'calculated (V × A)',
  'c.ajuda_vbat': 'Does not match your multimeter? See NOBREAK_FATOR_VBAT in the README.',
  'ajuda': 'help',
  'v.24h': '24 hours',
  'v.30d': '30 days',
  'v.periodo': 'Chart period',
  'g.rede': 'Mains voltage',
  'g.bateria': 'Battery',
  'g.vbat': 'Battery voltage',
  'g.carga': 'Output load',
  'g.temp': 'Temperature',
  's.entrada': 'input',
  's.saida': 'output',
  's.carga': 'load',
  's.bateria': 'battery',
  's.inversor': 'inverter',
  's.media': 'average',
  'g.faixa': 'min–max band',
  'g.agora': 'now',
  'g.minimo': 'minimum',
  'g.maximo': 'maximum',
  'g.vazio': 'Collecting data… (one point every few minutes)',
  'g.legenda30': 'Line = average. Band = minimum and maximum of each {int} interval. The line breaks where the service was stopped.',
  'g.legenda24': 'Each point is one reading. The line breaks where the service was stopped.',
  'q.periodo': 'Period',
  'q.quedas': 'Outages',
  'q.tempo': 'On battery',
  'q.maior': 'Longest',
  'q.bat_min': 'Min.',
  'q.resumo': 'Outage summary by period',
  'q.lista': 'Outage list',
  'q.inicio': 'Start',
  'q.duracao': 'Duration',
  'q.bateria': 'Battery',
  'q.rede_min': 'Lowest mains',
  'q.interrompida': 'interrupted (service stopped)',
  'q.em_curso': 'ongoing, {dur}',
  'q.nenhuma': 'No outage recorded.',
  'b.repouso': 'Resting voltage',
  'b.medindo': 'measuring (about 5 min with a full battery)',
  'b.nominal': '{v} ({n} battery)',
  'b.temp': 'Inverter temperature',
  'b.idade': 'Battery age',
  'b.nao_informada': 'not provided',
  'b.autonomia': 'Projected runtime (last outage)',
  'b.anterior': ', previous average {v}',
  'b.afundou': 'Voltage drop in the outage',
  'b.cargamax': 'Peak load in the outage',
  'b.recarga': 'Recharge to 99%',
  'b.em_andamento': 'in progress',
  'b.ultima': 'Last analysable outage',
  'b.nenhuma': 'none yet (needs 30 s or more with charge loss)',
  'b.sem_aviso': 'No warning: the battery shows no sign of wear.',
  'ev.quando': 'When',
  'ev.oque': 'What',
  'ev.mudanca': 'Change',
  'ev.rede': 'Mains',
  'ev.bateria': 'Battery',
  'ev.inicio': 'start',
  'ev.estado': 'state',
  'ev.conexao': 'link',
  'ev.nenhum': 'No events yet.',
  'ev.ruido': 'Registers that change all the time (only counted, no event): {lista}.',
  'ev.nota': 'Measures that always vary (voltages, current, temperature) are not listed here; see the charts. Events are kept while the service runs.',
  'ev.cortado': 'Showing the {n} most recent of {total}.',
  'ev.restabelecida': 'communication restored',
  'ev.sem_comunicacao': 'no communication: {motivo}',
  'ev.lista': 'Recent events',
  'st.OL': 'Mains OK',
  'st.OB': 'On battery',
  'st.LB': 'low battery',
  'st.CHRG': 'charging',
  'st.OVER': 'overload',
  'f.reg': 'Register',
  'f.end': 'Addr.',
  'f.hex': 'Hex',
  'f.bits': 'Bits (7→0)',
  'f.nota': 'Blue underlined bit = on. Named bits show up as alarms at the top; the others have no known meaning yet.',
  'f.tabela': 'Raw bytes of the status registers',
  'r.copiar': 'Copy',
  'r.copiado': 'Copied',
  'rod.ultima': 'Last reading: {hora} ({idade} ago)',
  'rod.nenhuma': 'No reading yet',
  'rod.dev': 'For developers',
  'rod.leituras': '{n} successful readings',
  'rod.idioma': 'Português',
  'reg.0x80': 'NONVOLATILE',
  'reg.0x90': 'EVENTS',
  'reg.0x91': 'FAULTS',
  'reg.0x92': 'USER',
  'reg.0x95': 'GENERAL2',
  'alarme.bateria_baixa': 'Low battery',
  'alarme.sobretemperatura': 'Over-temperature',
  'alarme.fim_de_bateria': 'End of battery',
  'alarme.sobrecarga': 'Output overload',
  'alarme.tensao_saida_anormal': 'Abnormal output voltage',
  'alarme.tensao_bateria_anormal': 'Abnormal battery voltage',
  'alarme.falha_inversor': 'Inverter fault',
  'alarme.descarga_completa': 'Full battery discharge enabled',
  'alarme.sem_rede': 'No mains power',
  'alarme.rede_baixa': 'Mains voltage low',
  'alarme.rede_alta': 'Mains voltage high',
  'alarme.software_presente': 'Monitoring software present',
  'aviso.repouso_muito_baixo': 'Resting voltage very low ({v}): battery is probably at end of life',
  'aviso.repouso_baixo': 'Resting voltage low ({v}) with mains on and a full charge',
  'aviso.repouso_alto': 'Resting voltage high ({v}): the charger may be faulty',
  'aviso.autonomia_critica': 'Projected runtime of {m} in the last outage: battery very weak',
  'aviso.autonomia_curta': 'Projected runtime of {m} in the last outage',
  'aviso.autonomia_caindo': 'Runtime dropped: {m} against an average of {media} in earlier outages',
  'aviso.tensao_afunda': 'Voltage dropped {d} during the last outage: battery has high internal resistance',
  'aviso.recarga_lenta': 'The last recharge took {h} to reach 99%',
  'aviso.temperatura_critica': 'Inverter temperature at {t}',
  'aviso.temperatura_alta': 'Inverter temperature at {t}',
  'aviso.bateria_velha': 'Battery is {anos} old: replace it (typical life is 3 to 5 years)',
  'aviso.bateria_envelhecendo': 'Battery is {anos} old: plan to replace it',
}

const dicionarios: Record<string, Dic> = { pt, en }

function inicial(): 'pt' | 'en' {
  let salvo: string | null = null
  try { salvo = localStorage.getItem('idioma') } catch { /* sem armazenamento */ }
  const base = (salvo ?? (typeof navigator !== 'undefined' ? navigator.language : 'pt')).toLowerCase()
  return base.startsWith('en') ? 'en' : 'pt'
}

export const idioma = $state<{ v: 'pt' | 'en' }>({ v: inicial() })

export function trocarIdioma() {
  idioma.v = idioma.v === 'pt' ? 'en' : 'pt'
  try { localStorage.setItem('idioma', idioma.v) } catch { /* ignora */ }
}

export const loc = () => (idioma.v === 'en' ? 'en-US' : 'pt-BR')

function subst(s: string, p?: Record<string, string | number>) {
  return p ? s.replace(/\{(\w+)\}/g, (_, k) => (k in p ? String(p[k]) : `{${k}}`)) : s
}

/** Traduz a chave; cai no português e, por fim, na própria chave. */
export function t(k: string, p?: Record<string, string | number>): string {
  return subst(dicionarios[idioma.v][k] ?? pt[k] ?? k, p)
}

/** Como `t`, mas devolve `reserva` quando a chave não existe (textos que vêm do serviço). */
export function trad(k: string, reserva: string, p?: Record<string, string | number>): string {
  const s = dicionarios[idioma.v][k] ?? (idioma.v === 'pt' ? pt[k] : undefined)
  return s === undefined ? reserva : subst(s, p)
}

/** Plural pela regra do idioma: procura `chave_one`, `chave_other`... */
export function tp(k: string, n: number, p: Record<string, string | number> = {}): string {
  const cat = new Intl.PluralRules(loc()).select(n)
  const d = dicionarios[idioma.v]
  const chave = `${k}_${cat}` in d ? `${k}_${cat}` : `${k}_other`
  return t(chave, { n, ...p })
}

// ---- formatadores (as instâncias de Intl são caras: ficam em cache) ----
const cache = new Map<string, Intl.NumberFormat | Intl.DateTimeFormat>()
function nf(o: Intl.NumberFormatOptions) {
  const k = 'n' + loc() + JSON.stringify(o)
  let f = cache.get(k) as Intl.NumberFormat | undefined
  if (!f) cache.set(k, (f = new Intl.NumberFormat(loc(), o)))
  return f
}
function df(o: Intl.DateTimeFormatOptions) {
  const k = 'd' + loc() + JSON.stringify(o)
  let f = cache.get(k) as Intl.DateTimeFormat | undefined
  if (!f) cache.set(k, (f = new Intl.DateTimeFormat(loc(), o)))
  return f
}

const NBSP = '\u00a0'

/** Número no formato do idioma. `max` permite casas opcionais (padrão: igual a `d`). */
export function num(n: number | null | undefined, d = 1, max = d): string {
  if (n === null || n === undefined || Number.isNaN(n)) return '—'
  return nf({ minimumFractionDigits: d, maximumFractionDigits: Math.max(d, max) }).format(n)
}
/** Número com unidade, separados por espaço sem quebra: `13,2 V`, `92 %`. */
export function val(n: number | null | undefined, d: number, u: string): string {
  return n === null || n === undefined ? '—' : `${num(n, d)}${NBSP}${u}`
}
type Unidade = 'second' | 'minute' | 'hour' | 'day' | 'year'
export function un(n: number, unidade: Unidade, d = 0): string {
  const longo = unidade === 'day' || unidade === 'year'
  return nf({ style: 'unit', unit: unidade, unitDisplay: longo ? 'long' : 'short', maximumFractionDigits: d }).format(n)
}
/** Duração em segundos: `45 s`, `3 min 10 s`, `2 h 5 min`. */
export function dur(s: number | null | undefined): string {
  if (s === null || s === undefined) return '—'
  s = Math.round(s)
  if (s < 60) return un(s, 'second')
  const m = Math.floor(s / 60)
  if (m < 60) return s % 60 ? `${un(m, 'minute')} ${un(s % 60, 'second')}` : un(m, 'minute')
  return m % 60 ? `${un(Math.floor(m / 60), 'hour')} ${un(m % 60, 'minute')}` : un(Math.floor(m / 60), 'hour')
}
/** Duração em minutos (autonomia). */
export const minutos = (m: number | null | undefined) => (m === null || m === undefined ? '—' : dur(m * 60))
/** Tamanho de um intervalo: `5 s`, `15 min`, `2 h`. */
export function intervalo(s: number): string {
  if (s < 60) return un(Math.max(1, Math.round(s)), 'second')
  if (s < 3600) return un(Math.round(s / 60), 'minute')
  return un(Math.round(s / 3600), 'hour')
}
export function idade(dias: number | null): string {
  if (dias === null) return t('b.nao_informada')
  return dias < 365 ? un(dias, 'day') : un(dias / 365, 'year', 1)
}
export const hora = (s: number) => df({ hour: '2-digit', minute: '2-digit' }).format(s * 1000)
export const horaSeg = (s: number) => df({ hour: '2-digit', minute: '2-digit', second: '2-digit' }).format(s * 1000)
export const horaFuso = (s: number) => df({ hour: '2-digit', minute: '2-digit', second: '2-digit', timeZoneName: 'short' }).format(s * 1000)
/** Dia e hora; acrescenta o ano quando `ano` é verdadeiro (série que cruza a virada do ano). */
export const dataHora = (s: number, seg = false, ano = false) =>
  df({
    ...(ano ? { year: '2-digit' } : {}), day: '2-digit', month: '2-digit',
    hour: '2-digit', minute: '2-digit', ...(seg ? { second: '2-digit' } : {}),
  }).format(s * 1000)

/** Texto de um aviso de saúde da bateria: dicionário + parâmetros; reserva = `texto` do serviço. */
export function textoAviso(a: { codigo: string; texto: string; params?: Record<string, number> }): string {
  const p = a.params ?? {}
  const f: Record<string, string> = {}
  for (const [k, v] of Object.entries(p)) {
    if (k === 'v' || k === 'd') f[k] = val(v, k === 'v' ? 2 : 1, 'V')
    else if (k === 'm' || k === 'media') f[k] = minutos(v)
    else if (k === 'h') f[k] = un(v, 'hour')
    else if (k === 't') f[k] = val(v, 0, '°C')
    else if (k === 'anos') f[k] = un(v, 'year', 1)
    else f[k] = num(v, 0, 2)
  }
  return trad(`aviso.${a.codigo}`, a.texto, f)
}
export const textoAlarme = (a: { codigo: string; texto: string }) => trad(`alarme.${a.codigo}`, a.texto)

/** Estado do NUT (`OB LB`) em palavras: `Na bateria, bateria baixa`. */
export function statusHumano(s: string): string {
  return s.split(/\s+/).filter(Boolean).map((x) => trad(`st.${x}`, x)).join(', ')
}

const reverso = (() => {
  const m = new Map<string, string>()
  for (const [k, v] of Object.entries(pt)) if (k.startsWith('alarme.')) m.set(v, k.slice(7))
  return m
})()

/** Texto de um evento do histórico, traduzido quando o serviço o mandou em português. */
export function textoEvento(tipo: string, texto: string | null): string {
  if (!texto) return ''
  if (tipo === 'status') return texto.split(' → ').map(statusHumano).join(' → ')
  if (tipo === 'conexao') {
    if (texto === 'comunicação restabelecida') return t('ev.restabelecida')
    if (texto.startsWith('sem comunicação: ')) return t('ev.sem_comunicacao', { motivo: texto.slice(17) })
    return texto
  }
  // mudança de bits: "+ Ausência de rede elétrica, − Bateria baixa"
  return texto.split(', ').map((p) => {
    const sinal = p.slice(0, 1), nome = p.slice(2)
    const cod = reverso.get(nome)
    return cod ? `${sinal} ${t('alarme.' + cod)}` : p
  }).join(', ')
}
