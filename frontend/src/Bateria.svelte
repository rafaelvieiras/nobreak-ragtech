<script lang="ts">
  import type { Saude } from './api'
  import { t, trad, num, val, dur, minutos, idade, textoAviso } from './i18n.svelte'
  let { dados }: { dados: Saude | null } = $props()
  const grav = (g: string) => t('grav.' + g)
</script>

{#if dados}
  {#if dados.avisos.length}
    {#each dados.avisos as a}
      <div class="aviso-box" class:erro-box={a.gravidade === 'erro'} role={a.gravidade === 'erro' ? 'alert' : 'status'}>
        <span class="sr">{grav(a.gravidade)}: </span>{a.gravidade === 'erro' ? '⚠' : '•'} {textoAviso(a)}
      </div>
    {/each}
  {:else}
    <div class="sub" style="margin-bottom:8px">{t('b.sem_aviso')}</div>
  {/if}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="card rolagem" role="region" aria-label={t('sec.saude')} tabindex="0">
    <table class="humano">
      <tbody>
        <tr><th scope="row">{t('b.repouso')}</th><td>{dados.tensao_repouso_v === null ? t('b.medindo') : t('b.nominal', { v: val(dados.tensao_repouso_v, 2, 'V'), n: val(dados.nominal_v, 0, 'V') })}</td></tr>
        <tr><th scope="row">{t('b.temp')}</th><td>{val(dados.temperatura_c, 0, '°C')}</td></tr>
        <tr><th scope="row">{t('b.idade')}</th><td>{idade(dados.idade_dias)}</td></tr>
        {#if dados.ultima_queda}
          <tr><th scope="row">{t('b.autonomia')}</th><td>{minutos(dados.ultima_queda.autonomia_projetada_min)}{dados.autonomia_anterior_min !== null ? t('b.anterior', { v: minutos(dados.autonomia_anterior_min) }) : ''}</td></tr>
          <tr><th scope="row">{t('b.afundou')}</th><td>{val(dados.ultima_queda.queda_tensao_v, 1, 'V')}</td></tr>
          <tr><th scope="row">{t('b.cargamax')}</th><td>{val(dados.ultima_queda.carga_max_pct, 0, '%')}</td></tr>
          <tr><th scope="row">{t('b.recarga')}</th><td>{dados.recarregando ? t('b.em_andamento') : dur(dados.ultima_queda.recarga_s)}</td></tr>
        {:else}
          <tr><th scope="row">{t('b.ultima')}</th><td>{t('b.nenhuma')}</td></tr>
        {/if}
      </tbody>
    </table>
  </div>
{/if}

<style>
  th { white-space: nowrap; padding-right: 16px; font-weight: 400; }
  @media (max-width: 480px) {
    th, td { display: block; padding: 2px 0; border: 0; white-space: normal; }
    tr { display: block; padding: 6px 0; border-bottom: 1px solid var(--borda); }
  }
</style>
