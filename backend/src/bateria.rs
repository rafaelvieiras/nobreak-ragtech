//! Saúde da bateria: avisos calculados a partir do que o serviço já mede.
//! Nada aqui lê registrador novo. Usa a tensão em repouso, o comportamento nas quedas
//! (afundamento de tensão, autonomia projetada, tempo de recarga), a temperatura e,
//! se o dono informar, a idade da bateria.

use crate::alarmes::Gravidade;
use crate::quedas::{Queda, Quedas};
use serde::Serialize;

/// Amostras em repouso necessárias antes de confiar na média (5 s cada: cerca de 5 min).
const AMOSTRAS_MIN: u32 = 60;
const ALFA: f32 = 0.01;
const ANO_S: u64 = 365 * 86_400;

/// Tensão média da bateria com a rede ligada e a carga completa (flutuação).
#[derive(Clone, Copy, Debug, Default)]
pub struct Repouso {
    media: f32,
    n: u32,
}

impl Repouso {
    pub fn amostrar(&mut self, v: f32) {
        if !v.is_finite() || v <= 0.0 {
            return;
        }
        if self.n == 0 {
            self.media = v;
        } else {
            self.media += ALFA * (v - self.media);
        }
        self.n = self.n.saturating_add(1);
    }

    pub fn tensao(&self) -> Option<f32> {
        (self.n >= AMOSTRAS_MIN).then_some(self.media)
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Aviso {
    pub codigo: &'static str,
    pub texto: String,
    pub gravidade: Gravidade,
}

fn aviso(codigo: &'static str, gravidade: Gravidade, texto: String) -> Aviso {
    Aviso {
        codigo,
        texto,
        gravidade,
    }
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct AnaliseQueda {
    pub inicio: u64,
    pub duracao_s: u64,
    /// Tensão no começo menos a menor tensão da queda.
    pub queda_tensao_v: Option<f32>,
    pub carga_max_pct: Option<f32>,
    /// Quanto duraria de 100% a 0%, mantido o ritmo medido nesta queda.
    pub autonomia_projetada_min: Option<f32>,
    pub recarga_s: Option<u64>,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Saude {
    pub tensao_repouso_v: Option<f32>,
    /// 12 ou 24, deduzido da tensão em repouso.
    pub nominal_v: Option<u8>,
    pub temperatura_c: Option<f32>,
    pub idade_dias: Option<u32>,
    /// Última queda que dá para analisar (30 s ou mais, com perda de carga visível).
    pub ultima_queda: Option<AnaliseQueda>,
    /// Média da autonomia projetada nas quedas anteriores à última.
    pub autonomia_anterior_min: Option<f32>,
    pub recarregando: bool,
    pub avisos: Vec<Aviso>,
}

/// Autonomia projetada (min) de uma queda; precisa de 30 s e de 5 pontos de carga perdidos.
pub fn autonomia_min(q: &Queda) -> Option<f32> {
    let dur = q.duracao_s? as f32;
    let perda = q.bateria_inicio_pct - q.bateria_min_pct;
    (!q.interrompida && dur >= 30.0 && perda >= 5.0).then(|| dur / 60.0 * 100.0 / perda)
}

fn analise(q: &Queda) -> AnaliseQueda {
    AnaliseQueda {
        inicio: q.inicio,
        duracao_s: q.duracao_s.unwrap_or(0),
        queda_tensao_v: match (q.bateria_v_inicio, q.bateria_v_min) {
            (Some(a), Some(b)) => Some((a - b).max(0.0)),
            _ => None,
        },
        carga_max_pct: q.carga_max_pct,
        autonomia_projetada_min: autonomia_min(q),
        recarga_s: q.recarga_s,
    }
}

/// Avalia a bateria. `instalada` é a data de instalação (segundos Unix), se informada.
pub fn avaliar(
    quedas: &Quedas,
    temperatura_c: Option<f32>,
    instalada: Option<u64>,
    agora: u64,
) -> Saude {
    let mut s = Saude {
        temperatura_c,
        recarregando: quedas.recarregando(),
        ..Saude::default()
    };
    let mut av = Vec::new();

    if let Some(v) = quedas.repouso.tensao() {
        let nominal: u8 = if v > 20.0 { 24 } else { 12 };
        let k = nominal as f32 / 12.0;
        s.tensao_repouso_v = Some(v);
        s.nominal_v = Some(nominal);
        if v < 12.4 * k {
            av.push(aviso(
                "repouso_muito_baixo",
                Gravidade::Erro,
                format!("Tensão em repouso muito baixa ({v:.2} V): bateria provavelmente no fim da vida"),
            ));
        } else if v < 13.0 * k {
            av.push(aviso(
                "repouso_baixo",
                Gravidade::Aviso,
                format!("Tensão em repouso baixa ({v:.2} V) com a rede ligada e a carga completa"),
            ));
        } else if v > 14.6 * k {
            av.push(aviso(
                "repouso_alto",
                Gravidade::Aviso,
                format!("Tensão em repouso alta ({v:.2} V): carregador pode estar com defeito"),
            ));
        }
    }

    // análise das quedas já medidas, da mais nova para a mais antiga
    let analisaveis: Vec<&Queda> = quedas
        .lista()
        .iter()
        .rev()
        .filter(|q| autonomia_min(q).is_some())
        .collect();
    if let Some(ultima) = analisaveis.first() {
        let a = analise(ultima);
        let nominal = s.nominal_v.unwrap_or(12) as f32 / 12.0;
        if let Some(m) = a.autonomia_projetada_min {
            if m < 3.0 {
                av.push(aviso(
                    "autonomia_critica",
                    Gravidade::Erro,
                    format!(
                        "Autonomia projetada de {m:.0} min na última queda: bateria muito fraca"
                    ),
                ));
            } else if m < 10.0 {
                av.push(aviso(
                    "autonomia_curta",
                    Gravidade::Aviso,
                    format!("Autonomia projetada de {m:.0} min na última queda"),
                ));
            }
            let antes: Vec<f32> = analisaveis[1..]
                .iter()
                .take(5)
                .filter_map(|q| autonomia_min(q))
                .collect();
            if !antes.is_empty() {
                let media = antes.iter().sum::<f32>() / antes.len() as f32;
                s.autonomia_anterior_min = Some(media);
                if m < media * 0.6 {
                    av.push(aviso(
                        "autonomia_caindo",
                        Gravidade::Aviso,
                        format!("Autonomia caiu: {m:.0} min contra média de {media:.0} min nas quedas anteriores"),
                    ));
                }
            }
        }
        if let Some(d) = a.queda_tensao_v {
            if d >= 1.5 * nominal {
                av.push(aviso(
                    "tensao_afunda",
                    Gravidade::Aviso,
                    format!("A tensão caiu {d:.1} V durante a última queda: bateria com resistência interna alta"),
                ));
            }
        }
        s.ultima_queda = Some(a);
    }

    if let Some(q) = quedas.lista().iter().rev().find(|q| q.recarga_s.is_some()) {
        if let Some(r) = q.recarga_s {
            if r > 12 * 3600 {
                av.push(aviso(
                    "recarga_lenta",
                    Gravidade::Aviso,
                    format!("A última recarga levou {} h até 99%", r / 3600),
                ));
            }
        }
    }

    if let Some(t) = temperatura_c {
        if t >= 70.0 {
            av.push(aviso(
                "temperatura_critica",
                Gravidade::Erro,
                format!("Temperatura do inversor em {t:.0} °C"),
            ));
        } else if t >= 60.0 {
            av.push(aviso(
                "temperatura_alta",
                Gravidade::Aviso,
                format!("Temperatura do inversor em {t:.0} °C"),
            ));
        }
    }

    if let Some(i) = instalada {
        let idade = agora.saturating_sub(i);
        s.idade_dias = Some((idade / 86_400) as u32);
        let anos = idade as f32 / ANO_S as f32;
        if idade >= 4 * ANO_S {
            av.push(aviso(
                "bateria_velha",
                Gravidade::Erro,
                format!("Bateria com {anos:.1} anos: troque (a vida típica é de 3 a 5 anos)"),
            ));
        } else if idade >= 3 * ANO_S {
            av.push(aviso(
                "bateria_envelhecendo",
                Gravidade::Aviso,
                format!("Bateria com {anos:.1} anos: planeje a troca"),
            ));
        }
    }

    s.avisos = av;
    s
}

/// `AAAA-MM-DD` ou `AAAA-MM` (dia 1) para segundos Unix à meia-noite UTC.
pub fn parse_data(t: &str) -> Option<u64> {
    let mut p = t.trim().split('-');
    let a: i64 = p.next()?.parse().ok()?;
    let m: i64 = p.next()?.parse().ok()?;
    let d: i64 = match p.next() {
        Some(x) => x.parse().ok()?,
        None => 1,
    };
    if p.next().is_some() || !(1..=12).contains(&m) || !(1..=31).contains(&d) || a < 1970 {
        return None;
    }
    // dias desde 1970-01-01 (algoritmo de Howard Hinnant)
    let a2 = if m <= 2 { a - 1 } else { a };
    let era = a2.div_euclid(400);
    let yoe = a2 - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let dias = era * 146_097 + doe - 719_468;
    u64::try_from(dias).ok().map(|d| d * 86_400)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::quedas::Detalhe;

    fn repouso(v: f32) -> Quedas {
        let mut q = Quedas::default();
        for _ in 0..100 {
            q.repouso.amostrar(v);
        }
        q
    }

    fn codigos(s: &Saude) -> Vec<&'static str> {
        s.avisos.iter().map(|a| a.codigo).collect()
    }

    /// Queda de `dur` s que perde `perda` pontos de carga e `sag` V.
    fn queda(q: &mut Quedas, t: u64, dur: u64, perda: f32, sag: f32) {
        q.atualizar(t, true, 100.0, 5.0);
        q.detalhar(
            t,
            Detalhe {
                bateria_pct: 100.0,
                bateria_v: 13.0,
                carga_pct: 10.0,
            },
        );
        q.atualizar(t + dur, true, 100.0 - perda, 5.0);
        q.detalhar(
            t + dur,
            Detalhe {
                bateria_pct: 100.0 - perda,
                bateria_v: 13.0 - sag,
                carga_pct: 12.0,
            },
        );
        q.atualizar(t + dur + 1, false, 100.0 - perda, 120.0);
    }

    #[test]
    fn repouso_so_vale_com_amostras_suficientes() {
        let mut r = Repouso::default();
        for _ in 0..10 {
            r.amostrar(13.3);
        }
        assert_eq!(r.tensao(), None);
        for _ in 0..60 {
            r.amostrar(13.3);
        }
        assert!((r.tensao().unwrap() - 13.3).abs() < 0.01);
    }

    #[test]
    fn tensao_de_repouso() {
        assert!(codigos(&avaliar(&repouso(13.4), None, None, 0)).is_empty());
        assert_eq!(
            codigos(&avaliar(&repouso(12.8), None, None, 0)),
            ["repouso_baixo"]
        );
        assert_eq!(
            codigos(&avaliar(&repouso(12.0), None, None, 0)),
            ["repouso_muito_baixo"]
        );
        assert_eq!(
            codigos(&avaliar(&repouso(15.0), None, None, 0)),
            ["repouso_alto"]
        );
        // 24 V: limites dobram
        let s = avaliar(&repouso(26.8), None, None, 0);
        assert_eq!(s.nominal_v, Some(24));
        assert!(s.avisos.is_empty());
        assert_eq!(
            codigos(&avaliar(&repouso(25.6), None, None, 0)),
            ["repouso_baixo"]
        );
    }

    #[test]
    fn autonomia_projetada() {
        let mut q = Quedas::default();
        queda(&mut q, 1000, 600, 20.0, 0.5); // 10 min para 20% => 50 min de 100% a 0
        let s = avaliar(&q, None, None, 5000);
        let a = s.ultima_queda.unwrap();
        assert!((a.autonomia_projetada_min.unwrap() - 50.0).abs() < 0.5);
        assert!(s.avisos.is_empty());
    }

    #[test]
    fn autonomia_curta_e_critica() {
        let mut q = Quedas::default();
        queda(&mut q, 1000, 49, 16.0, 0.4); // ~5 min
        assert_eq!(codigos(&avaliar(&q, None, None, 5000)), ["autonomia_curta"]);
        let mut q = Quedas::default();
        queda(&mut q, 1000, 40, 40.0, 0.4); // 1,7 min
        assert_eq!(
            codigos(&avaliar(&q, None, None, 5000)),
            ["autonomia_critica"]
        );
    }

    #[test]
    fn queda_curta_ou_pequena_nao_entra() {
        let mut q = Quedas::default();
        queda(&mut q, 1000, 20, 30.0, 0.4); // curta demais
        queda(&mut q, 2000, 600, 2.0, 0.1); // perdeu menos de 5 pontos
        let s = avaliar(&q, None, None, 5000);
        assert!(s.ultima_queda.is_none());
        assert!(s.avisos.is_empty());
    }

    #[test]
    fn autonomia_caindo_e_tensao_afundando() {
        let mut q = Quedas::default();
        queda(&mut q, 1000, 1200, 20.0, 0.5); // 100 min
        queda(&mut q, 5000, 1200, 20.0, 0.5); // 100 min
        queda(&mut q, 9000, 600, 20.0, 1.8); // 50 min, tensão afunda
        let c = codigos(&avaliar(&q, None, None, 20_000));
        assert_eq!(c, ["autonomia_caindo", "tensao_afunda"]);
    }

    #[test]
    fn temperatura() {
        let q = Quedas::default();
        assert!(avaliar(&q, Some(51.0), None, 0).avisos.is_empty());
        assert_eq!(
            codigos(&avaliar(&q, Some(62.0), None, 0)),
            ["temperatura_alta"]
        );
        assert_eq!(
            codigos(&avaliar(&q, Some(75.0), None, 0)),
            ["temperatura_critica"]
        );
    }

    #[test]
    fn idade_da_bateria() {
        let q = Quedas::default();
        let inst = parse_data("2023-01-01").unwrap();
        let em = |anos: f32| inst + (anos * ANO_S as f32) as u64;
        assert!(avaliar(&q, None, Some(inst), em(2.0)).avisos.is_empty());
        assert_eq!(
            codigos(&avaliar(&q, None, Some(inst), em(3.2))),
            ["bateria_envelhecendo"]
        );
        let s = avaliar(&q, None, Some(inst), em(4.5));
        assert_eq!(codigos(&s), ["bateria_velha"]);
        assert!(s.idade_dias.unwrap() > 1600);
    }

    #[test]
    fn datas() {
        assert_eq!(parse_data("1970-01-01"), Some(0));
        assert_eq!(parse_data("2026-10-30"), Some(1_793_318_400));
        assert_eq!(parse_data("2026-10"), parse_data("2026-10-01"));
        assert_eq!(parse_data("2026-13-01"), None);
        assert_eq!(parse_data("abc"), None);
        assert_eq!(parse_data("2026-10-01-02"), None);
    }
}
