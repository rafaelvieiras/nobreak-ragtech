//! Desligamento do host por bateria baixa.
//!
//! Medição que define os padrões (queda real de 30/out/2026 no nobreak do Moon,
//! registrada em `quedas.jsonl`): 100 % → 18 % em 508 s com 3 % de carga, ou seja
//! **9,7 % por minuto** e autonomia total de ~8,5 min. Por isso a porcentagem sozinha
//! engana: a 25 % restam cerca de 45 s, insuficiente para desligar com folga. A política
//! combina dois gatilhos — bateria abaixo do limite **ou** tempo em bateria acima do
//! máximo — e exige confirmação para não reagir a oscilação de leitura (a entrada chega
//! a ser lida como 6–7 V em alguns ciclos).
//!
//! O serviço **não desliga nada**: ele escreve um pedido num arquivo, e quem desliga é um
//! serviço do host (`nobreak-shutdown.path`). Assim o container continua sem privilégio.

use serde::Serialize;
use std::fmt::Write as _;

/// Fase da política, para o painel e as métricas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Fase {
    /// Nada a fazer: com rede, desarmado, ou condição ainda não confirmada.
    #[default]
    Normal,
    /// Condição confirmada, contagem regressiva antes do pedido.
    Aviso,
    /// Pedido de desligamento já escrito.
    Pedido,
}

/// O que aconteceu neste ciclo e precisa ser anunciado (uma vez).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evento {
    /// Entrou na contagem regressiva.
    Aviso { restante_s: u64, motivo: String },
    /// Escreveu o pedido de desligamento (ou o registrou, se não há arquivo).
    /// `erro` só é `Some` quando a escrita no arquivo falhou: o host não vai desligar nada.
    Pedido {
        motivo: String,
        erro: Option<String>,
    },
    /// A condição passou (voltou a rede, bateria recuperou): contagem cancelada.
    Cancelado { motivo: String },
}

/// Uma leitura, do ponto de vista da política.
#[derive(Debug, Clone, Copy)]
pub struct Leitura {
    /// Está na bateria (rede ausente).
    pub em_queda: bool,
    pub bateria_pct: f32,
    /// Carga no início da queda atual (a taxa de queda sai da diferença).
    pub bateria_inicio_pct: f32,
    /// Há quanto tempo a queda atual começou.
    pub em_bateria_s: u64,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// `false` = só observa e registra, nunca pede desligamento.
    pub ligada: bool,
    /// Projeta desligar quando a autonomia estimada cair a este valor.
    /// Precisa cobrir confirmação + aviso + o próprio desligamento.
    pub margem_s: u64,
    /// Piso absoluto de bateria, para quando ainda não dá para projetar.
    pub bateria_minima_pct: f32,
    pub confirma_s: u64,
    /// Teto de tempo em bateria, mesmo que a projeção diga que sobra carga.
    pub tempo_maximo_s: u64,
    pub aviso_s: u64,
    /// Bateria considerada vazia na projeção (o nobreak corta antes de 0 %).
    pub piso_pct: f32,
    /// Tempo mínimo em bateria antes de acreditar na taxa de queda.
    pub dados_minimos_s: u64,
    /// Arquivo onde o pedido é escrito. Vazio = não escreve (só evento/métrica).
    pub arquivo: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ligada: false,
            margem_s: 120,
            bateria_minima_pct: 40.0,
            confirma_s: 10,
            tempo_maximo_s: 180,
            aviso_s: 20,
            piso_pct: 15.0,
            dados_minimos_s: 20,
            arquivo: None,
        }
    }
}

/// Estimativa de autonomia a partir da taxa de queda observada.
///
/// Devolve `(segundos_restantes, %_por_minuto)`. `None` quando ainda há poucos dados
/// ou a bateria não está caindo — nesse caso quem manda é o piso absoluto.
pub fn projetar(
    inicio_pct: f32,
    atual_pct: f32,
    em_bateria_s: u64,
    piso_pct: f32,
    dados_minimos_s: u64,
) -> Option<(u64, f32)> {
    if em_bateria_s == 0 || em_bateria_s < dados_minimos_s {
        return None;
    }
    let queda = inicio_pct - atual_pct;
    if queda <= 0.5 {
        return None;
    }
    let taxa = queda / (em_bateria_s as f32 / 60.0); // % por minuto
    let restante_pct = (atual_pct - piso_pct).max(0.0);
    Some(((restante_pct / taxa * 60.0) as u64, taxa))
}

#[derive(Debug, Default)]
pub struct Politica {
    pub cfg: Config,
    fase: Fase,
    /// Desde quando a condição perigosa é verdadeira, sem interrupção.
    perigo_desde: Option<u64>,
    /// Quando entrou em aviso.
    aviso_desde: Option<u64>,
    /// Motivo do pedido em curso.
    motivo: String,
    /// Última autonomia projetada, para o painel.
    autonomia_s: Option<u64>,
    /// Aviso efetivamente usado nesta contagem (pode ser menor que o configurado).
    aviso_efetivo_s: u64,
}

