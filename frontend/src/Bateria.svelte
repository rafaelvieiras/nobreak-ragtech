<script lang="ts">
  import type { Saude } from './api'
  let { dados }: { dados: Saude | null } = $props()

  const min = (m: number | null) => (m === null ? '—' : m < 60 ? `${m.toFixed(0)} min` : `${Math.floor(m / 60)} h ${Math.round(m % 60)} min`)
  const dur = (s: number | null) => (s === null ? '—' : s < 3600 ? `${Math.round(s / 60)} min` : `${(s / 3600).toFixed(1)} h`)
  const idade = (d: number | null) => (d === null ? 'não informada' : d < 365 ? `${d} dias` : `${(d / 365).toFixed(1)} anos`)
</script>

{#if dados}
  {#if dados.avisos.length}
    {#each dados.avisos as a}
      <div class="aviso-box" class:erro-box={a.gravidade === 'erro'}>{a.gravidade === 'erro' ? '⚠' : '•'} {a.texto}</div>
    {/each}
  {:else}
    <div class="sub" style="margin-bottom:8px">Nenhum aviso: a bateria não mostra sinal de desgaste.</div>
  {/if}
  <div class="card">
    <table>
      <tbody>
        <tr><td>Tensão em repouso</td><td>{dados.tensao_repouso_v === null ? 'medindo (cerca de 5 min com a bateria cheia)' : `${dados.tensao_repouso_v.toFixed(2)} V (bateria de ${dados.nominal_v} V)`}</td></tr>
        <tr><td>Temperatura do inversor</td><td>{dados.temperatura_c === null ? '—' : `${dados.temperatura_c.toFixed(0)} °C`}</td></tr>
        <tr><td>Idade da bateria</td><td>{idade(dados.idade_dias)}</td></tr>
        {#if dados.ultima_queda}
          <tr><td>Autonomia projetada (última queda)</td><td>{min(dados.ultima_queda.autonomia_projetada_min)}{dados.autonomia_anterior_min !== null ? `, média anterior ${min(dados.autonomia_anterior_min)}` : ''}</td></tr>
          <tr><td>Tensão caiu na queda</td><td>{dados.ultima_queda.queda_tensao_v === null ? '—' : `${dados.ultima_queda.queda_tensao_v.toFixed(1)} V`}</td></tr>
          <tr><td>Carga máxima na queda</td><td>{dados.ultima_queda.carga_max_pct === null ? '—' : `${dados.ultima_queda.carga_max_pct.toFixed(0)}%`}</td></tr>
          <tr><td>Recarga até 99%</td><td>{dados.recarregando ? 'em andamento' : dur(dados.ultima_queda.recarga_s)}</td></tr>
        {:else}
          <tr><td>Última queda analisável</td><td>nenhuma ainda (precisa de 30 s ou mais com perda de carga)</td></tr>
        {/if}
      </tbody>
    </table>
  </div>
{/if}

<style>
  td:first-child { color: var(--mudo); white-space: nowrap; padding-right: 16px; }
</style>
