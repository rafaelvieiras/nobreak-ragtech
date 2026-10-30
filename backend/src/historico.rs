//! Memória de curto prazo do serviço: log de mudanças de registrador (para decodificar
//! bits desconhecidos) e série temporal das medidas (gráficos do painel).
//! Tudo em memória, com tamanho fixo. O log de eventos pode ser espelhado num arquivo
//! `.jsonl` opcional para sobreviver a reinícios; a série nunca vai para o disco.

use crate::leitura::{Brutos, Medidas};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

pub const MAX_EVENTOS: usize = 500;
/// Medidas que variam o tempo todo: não geram evento (a série já as mostra).
const MEDIDAS: [u16; 8] = [0x87, 0x8A, 0x8B, 0x8C, 0x8D, 0x8E, 0x97, 0x9D];
/// Um registrador que muda mais de `LIMITE_RUIDO` vezes numa janela é só contado.
const LIMITE_RUIDO: u32 = 30;
const JANELA_RUIDO_S: u64 = 600;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Evento {
    pub t: u64,
    /// `inicio`, `registrador`, `status` ou `conexao`.
    pub tipo: String,
    pub endereco: Option<u16>,
    pub de: Option<u8>,
    pub para: Option<u8>,
    pub texto: Option<String>,
    /// Contexto no momento da mudança, para correlacionar com a queda de energia.
    pub entrada_v: f32,
    pub bateria_v: f32,
    pub bateria_pct: f32,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Contagem {
    pub mudancas: u32,
    pub ruidoso: bool,
}

#[derive(Default)]
struct Janela {
    inicio: u64,
    n: u32,
}

#[derive(Serialize, Clone, Copy, Debug)]
struct Ponto {
    t: u32,
    bateria_pct: f32,
    bateria_v: f32,
    entrada_v: f32,
    saida_v: f32,
    corrente_a: f32,
    carga_pct: f32,
    temperatura_c: f32,
}

pub struct Historico {
    eventos: VecDeque<Evento>,
    contagem: BTreeMap<u16, Contagem>,
    janelas: BTreeMap<u16, Janela>,
    anterior: Option<Vec<(u16, u8)>>,
    status_anterior: Option<String>,
    conectado_anterior: Option<bool>,
    serie: VecDeque<Ponto>,
    max_pontos: usize,
    intervalo_serie_s: u64,
    ultimo_ponto: u64,
}

/// Todos os bytes lidos, como pares (endereço, valor).
pub fn pares(b: &Brutos) -> Vec<(u16, u8)> {
    let mut v: Vec<(u16, u8)> = b
        .bloco
        .iter()
        .enumerate()
        .map(|(i, x)| (crate::leitura::BASE + i as u16, *x))
        .collect();
    v.extend([
        (0x00F3, b.calibracao),
        (0x0136, b.cap_bateria),
        (0x0202, b.osc53),
        (0x0203, b.osc57),
    ]);
    v
}

impl Historico {
    pub fn novo(max_pontos: usize, intervalo_serie_s: u64) -> Self {
        Historico {
            eventos: VecDeque::new(),
            contagem: BTreeMap::new(),
            janelas: BTreeMap::new(),
            anterior: None,
            status_anterior: None,
            conectado_anterior: None,
            serie: VecDeque::new(),
            max_pontos: max_pontos.max(10),
            intervalo_serie_s: intervalo_serie_s.max(1),
            ultimo_ponto: 0,
        }
    }

    /// Eventos carregados de um arquivo anterior (mais antigos primeiro).
    pub fn restaurar(&mut self, eventos: Vec<Evento>) {
        for e in eventos {
            self.empurrar(e);
        }
    }

    fn empurrar(&mut self, e: Evento) {
        if self.eventos.len() >= MAX_EVENTOS {
            self.eventos.pop_front();
        }
        self.eventos.push_back(e);
    }

    /// Registra a leitura. Devolve os eventos novos (para espelhar no arquivo).
    pub fn registrar(&mut self, t: u64, b: &Brutos, m: &Medidas, status: &str) -> Vec<Evento> {
        let ctx = |tipo: &str, endereco, de, para, texto: Option<String>| Evento {
            t,
            tipo: tipo.into(),
            endereco,
            de,
            para,
            texto,
            entrada_v: m.tensao_entrada_v,
            bateria_v: m.tensao_bateria_v,
            bateria_pct: m.carga_bateria_pct,
        };
        let mut novos = Vec::new();
        let atual = pares(b);

        match self.anterior.take() {
            None => {
                let hex: String = b.bloco.iter().map(|x| format!("{x:02x}")).collect();
                novos.push(ctx("inicio", None, None, None, Some(hex)));
            }
            Some(ant) => {
                for ((a, antes), (_, agora)) in ant.iter().zip(atual.iter()) {
                    if antes == agora || MEDIDAS.contains(a) {
                        continue;
                    }
                    let c = self.contagem.entry(*a).or_default();
                    c.mudancas += 1;
                    let j = self.janelas.entry(*a).or_default();
                    if t.saturating_sub(j.inicio) > JANELA_RUIDO_S {
                        j.inicio = t;
                        j.n = 0;
                        c.ruidoso = false;
                    }
                    j.n += 1;
                    if j.n > LIMITE_RUIDO {
                        c.ruidoso = true;
                    }
                    if !c.ruidoso {
                        novos.push(ctx(
                            "registrador",
                            Some(*a),
                            Some(*antes),
                            Some(*agora),
                            None,
                        ));
                    }
                }
            }
        }
        self.anterior = Some(atual);

        if let Some(ant) = self.status_anterior.as_deref() {
            if ant != status {
                novos.push(ctx(
                    "status",
                    None,
                    None,
                    None,
                    Some(format!("{ant} → {status}")),
                ));
            }
        }
        self.status_anterior = Some(status.to_string());

        for e in &novos {
            self.empurrar(e.clone());
        }
        if !novos.is_empty() || t.saturating_sub(self.ultimo_ponto) >= self.intervalo_serie_s {
            self.amostrar(t, m);
        }
        novos
    }

    /// Registra a queda ou o retorno da comunicação com o nobreak.
    pub fn conexao(&mut self, t: u64, conectado: bool, motivo: &str) -> Option<Evento> {
        if self.conectado_anterior == Some(conectado) {
            return None;
        }
        let primeira = self.conectado_anterior.is_none();
        self.conectado_anterior = Some(conectado);
        if primeira && conectado {
            return None; // o evento `inicio` já cobre
        }
        let texto = if conectado {
            "comunicação restabelecida".to_string()
        } else {
            format!("sem comunicação: {motivo}")
        };
        let e = Evento {
            t,
            tipo: "conexao".into(),
            endereco: None,
            de: None,
            para: None,
            texto: Some(texto),
            entrada_v: 0.0,
            bateria_v: 0.0,
            bateria_pct: 0.0,
        };
        self.empurrar(e.clone());
        Some(e)
    }

    fn amostrar(&mut self, t: u64, m: &Medidas) {
        if self.serie.len() >= self.max_pontos {
            self.serie.pop_front();
        }
        self.serie.push_back(Ponto {
            t: t as u32,
            bateria_pct: m.carga_bateria_pct,
            bateria_v: m.tensao_bateria_v,
            entrada_v: m.tensao_entrada_v,
            saida_v: m.tensao_saida_v,
            corrente_a: m.corrente_saida_a,
            carga_pct: m.carga_saida_pct,
            temperatura_c: m.temperatura_c,
        });
        self.ultimo_ponto = t;
    }

    pub fn eventos_json(&self) -> String {
        #[derive(Serialize)]
        struct R<'a> {
            eventos: Vec<&'a Evento>,
            registradores: BTreeMap<String, &'a Contagem>,
        }
        let r = R {
            // mais novos primeiro
            eventos: self.eventos.iter().rev().collect(),
            registradores: self
                .contagem
                .iter()
                .map(|(a, c)| (format!("0x{a:02x}"), c))
                .collect(),
        };
        serde_json::to_string(&r).unwrap_or_else(|_| "{}".into())
    }

    /// Série em colunas (menor que objetos por ponto).
    pub fn serie_json(&self) -> String {
        let col = |f: fn(&Ponto) -> f32| -> Vec<f32> { self.serie.iter().map(f).collect() };
        serde_json::json!({
            "intervalo_s": self.intervalo_serie_s,
            "t": self.serie.iter().map(|p| p.t).collect::<Vec<_>>(),
            "bateria_pct": col(|p| p.bateria_pct),
            "bateria_v": col(|p| p.bateria_v),
            "entrada_v": col(|p| p.entrada_v),
            "saida_v": col(|p| p.saida_v),
            "corrente_a": col(|p| p.corrente_a),
            "carga_pct": col(|p| p.carga_pct),
            "temperatura_c": col(|p| p.temperatura_c),
        })
        .to_string()
    }
}

