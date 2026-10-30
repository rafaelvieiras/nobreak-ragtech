<script lang="ts">
  import { t } from './i18n.svelte'
  // `limiar`: [aviso, erro] em %. Com `inverso`, valores BAIXOS são ruins (carga da bateria).
  let { rotulo, valor, unidade = '', dica = '', ajuda = '', barra = null, limiar = null, inverso = false }: {
    rotulo: string; valor: string; unidade?: string; dica?: string; ajuda?: string
    barra?: number | null; limiar?: [number, number] | null; inverso?: boolean
  } = $props()

  const pct = $derived(barra === null ? 0 : Math.max(0, Math.min(100, barra)))
  const nivel = $derived.by(() => {
    if (barra === null || !limiar) return 'ok'
    const [a, e] = limiar
    if (inverso) return barra <= e ? 'erro' : barra <= a ? 'aviso' : 'ok'
    return barra >= e ? 'erro' : barra >= a ? 'aviso' : 'ok'
  })
</script>

<div class="card">
  <div class="rot">{rotulo}</div>
  <div class="val">{valor}{#if unidade}<span class="un">{unidade}</span>{/if}</div>
  {#if barra !== null}
    <div class="barra" role="progressbar" aria-label={rotulo} aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(pct)}><i class={nivel} style="width:{pct}%"></i></div>
  {/if}
  {#if dica}<div class="dica">{dica}</div>{/if}
  {#if ajuda}<details class="ajuda"><summary aria-label={t('ajuda')}>ⓘ</summary><div class="dica">{ajuda}</div></details>{/if}
</div>
