<script lang="ts">
  import { onMount } from 'svelte'
  import { buscarEstado, type Estado } from './api'
  import Cartao from './Cartao.svelte'

  let e = $state<Estado | null>(null)
  let falhaApi = $state<string | null>(null)
  let agora = $state(Date.now())

  async function atualizar() {
    try { e = await buscarEstado(); falhaApi = null } catch (x) { falhaApi = String(x) }
    agora = Date.now()
  }
  onMount(() => { atualizar(); const t = setInterval(atualizar, 3000); return () => clearInterval(t) })

  const f = (n: number | null | undefined, d = 1) => (n === null || n === undefined ? '—' : n.toFixed(d))
  const bin = (n: number) => n.toString(2).padStart(8, '0')
  const hex = (n: number) => '0x' + n.toString(16).padStart(2, '0')

  let idade = $derived(e?.atualizado_em ? Math.max(0, Math.round(agora / 1000 - e.atualizado_em)) : null)
  let situacao = $derived.by(() => {
    if (falhaApi) return { cls: 'erro', txt: 'Sem resposta do serviço' }
    if (!e || !e.conectado) return { cls: 'erro', txt: 'Nobreak desconectado' }
    if (e.status?.startsWith('OB')) return { cls: 'aviso', txt: 'Na bateria (falta de energia)' }
    if (e.status?.includes('CHRG')) return { cls: 'ok', txt: 'Rede normal, carregando bateria' }
    return { cls: 'ok', txt: 'Rede normal' }
  })
  const rotulosFlags: [keyof NonNullable<Estado['flags']>, string, number][] = [
    ['nvolatil', 'NVOLATIL', 0x80], ['eventos', 'EVENTOS', 0x90], ['falhas', 'FALHAS', 0x91],
    ['usuario', 'USUARIO', 0x92], ['gerais2', 'GERAIS2', 0x95],
  ]
</script>

<main>
  <header>
    <div>
      <h1>Nobreak Ragtech</h1>
      <div class="sub">{e?.modelo?.nome ?? '—'}{#if e?.medidas} · firmware {f(e.medidas.firmware)}{/if}</div>
    </div>
    <span class="selo {situacao.cls}">{situacao.txt}</span>
  </header>

  {#if falhaApi}
    <div class="aviso-box erro-box">Não consegui falar com o serviço: {falhaApi}</div>
  {:else if e && !e.conectado}
    <div class="aviso-box erro-box">Sem leitura do nobreak. Último erro: {e.ultimo_erro ?? 'aguardando primeira leitura'}</div>
  {/if}
  {#if e?.modelo && !e.modelo.reconhecido}
    <div class="aviso-box">Modelo {e.modelo.indice} ainda não está na tabela. Os valores podem estar em escala errada. Abra uma issue com a linha “Registradores” abaixo.</div>
  {/if}

  {#if e?.medidas}
    {@const m = e.medidas}
    <h2>Bateria</h2>
    <div class="grade">
      <Cartao rotulo="Carga da bateria" valor={f(m.carga_bateria_pct, 0)} unidade="%" barra={m.carga_bateria_pct} />
      <Cartao rotulo="Tensão da bateria" valor={f(m.tensao_bateria_v, 1)} unidade="V"
        dica="Se o valor parecer metade do real, ajuste NOBREAK_FATOR_VBAT=2 (veja o README)." />
      <Cartao rotulo="Temperatura" valor={f(m.temperatura_c, 0)} unidade="°C" />
    </div>

    <h2>Rede e saída</h2>
    <div class="grade">
      <Cartao rotulo="Tensão de entrada" valor={f(m.tensao_entrada_v, 0)} unidade="V" />
      <Cartao rotulo="Tensão de saída" valor={f(m.tensao_saida_v, 0)} unidade="V" />
      <Cartao rotulo="Frequência" valor={f(m.frequencia_hz, 1)} unidade="Hz" dica={m.frequencia_hz === null ? 'sem calibração' : 'estimada'} />
      <Cartao rotulo="Corrente de saída" valor={f(m.corrente_saida_a, 2)} unidade="A" />
      <Cartao rotulo="Carga na saída" valor={f(m.carga_saida_pct, 0)} unidade="%" barra={m.carga_saida_pct} />
      <Cartao rotulo="Potência aparente" valor={f(m.tensao_saida_v * m.corrente_saida_a, 0)} unidade="VA" />
    </div>
  {/if}

  {#if e?.flags}
    <h2>Flags (bits ainda em decodificação)</h2>
    <div class="card">
      <table>
        <thead><tr><th>Registrador</th><th>End.</th><th>Hex</th><th>Bits (7→0)</th></tr></thead>
        <tbody>
          {#each rotulosFlags as [k, nome, end]}
            {@const v = e.flags[k]}
            <tr><td>{nome}</td><td>{hex(end)}</td><td>{hex(v)}</td>
              <td class="bits">{#each bin(v).split('') as b}<span class:lig={b === '1'}>{b}</span>{/each}</td></tr>
          {/each}
        </tbody>
      </table>
      <div class="dica sub" style="margin-top:8px">Compare estes valores antes e depois de uma queda de energia e conte no repositório o que mudou.</div>
    </div>
  {/if}

  {#if e?.registradores_hex}
    <h2>Registradores</h2>
    <div class="card">
      <div class="hex">0x80…0x9D: {e.registradores_hex}</div>
      <div class="hex" style="margin-top:6px">0xF3: {e.calibracao_0xf3?.toString(16).padStart(2, '0')} · 0x136: {e.capacidade_0x136?.toString(16).padStart(2, '0')} · 0x202–0x203: {e.osc_0x202?.map((x) => x.toString(16).padStart(2, '0')).join(' ')}</div>
    </div>
  {/if}

  <footer>
    {#if idade !== null}Atualizado há {idade} s · {/if}{e?.leituras_ok ?? 0} leituras ·
    <a href="/api/estado">/api/estado</a> · <a href="/metrics">/metrics</a>
  </footer>
</main>
