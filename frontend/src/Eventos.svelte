<script lang="ts">
  import type { Eventos } from './api'
  let { dados }: { dados: Eventos | null } = $props()

  const nomes: Record<number, string> = { 0x80: 'NVOLATIL', 0x90: 'EVENTOS', 0x91: 'FALHAS', 0x92: 'USUARIO', 0x95: 'GERAIS2' }
  const hex = (n: number) => '0x' + n.toString(16).padStart(2, '0')
  const bin = (n: number) => n.toString(2).padStart(8, '0')
  const quando = (s: number) => new Date(s * 1000).toLocaleString('pt-BR', { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit' })
  // bits que ligaram (+) e desligaram (-)
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
</script>

{#if dados}
  <div class="card">
    {#if dados.eventos.length === 0}
      <div class="sub">Nenhum evento ainda.</div>
    {:else}
      <div class="rolagem">
        <table>
          <thead><tr><th>Quando</th><th>O que</th><th>Mudança</th><th>Rede</th><th>Bateria</th></tr></thead>
          <tbody>
            {#each dados.eventos.slice(0, 80) as e}
              <tr class:destaque={e.tipo === 'status' || e.tipo === 'conexao'}>
                <td>{quando(e.t)}</td>
                {#if e.tipo === 'registrador' && e.endereco !== null && e.de !== null && e.para !== null}
                  <td>{hex(e.endereco)} {nomes[e.endereco] ?? ''}</td>
                  <td>{hex(e.de)} → {hex(e.para)} <span class="bits">{bin(e.de)} → {bin(e.para)}</span> <b>{bits(e.de, e.para)}</b>{#if e.texto} <span class="nome">{e.texto}</span>{/if}</td>
                {:else if e.tipo === 'inicio'}
                  <td>início</td><td class="hex">{e.texto}</td>
                {:else}
                  <td>{e.tipo === 'status' ? 'estado' : 'conexão'}</td><td>{e.texto}</td>
                {/if}
                <td>{e.tipo === 'conexao' ? '—' : e.entrada_v.toFixed(0) + ' V'}</td>
                <td>{e.tipo === 'conexao' ? '—' : e.bateria_pct.toFixed(0) + '% · ' + e.bateria_v.toFixed(1) + ' V'}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
    {#if ruidosos.length}
      <div class="dica sub" style="margin-top:8px">Registradores que mudam o tempo todo (só contados, sem evento): {ruidosos.map(([a, c]) => `${a} (${c.mudancas}×)`).join(', ')}.</div>
    {/if}
    <div class="dica sub" style="margin-top:8px">Medidas que variam sempre (tensões, corrente, temperatura) não entram aqui; veja os gráficos. Os eventos ficam guardados enquanto o serviço roda{'.'}</div>
  </div>
{/if}

<style>
  .rolagem { overflow-x: auto; max-height: 420px; overflow-y: auto; }
  .destaque td { color: var(--aviso); }
  .bits { color: var(--mudo); margin: 0 4px; letter-spacing: .08em; }
  b { color: var(--azul); font-weight: 600; }
  .nome { color: var(--txt); margin-left: 6px; }
  .hex { word-break: break-all; color: var(--mudo); }
  td { white-space: nowrap; } td.hex { white-space: normal; }
</style>
