//! Registro de quedas de energia (períodos em que o nobreak ficou na bateria) e resumo por período.
//! Cada queda grava uma linha ao começar e outra ao terminar, num arquivo `.jsonl` opcional.
//! Assim, se o serviço ou o servidor morrer no meio da queda (bateria esgotada), o começo não se perde.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

const MAX_QUEDAS: usize = 200;

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Queda {
    pub inicio: u64,
    /// `None` enquanto a queda dura, ou quando o serviço parou no meio dela (`interrompida`).
    pub fim: Option<u64>,
    pub duracao_s: Option<u64>,
    pub bateria_inicio_pct: f32,
    pub bateria_min_pct: f32,
    pub entrada_min_v: f32,
    /// Tensão da bateria no começo da queda e a menor vista nela (`None` em registros antigos).
    pub bateria_v_inicio: Option<f32>,
    pub bateria_v_min: Option<f32>,
    /// Maior carga na saída durante a queda.
    pub carga_max_pct: Option<f32>,
    /// Tempo entre o fim da queda e a bateria voltar a 99%. `None` enquanto recarrega.
    pub recarga_s: Option<u64>,
    /// O serviço reiniciou durante a recarga: o tempo real é desconhecido.
    pub recarga_perdida: bool,
    /// O serviço parou durante a queda: a duração real é desconhecida.
    pub interrompida: bool,
}

/// Uma linha do arquivo.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Linha {
    pub t: u64,
    /// `inicio` ou `fim`.
    pub ev: String,
    pub bateria_pct: f32,
    pub entrada_v: f32,
    /// `inicio`: tensão atual. `fim`: menor tensão da queda.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bateria_v: Option<f32>,
    /// `inicio`: carga atual. `fim`: maior carga da queda.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carga_pct: Option<f32>,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Resumo {
    pub quedas: u32,
    /// Soma das durações conhecidas (a queda em curso conta até agora).
    pub segundos_em_bateria: u64,
    pub maior_s: u64,
    /// Menor carga de bateria vista nas quedas do período.
    pub bateria_min_pct: Option<f32>,
    pub interrompidas: u32,
}

#[derive(Default)]
pub struct Quedas {
    lista: VecDeque<Queda>,
    pendentes: Vec<Linha>,
    /// Tensão da bateria em repouso (rede ligada, 100%, carga completa).
    pub repouso: crate::bateria::Repouso,
}

/// Leitura da bateria e da saída, para detalhar a queda e a recarga.
#[derive(Clone, Copy)]
pub struct Detalhe {
    pub bateria_pct: f32,
    pub bateria_v: f32,
    pub carga_pct: f32,
}

impl Quedas {
    pub fn em_curso(&self) -> bool {
        self.lista
            .back()
            .map(|q| q.fim.is_none() && !q.interrompida)
            .unwrap_or(false)
    }

    pub fn lista(&self) -> &VecDeque<Queda> {
        &self.lista
    }

    /// A última queda terminou e a bateria ainda não voltou a 99%.
    pub fn recarregando(&self) -> bool {
        self.lista
            .back()
            .map(|q| {
                q.fim.is_some() && q.recarga_s.is_none() && !q.recarga_perdida && !q.interrompida
            })
            .unwrap_or(false)
    }

    fn nova(
        t: u64,
        bateria_pct: f32,
        entrada_v: f32,
        bateria_v: Option<f32>,
        carga: Option<f32>,
    ) -> Queda {
        Queda {
            inicio: t,
            fim: None,
            duracao_s: None,
            bateria_inicio_pct: bateria_pct,
            bateria_min_pct: bateria_pct,
            entrada_min_v: entrada_v,
            bateria_v_inicio: bateria_v,
            bateria_v_min: bateria_v,
            carga_max_pct: carga,
            recarga_s: None,
            recarga_perdida: false,
            interrompida: false,
        }
    }

    fn empurrar(&mut self, q: Queda) {
        if self.lista.len() >= MAX_QUEDAS {
            self.lista.pop_front();
        }
        self.lista.push_back(q);
    }

