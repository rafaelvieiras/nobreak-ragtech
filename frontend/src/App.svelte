<script lang="ts">
  import { onMount } from 'svelte'
  import { buscarEstado, buscarEventos, buscarSerie, buscarQuedas, buscarBateria, buscarSerieLonga, type SerieLonga, type Saude, type Estado, type Eventos as TEventos, type Serie, type Quedas as TQuedas } from './api'
  import Grafico from './Grafico.svelte'
  import Eventos from './Eventos.svelte'
  import Cartao from './Cartao.svelte'
  import Quedas from './Quedas.svelte'
  import Bateria from './Bateria.svelte'
  import { t, tp, trad, num, val, dur, minutos, intervalo, horaSeg, horaFuso, textoAlarme, idioma, trocarIdioma } from './i18n.svelte'

  let e = $state<Estado | null>(null)
  let falhaApi = $state<string | null>(null)
  let falhasSeguidas = $state(0)
  let ultimoOk = $state<number | null>(null) // quando o painel recebeu o último estado (ms)
  let agora = $state(Date.now())
  let ev = $state<TEventos | null>(null)
  let serie = $state<Serie | null>(null)
  let quedas = $state<TQuedas | null>(null)
  let saude = $state<Saude | null>(null)
  let longa = $state<SerieLonga | null>(null)
  let periodo = $state<'24h' | '30d'>('24h')
  let copiado = $state(false)
  const col = (c: string) => (longa?.[c] as number[] | undefined) ?? []

  async function atualizar() {
    try { e = await buscarEstado(); falhaApi = null; falhasSeguidas = 0; ultimoOk = Date.now() } catch (x) {
      falhasSeguidas++
      // uma falha isolada não vira alarme: só a segunda seguida
      if (falhasSeguidas >= 2) falhaApi = x instanceof Error ? x.message : String(x)
    }
    agora = Date.now()
  }
  async function atualizarHistorico() {
    const [a, b, c, d, g] = await Promise.allSettled([buscarEventos(), buscarSerie(), buscarQuedas(), buscarBateria(), buscarSerieLonga()])
    if (a.status === 'fulfilled') ev = a.value
    if (b.status === 'fulfilled') serie = b.value
    if (c.status === 'fulfilled') quedas = c.value
    if (d.status === 'fulfilled') saude = d.value
    if (g.status === 'fulfilled') longa = g.value
  }
  onMount(() => {
    atualizar(); atualizarHistorico()
    const a = setInterval(atualizar, 3000), b = setInterval(atualizarHistorico, 15000), c = setInterval(() => (agora = Date.now()), 1000)
    return () => { clearInterval(a); clearInterval(b); clearInterval(c) }
  })

  const bin = (n: number) => n.toString(2).padStart(8, '0')
  const hex = (n: number) => '0x' + n.toString(16).padStart(2, '0')
  const h2 = (n: number | null | undefined) => (n === null || n === undefined ? '—' : n.toString(16).padStart(2, '0'))

  const carregando = $derived(e === null && !falhaApi)
  const obsoleto = $derived(!!falhaApi || (e !== null && !e.conectado))
  // idade da última leitura boa, em segundos
  const ultimaLeitura = $derived(e?.atualizado_em ?? (ultimoOk ? ultimoOk / 1000 : null))
  const idade = $derived(ultimaLeitura ? Math.max(0, Math.round(agora / 1000 - ultimaLeitura)) : null)
  const naBateria = $derived(!obsoleto && !!e?.status?.startsWith('OB'))
  const situacao = $derived.by(() => {
    if (carregando) return { cls: '', txt: t('sel.carregando') }
    if (falhaApi) return { cls: 'erro', txt: t('sel.sem_resposta') }
    if (!e || !e.conectado) return { cls: 'erro', txt: t('sel.desconectado') }
    const er = e.alarmes?.find((a) => a.gravidade === 'erro')
    if (er) return { cls: 'erro', txt: textoAlarme(er) }
    if (naBateria) return { cls: 'aviso', txt: t('sel.bateria') }
    if (e.status?.includes('CHRG')) return { cls: 'ok', txt: t('sel.carregando_bat') }
    return { cls: 'ok', txt: t('sel.normal') }
  })
  const nAvisos = $derived(saude?.avisos.length ?? 0)
  const queda = $derived(quedas?.lista.find((q) => q.fim === null && !q.interrompida) ?? null)
  const alarmes = $derived((e?.alarmes ?? []).filter((a) => !(naBateria && a.codigo === 'sem_rede')))

  $effect(() => {
    const m = e?.medidas
    document.title = obsoleto ? `${t('app.titulo')} · ${situacao.txt}`
      : naBateria && m ? `${t('app.titulo')} · ${situacao.txt} ${num(m.carga_bateria_pct, 0)}%` : t('app.titulo')
    document.documentElement.lang = idioma.v === 'en' ? 'en' : 'pt-BR'
  })

  const rotulosFlags: [keyof NonNullable<Estado['flags']>, number][] = [['nvolatil', 0x80], ['eventos', 0x90], ['falhas', 0x91], ['usuario', 0x92], ['gerais2', 0x95]]
  const nomeReg = (end: number) => trad('reg.' + hex(end).replace(/^0x(.*)$/, (_, x) => '0x' + x.toUpperCase()), hex(end))
  const regsTexto = $derived(e?.registradores_hex ? `0x80…0x9D: ${e.registradores_hex.match(/../g)?.join(' ')} · 0xF3: ${h2(e.calibracao_0xf3)} · 0x136: ${h2(e.capacidade_0x136)} · 0x202–0x203: ${e.osc_0x202?.map(h2).join(' ') ?? '—'}` : '')
  async function copiar() {
    try { await navigator.clipboard.writeText(regsTexto); copiado = true; setTimeout(() => (copiado = false), 1500) } catch { /* sem permissão */ }
  }
  const linhaLonga = $derived(longa ? intervalo(longa.intervalo_s) : '')
  const q = (c: string) => (c === 'cinza' ? '' : c)
