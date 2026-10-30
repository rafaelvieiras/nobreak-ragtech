<script lang="ts">
  import type { Quedas } from './api'
  let { dados }: { dados: Quedas | null } = $props()

  const dur = (s: number | null) => {
    if (s === null) return '—'
    if (s < 60) return `${s} s`
    const m = Math.floor(s / 60)
    return m < 60 ? `${m} min ${s % 60} s` : `${Math.floor(m / 60)} h ${m % 60} min`
  }
  const quando = (s: number) => new Date(s * 1000).toLocaleString('pt-BR', { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' })
  const periodos = $derived(dados ? ([['24 h', dados.ultimas_24h], ['7 dias', dados.ultimos_7_dias], ['30 dias', dados.ultimos_30_dias]] as const) : [])
</script>

{#if dados}
  <div class="card">
    <table>
      <thead><tr><th>Período</th><th>Quedas</th><th>Tempo na bateria</th><th>Maior queda</th><th>Bateria mínima</th></tr></thead>
      <tbody>
        {#each periodos as [nome, r]}
          <tr><td>{nome}</td><td>{r.quedas}</td><td>{dur(r.segundos_em_bateria)}</td><td>{dur(r.maior_s)}</td>
            <td>{r.bateria_min_pct === null ? '—' : r.bateria_min_pct.toFixed(0) + '%'}</td></tr>
        {/each}
      </tbody>
    </table>
    {#if dados.lista.length}
      <div class="rolagem" style="margin-top:12px">
        <table>
          <thead><tr><th>Início</th><th>Duração</th><th>Bateria</th><th>Rede mínima</th></tr></thead>
          <tbody>
            {#each dados.lista as q}
              <tr class:destaque={q.fim === null && !q.interrompida}>
                <td>{quando(q.inicio)}</td>
                <td>{#if q.interrompida}interrompida (serviço parou){:else if q.fim === null}em curso, {dur(q.duracao_s)}{:else}{dur(q.duracao_s)}{/if}</td>
                <td>{q.bateria_inicio_pct.toFixed(0)}% → {q.bateria_min_pct.toFixed(0)}%</td>
                <td>{q.entrada_min_v.toFixed(0)} V</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else}
      <div class="sub" style="margin-top:8px">Nenhuma queda registrada.</div>
    {/if}
  </div>
{/if}

<style>
  .rolagem { overflow-x: auto; max-height: 260px; overflow-y: auto; }
  .destaque td { color: var(--aviso); }
  td { white-space: nowrap; }
</style>