/// Estado atual, exposto no `/api/estado` e nas métricas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Situacao {
    pub armado: bool,
    pub fase: Fase,
    pub motivo: Option<String>,
    /// Segundos até o pedido, quando em aviso.
    pub restante_s: Option<u64>,
    /// Autonomia estimada pela taxa de queda atual (None fora de queda ou sem dados).
    pub autonomia_s: Option<u64>,
}

impl Politica {
    pub fn nova(cfg: Config) -> Self {
        Politica {
            cfg,
            ..Default::default()
        }
    }

    pub fn situacao(&self, t: u64) -> Situacao {
        Situacao {
            armado: self.cfg.ligada,
            fase: self.fase,
            motivo: if self.fase == Fase::Normal {
                None
            } else {
                Some(self.motivo.clone())
            },
            restante_s: self.aviso_desde.map(|d| {
                let usado = t.saturating_sub(d);
                self.aviso_efetivo_s.saturating_sub(usado)
            }),
            autonomia_s: self.autonomia_s,
        }
    }

    /// Avalia uma leitura boa. Nunca age sem saber que está na bateria.
    pub fn avaliar(&mut self, t: u64, l: Leitura) -> Option<Evento> {
        if !self.cfg.ligada {
            return None;
        }
        if !l.em_queda {
            self.autonomia_s = None;
        }
        let projecao = if l.em_queda {
            projetar(
                l.bateria_inicio_pct,
                l.bateria_pct,
                l.em_bateria_s,
                self.cfg.piso_pct,
                self.cfg.dados_minimos_s,
            )
        } else {
            None
        };
        if let Some((s, _)) = projecao {
            self.autonomia_s = Some(s);
        }
        let autonomia_curta = projecao
            .map(|(s, _)| s <= self.cfg.margem_s)
            .unwrap_or(false);
        let bateria_baixa = l.bateria_pct <= self.cfg.bateria_minima_pct;
        let tempo_estourado =
            self.cfg.tempo_maximo_s > 0 && l.em_bateria_s >= self.cfg.tempo_maximo_s;
        let perigo = l.em_queda && (autonomia_curta || bateria_baixa || tempo_estourado);

        if !perigo {
            let estava_avisando = self.fase != Fase::Normal;
            self.perigo_desde = None;
            self.aviso_desde = None;
            self.fase = Fase::Normal;
            self.motivo.clear();
            return estava_avisando.then(|| Evento::Cancelado {
                motivo: "rede voltou ou a bateria recuperou antes do pedido".to_string(),
            });
        }

        let motivo = match (projecao, autonomia_curta) {
            (Some((s, taxa)), true) => format!(
                "autonomia estimada de {s}s (bateria em {:.0}%, caindo {taxa:.0}%/min, limite {}s)",
                l.bateria_pct, self.cfg.margem_s
            ),
            _ if bateria_baixa => format!(
                "bateria em {:.0}% (piso {:.0}%), {}s sem rede",
                l.bateria_pct, self.cfg.bateria_minima_pct, l.em_bateria_s
            ),
            _ => format!(
                "{}s sem rede (limite {}s), bateria em {:.0}%",
                l.em_bateria_s, self.cfg.tempo_maximo_s, l.bateria_pct
            ),
        };

        let inicio = *self.perigo_desde.get_or_insert(t);
        // A projeção de autonomia já nasce de `dados_minimos_s` de queda real: exigir mais
        // uma janela de confirmação comeria a autonomia que sobrou. Os gatilhos baratos
        // (piso de bateria, teto de tempo) é que precisam de confirmação.
        let confirmado = autonomia_curta || t.saturating_sub(inicio) >= self.cfg.confirma_s;
        if !confirmado {
            return None;
        }

        if self.fase == Fase::Pedido {
            return None;
        }
        if self.fase == Fase::Normal {
            // Com pouca autonomia o aviso encolhe: a contagem não pode consumir o tempo
            // que ainda existe. Um terço da autonomia projetada é o teto do aviso.
            let aviso = match projecao {
                Some((s, _)) if autonomia_curta => self.cfg.aviso_s.min(s / 3),
                _ => self.cfg.aviso_s,
            };
            self.fase = Fase::Aviso;
            self.aviso_desde = Some(t);
            self.aviso_efetivo_s = aviso;
            self.motivo = motivo.clone();
            if aviso > 0 {
                return Some(Evento::Aviso {
                    restante_s: aviso,
                    motivo,
                });
            }
        }
        let desde = self.aviso_desde.unwrap_or(t);
        if t.saturating_sub(desde) < self.aviso_efetivo_s {
            return None;
        }
        Some(self.pedir(t, motivo, l))
    }