    /// Reconstrói a lista a partir do arquivo (linhas mais antigas primeiro).
    pub fn restaurar(&mut self, linhas: &[Linha]) {
        for l in linhas {
            match l.ev.as_str() {
                "inicio" => {
                    // um `inicio` sem `fim` antes deste: o serviço parou no meio da queda
                    if self.em_curso() {
                        if let Some(q) = self.lista.back_mut() {
                            q.interrompida = true;
                        }
                    }
                    self.empurrar(Self::nova(
                        l.t,
                        l.bateria_pct,
                        l.entrada_v,
                        l.bateria_v,
                        l.carga_pct,
                    ));
                }
                "fim" => {
                    if let Some(q) = self.lista.back_mut() {
                        if q.fim.is_none() && !q.interrompida {
                            q.fim = Some(l.t);
                            q.duracao_s = Some(l.t.saturating_sub(q.inicio));
                            q.bateria_min_pct = q.bateria_min_pct.min(l.bateria_pct);
                            q.entrada_min_v = q.entrada_min_v.min(l.entrada_v);
                            q.bateria_v_min = menor(q.bateria_v_min, l.bateria_v);
                            q.carga_max_pct = maior(q.carga_max_pct, l.carga_pct);
                        }
                    }
                }
                "recarga" => {
                    if let Some(q) = self.lista.back_mut() {
                        if let (Some(f), None) = (q.fim, q.recarga_s) {
                            q.recarga_s = Some(l.t.saturating_sub(f));
                        }
                    }
                }
                _ => {}
            }
        }
        // depois do último `inicio` sem `fim`, o serviço reiniciou: a queda ficou aberta no arquivo
        if self.em_curso() {
            if let Some(q) = self.lista.back_mut() {
                q.interrompida = true;
            }
        }
        // queda terminada sem linha `recarga`: o serviço parou durante a recarga
        if self.recarregando() {
            if let Some(q) = self.lista.back_mut() {
                q.recarga_perdida = true;
            }
        }
    }

    /// Chamada a cada leitura. `em_bateria` vem do estado já decodificado.
    pub fn atualizar(&mut self, t: u64, em_bateria: bool, bateria_pct: f32, entrada_v: f32) {
        if em_bateria {
            if self.em_curso() {
                if let Some(q) = self.lista.back_mut() {
                    q.bateria_min_pct = q.bateria_min_pct.min(bateria_pct);
                    q.entrada_min_v = q.entrada_min_v.min(entrada_v);
                }
            } else {
                self.empurrar(Self::nova(t, bateria_pct, entrada_v, None, None));
                self.pendentes.push(Linha {
                    t,
                    ev: "inicio".into(),
                    bateria_pct,
                    entrada_v,
                    bateria_v: None,
                    carga_pct: None,
                });
            }
        } else if self.em_curso() {
            let (min_bat, min_v, vbat, carga) = match self.lista.back_mut() {
                Some(q) => {
                    q.fim = Some(t);
                    q.duracao_s = Some(t.saturating_sub(q.inicio));
                    (
                        q.bateria_min_pct,
                        q.entrada_min_v,
                        q.bateria_v_min,
                        q.carga_max_pct,
                    )
                }
                None => (bateria_pct, entrada_v, None, None),
            };
            // a linha `fim` leva os mínimos, para o resumo sobreviver a reinícios
            self.pendentes.push(Linha {
                t,
                ev: "fim".into(),
                bateria_pct: min_bat,
                entrada_v: min_v,
                bateria_v: vbat,
                carga_pct: carga,
            });
        }
    }

    /// Detalhes da leitura: tensão mínima e carga máxima da queda em curso, tempo de recarga
    /// depois dela e tensão de repouso. Chamar depois de `atualizar`.
    pub fn detalhar(&mut self, t: u64, d: Detalhe) {
        if self.em_curso() {
            if let Some(q) = self.lista.back_mut() {
                if q.bateria_v_inicio.is_none() {
                    q.bateria_v_inicio = Some(d.bateria_v);
                }
                q.bateria_v_min = menor(q.bateria_v_min, Some(d.bateria_v));
                q.carga_max_pct = maior(q.carga_max_pct, Some(d.carga_pct));
            }
            return;
        }
        if self.recarregando() {
            if d.bateria_pct >= 99.0 {
                if let Some(q) = self.lista.back_mut() {
                    q.recarga_s = q.fim.map(|f| t.saturating_sub(f));
                }
                self.pendentes.push(Linha {
                    t,
                    ev: "recarga".into(),
                    bateria_pct: d.bateria_pct,
                    entrada_v: 0.0,
                    bateria_v: Some(d.bateria_v),
                    carga_pct: None,
                });
            }
            return;
        }
        if d.bateria_pct >= 99.0 {
            self.repouso.amostrar(d.bateria_v);
        }
    }

    /// Linhas novas para gravar no arquivo.
    pub fn drenar(&mut self) -> Vec<Linha> {
        std::mem::take(&mut self.pendentes)
    }

    /// Resumo das quedas que começaram nos últimos `janela_s` segundos.
    pub fn resumo(&self, agora: u64, janela_s: u64) -> Resumo {
        let corte = agora.saturating_sub(janela_s);
        let mut r = Resumo::default();
        for q in self.lista.iter().filter(|q| q.inicio >= corte) {
            r.quedas += 1;
            if q.interrompida {
                r.interrompidas += 1;
            }
            let dur = match (q.fim, q.interrompida) {
                (Some(f), _) => f.saturating_sub(q.inicio),
                (None, false) => agora.saturating_sub(q.inicio),
                (None, true) => 0,
            };
            r.segundos_em_bateria += dur;
            r.maior_s = r.maior_s.max(dur);
            r.bateria_min_pct = Some(match r.bateria_min_pct {
                Some(m) => m.min(q.bateria_min_pct),
                None => q.bateria_min_pct,
            });
        }
        r
    }