/// Lê um arquivo `.jsonl` de eventos, ignorando linhas inválidas. Fica com os mais recentes.
pub fn ler_arquivo(caminho: &str) -> Vec<Evento> {
    let txt = std::fs::read_to_string(caminho).unwrap_or_default();
    let mut v: Vec<Evento> = txt
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    if v.len() > MAX_EVENTOS {
        v.drain(..v.len() - MAX_EVENTOS);
    }
    v
}

/// Acrescenta eventos ao arquivo. Quando passa de ~128 KB, reescreve só com os recentes.
pub fn anexar_arquivo(caminho: &str, novos: &[Evento]) {
    use std::io::Write;
    if novos.is_empty() {
        return;
    }
    let grande = std::fs::metadata(caminho)
        .map(|m| m.len() > 128 * 1024)
        .unwrap_or(false);
    if grande {
        let mut v = ler_arquivo(caminho);
        v.extend_from_slice(novos);
        let txt: String = v
            .iter()
            .filter_map(|e| serde_json::to_string(e).ok())
            .map(|l| l + "\n")
            .collect();
        let _ = std::fs::write(caminho, txt);
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(caminho)
    {
        for e in novos {
            if let Ok(l) = serde_json::to_string(e) {
                let _ = writeln!(f, "{l}");
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::leitura::{decodificar, TAM_BLOCO};

    fn brutos(muda: &[(usize, u8)]) -> Brutos {
        let mut bloco = [0u8; TAM_BLOCO];
        bloco[(0x9A - 0x80) as usize] = 6;
        bloco[(0x8B - 0x80) as usize] = 118;
        for (i, v) in muda {
            bloco[*i] = *v;
        }
        Brutos {
            bloco,
            calibracao: 0x34,
            cap_bateria: 0xB4,
            osc53: 0x68,
            osc57: 0x46,
        }
    }

    fn rodar(h: &mut Historico, t: u64, b: &Brutos, st: &str) -> Vec<Evento> {
        let (_, m, _) = decodificar(b, 1.0);
        h.registrar(t, b, &m, st)
    }

    #[test]
    fn primeira_leitura_vira_inicio() {
        let mut h = Historico::novo(100, 60);
        let n = rodar(&mut h, 1000, &brutos(&[]), "OL");
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].tipo, "inicio");
    }

    #[test]
    fn mudanca_de_flag_gera_evento_e_medida_nao() {
        let mut h = Historico::novo(100, 60);
        rodar(&mut h, 1000, &brutos(&[]), "OL");
        // 0x91 (FALHAS) muda; 0x8B (tensão de entrada) também, mas é medida
        let n = rodar(&mut h, 1005, &brutos(&[(0x11, 0x04), (0x0B, 0)]), "OL");
        assert_eq!(n.len(), 1, "{n:?}");
        assert_eq!(
            (n[0].endereco, n[0].de, n[0].para),
            (Some(0x91), Some(0), Some(4))
        );
        assert!(h.eventos_json().contains("\"0x91\""));
    }

    #[test]
    fn mudanca_de_status() {
        let mut h = Historico::novo(100, 60);
        rodar(&mut h, 1000, &brutos(&[]), "OL");
        let n = rodar(&mut h, 1005, &brutos(&[]), "OB");
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].tipo, "status");
        assert_eq!(n[0].texto.as_deref(), Some("OL → OB"));
    }

    #[test]
    fn registrador_ruidoso_e_so_contado() {
        let mut h = Historico::novo(100, 60);
        rodar(&mut h, 1000, &brutos(&[]), "OL");
        let mut gerados = 0;
        for i in 0..100u64 {
            gerados += rodar(
                &mut h,
                1001 + i,
                &brutos(&[(0x04, (i % 2) as u8 + 1)]),
                "OL",
            )
            .len();
        }
        assert!(gerados <= LIMITE_RUIDO as usize + 1, "{gerados}");
        assert!(h.eventos_json().contains("\"ruidoso\":true"));
    }

    #[test]
    fn serie_respeita_o_limite_e_o_intervalo() {
        let mut h = Historico::novo(10, 60);
        for i in 0..50u64 {
            rodar(&mut h, 1000 + i * 60, &brutos(&[]), "OL");
        }
        assert_eq!(h.serie.len(), 10);
        let mut h = Historico::novo(100, 60);
        for i in 0..30u64 {
            rodar(&mut h, 1000 + i * 5, &brutos(&[]), "OL");
        }
        assert_eq!(h.serie.len(), 3); // 150 s com amostra a cada 60 s: 1000, 1060, 1120
    }

    #[test]
    fn conexao_perdida_e_restabelecida() {
        let mut h = Historico::novo(100, 60);
        assert!(h.conexao(1, true, "").is_none());
        assert!(h.conexao(2, true, "").is_none());
        assert!(h.conexao(3, false, "timeout").is_some());
        assert!(h.conexao(4, false, "timeout").is_none());
        assert!(h.conexao(5, true, "").is_some());
    }

    #[test]
    fn arquivo_de_eventos_ida_e_volta() {
        let dir = std::env::temp_dir().join(format!("nb-teste-{}", std::process::id()));
        let _ = std::fs::remove_file(&dir);
        let p = dir.to_str().unwrap();
        let mut h = Historico::novo(100, 60);
        let a = rodar(&mut h, 1000, &brutos(&[]), "OL");
        let b = rodar(&mut h, 1005, &brutos(&[(0x10, 0x20)]), "OL");
        anexar_arquivo(p, &a);
        anexar_arquivo(p, &b);
        let lidos = ler_arquivo(p);
        assert_eq!(lidos.len(), 2);
        assert_eq!(lidos[1].endereco, Some(0x90));
        let _ = std::fs::remove_file(p);
    }
}
