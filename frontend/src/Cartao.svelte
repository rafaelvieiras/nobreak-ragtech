<script lang="ts">
  let { rotulo, valor, unidade = '', dica = '', barra = null }: {
    rotulo: string; valor: string; unidade?: string; dica?: string; barra?: number | null
  } = $props()
</script>

<div class="card">
  <div class="rot">{rotulo}</div>
  <div class="val">{valor}<span class="un">{unidade}</span></div>
  {#if barra !== null}<div class="barra"><i style="width:{Math.max(0, Math.min(100, barra))}%"></i></div>{/if}
  {#if dica}<div class="dica">{dica}</div>{/if}
</div>
