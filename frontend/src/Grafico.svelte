<script lang="ts">
  // Gráfico de linha em SVG puro (sem biblioteca). `series` compartilham o eixo X (tempo).
  let { titulo, t, series, unidade = '', min = null, max = null, faixa = null, quebra = 0, datas = false }: {
    titulo: string; t: number[]
    series: { nome: string; cor: string; valores: number[] }[]
    unidade?: string; min?: number | null; max?: number | null
    /** Área sombreada entre dois valores por ponto (ex.: mínimo e máximo do balde). */
    faixa?: { cor: string; min: number[]; max: number[] } | null
    /** Se a distância entre dois pontos passar disso (s), a linha é cortada (serviço parado). */
    quebra?: number
    /** Rótulos do eixo X com dia e hora. */
    datas?: boolean
  } = $props()

  const L = 560, A = 130, ME = 40, MD = 8, MS = 8, MI = 18
  let calc = $derived.by(() => {
    const todos = [...series.flatMap((s) => s.valores), ...(faixa ? [...faixa.min, ...faixa.max] : [])]
    if (t.length < 2 || todos.length === 0) return null
    let lo = min ?? Math.min(...todos), hi = max ?? Math.max(...todos)
    if (hi - lo < 1e-6) { lo -= 1; hi += 1 }
    const pad = min === null || max === null ? (hi - lo) * 0.08 : 0
    if (min === null) lo -= pad
    if (max === null) hi += pad
    const t0 = t[0], t1 = t[t.length - 1]
    const x = (v: number) => ME + ((v - t0) / Math.max(1, t1 - t0)) * (L - ME - MD)
    const y = (v: number) => MS + (1 - (v - lo) / (hi - lo)) * (A - MS - MI)
    const corta = (i: number) => i === 0 || (quebra > 0 && t[i] - t[i - 1] > quebra)
    const caminho = (vals: number[]) => vals.map((v, i) => `${corta(i) ? 'M' : 'L'}${x(t[i]).toFixed(1)},${y(v).toFixed(1)}`).join('')
    // faixa: um polígono por trecho contínuo
    const area = () => {
      if (!faixa) return ''
      let d = '', ini = 0
      for (let i = 1; i <= t.length; i++) {
        if (i === t.length || corta(i)) {
          const idx = Array.from({ length: i - ini }, (_, k) => ini + k)
          const cima = idx.map((j) => `${x(t[j]).toFixed(1)},${y(faixa.max[j]).toFixed(1)}`)
          const baixo = idx.reverse().map((j) => `${x(t[j]).toFixed(1)},${y(faixa.min[j]).toFixed(1)}`)
          d += `M${[...cima, ...baixo].join('L')}Z`
          ini = i
        }
      }
      return d
    }
    const marcas = [lo, (lo + hi) / 2, hi]
    const hora = (s: number) => datas
      ? new Date(s * 1000).toLocaleString('pt-BR', { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' })
      : new Date(s * 1000).toLocaleTimeString('pt-BR', { hour: '2-digit', minute: '2-digit' })
    return { caminho, area, y, marcas, t0, t1, hora }
  })
  const f = (v: number) => (Math.abs(v) >= 100 ? v.toFixed(0) : v.toFixed(1))
</script>

<div class="card graf">
  <div class="topo">
    <span class="rot">{titulo}</span>
    <span class="leg">
      {#each series as s}
        <span><i style="background:{s.cor}"></i>{s.nome}{#if s.valores.length}&nbsp;<b>{f(s.valores[s.valores.length - 1])}{unidade}</b>{/if}</span>
      {/each}
    </span>
  </div>
  {#if calc}
    <svg viewBox="0 0 {L} {A}" role="img" aria-label={titulo}>
      {#each calc.marcas as m}
        <line x1={ME} x2={L - MD} y1={calc.y(m)} y2={calc.y(m)} class="grade-h" />
        <text x={ME - 5} y={calc.y(m) + 4} class="eixo" text-anchor="end">{f(m)}</text>
      {/each}
      <text x={ME} y={A - 3} class="eixo">{calc.hora(calc.t0)}</text>
      <text x={L - MD} y={A - 3} class="eixo" text-anchor="end">{calc.hora(calc.t1)}</text>
      {#if faixa}<path d={calc.area()} fill={faixa.cor} opacity="0.18" />{/if}
      {#each series as s}<path d={calc.caminho(s.valores)} fill="none" stroke={s.cor} stroke-width="1.6" stroke-linejoin="round" />{/each}
    </svg>
  {:else}
    <div class="vazio">Juntando dados… (um ponto a cada poucos minutos)</div>
  {/if}
</div>

<style>
  .graf { padding: 10px 12px 6px; }
  .topo { display: flex; justify-content: space-between; flex-wrap: wrap; gap: 6px; font-size: 12px; }
  .rot { color: var(--mudo); text-transform: uppercase; letter-spacing: .05em; }
  .leg { display: flex; gap: 12px; color: var(--txt); font-variant-numeric: tabular-nums; }
  .leg b { font-weight: 600; }
  .leg i { display: inline-block; width: 10px; height: 3px; margin-right: 5px; vertical-align: middle; border-radius: 2px; }
  svg { width: 100%; height: auto; display: block; margin-top: 4px; }
  .grade-h { stroke: var(--borda); stroke-width: 1; }
  .eixo { fill: var(--mudo); font-size: 10px; font-family: ui-monospace, monospace; }
  .vazio { color: var(--mudo); font-size: 13px; padding: 28px 0; text-align: center; }
</style>
