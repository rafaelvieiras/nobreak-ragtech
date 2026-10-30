<script lang="ts">
  import { t, num, hora, horaSeg, dataHora } from './i18n.svelte'
  // Gráfico de linha em SVG puro (sem biblioteca), desenhado na largura real do cartão para o texto ficar legível.
  let { titulo, t: ts, series, unidade = '', min = null, max = null, faixa = null, quebra = 0, datas = false, casas = 0 }: {
    titulo: string; t: number[]
    series: { nome: string; cor: string; valores: number[] }[]
    unidade?: string; min?: number | null; max?: number | null
    /** Área sombreada entre dois valores por ponto (ex.: mínimo e máximo do balde). */
    faixa?: { cor: string; min: number[]; max: number[] } | null
    /** Se a distância entre dois pontos passar disso (s), a linha é cortada (serviço parado). */
    quebra?: number
    /** Rótulos do eixo X com dia e hora. */
    datas?: boolean
    casas?: number
  } = $props()

  let largura = $state(320)
  let passoAtual = 1
  const L = $derived(Math.max(240, largura))
  const A = 170, ME = 44, MD = 10, MS = 10, MI = 26

  function passo(span: number, alvo = 4) {
    const bruto = span / alvo, mag = 10 ** Math.floor(Math.log10(bruto)), r = bruto / mag
    return (r < 1.5 ? 1 : r < 3 ? 2 : r < 7 ? 5 : 10) * mag
  }

  let calc = $derived.by(() => {
    const todos = [...series.flatMap((s) => s.valores), ...(faixa ? [...faixa.min, ...faixa.max] : [])].filter((v) => Number.isFinite(v))
    if (ts.length < 2 || todos.length === 0) return null
    let lo = min ?? Math.min(...todos), hi = max ?? Math.max(...todos)
    if (hi - lo < 1e-6) { lo -= 1; hi += 1 }
    const p = passo(hi - lo)
    if (min === null) lo = Math.floor(lo / p) * p
    if (max === null) hi = Math.ceil(hi / p) * p
    passoAtual = p
    const marcas: number[] = []
    for (let v = Math.ceil(lo / p) * p; v <= hi + p * 1e-6; v += p) marcas.push(Math.round(v / p) * p)
    const t0 = ts[0], t1 = ts[ts.length - 1]
    const x = (v: number) => ME + ((v - t0) / Math.max(1, t1 - t0)) * (L - ME - MD)
    const y = (v: number) => MS + (1 - (v - lo) / (hi - lo)) * (A - MS - MI)
    const corta = (i: number) => i === 0 || (quebra > 0 && ts[i] - ts[i - 1] > quebra)
    const caminho = (vals: number[]) => vals.map((v, i) => `${corta(i) ? 'M' : 'L'}${x(ts[i]).toFixed(1)},${y(v).toFixed(1)}`).join('')
    // trechos de um ponto só não desenham linha: viram círculo
    const isolados = (vals: number[]) => vals.flatMap((v, i) => (corta(i) && (i === ts.length - 1 || corta(i + 1)) ? [[x(ts[i]), y(v)]] : []))
    const area = () => {
      if (!faixa) return ''
      let d = '', ini = 0
      for (let i = 1; i <= ts.length; i++) {
        if (i === ts.length || corta(i)) {
          const idx = Array.from({ length: i - ini }, (_, k) => ini + k)
          const cima = idx.map((j) => `${x(ts[j]).toFixed(1)},${y(faixa.max[j]).toFixed(1)}`)
          const baixo = idx.reverse().map((j) => `${x(ts[j]).toFixed(1)},${y(faixa.min[j]).toFixed(1)}`)
          d += `M${[...cima, ...baixo].join('L')}Z`
          ini = i
        }
      }
      return d
    }
    // 3 a 5 marcas no eixo X
    const n = L < 400 ? 3 : 5
    const curto = t1 - t0 < 1800
    const ano = new Date(t0 * 1000).getFullYear() !== new Date(t1 * 1000).getFullYear()
    const xs = Array.from({ length: n }, (_, k) => {
      const s = t0 + ((t1 - t0) * k) / (n - 1)
      return { x: x(s), r: datas ? dataHora(s, false, ano).replace(/, /, ' ') : curto ? horaSeg(s) : hora(s), a: k === 0 ? 'start' : k === n - 1 ? 'end' : 'middle' }
    })
    return { caminho, area, y, marcas, xs, isolados }
  })

  const dec = (p: number) => (p >= 1 ? 0 : Math.min(3, Math.ceil(-Math.log10(p) - 1e-9)))
  const f = (v: number) => num(v, casas, casas)
  const fe = (v: number) => num(v, Math.max(casas, dec(passoAtual)))
  const u = $derived(unidade ? '\u00a0' + unidade.trim() : '')
  const ultimo = (v: number[]) => (v.length ? v[v.length - 1] : null)
  const resumo = $derived.by(() => {
    if (!calc) return titulo
    const partes = series.map((s) => {
      const fin = s.valores.filter(Number.isFinite)
      if (!fin.length) return ''
      return `${s.nome}: ${t('g.agora')} ${f(ultimo(s.valores) ?? 0)}${u}, ${t('g.minimo')} ${f(Math.min(...fin))}${u}, ${t('g.maximo')} ${f(Math.max(...fin))}${u}`
    })
    return `${titulo}. ${partes.filter(Boolean).join('; ')}`
  })
  const tracos = ['', '5 3', '2 3']