</script>

<main>
  <header>
    <div>
      <h1>{t('app.titulo')}</h1>
      <div class="sub">{e?.modelo?.nome ?? '—'}{#if e?.medidas}<span aria-hidden="true">&nbsp;·&nbsp;</span><span class="sr">, </span>firmware&nbsp;{num(e.medidas.firmware, 1, 2)}{/if}</div>
    </div>
    <div class="topo-sel">
      {#if nAvisos > 0}<a class="chip" href="#saude">{tp('chip.avisos', nAvisos)}</a>{/if}
      <span class="selo {situacao.cls}" role="status">{situacao.txt}</span>
    </div>
  </header>

  {#if falhaApi}
    <div class="aviso-box erro-box" role="alert">
      {t('err.api')}
      {#if ultimaLeitura}<small>{t('err.dados_de', { hora: horaSeg(ultimaLeitura), idade: dur(idade) })}</small>{/if}
      <small>{t('err.detalhe', { erro: falhaApi })}</small>
    </div>
  {:else if e && !e.conectado}
    <div class="aviso-box erro-box" role="alert">
      {t('err.nobreak', { erro: e.ultimo_erro ?? t('err.aguardando') })}
      {#if ultimaLeitura}<small>{t('err.dados_de', { hora: horaSeg(ultimaLeitura), idade: dur(idade) })}</small>{/if}
    </div>
  {/if}

  {#if naBateria && e?.medidas}
    <section class="queda" role="alert" aria-label={t('queda.titulo')}>
      <strong>{t('queda.titulo')}</strong>
      <p>{queda ? t('queda.agora', { dur: dur(agora / 1000 - queda.inicio), pct: val(e.medidas.carga_bateria_pct, 0, '%') }) : t('queda.agora_sem_tempo', { pct: val(e.medidas.carga_bateria_pct, 0, '%') })}</p>
      {#if e.desligamento?.autonomia_s != null}<p class="sub">{t('queda.agora_autonomia', { dur: dur(e.desligamento.autonomia_s) })}</p>
      {:else if saude?.ultima_queda?.autonomia_projetada_min != null}<p class="sub">{t('queda.autonomia', { min: minutos(saude.ultima_queda.autonomia_projetada_min) })}</p>{/if}
    </section>
  {/if}
  {#if e?.desligamento && e.desligamento.fase !== 'normal'}
    <div class="aviso-box erro-box" role="alert">
      {#if e.desligamento.fase === 'aviso' && e.desligamento.restante_s != null}
        {t('desl.aviso', { s: dur(e.desligamento.restante_s), motivo: e.desligamento.motivo ?? '' })}
      {:else}
        {t('desl.pedido', { motivo: e.desligamento.motivo ?? '' })}
      {/if}
    </div>
  {/if}
  {#each alarmes as a}
    <div class="aviso-box" class:erro-box={a.gravidade === 'erro'} role={a.gravidade === 'erro' ? 'alert' : 'status'}>
      <span class="sr">{t('grav.' + a.gravidade)}: </span>{a.gravidade === 'erro' ? '⚠' : '•'} {textoAlarme(a)}
    </div>
  {/each}
  {#if e?.medidas && e.medidas.contagem_regressiva_s > 0}
    <div class="aviso-box" role="alert">{t('contagem', { dur: dur(e.medidas.contagem_regressiva_s) })}</div>
  {/if}
  {#if e?.modelo && !e.modelo.reconhecido}
    <div class="aviso-box" role="status">{t('modelo.desconhecido', { n: e.modelo.indice })}</div>
  {/if}

  {#if e?.medidas}
    {@const m = e.medidas}
    <h2>{t('sec.bateria')}</h2>
    <div class="grade" class:obsoleto>
      <Cartao rotulo={t('c.carga_bat')} valor={num(m.carga_bateria_pct, 0)} unidade="%" barra={m.carga_bateria_pct} limiar={[30, 15]} inverso />
      <Cartao rotulo={t('c.tensao_bat')} valor={num(m.tensao_bateria_v, 1)} unidade="V" ajuda={t('c.ajuda_vbat')} />
      <Cartao rotulo={t('c.temp')} valor={num(m.temperatura_c, 0)} unidade="°C" />
    </div>

    <h2>{t('sec.rede')}</h2>
    <div class="grade" class:obsoleto>
      <Cartao rotulo={t('c.ent')} valor={num(m.tensao_entrada_v, 0)} unidade="V" />
      <Cartao rotulo={t('c.sai')} valor={num(m.tensao_saida_v, 0)} unidade="V" />
      <Cartao rotulo={t('c.freq')} valor={num(m.frequencia_hz, 1)} unidade="Hz" dica={m.frequencia_hz === null ? t('c.sem_cal') : t('c.estimada')} />
      <Cartao rotulo={t('c.corr')} valor={num(m.corrente_saida_a, 2)} unidade="A" />
      <Cartao rotulo={t('c.carga_saida')} valor={num(m.carga_saida_pct, 0)} unidade="%" barra={m.carga_saida_pct} limiar={[80, 100]} />
      <Cartao rotulo={t('c.pot')} valor={num(m.tensao_saida_v * m.corrente_saida_a, 0)} unidade="VA" dica={t('c.calculada')} />
    </div>
  {:else if carregando}
    <div class="grade obsoleto" aria-busy="true">
      {#each Array(6) as _}<Cartao rotulo="…" valor="—" />{/each}
    </div>
  {/if}

  {#if saude}
    <h2 id="saude">{t('sec.saude')}</h2>
    <Bateria dados={saude} />
  {/if}

  {#if quedas}
    <h2>{t('sec.quedas')}</h2>
    <Quedas dados={quedas} />
  {/if}

  {#if serie}
    <h2>{t('sec.graficos')}</h2>
    {#if longa && longa.t.length > 1}
      <div class="abas" role="group" aria-label={t('v.periodo')}>
        <button type="button" aria-pressed={periodo === '24h'} onclick={() => (periodo = '24h')}>{t('v.24h')}</button>
        <button type="button" aria-pressed={periodo === '30d'} onclick={() => (periodo = '30d')}>{t('v.30d')}</button>
      </div>
    {/if}
    {#if periodo === '24h' || !longa || longa.t.length < 2}
      <div class="grade graficos" class:obsoleto>
        <Grafico titulo={t('g.rede')} t={serie.t} unidade="V" series={[{ nome: t('s.entrada'), cor: '#58a6ff', valores: serie.entrada_v }, { nome: t('s.saida'), cor: '#3fb950', valores: serie.saida_v }]} />
        <Grafico titulo={t('g.bateria')} t={serie.t} unidade="%" min={0} max={100} series={[{ nome: t('s.carga'), cor: '#d29922', valores: serie.bateria_pct }]} />
        <Grafico titulo={t('g.vbat')} t={serie.t} unidade="V" casas={1} series={[{ nome: t('s.bateria'), cor: '#bc8cff', valores: serie.bateria_v }]} />
        <Grafico titulo={t('g.carga')} t={serie.t} unidade="%" min={0} max={100} series={[{ nome: t('s.carga'), cor: '#f0883e', valores: serie.carga_pct }]} />
        <Grafico titulo={t('g.temp')} t={serie.t} unidade="°C" series={[{ nome: t('s.inversor'), cor: '#f85149', valores: serie.temperatura_c }]} />
      </div>
      <p class="sub">{t('g.legenda24')}</p>
    {:else if longa}
      <div class="grade graficos" class:obsoleto>
        <Grafico titulo={t('g.rede')} datas quebra={longa.intervalo_s * 3} t={longa.t} unidade="V" faixa={{ cor: '#58a6ff', min: col('entrada_v_min'), max: col('entrada_v_max') }} series={[{ nome: t('s.media'), cor: '#58a6ff', valores: col('entrada_v_media') }]} />
        <Grafico titulo={t('g.bateria')} datas quebra={longa.intervalo_s * 3} t={longa.t} unidade="%" min={0} max={100} faixa={{ cor: '#d29922', min: col('bateria_pct_min'), max: col('bateria_pct_max') }} series={[{ nome: t('s.media'), cor: '#d29922', valores: col('bateria_pct_media') }]} />
        <Grafico titulo={t('g.vbat')} datas quebra={longa.intervalo_s * 3} t={longa.t} unidade="V" casas={1} faixa={{ cor: '#bc8cff', min: col('bateria_v_min'), max: col('bateria_v_max') }} series={[{ nome: t('s.media'), cor: '#bc8cff', valores: col('bateria_v_media') }]} />
        <Grafico titulo={t('g.carga')} datas quebra={longa.intervalo_s * 3} t={longa.t} unidade="%" min={0} max={100} faixa={{ cor: '#f0883e', min: col('carga_pct_min'), max: col('carga_pct_max') }} series={[{ nome: t('s.media'), cor: '#f0883e', valores: col('carga_pct_media') }]} />
        <Grafico titulo={t('g.temp')} datas quebra={longa.intervalo_s * 3} t={longa.t} unidade="°C" faixa={{ cor: '#f85149', min: col('temperatura_c_min'), max: col('temperatura_c_max') }} series={[{ nome: t('s.media'), cor: '#f85149', valores: col('temperatura_c_media') }]} />
      </div>
      <p class="sub">{t('g.legenda30', { int: linhaLonga })}</p>
    {/if}
  {/if}

  {#if e?.flags || ev || e?.registradores_hex}
    <details class="diag">
      <summary>{t('sec.diag')}</summary>
      <p class="sub">{t('diag.nota')}</p>

      {#if e?.flags}
        <h2>{t('sec.flags')}</h2>
        <div class="card">
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div class="rolagem" role="region" aria-label={t('f.tabela')} tabindex="0">
            <table>
              <caption class="sr">{t('f.tabela')}</caption>
              <thead><tr><th scope="col">{t('f.reg')}</th><th scope="col">{t('f.end')}</th><th scope="col">{t('f.hex')}</th><th scope="col">{t('f.bits')}</th></tr></thead>
              <tbody>
                {#each rotulosFlags as [k, end]}
                  {@const v = e.flags[k]}
                  <tr><th scope="row">{nomeReg(end)}</th><td>{hex(end)}</td><td>{hex(v)}</td>
                    <td class="bits">{#each bin(v).split('') as b}<span class:lig={b === '1'}>{b}</span>{/each}</td></tr>
                {/each}
              </tbody>
            </table>
          </div>
          <div class="dica sub" style="margin-top:8px">{t('f.nota')}</div>
        </div>
      {/if}

      {#if ev}
        <h2>{t('sec.eventos')}</h2>
        <Eventos dados={ev} />
      {/if}

      {#if e?.registradores_hex}
        <h2>{t('sec.regs')}</h2>
        <div class="card">
          <div class="hex">{regsTexto}</div>
          <button type="button" class="link" onclick={copiar}>{copiado ? t('r.copiado') : t('r.copiar')}</button>
        </div>
      {/if}
    </details>
  {/if}

  <footer>
    <span>{ultimaLeitura ? t('rod.ultima', { hora: horaFuso(ultimaLeitura), idade: dur(idade) }) : t('rod.nenhuma')}</span>
    <span>{t('rod.leituras', { n: num(e?.leituras_ok ?? 0, 0) })}</span>
    <details class="ajuda"><summary>{t('rod.dev')}</summary>
      <a href="/api/estado">/api/estado</a> · <a href="/metrics">/metrics</a></details>
    <button type="button" class="link" onclick={trocarIdioma} lang={idioma.v === 'pt' ? 'en' : 'pt-BR'}>{t('rod.idioma')}</button>
  </footer>
</main>
