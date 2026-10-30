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
}

impl Quedas {
    pub fn em_curso(&self) -> bool {
        self.lista
            .back()
            .map(|q| q.fim.is_none() && !q.interrompida)
            .unwrap_or(false)
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
                    self.empurrar(Queda {
                        inicio: l.t,
                        fim: None,
                        duracao_s: None,
                        bateria_inicio_pct: l.bateria_pct,
                        bateria_min_pct: l.bateria_pct,
                        entrada_min_v: l.entrada_v,
                        interrompida: false,
                    });
                }
                "fim" => {
                    if let Some(q) = self.lista.back_mut() {
                        if q.fim.is_none() && !q.interrompida {
                            q.fim = Some(l.t);
                            q.duracao_s = Some(l.t.saturating_sub(q.inicio));
                            q.bateria_min_pct = q.bateria_min_pct.min(l.bateria_pct);
                            q.entrada_min_v = q.entrada_min_v.min(l.entrada_v);
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
                self.empurrar(Queda {
                    inicio: t,
                    fim: None,
                    duracao_s: None,
                    bateria_inicio_pct: bateria_pct,
                    bateria_min_pct: bateria_pct,
                    entrada_min_v: entrada_v,
                    interrompida: false,
                });
                self.pendentes.push(Linha {
                    t,
                    ev: "inicio".into(),
                    bateria_pct,
                    entrada_v,
                });
            }
        } else if self.em_curso() {
            let (min_bat, min_v) = match self.lista.back_mut() {
                Some(q) => {
                    q.fim = Some(t);
                    q.duracao_s = Some(t.saturating_sub(q.inicio));
                    (q.bateria_min_pct, q.entrada_min_v)
                }
                None => (bateria_pct, entrada_v),
            };
            // a linha `fim` leva os mínimos, para o resumo sobreviver a reinícios
            self.pendentes.push(Linha {
                t,
                ev: "fim".into(),
                bateria_pct: min_bat,
                entrada_v: min_v,
            });
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