</script>

<div class="card graf">
  <div class="topo">
    <span class="rot">{titulo}</span>
    <span class="leg">
      {#each series as s, i}
        <span><svg class="sw" width="16" height="6" aria-hidden="true"><line x1="0" y1="3" x2="16" y2="3" stroke={s.cor} stroke-width="2.5" stroke-dasharray={tracos[i % 3]} /></svg>{s.nome}{#if s.valores.length}&nbsp;<b>{f(ultimo(s.valores) ?? 0)}{u}</b>{/if}</span>
      {/each}
      {#if faixa}<span><svg class="sw" width="16" height="8" aria-hidden="true"><rect width="16" height="8" fill={faixa.cor} opacity="0.3" /></svg>{t('g.faixa')}</span>{/if}
    </span>
  </div>
  <div class="area" bind:clientWidth={largura}>
    {#if calc}
      <svg width={L} height={A} viewBox="0 0 {L} {A}" role="img" aria-label={resumo}>
        {#each calc.marcas as m}
          <line x1={ME} x2={L - MD} y1={calc.y(m)} y2={calc.y(m)} class="grade-h" />
          <text x={ME - 6} y={calc.y(m) + 4} class="eixo" text-anchor="end">{fe(m)}</text>
        {/each}
        {#each calc.xs as k}
          <text x={k.x} y={A - 6} class="eixo" text-anchor={k.a}>{k.r}</text>
        {/each}
        {#if faixa}<path d={calc.area()} fill={faixa.cor} opacity="0.3" />{/if}
        {#each series as s, i}
          <path d={calc.caminho(s.valores)} fill="none" stroke={s.cor} stroke-width="1.8" stroke-linejoin="round" stroke-dasharray={tracos[i % 3]} />
          {#each calc.isolados(s.valores) as [cx, cy]}<circle {cx} {cy} r="2.2" fill={s.cor} />{/each}
        {/each}
      </svg>
    {:else}
      <div class="vazio">{t('g.vazio')}</div>
    {/if}
  </div>
</div>

<style>
  .graf { padding: 10px 12px 6px; }
  .topo { display: flex; justify-content: space-between; flex-wrap: wrap; gap: 6px; font-size: 13px; }
  .rot { color: var(--mudo); text-transform: uppercase; letter-spacing: .05em; font-size: 12px; }
  .leg { display: flex; flex-wrap: wrap; gap: 4px 12px; color: var(--txt); font-variant-numeric: tabular-nums; }
  .leg b { font-weight: 600; }
  .sw { margin-right: 5px; vertical-align: middle; }
  .area { width: 100%; min-height: 100px; }
  svg:not(.sw) { display: block; margin-top: 4px; }
  .grade-h { stroke: var(--borda); stroke-width: 1; }
  .eixo { fill: var(--mudo); font-size: 11px; font-family: system-ui, sans-serif; font-variant-numeric: tabular-nums; }
  .vazio { color: var(--mudo); font-size: 13px; padding: 28px 0; text-align: center; }
</style>
