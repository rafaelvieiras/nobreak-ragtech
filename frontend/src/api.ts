export interface Medidas {
  carga_bateria_pct: number; tensao_bateria_v: number
  tensao_entrada_v: number; tensao_saida_v: number; corrente_saida_a: number
  carga_saida_pct: number; temperatura_c: number; frequencia_hz: number | null; firmware: number
  contagem_regressiva_s: number
}
export interface Flags { nvolatil: number; eventos: number; falhas: number; usuario: number; gerais2: number }
export interface Alarme { codigo: string; texto: string; gravidade: 'info' | 'aviso' | 'erro'; registrador: number; mascara: number }
export interface Estado {
  conectado: boolean; ultimo_erro: string | null; atualizado_em: number | null
  falhas_seguidas: number; leituras_ok: number
  modelo: { indice: number; nome: string; reconhecido: boolean } | null
  status: string | null; medidas: Medidas | null; flags: Flags | null; alarmes: Alarme[]
  registradores_hex: string | null; calibracao_0xf3: number | null; capacidade_0x136: number | null; osc_0x202: [number, number] | null
}

export async function buscarEstado(): Promise<Estado> {
  const r = await fetch('/api/estado', { cache: 'no-store' })
  if (!r.ok) throw new Error(`HTTP ${r.status}`)
  return r.json()
}

export interface Evento {
  t: number; tipo: 'inicio' | 'registrador' | 'status' | 'conexao'
  endereco: number | null; de: number | null; para: number | null; texto: string | null
  entrada_v: number; bateria_v: number; bateria_pct: number
}
export interface Eventos { eventos: Evento[]; registradores: Record<string, { mudancas: number; ruidoso: boolean }> }
export interface Serie {
  intervalo_s: number; t: number[]
  bateria_pct: number[]; bateria_v: number[]; entrada_v: number[]; saida_v: number[]
  corrente_a: number[]; carga_pct: number[]; temperatura_c: number[]
}

async function get<T>(url: string): Promise<T> {
  const r = await fetch(url, { cache: 'no-store' })
  if (!r.ok) throw new Error(`HTTP ${r.status}`)
  return r.json()
}
export const buscarEventos = () => get<Eventos>('/api/eventos')
export const buscarSerie = () => get<Serie>('/api/historico')

export interface Queda {
  inicio: number; fim: number | null; duracao_s: number | null
  bateria_inicio_pct: number; bateria_min_pct: number; entrada_min_v: number; interrompida: boolean
  bateria_v_inicio: number | null; bateria_v_min: number | null; carga_max_pct: number | null; recarga_s: number | null
}
export interface ResumoQuedas { quedas: number; segundos_em_bateria: number; maior_s: number; bateria_min_pct: number | null; interrompidas: number }
export interface Quedas {
  em_curso: boolean; ultimas_24h: ResumoQuedas; ultimos_7_dias: ResumoQuedas; ultimos_30_dias: ResumoQuedas; lista: Queda[]
}
export const buscarQuedas = () => get<Quedas>('/api/quedas')

export interface AvisoBateria { codigo: string; texto: string; gravidade: 'info' | 'aviso' | 'erro'; params?: Record<string, number> }
export interface AnaliseQueda {
  inicio: number; duracao_s: number; queda_tensao_v: number | null; carga_max_pct: number | null
  autonomia_projetada_min: number | null; recarga_s: number | null
}
export interface Saude {
  tensao_repouso_v: number | null; nominal_v: number | null; temperatura_c: number | null; idade_dias: number | null
  ultima_queda: AnaliseQueda | null; autonomia_anterior_min: number | null; recarregando: boolean; avisos: AvisoBateria[]
}
export const buscarBateria = () => get<Saude>('/api/bateria')

export interface SerieLonga {
  intervalo_s: number; t: number[]
  [coluna: string]: number | number[]
}
export const buscarSerieLonga = () => get<SerieLonga>('/api/historico/longo')
