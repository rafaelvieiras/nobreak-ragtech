<script lang="ts">
  import type { Eventos } from './api'
  import { t, val, dataHora, textoEvento, trad } from './i18n.svelte'
  let { dados }: { dados: Eventos | null } = $props()

  const MAX = 80
  const hex = (n: number) => '0x' + n.toString(16).padStart(2, '0')
  const bin = (n: number) => n.toString(2).padStart(8, '0')
  const nomeReg = (e: number) => trad(`reg.${hex(e).toUpperCase().replace('0X', '0x')}`, '')
  // bits que ligaram (+) e desligaram (−)
  function bits(de: number, para: number) {
    const lig: number[] = [], des: number[] = []
    for (let i = 0; i < 8; i++) {
      const a = (de >> i) & 1, b = (para >> i) & 1
      if (!a && b) lig.push(i)
      if (a && !b) des.push(i)
    }
    return [...lig.map((i) => `+bit${i}`), ...des.map((i) => `−bit${i}`)].join(' ')
  }
  let ruidosos = $derived(dados ? Object.entries(dados.registradores).filter(([, c]) => c.ruidoso) : [])
  const virouAno = $derived(dados ? new Set(dados.eventos.map((e) => new Date(e.t * 1000).getFullYear())).size > 1 : false)
</script>

{#if dados}
  <div class="card">
    {#if dados.eventos.length === 0}
      <div class="sub">{t('ev.nenhum')}</div>
    {:else}
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <div class="rolagem" role="region" aria-label={t('ev.lista')} tabindex="0">
        <table>
          <caption class="sr">{t('ev.lista')}</caption>
          <thead><tr><th scope="col">{t('ev.quando')}</th><th scope="col">{t('ev.oque')}</th><th scope="col">{t('ev.mudanca')}</th><th scope="col">{t('ev.rede')}</th><th scope="col">{t('ev.bateria')}</th></tr></thead>
          <tbody>
            {#each dados.eventos.slice(0, MAX) as e}
              <tr class:destaque={e.tipo === 'status' || e.tipo === 'conexao'}>
                <td data-r={t('ev.quando')}>{dataHora(e.t, true, virouAno)}</td>
                {#if e.tipo === 'registrador' && e.endereco !== null && e.de !== null && e.para !== null}
                  <td data-r={t('ev.oque')}>{hex(e.endereco)} {nomeReg(e.endereco)}</td>
                  <td data-r={t('ev.mudanca')} class="mud">{#if e.texto}<span class="nome">{textoEvento(e.tipo, e.texto)}</span>{/if}
                    <span class="tec">{hex(e.de)} → {hex(e.para)} <span class="bits">{bin(e.de)} → {bin(e.para)}</span> <b>{bits(e.de, e.para)}</b></span></td>
                {:else if e.tipo === 'inicio'}
                  <td data-r={t('ev.oque')}>{t('ev.inicio')}</td><td data-r={t('ev.mudanca')} class="hex">{e.texto}</td>
                {:else}
                  <td data-r={t('ev.oque')}>{e.tipo === 'status' ? t('ev.estado') : t('ev.conexao')}</td><td data-r={t('ev.mudanca')}>{textoEvento(e.tipo, e.texto)}</td>
                {/if}
                <td data-r={t('ev.rede')}>{e.tipo === 'conexao' ? '—' : val(e.entrada_v, 0, 'V')}</td>
                <td data-r={t('ev.bateria')}>{e.tipo === 'conexao' ? '—' : `${val(e.bateria_pct, 0, '%')} · ${val(e.bateria_v, 1, 'V')}`}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      {#if dados.eventos.length > MAX}<div class="dica sub" style="margin-top:8px">{t('ev.cortado', { n: MAX, total: dados.eventos.length })}</div>{/if}
    {/if}
    {#if ruidosos.length}
      <div class="dica sub" style="margin-top:8px">{t('ev.ruido', { lista: ruidosos.map(([a, c]) => `${a} (${c.mudancas}×)`).join(', ') })}</div>
    {/if}
    <div class="dica sub" style="margin-top:8px">{t('ev.nota')}</div>
  </div>
{/if}

<style>
  .rolagem { overflow: auto; max-height: 420px; }
  .destaque td { color: var(--aviso); }
  .bits { color: var(--mudo); margin: 0 4px; letter-spacing: .08em; }
  b { color: var(--azul); font-weight: 600; }
  .nome { color: var(--txt); margin-right: 6px; }
  .hex { overflow-wrap: anywhere; color: var(--mudo); }
  .tec { color: var(--mudo); }
  td { white-space: nowrap; } td.hex { white-space: normal; }
  @media (max-width: 480px) {
    .rolagem { max-height: 360px; }
    thead { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); }
    tr { display: block; padding: 8px 0; border-bottom: 1px solid var(--borda); }
    td { display: flex; justify-content: space-between; gap: 12px; border: 0; padding: 2px 0; white-space: normal; text-align: right; }
    td::before { content: attr(data-r); color: var(--mudo); text-align: left; flex: none; }
    td.mud { display: block; text-align: left; } td.mud::before { content: none; }
    .tec { display: block; overflow-wrap: anywhere; }
  }
</style>