    /// Grava o pedido (quando há arquivo configurado) e vira `Fase::Pedido`.
    fn pedir(&mut self, t: u64, motivo: String, l: Leitura) -> Evento {
        self.fase = Fase::Pedido;
        self.motivo = motivo.clone();
        let erro = match &self.cfg.arquivo {
            Some(caminho) => {
                let conteudo = pedido_json(t, &motivo, l.bateria_pct, l.em_bateria_s);
                match std::fs::write(caminho, conteudo) {
                    Ok(()) => None,
                    Err(e) => Some(format!("não consegui escrever o pedido em {caminho}: {e}")),
                }
            }
            None => Some(
                "NOBREAK_ARQUIVO_DESLIGAR não está definido: nenhum host vai desligar".to_string(),
            ),
        };
        Evento::Pedido { motivo, erro }
    }
}

fn pedido_json(t: u64, motivo: &str, bateria_pct: f32, em_bateria_s: u64) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"t\":{t},\"motivo\":\"{}\",\"bateria_pct\":{bateria_pct:.1},\"sem_rede_s\":{em_bateria_s}}}",
        motivo.replace('"', "'")
    );
    s + "\n"
}

#[cfg(test)]
mod testes {
    use super::*;

    fn cfg() -> Config {
        Config {
            ligada: true,
            margem_s: 120,
            bateria_minima_pct: 40.0,
            confirma_s: 10,
            tempo_maximo_s: 180,
            aviso_s: 20,
            piso_pct: 15.0,
            dados_minimos_s: 20,
            arquivo: None,
        }
    }

    fn politica() -> Politica {
        Politica::nova(cfg())
    }

    fn queda(pct: f32, inicio: f32, s: u64) -> Leitura {
        Leitura {
            em_queda: true,
            bateria_pct: pct,
            bateria_inicio_pct: inicio,
            em_bateria_s: s,
        }
    }

    fn rede(pct: f32) -> Leitura {
        Leitura {
            em_queda: false,
            bateria_pct: pct,
            bateria_inicio_pct: pct,
            em_bateria_s: 0,
        }
    }

    #[test]
    fn projecao_do_teste_de_30_out() {
        // 100 % -> 18 % em 508 s (carga de 3 %): ~9,7 %/min, sobram ~18 s até o piso de 15 %
        let (s, taxa) = projetar(100.0, 18.0, 508, 15.0, 20).unwrap();
        assert!((taxa - 9.685).abs() < 0.01, "taxa {taxa}");
        assert!((s as i64 - 18).abs() <= 1, "autonomia {s}");
    }

    #[test]
    fn projecao_com_a_carga_de_hoje() {
        // com 35 % de carga a queda é muito mais rápida: 100 % -> 55 % em 30 s
        let (s, taxa) = projetar(100.0, 55.0, 30, 15.0, 20).unwrap();
        assert!((taxa - 90.0).abs() < 0.01, "taxa {taxa}");
        assert_eq!(s, 26); // (55-15)/90 min -> 26 s
    }

    #[test]
    fn projecao_precisa_de_dados() {
        assert_eq!(projetar(100.0, 80.0, 10, 15.0, 20), None); // cedo demais
        assert_eq!(projetar(100.0, 99.9, 60, 15.0, 20), None); // não está caindo
        assert_eq!(projetar(100.0, 80.0, 0, 15.0, 20), None);
    }

    #[test]
    fn com_rede_nunca_age() {
        let mut p = politica();
        for t in 0..600 {
            assert_eq!(p.avaliar(t, rede(5.0)), None);
        }
        assert_eq!(p.situacao(600).fase, Fase::Normal);
    }

    #[test]
    fn desarmado_nunca_age() {
        let mut p = Politica::nova(Config {
            ligada: false,
            ..Config::default()
        });
        for t in 0..600 {
            assert_eq!(p.avaliar(t, queda(1.0, 100.0, 500)), None);
        }
    }

    #[test]
    fn autonomia_curta_dispara_mesmo_com_bateria_alta() {
        // queda rápida (carga alta): 38 % em 20 s = 114 %/min; sobram ~24 s até o piso
        let mut p = politica();
        assert_eq!(p.avaliar(100, queda(88.0, 100.0, 8)), None); // dados insuficientes
        match p.avaliar(112, queda(62.0, 100.0, 20)) {
            Some(Evento::Aviso { restante_s, motivo }) => {
                // o aviso encolhe para um terço da autonomia (~24 s -> 8 s)
                assert_eq!(restante_s, 8, "aviso com autonomia curta");
                assert!(motivo.contains("autonomia estimada"), "{motivo}");
            }
            outro => panic!("esperava Aviso, veio {outro:?}"),
        }
        assert_eq!(p.situacao(112).autonomia_s, Some(24));
        // pedido 8 s depois do aviso, sem esperar a confirmação longa
        assert!(matches!(
            p.avaliar(121, queda(45.0, 100.0, 29)),
            Some(Evento::Pedido { .. })
        ));
        assert_eq!(p.situacao(121).fase, Fase::Pedido);
        assert_eq!(p.avaliar(200, queda(20.0, 100.0, 100)), None);
    }

