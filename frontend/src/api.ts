export interface Medidas {
  carga_bateria_pct: number; tensao_bateria_v: number
  tensao_entrada_v: number; tensao_saida_v: number; corrente_saida_a: number
  carga_saida_pct: number; temperatura_c: number; frequencia_hz: number | null; firmware: number
}
export interface Flags { nvolatil: number; eventos: number; falhas: number; usuario: number; gerais2: number }
export interface Estado {
  conectado: boolean; ultimo_erro: string | null; atualizado_em: number | null
  falhas_seguidas: number; leituras_ok: number
  modelo: { indice: number; nome: string; reconhecido: boolean } | null
  status: string | null; medidas: Medidas | null; flags: Flags | null
  registradores_hex: string | null; calibracao_0xf3: number | null; capacidade_0x136: number | null; osc_0x202: [number, number] | null
}

export async function buscarEstado(): Promise<Estado> {
  const r = await fetch('/api/estado', { cache: 'no-store' })
  if (!r.ok) throw new Error(`HTTP ${r.status}`)
  return r.json()
}