    /// Lista para o painel, a mais nova primeiro. Recalcula a duração da queda em curso.
    pub fn lista_json(&self, agora: u64) -> Vec<Queda> {
        self.lista
            .iter()
            .rev()
            .take(50)
            .map(|q| {
                let mut q = q.clone();
                if q.fim.is_none() && !q.interrompida {
                    q.duracao_s = Some(agora.saturating_sub(q.inicio));
                }
                q
            })
            .collect()
    }
}

fn menor(a: Option<f32>, b: Option<f32>) -> Option<f32> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, y) => x.or(y),
    }
}

fn maior(a: Option<f32>, b: Option<f32>) -> Option<f32> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, y) => x.or(y),
    }
}

/// Lê o arquivo de quedas. Linhas quebradas ou com bytes nulos (queda de energia) são ignoradas.
pub fn ler_arquivo(caminho: &str) -> Vec<Linha> {
    let bytes = std::fs::read(caminho).unwrap_or_default();
    String::from_utf8_lossy(&bytes)
        .split('\n')
        .filter_map(|l| {
            serde_json::from_str(l.trim_matches(|c: char| c == '\0' || c.is_whitespace())).ok()
        })
        .collect()
}

/// Acrescenta linhas. O arquivo cresce pouco (2 linhas por queda); passando de 64 KB, mantém as últimas.
pub fn anexar_arquivo(caminho: &str, novas: &[Linha]) {
    use std::io::Write;
    if novas.is_empty() {
        return;
    }
    let grande = std::fs::metadata(caminho)
        .map(|m| m.len() > 64 * 1024)
        .unwrap_or(false);
    if grande {
        let mut v = ler_arquivo(caminho);
        v.extend_from_slice(novas);
        let corte = v.len().saturating_sub(MAX_QUEDAS * 2);
        let txt: String = v[corte..]
            .iter()
            .filter_map(|l| serde_json::to_string(l).ok())
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
        for l in novas {
            if let Ok(s) = serde_json::to_string(l) {
                let _ = writeln!(f, "{s}");
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn queda_completa() {
        let mut q = Quedas::default();
        q.atualizar(100, false, 100.0, 120.0);
        assert!(q.drenar().is_empty());
        q.atualizar(200, true, 100.0, 7.0);
        q.atualizar(210, true, 95.0, 6.0);
        q.atualizar(249, true, 84.0, 7.0);
        assert!(q.em_curso());
        q.atualizar(260, false, 84.0, 121.0);
        assert!(!q.em_curso());
        let l = q.drenar();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].ev, "inicio");
        assert_eq!(l[1].ev, "fim");
        assert_eq!(l[1].bateria_pct, 84.0); // a linha fim leva o mínimo
        let r = q.resumo(300, 3600);
        assert_eq!(r.quedas, 1);
        assert_eq!(r.segundos_em_bateria, 60);
        assert_eq!(r.maior_s, 60);
        assert_eq!(r.bateria_min_pct, Some(84.0));
    }

    #[test]
    fn queda_em_curso_conta_ate_agora() {
        let mut q = Quedas::default();
        q.atualizar(100, true, 100.0, 0.0);
        let r = q.resumo(160, 3600);
        assert_eq!(r.segundos_em_bateria, 60);
        assert_eq!(q.lista_json(160)[0].duracao_s, Some(60));
    }

    #[test]
    fn janela_exclui_quedas_antigas() {
        let mut q = Quedas::default();
        q.atualizar(1000, true, 100.0, 0.0);
        q.atualizar(1030, false, 90.0, 120.0);
        q.atualizar(90_000, true, 100.0, 0.0);
        q.atualizar(90_020, false, 95.0, 120.0);
        assert_eq!(q.resumo(90_100, 86_400).quedas, 1);
        assert_eq!(q.resumo(90_100, 7 * 86_400).quedas, 2);
    }

    #[test]
    fn restaura_do_arquivo_e_marca_interrompida() {
        let l = |t: u64, ev: &str, b: f32| Linha {
            t,
            ev: ev.into(),
            bateria_pct: b,
            entrada_v: 5.0,
            bateria_v: None,
            carga_pct: None,
        };
        let mut q = Quedas::default();
        // 1ª completa; 2ª sem `fim` (o servidor morreu na queda)
        q.restaurar(&[
            l(100, "inicio", 100.0),
            l(150, "fim", 80.0),
            l(500, "inicio", 90.0),
        ]);
        let v = q.lista_json(1000);
        assert_eq!(v.len(), 2);
        assert!(v[0].interrompida);
        assert_eq!(v[0].duracao_s, None);
        assert_eq!(v[1].duracao_s, Some(50));
        assert!(!q.em_curso());
        let r = q.resumo(1000, 3600);
        assert_eq!(
            (r.quedas, r.interrompidas, r.segundos_em_bateria),
            (2, 1, 50)
        );
    }

    fn d(pct: f32, v: f32, carga: f32) -> Detalhe {
        Detalhe {
            bateria_pct: pct,
            bateria_v: v,
            carga_pct: carga,
        }
    }

    #[test]
    fn detalha_queda_e_recarga() {
        let mut q = Quedas::default();
        q.atualizar(100, true, 100.0, 5.0);
        q.detalhar(100, d(100.0, 13.0, 10.0));
        q.atualizar(130, true, 80.0, 5.0);
        q.detalhar(130, d(80.0, 11.8, 25.0));
        q.atualizar(160, false, 80.0, 120.0);
        q.detalhar(160, d(80.0, 12.6, 25.0));
        assert!(q.recarregando());
        q.atualizar(1000, false, 95.0, 120.0);
        q.detalhar(1000, d(95.0, 13.8, 5.0));
        assert!(q.recarregando());
        q.atualizar(4000, false, 100.0, 120.0);
        q.detalhar(4000, d(100.0, 13.4, 5.0));
        assert!(!q.recarregando());
        let x = &q.lista()[0];
        assert_eq!(x.bateria_v_min, Some(11.8));
        assert_eq!(x.carga_max_pct, Some(25.0));
        assert_eq!(x.recarga_s, Some(3840));
        let l = q.drenar();
        assert_eq!(
            l.iter().map(|l| l.ev.as_str()).collect::<Vec<_>>(),
            ["inicio", "fim", "recarga"]
        );
        assert_eq!(l[1].bateria_v, Some(11.8));
    }

    #[test]
    fn restaura_detalhes_e_aceita_linhas_antigas() {
        let mut q = Quedas::default();
        q.restaurar(&[
            Linha {
                t: 100,
                ev: "inicio".into(),
                bateria_pct: 100.0,
                entrada_v: 5.0,
                bateria_v: Some(13.0),
                carga_pct: Some(10.0),
            },
            Linha {
                t: 160,
                ev: "fim".into(),
                bateria_pct: 70.0,
                entrada_v: 5.0,
                bateria_v: Some(11.9),
                carga_pct: Some(30.0),
            },
            Linha {
                t: 3760,
                ev: "recarga".into(),
                bateria_pct: 99.0,
                entrada_v: 0.0,
                bateria_v: None,
                carga_pct: None,
            },
        ]);
        let x = &q.lista()[0];
        assert_eq!(
            (x.bateria_v_min, x.carga_max_pct, x.recarga_s),
            (Some(11.9), Some(30.0), Some(3600))
        );
        let velha: Linha =
            serde_json::from_str(r#"{"t":1,"ev":"inicio","bateria_pct":100.0,"entrada_v":5.0}"#)
                .unwrap();
        assert_eq!(velha.bateria_v, None);
    }

    #[test]
    fn reinicio_durante_a_recarga_marca_recarga_perdida() {
        let l = |t: u64, ev: &str| Linha {
            t,
            ev: ev.into(),
            bateria_pct: 80.0,
            entrada_v: 5.0,
            bateria_v: None,
            carga_pct: None,
        };
        let mut q = Quedas::default();
        q.restaurar(&[l(100, "inicio"), l(160, "fim")]);
        assert!(!q.recarregando());
        assert!(q.lista()[0].recarga_perdida);
        assert_eq!(q.lista()[0].recarga_s, None);
    }

    #[test]
    fn arquivo_tolera_lixo() {
        let p = std::env::temp_dir().join(format!("nb-quedas-{}", std::process::id()));
        let mut b = Vec::new();
        b.extend(b"{\"t\":1,\"ev\":\"inicio\",\"bateria_pct\":100.0,\"entrada_v\":5.0}\n");
        b.extend(std::iter::repeat(0u8).take(40));
        b.extend(
            b"{\"t\":2,\"ev\":\"fim\",\"bateria_pct\":90.0,\"entrada_v\":5.0}\n{\"t\":3,\"ev\n",
        );
        std::fs::write(&p, &b).unwrap();
        let v = ler_arquivo(p.to_str().unwrap());
        let _ = std::fs::remove_file(&p);
        assert_eq!(v.len(), 2); // o bloco nulo cola na 2ª linha, mas é removido; a 3ª está truncada
    }
}