    #[test]
    fn piso_absoluto_age_sem_projecao() {
        // sem 20 s de dados a projeção não existe: ainda assim, 35 % é perigo
        let mut p = politica();
        assert_eq!(p.avaliar(10, queda(35.0, 100.0, 7)), None);
        assert!(matches!(
            p.avaliar(21, queda(34.0, 100.0, 18)),
            Some(Evento::Aviso { .. })
        ));
    }

    #[test]
    fn queda_longa_dispara_por_tempo() {
        let mut p = politica();
        // 200 s sem rede e bateria em 90 %: o teto de 180 s manda desligar,
        // mas esse gatilho precisa dos 10 s de confirmação
        assert_eq!(p.avaliar(200, queda(90.0, 100.0, 200)), None);
        assert!(matches!(
            p.avaliar(211, queda(89.0, 100.0, 211)),
            Some(Evento::Aviso { .. })
        ));
    }

    #[test]
    fn volta_da_rede_cancela() {
        let mut p = politica();
        p.avaliar(10, queda(80.0, 100.0, 8));
        assert!(matches!(
            p.avaliar(25, queda(60.0, 100.0, 23)),
            Some(Evento::Aviso { .. })
        ));
        assert!(matches!(
            p.avaliar(30, rede(62.0)),
            Some(Evento::Cancelado { .. })
        ));
        assert_eq!(p.situacao(30).fase, Fase::Normal);
        assert_eq!(p.situacao(30).autonomia_s, None);
        // nova queda volta a avisar
        p.avaliar(100, queda(95.0, 100.0, 5));
        assert!(matches!(
            p.avaliar(125, queda(70.0, 100.0, 25)),
            Some(Evento::Aviso { .. })
        ));
    }

    #[test]
    fn oscilacao_curta_nao_dispara() {
        let mut p = politica();
        assert_eq!(p.avaliar(10, queda(30.0, 100.0, 3)), None);
        assert_eq!(p.avaliar(15, rede(100.0)), None); // nem chegou a avisar
        assert_eq!(p.situacao(15).fase, Fase::Normal);
    }

    #[test]
    fn sem_arquivo_configurado_avisa_que_ninguem_vai_desligar() {
        let mut p = Politica::nova(Config {
            aviso_s: 0,
            ..cfg()
        });
        p.avaliar(10, queda(80.0, 100.0, 10));
        match p.avaliar(20, queda(60.0, 100.0, 20)) {
            Some(Evento::Pedido { erro, .. }) => {
                assert!(erro.unwrap().contains("NOBREAK_ARQUIVO_DESLIGAR"));
            }
            outro => panic!("esperava Pedido, veio {outro:?}"),
        }
    }

    #[test]
    fn falha_de_escrita_aparece_no_evento() {
        let mut p = Politica::nova(Config {
            aviso_s: 0,
            arquivo: Some("/caminho/que/nao/existe/desligar".to_string()),
            ..cfg()
        });
        p.avaliar(10, queda(80.0, 100.0, 10));
        match p.avaliar(20, queda(60.0, 100.0, 20)) {
            Some(Evento::Pedido { erro, .. }) => {
                assert!(erro.unwrap().contains("não consegui escrever"));
            }
            outro => panic!("esperava Pedido, veio {outro:?}"),
        }
    }

    #[test]
    fn pedido_escreve_arquivo() {
        let dir = std::env::temp_dir().join("nobreak-politica-teste");
        let _ = std::fs::create_dir_all(&dir);
        let arq = dir.join("desligar");
        let _ = std::fs::remove_file(&arq);
        let mut p = Politica::nova(Config {
            aviso_s: 10,
            arquivo: Some(arq.to_string_lossy().into_owned()),
            ..cfg()
        });
        p.avaliar(10, queda(80.0, 100.0, 10));
        assert!(matches!(
            p.avaliar(20, queda(60.0, 100.0, 20)),
            Some(Evento::Aviso { .. })
        ));
        assert!(matches!(
            p.avaliar(30, queda(40.0, 100.0, 30)),
            Some(Evento::Pedido { .. })
        ));
        let txt = std::fs::read_to_string(&arq).unwrap();
        assert!(txt.contains("\"bateria_pct\":40.0"), "{txt}");
        assert!(txt.contains("sem_rede_s\":30"), "{txt}");
        assert!(txt.starts_with("{\"t\":30,"), "{txt}");
        let _ = std::fs::remove_file(&arq);
    }
}
