//! Decodifica os registradores da família 10/13/15 em medidas físicas.
//! Fórmulas obtidas por engenharia reversa (ver `docs/protocolo.md`).

use crate::modelos::{self, Modelo};
use serde::Serialize;

pub const BASE: u16 = 0x0080;
pub const TAM_BLOCO: usize = 30; // registradores 0x80..=0x9D

/// Bytes crus lidos do nobreak num ciclo de polling.
#[derive(Clone, Debug)]
pub struct Brutos {
    pub bloco: [u8; TAM_BLOCO],
    pub calibracao: u8,  // 0xF3
    pub cap_bateria: u8, // 0x136
    pub osc53: u8,       // 0x202
    pub osc57: u8,       // 0x203
}

impl Brutos {
    pub fn reg(&self, endereco: u16) -> u8 {
        self.bloco[(endereco - BASE) as usize]
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct Flags {
    pub nvolatil: u8,
    pub eventos: u8,
    pub falhas: u8,
    pub usuario: u8,
    pub gerais2: u8,
}

#[derive(Serialize, Clone, Debug)]
pub struct Medidas {
    pub carga_bateria_pct: f32,
    /// Tensão da bateria: registrador 0x8A × escala do modelo × `fator_vbat`.
    pub tensao_bateria_v: f32,
    pub tensao_entrada_v: f32,
    pub tensao_saida_v: f32,
    pub corrente_saida_a: f32,
    pub carga_saida_pct: f32,
    pub temperatura_c: f32,
    pub frequencia_hz: Option<f32>,
    pub firmware: f32,
    /// Contador regressivo de desligamento (registrador `0x98`), em segundos. `0` = parado.
    pub contagem_regressiva_s: u8,
}

#[derive(Serialize, Clone, Debug)]
pub struct InfoModelo {
    pub indice: u8,
    pub nome: String,
    pub reconhecido: bool,
}

pub fn decodificar(b: &Brutos, fator_vbat: f32) -> (InfoModelo, Medidas, Flags) {
    let idx = b.reg(0x9A);
    let m: Option<&Modelo> = modelos::buscar(idx);
    let (esc_vbat, esc_i, m220) = m
        .map(|m| (m.escala_vbat, m.escala_isaida, m.tensao_220))
        .unwrap_or((0.067, 1.0, false));
    let cal = if b.calibracao == 0 {
        1.0
    } else {
        b.calibracao as f32
    };

    let freq = if b.osc57 != b.osc53 {
        let fs = b.reg(0x97) as f32;
        let (o53, o57) = (b.osc53 as f32, b.osc57 as f32);
        Some((((fs - o53) * 4.0 / (o57 - o53)) + 53.0) * 0.9806 + 1.46)
    } else {
        None
    };

    let vbat_raw = b.reg(0x8A) as f32;
    let medidas = Medidas {
        carga_bateria_pct: (b.reg(0x87) as f32 * 0.393).min(100.0),
        tensao_bateria_v: vbat_raw * esc_vbat * fator_vbat,
        tensao_entrada_v: b.reg(0x8B) as f32 * 1.06,
        tensao_saida_v: b.reg(0x9D) as f32 * if m220 { 1.03 } else { 0.555 },
        corrente_saida_a: b.reg(0x8C) as f32 * esc_i / cal,
        carga_saida_pct: b.reg(0x8D) as f32,
        temperatura_c: b.reg(0x8E) as f32,
        frequencia_hz: freq,
        firmware: b.reg(0x9B) as f32 * 0.1,
        contagem_regressiva_s: b.reg(0x98),
    };
    let info = InfoModelo {
        indice: idx,
        nome: m
            .map(|m| m.nome.to_string())
            .unwrap_or_else(|| format!("desconhecido ({idx})")),
        reconhecido: m.is_some(),
    };
    let flags = Flags {
        nvolatil: b.reg(0x80),
        eventos: b.reg(0x90),
        falhas: b.reg(0x91),
        usuario: b.reg(0x92),
        gerais2: b.reg(0x95),
    };
    (info, medidas, flags)
}

/// Estado em notação NUT, a partir dos bits conhecidos (ver `alarmes.rs`).
/// `OB` quando o bit "ausência de rede" liga; a tensão de entrada baixa serve de reforço.
pub fn status_nut(m: &Medidas, f: &Flags) -> String {
    let sem_rede = f.eventos & 0x08 != 0 || m.tensao_entrada_v < 80.0;
    let mut s: Vec<&str> = vec![if sem_rede { "OB" } else { "OL" }];
    if f.falhas & 0x05 != 0 {
        s.push("LB"); // bateria baixa (bit 0) ou fim de bateria (bit 2)
    }
    if f.falhas & 0x08 != 0 {
        s.push("OVER");
    }
    if !sem_rede && m.carga_bateria_pct < 99.0 {
        s.push("CHRG");
    }
    s.join(" ")
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Captura real (30 bytes a partir de 0x80) de um Easy Pro 1200 VA TI, 29/out/2026.
    fn captura() -> Brutos {
        let hex = "09000c71027dc9ff00d1c576020231ac00000186c201a2320076060c00cf";
        let v: Vec<u8> = (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap())
            .collect();
        let mut bloco = [0u8; TAM_BLOCO];
        bloco.copy_from_slice(&v[..TAM_BLOCO]);
        Brutos {
            bloco,
            calibracao: 0x34,
            cap_bateria: 0xB4,
            osc53: 0x68,
            osc57: 0x46,
        }
    }

    #[test]
    fn decodifica_captura_real() {
        let (info, m, _) = decodificar(&captura(), 1.0);
        assert_eq!(info.indice, 6);
        assert!(info.nome.contains("1200 TI"));
        assert!((m.carga_bateria_pct - 100.0).abs() < 1.0);
        assert!(
            (m.tensao_entrada_v - 125.0).abs() < 3.0,
            "{}",
            m.tensao_entrada_v
        );
        assert!(
            (m.tensao_saida_v - 115.0).abs() < 3.0,
            "{}",
            m.tensao_saida_v
        );
        assert!(m.corrente_saida_a < 1.0);
    }

    #[test]
    fn fator_de_bateria() {
        let (_, a, _) = decodificar(&captura(), 1.0);
        let (_, b, _) = decodificar(&captura(), 2.0);
        assert!((b.tensao_bateria_v - 2.0 * a.tensao_bateria_v).abs() < 0.01);
    }

    #[test]
    fn status() {
        let (_, mut m, mut f) = decodificar(&captura(), 1.0);
        assert_eq!(status_nut(&m, &f), "OL");
        m.carga_bateria_pct = 84.0;
        assert_eq!(status_nut(&m, &f), "OL CHRG");
        // queda real: rede some e o bit 3 de EVENTOS liga (visto em 30/out/2026)
        m.tensao_entrada_v = 7.0;
        f.eventos = 0x48;
        assert_eq!(status_nut(&m, &f), "OB");
        // o bit sozinho basta, mesmo com a tensão ainda alta
        m.tensao_entrada_v = 100.0;
        assert_eq!(status_nut(&m, &f), "OB");
        f.eventos = 0;
        f.falhas = 0x04;
        assert_eq!(status_nut(&m, &f), "OL LB CHRG"); // bateria ainda em 84%
        m.carga_bateria_pct = 100.0;
        assert_eq!(status_nut(&m, &f), "OL LB");
        f.falhas = 0x08;
        assert_eq!(status_nut(&m, &f), "OL OVER");
    }

    #[test]
    fn contador_regressivo() {
        let mut b = captura();
        b.bloco[0x98 - 0x80] = 42;
        assert_eq!(decodificar(&b, 1.0).1.contagem_regressiva_s, 42);
    }
}
