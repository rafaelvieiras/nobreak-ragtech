<script lang="ts">
  import type { Quedas } from './api'
  import { t, num, val, dur, dataHora, un } from './i18n.svelte'
  let { dados }: { dados: Quedas | null } = $props()

  const periodos = $derived(dados ? ([[un(24, 'hour'), dados.ultimas_24h], [un(7, 'day'), dados.ultimos_7_dias], [un(30, 'day'), dados.ultimos_30_dias]] as const) : [])
  const virouAno = $derived(dados ? new Set(dados.lista.map((q) => new Date(q.inicio * 1000).getFullYear())).size > 1 : false)
  const duracao = (q: NonNullable<Quedas>['lista'][number]) =>
    q.interrompida ? t('q.interrompida') : q.fim === null ? t('q.em_curso', { dur: dur(q.duracao_s) }) : dur(q.duracao_s)
</script>

{#if dados}
  <div class="card">
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div class="rolagem" role="region" aria-label={t('q.resumo')} tabindex="0">
      <table class="humano">
        <caption class="sr">{t('q.resumo')}</caption>
        <thead><tr><th scope="col">{t('q.periodo')}</th><th scope="col" class="num">{t('q.quedas')}</th><th scope="col" class="num">{t('q.tempo')}</th><th scope="col" class="num">{t('q.maior')}</th><th scope="col" class="num">{t('q.bat_min')}</th></tr></thead>
        <tbody>
          {#each periodos as [nome, r]}
            <tr><th scope="row">{nome}</th><td class="num">{r.quedas}</td><td class="num">{dur(r.segundos_em_bateria)}</td><td class="num">{dur(r.maior_s)}</td>
              <td class="num">{val(r.bateria_min_pct, 0, '%')}</td></tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if dados.lista.length}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div class="lista" role="region" aria-label={t('q.lista')} tabindex="0" style="margin-top:12px">
        <table class="humano">
          <caption class="sr">{t('q.lista')}</caption>
          <thead><tr><th scope="col">{t('q.inicio')}</th><th scope="col">{t('q.duracao')}</th><th scope="col" class="num">{t('q.bateria')}</th><th scope="col" class="num">{t('q.rede_min')}</th></tr></thead>
          <tbody>
            {#each dados.lista as q}
              <tr class:destaque={q.fim === null && !q.interrompida}>
                <td data-r={t('q.inicio')}>{dataHora(q.inicio, false, virouAno)}</td>
                <td data-r={t('q.duracao')}>{duracao(q)}</td>
                <td data-r={t('q.bateria')} class="num">{num(q.bateria_inicio_pct, 0)} → {val(q.bateria_min_pct, 0, '%')}</td>
                <td data-r={t('q.rede_min')} class="num">{val(q.entrada_min_v, 0, 'V')}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else}
      <div class="sub" style="margin-top:8px">{t('q.nenhuma')}</div>
    {/if}
  </div>
{/if}

<style>
  .rolagem, .lista { overflow: auto; }
  .lista { max-height: 260px; }
  .destaque td { color: var(--aviso); }
  td, th { white-space: nowrap; }
  @media (max-width: 480px) {
    /* lista de quedas empilhada: um bloco por queda */
    .lista thead { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); }
    .lista tr { display: block; padding: 8px 0; border-bottom: 1px solid var(--borda); }
    .lista td { display: flex; justify-content: space-between; gap: 12px; border: 0; padding: 2px 0; white-space: normal; text-align: right; }
    .lista td::before { content: attr(data-r); color: var(--mudo); text-align: left; }
    th, td { padding: 6px 6px; }
  }
</style>
