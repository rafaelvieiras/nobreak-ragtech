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
    Pedido { motivo: String },
    /// A condição passou (voltou a rede, bateria recuperou): contagem cancelada.
    Cancelado { motivo: String },
}

#[derive(Debug, Clone)]
pub struct Config {
    /// `false` = só observa e registra, nunca pede desligamento.
    pub ligada: bool,
    pub bateria_minima_pct: f32,
    pub confirma_s: u64,
    pub tempo_maximo_s: u64,
    pub aviso_s: u64,
    /// Arquivo onde o pedido é escrito. Vazio = não escreve (só evento/métrica).
    pub arquivo: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ligada: false,
            bateria_minima_pct: 50.0,
            confirma_s: 20,
            tempo_maximo_s: 300,
            aviso_s: 45,
            arquivo: None,
        }
    }
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
}

/// Estado atual, exposto no `/api/estado` e nas métricas.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Situacao {
    pub armado: bool,
    pub fase: Fase,
    pub motivo: Option<String>,
    /// Segundos até o pedido, quando em aviso.
    pub restante_s: Option<u64>,
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
                self.cfg.aviso_s.saturating_sub(usado)
            }),
        }
    }

    /// Avalia uma leitura boa. `em_queda` = está na bateria; `em_bateria_s` = duração da
    /// queda atual (0 quando a rede está presente).
    pub fn avaliar(
        &mut self,
        t: u64,
        em_queda: bool,
        bateria_pct: f32,
        em_bateria_s: u64,
    ) -> Option<Evento> {
        if !self.cfg.ligada {
            return None;
        }
        let bateria_baixa = bateria_pct <= self.cfg.bateria_minima_pct;
        let tempo_estourado = em_bateria_s >= self.cfg.tempo_maximo_s && self.cfg.tempo_maximo_s > 0;
        let perigo = em_queda && (bateria_baixa || tempo_estourado);

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

        let motivo = if bateria_baixa && tempo_estourado {
            format!(
                "bateria em {bateria_pct:.0}% (limite {:.0}%) e {em_bateria_s}s sem rede (limite {}s)",
                self.cfg.bateria_minima_pct, self.cfg.tempo_maximo_s
            )
        } else if bateria_baixa {
            format!(
                "bateria em {bateria_pct:.0}% (limite {:.0}%), {em_bateria_s}s sem rede",
                self.cfg.bateria_minima_pct
            )
        } else {
            format!(
                "{em_bateria_s}s sem rede (limite {}s), bateria em {bateria_pct:.0}%",
                self.cfg.tempo_maximo_s
            )
        };

        let inicio = *self.perigo_desde.get_or_insert(t);
        if t.saturating_sub(inicio) < self.cfg.confirma_s {
            return None;
        }

        match self.fase {
            Fase::Normal => {
                self.fase = Fase::Aviso;
                self.aviso_desde = Some(t);
                self.motivo = motivo.clone();
                Some(Evento::Aviso {
                    restante_s: self.cfg.aviso_s,
                    motivo,
                })
            }
            Fase::Aviso => {
                let desde = self.aviso_desde.unwrap_or(t);
                if t.saturating_sub(desde) < self.cfg.aviso_s {
                    return None;
                }
                self.fase = Fase::Pedido;
                self.motivo = motivo.clone();
                if let Some(caminho) = &self.cfg.arquivo {
                    let conteudo = pedido_json(t, &motivo, bateria_pct, em_bateria_s);
                    if let Err(e) = std::fs::write(caminho, conteudo) {
                        eprintln!("não consegui escrever o pedido de desligamento em {caminho}: {e}");
                    }
                }
                Some(Evento::Pedido { motivo })
            }
            Fase::Pedido => None,
        }
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

    fn politica() -> Politica {
        Politica::nova(Config {
            ligada: true,
            bateria_minima_pct: 50.0,
            confirma_s: 20,
            tempo_maximo_s: 300,
            aviso_s: 45,
            arquivo: None,
        })
    }

    #[test]
    fn com_rede_nunca_age() {
        let mut p = politica();
        for t in 0..600 {
            assert_eq!(p.avaliar(t, false, 5.0, 0), None);
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
            assert_eq!(p.avaliar(t, true, 1.0, 500), None);
        }
    }

    #[test]
    fn aviso_depois_pedido() {
        let mut p = politica();
        // bateria baixa em t=100; confirma em 120
        assert_eq!(p.avaliar(100, true, 40.0, 30), None);
        assert_eq!(p.avaliar(115, true, 39.0, 45), None);
        assert!(matches!(
            p.avaliar(120, true, 38.0, 50),
            Some(Evento::Aviso { restante_s: 45, .. })
        ));
        assert_eq!(p.situacao(120).fase, Fase::Aviso);
        // durante o aviso não repete
        assert_eq!(p.avaliar(140, true, 35.0, 70), None);
        assert!(matches!(
            p.avaliar(165, true, 30.0, 95),
            Some(Evento::Pedido { .. })
        ));
        assert_eq!(p.situacao(165).fase, Fase::Pedido);
        // depois do pedido, silêncio
        assert_eq!(p.avaliar(200, true, 20.0, 130), None);
    }

    #[test]
    fn volta_da_rede_cancela() {
        let mut p = politica();
        p.avaliar(100, true, 40.0, 10);
        assert!(matches!(
            p.avaliar(120, true, 39.0, 30),
            Some(Evento::Aviso { .. })
        ));
        assert!(matches!(
            p.avaliar(130, false, 42.0, 0),
            Some(Evento::Cancelado { .. })
        ));
        assert_eq!(p.situacao(130).fase, Fase::Normal);
        // e pode voltar a avisar numa nova queda
        p.avaliar(200, true, 40.0, 5);
        assert!(matches!(
            p.avaliar(220, true, 38.0, 25),
            Some(Evento::Aviso { .. })
        ));
    }

    #[test]
    fn tempo_em_bateria_estourado_dispara_mesmo_com_bateria_alta() {
        let mut p = politica();
        // 300 s sem rede e bateria em 90 % (queda longa com carga baixa)
        assert_eq!(p.avaliar(300, true, 90.0, 295), None);
        // o perigo é notado em 310 (305 s sem rede) e confirmado 20 s depois
        assert_eq!(p.avaliar(310, true, 90.0, 305), None);
        assert!(matches!(
            p.avaliar(330, true, 89.0, 325),
            Some(Evento::Aviso { .. })
        ));
    }

    #[test]
    fn oscilacao_curta_nao_dispara() {
        let mut p = politica();
        // leitura ruim de entrada por 10 s com bateria abaixo do limite
        assert_eq!(p.avaliar(100, true, 30.0, 2), None);
        assert_eq!(p.avaliar(110, true, 45.0, 12), None);
        assert_eq!(p.avaliar(115, false, 100.0, 0), None); // nem chegou a avisar
        assert_eq!(p.situacao(115).fase, Fase::Normal);
    }

    #[test]
    fn pedido_escreve_arquivo() {
        let dir = std::env::temp_dir().join("nobreak-politica-teste");
        let _ = std::fs::create_dir_all(&dir);
        let arq = dir.join("desligar");
        let _ = std::fs::remove_file(&arq);
        let mut p = Politica::nova(Config {
            ligada: true,
            bateria_minima_pct: 50.0,
            confirma_s: 5,
            tempo_maximo_s: 300,
            aviso_s: 10,
            arquivo: Some(arq.to_string_lossy().into_owned()),
        });
        p.avaliar(10, true, 40.0, 5);
        assert!(matches!(
            p.avaliar(15, true, 38.0, 10),
            Some(Evento::Aviso { .. })
        ));
        assert!(matches!(
            p.avaliar(25, true, 35.0, 20),
            Some(Evento::Pedido { .. })
        ));
        let txt = std::fs::read_to_string(&arq).unwrap();
        assert!(txt.contains("\"bateria_pct\":35.0"));
        assert!(txt.contains("sem_rede_s\":20"));
        assert!(txt.starts_with("{\"t\":25,"));
        let _ = std::fs::remove_file(&arq);
    }
}
