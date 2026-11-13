//! Religar o nashome por Wake-on-LAN depois que a energia volta e a bateria recupera.
//!
//! Mesmo padrão do desligamento: o serviço **não executa nada** — grava um pedido num
//! arquivo, e um serviço do host (`nobreak-ligar-nashome.path` no Moon) manda o magic
//! packet. Assim o container continua `read_only` e sem privilégio.
//!
//! Condições (todas juntas, decididas em 13/nov/2026 com o Rafael):
//!   - houve uma queda que terminou há pouco (`NOBREAK_LIGAR_JANELA_H`, padrão 6 h);
//!     sem isso o serviço acordaria a máquina a cada reinício do Moon;
//!   - a rede ficou presente sem interrupção por `NOBREAK_LIGAR_ESTAVEL_S` (15 min),
//!     o que evita religar em energia que piscou de novo;
//!   - a bateria está em pelo menos `NOBREAK_LIGAR_BATERIA_PCT` (85 %), sem depender
//!     da precisão do medidor no fim da curva;
//!   - o mesmo fim de queda não gera pedido duas vezes.

use serde::Serialize;
use std::fmt::Write as _;

#[derive(Debug, Clone)]
pub struct Config {
    pub ligada: bool,
    pub estavel_s: u64,
    pub bateria_pct: f32,
    pub janela_s: u64,
    pub arquivo: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            ligada: false,
            estavel_s: 900,
            bateria_pct: 85.0,
            janela_s: 6 * 3600,
            arquivo: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Situacao {
    /// Pedido já gravado para a queda atual.
    pub ja_pediu: bool,
    /// Segundos de rede estável (None enquanto está na bateria).
    pub estavel_s: Option<u64>,
    /// Quanto falta para completar a janela de estabilidade.
    pub faltam_s: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evento {
    Ligar {
        motivo: String,
        erro: Option<String>,
    },
}

#[derive(Debug, Default)]
pub struct Acoes {
    pub cfg: Config,
    ol_desde: Option<u64>,
    /// Fim da queda já atendida: evita repetir o pedido.
    ja_pediu_para: Option<u64>,
}

impl Acoes {
    pub fn nova(cfg: Config) -> Self {
        Acoes {
            cfg,
            ..Default::default()
        }
    }

    pub fn situacao(&self, t: u64, em_queda: bool) -> Situacao {
        let estavel = if em_queda {
            None
        } else {
            self.ol_desde.map(|d| t.saturating_sub(d))
        };
        Situacao {
            ja_pediu: self.ja_pediu_para.is_some(),
            estavel_s: estavel,
            faltam_s: estavel.map(|s| self.cfg.estavel_s.saturating_sub(s)),
        }
    }

    pub fn avaliar(
        &mut self,
        t: u64,
        em_queda: bool,
        bateria_pct: f32,
        fim_ultima_queda: Option<u64>,
    ) -> Option<Evento> {
        if em_queda {
            self.ol_desde = None;
            return None;
        }
        if !self.cfg.ligada {
            return None;
        }
        let inicio = *self.ol_desde.get_or_insert(t);
        let estavel = t.saturating_sub(inicio);
        if estavel < self.cfg.estavel_s {
            return None;
        }
        let fim = fim_ultima_queda?;
        if t.saturating_sub(fim) > self.cfg.janela_s {
            return None;
        }
        if self.ja_pediu_para == Some(fim) {
            return None;
        }
        if bateria_pct < self.cfg.bateria_pct {
            return None;
        }
        self.ja_pediu_para = Some(fim);
        let motivo = format!(
            "rede estável há {} min e bateria em {bateria_pct:.0}% depois da queda de {}",
            estavel / 60,
            hora_local(fim)
        );
        let erro = match &self.cfg.arquivo {
            Some(caminho) => {
                let mut s = String::new();
                let _ = write!(
                    s,
                    "{{\"t\":{t},\"motivo\":\"{}\",\"bateria_pct\":{bateria_pct:.1},\"estavel_s\":{estavel}}}",
                    motivo.replace('"', "'")
                );
                s.push('\n');
                std::fs::write(caminho, s)
                    .err()
                    .map(|e| format!("não consegui escrever o pedido em {caminho}: {e}"))
            }
            None => Some(
                "NOBREAK_ARQUIVO_LIGAR_NASHOME não está definido: ninguém vai ligar a máquina"
                    .to_string(),
            ),
        };
        Some(Evento::Ligar { motivo, erro })
    }
}

/// Hora local no formato HH:MM, só para o texto do motivo.
fn hora_local(t: u64) -> String {
    // sem chrono: converte em UTC-3 (America/Sao_Paulo, sem horário de verão desde 2019)
    let s = t.saturating_sub(3 * 3600) % 86_400;
    format!("{:02}:{:02}", s / 3600, (s % 3600) / 60)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn cfg() -> Config {
        Config {
            ligada: true,
            estavel_s: 900,
            bateria_pct: 85.0,
            janela_s: 6 * 3600,
            arquivo: None,
        }
    }

    #[test]
    fn nao_liga_sem_queda_registrada() {
        let mut a = Acoes::nova(cfg());
        assert_eq!(a.avaliar(0, false, 100.0, None), None);
        assert_eq!(a.avaliar(2000, false, 100.0, None), None);
    }

    #[test]
    fn espera_estabilidade_e_bateria() {
        let mut a = Acoes::nova(cfg());
        let fim = 1000;
        // queda terminou em 1000; rede desde então
        assert_eq!(a.avaliar(1000, false, 40.0, Some(fim)), None);
        assert_eq!(a.avaliar(1500, false, 70.0, Some(fim)), None); // estável, bateria baixa
        assert_eq!(a.avaliar(1800, false, 90.0, Some(fim)), None); // 800 s, falta estabilidade
        assert_eq!(a.avaliar(1900, true, 90.0, Some(fim)), None); // caiu de novo: zera
        assert_eq!(a.avaliar(1905, false, 91.0, Some(fim)), None); // volta: reinicia a contagem
        assert_eq!(a.avaliar(2800, false, 95.0, Some(fim)), None); // 895 s: falta pouco
        assert!(matches!(
            a.avaliar(2805, false, 95.0, Some(fim)),
            Some(Evento::Ligar { .. })
        ));
        // não repete para a mesma queda
        assert_eq!(a.avaliar(3000, false, 99.0, Some(fim)), None);
        // e uma queda nova gera novo pedido
        assert_eq!(a.avaliar(3100, true, 80.0, Some(fim)), None);
        assert_eq!(a.avaliar(3105, false, 82.0, Some(fim)), None);
        assert!(matches!(
            a.avaliar(4100, false, 96.0, Some(4000)),
            Some(Evento::Ligar { .. })
        ));
    }

    #[test]
    fn queda_antiga_fora_da_janela_nao_liga() {
        let mut a = Acoes::nova(cfg());
        let fim = 0;
        assert_eq!(a.avaliar(7 * 3600, false, 100.0, Some(fim)), None);
    }

    #[test]
    fn desligado_nunca_age() {
        let mut a = Acoes::nova(Config {
            ligada: false,
            ..cfg()
        });
        assert_eq!(a.avaliar(5000, false, 100.0, Some(1000)), None);
    }

    #[test]
    fn avisa_quando_nao_ha_arquivo() {
        let mut a = Acoes::nova(cfg());
        a.avaliar(100, false, 40.0, Some(50)); // começa a contar rede estável
        match a.avaliar(5000, false, 100.0, Some(4000)) {
            Some(Evento::Ligar { erro, .. }) => {
                assert!(erro.unwrap().contains("NOBREAK_ARQUIVO_LIGAR_NASHOME"))
            }
            outro => panic!("esperava Ligar, veio {outro:?}"),
        }
    }

    #[test]
    fn escreve_o_pedido() {
        let dir = std::env::temp_dir().join("nobreak-acoes-teste");
        let _ = std::fs::create_dir_all(&dir);
        let arq = dir.join("ligar-nashome");
        let _ = std::fs::remove_file(&arq);
        let mut a = Acoes::nova(Config {
            arquivo: Some(arq.to_string_lossy().into_owned()),
            ..cfg()
        });
        a.avaliar(4000, true, 80.0, Some(4000)); // queda
        a.avaliar(4001, false, 80.0, Some(4000)); // rede volta: começa a contar
        match a.avaliar(5000, false, 96.0, Some(4000)) {
            Some(Evento::Ligar { erro: None, .. }) => {}
            outro => panic!("esperava Ligar sem erro, veio {outro:?}"),
        }
        let txt = std::fs::read_to_string(&arq).unwrap();
        assert!(txt.contains("\"estavel_s\":999"), "{txt}");
        assert!(txt.contains("\"bateria_pct\":96.0"), "{txt}");
        let _ = std::fs::remove_file(&arq);
    }
}
