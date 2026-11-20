//! Quadro dos nobreaks Ragtech Quadri (testado: Quadri 1200 VA, 115 V). Descoberto por
//! engenharia reversa para fins de interoperabilidade; ver `docs/quadri1200va.md`.
//!
//! O nobreak **transmite sozinho** um quadro de 32 bytes a cada ~1,33 s. Não há pedido:
//! monitorar é só escutar. Este módulo não monta nenhum quadro de envio.
//!
//! ```text
//! pos:  0  1  2 .. 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31
//!      55 55  constantes  flags          Vi Vo Cg Vb    Fq       Tp  soma
//! ```
//!
//! Soma de verificação: `S = soma(b0..=b29)`, `BE16(b30, b31) == (S + (S >> 8)) & 0xFFFF`.

use crate::leitura::{InfoModelo, Medidas};

pub const TAM_QUADRO: usize = 32;
pub const SINCRONISMO: [u8; 2] = [0x55, 0x55];

/// Posições dos bytes de medida (não entram no log de mudanças de bits).
pub const MEDIDAS: [usize; 8] = [
    POS_ENTRADA,
    POS_SAIDA,
    POS_CARGA,
    POS_BATERIA,
    POS_FREQUENCIA,
    POS_TEMPERATURA,
    30,
    31,
];

const POS_ENTRADA: usize = 21;
const POS_SAIDA: usize = 22;
const POS_CARGA: usize = 23;
const POS_BATERIA: usize = 24;
const POS_FREQUENCIA: usize = 26;
const POS_TEMPERATURA: usize = 29;
/// Byte 18, bit 4: liga no primeiro quadro sem rede e desliga no primeiro com rede.
pub const POS_SEM_REDE: usize = 18;
pub const MASCARA_SEM_REDE: u8 = 0x10;

/// Um quadro de 32 bytes com sincronismo e soma de verificação conferidos.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quadro(pub [u8; TAM_QUADRO]);

/// Soma de verificação dos bytes 0..=29, no formato dos bytes 30 e 31.
pub fn soma(bytes: &[u8]) -> u16 {
    let s: u32 = bytes[..30].iter().map(|&b| b as u32).sum();
    ((s + (s >> 8)) & 0xFFFF) as u16
}

impl Quadro {
    /// Aceita só um quadro completo, com `55 55` no início e soma de verificação certa.
    pub fn de_bytes(b: &[u8]) -> Option<Quadro> {
        if b.len() != TAM_QUADRO || b[..2] != SINCRONISMO {
            return None;
        }
        if soma(b) != u16::from_be_bytes([b[30], b[31]]) {
            return None;
        }
        let mut q = [0u8; TAM_QUADRO];
        q.copy_from_slice(b);
        Some(Quadro(q))
    }

    /// O nobreak está sem rede elétrica (na bateria).
    pub fn sem_rede(&self) -> bool {
        self.0[POS_SEM_REDE] & MASCARA_SEM_REDE != 0
    }

    pub fn hex(&self) -> String {
        self.0.iter().map(|x| format!("{x:02x}")).collect()
    }
}

/// Procura o primeiro quadro válido em `buf`, descarta tudo até o fim dele e o devolve.
/// Sem quadro completo, guarda só o que ainda pode virar um (a partir do último `55`).
pub fn extrair(buf: &mut Vec<u8>) -> Option<Quadro> {
    let mut i = 0;
    while i + 1 < buf.len() {
        if buf[i..i + 2] == SINCRONISMO {
            if i + TAM_QUADRO > buf.len() {
                buf.drain(..i); // pode ser o começo de um quadro: espera o resto
                return None;
            }
            if let Some(q) = Quadro::de_bytes(&buf[i..i + TAM_QUADRO]) {
                buf.drain(..i + TAM_QUADRO);
                return Some(q);
            }
        }
        i += 1;
    }
    // nenhum sincronismo completo: guarda o último byte (pode ser o primeiro `55`)
    let resto = buf.len().saturating_sub(1);
    buf.drain(..resto);
    None
}

/// Carga da bateria em %. O nobreak **não** manda esse número: ele é calculado pela tensão,
/// como faz o software oficial (≈ 0 % em 10,0 V e 100 % em 13,8 V). [hipótese]
pub fn carga_bateria_pct(bruto_vbat: u8) -> f32 {
    ((bruto_vbat as f32 - 161.7) * 1.625).clamp(0.0, 100.0)
}

pub fn decodificar(q: &Quadro, fator_vbat: f32) -> (InfoModelo, Medidas) {
    let b = &q.0;
    let medidas = Medidas {
        carga_bateria_pct: carga_bateria_pct(b[POS_BATERIA]),
        tensao_bateria_v: b[POS_BATERIA] as f32 * 0.0618 * fator_vbat,
        tensao_entrada_v: b[POS_ENTRADA] as f32 * 1.065,
        tensao_saida_v: b[POS_SAIDA] as f32 * 0.5892,
        // o quadro não traz corrente: a potência sai da carga em % e da potência nominal
        corrente_saida_a: None,
        // passos de 2,5 %; arredonda como o software oficial (2,5 → 3)
        carga_saida_pct: (b[POS_CARGA] as f32 * 2.5).round(),
        temperatura_c: b[POS_TEMPERATURA] as f32,
        // estimativa com dois pontos (99 → 59,9 Hz, 97 → 60,3 Hz)
        frequencia_hz: Some(79.7 - 0.2 * b[POS_FREQUENCIA] as f32),
        firmware: None,
        contagem_regressiva_s: 0,
    };
    let info = InfoModelo {
        indice: 0,
        nome: "Ragtech Quadri".to_string(),
        reconhecido: true,
    };
    (info, medidas)
}

/// Estado em notação NUT. `OB` pelo bit de rede; a tensão de entrada baixa serve de reforço.
/// Bateria baixa e sobrecarga ainda não foram vistas no quadro (o `LB` derivado continua valendo).
pub fn status_nut(q: &Quadro, m: &Medidas) -> String {
    let sem_rede = q.sem_rede() || m.tensao_entrada_v < 80.0;
    let mut s: Vec<&str> = vec![if sem_rede { "OB" } else { "OL" }];
    if !sem_rede && m.carga_bateria_pct < 99.0 {
        s.push("CHRG");
    }
    s.join(" ")
}

#[cfg(test)]
pub mod testes {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        s.split_whitespace()
            .map(|x| u8::from_str_radix(x, 16).unwrap())
            .collect()
    }

    pub fn quadro(s: &str) -> Quadro {
        Quadro::de_bytes(&hex(s)).expect("quadro de captura real com soma válida")
    }

    // Capturas reais (Quadri 1200 VA, 20/nov/2026). Entre parênteses, o que o software
    // oficial mostrava no mesmo instante.
    /// Rede normal (entrada 131 V, saída 113,7 V, bateria 14,1 V, 39 °C).
    pub const NORMAL: &str = "55 55 ae 0e 09 89 72 63 df 6c ac 00 27 c2 30 27 10 00 00 04 03 7b c1 01 e4 00 63 0f 11 27 08 ee";
    /// Servidor ligado (carga 10 %, bateria 14,2 V).
    const CARGA_10: &str = "55 55 ae 0e 09 89 72 63 df 6c ac 00 27 c2 30 27 10 00 00 04 03 7b c1 04 e5 00 63 0f 11 27 08 f2";
    /// No meio da queda de energia (entrada 0, saída 114,9 V, bateria 12,8 V e 74 %, 42 °C, carga 3 %).
    pub const QUEDA: &str = "55 55 ae 0e 09 89 72 63 df 6c ac 00 27 30 28 24 11 00 10 0c 03 07 c3 01 cf 00 63 0f 11 2a 07 e5";
    /// Primeiro quadro depois de tirar da tomada.
    const QUEDA_INICIO: &str = "55 55 ae 0e 09 89 72 63 df 6c ac 00 27 e0 28 24 11 00 10 0c 03 09 c5 02 d8 00 63 0f 11 2a 08 a4";
    /// Primeiro quadro com a rede de volta (o bit de rede já apagou).
    const VOLTA: &str = "55 55 ae 0e 09 89 72 63 df 6c ac 00 27 30 28 26 10 00 00 0c 03 7a c3 01 ce 00 63 0f 11 2c 08 4b";
    /// Recarregando depois da queda (bateria 13,4 V e 90 %).
    const RECARGA: &str = "55 55 ae 0e 09 89 72 63 df 6c ac 00 27 82 30 67 10 00 00 04 03 7b c1 03 d9 00 63 0f 11 2a 08 e8";

    fn perto(a: f32, b: f32, tol: f32) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn soma_confere_nas_capturas_reais() {
        for s in [NORMAL, CARGA_10, QUEDA, QUEDA_INICIO, VOLTA, RECARGA] {
            let b = hex(s);
            assert_eq!(soma(&b), u16::from_be_bytes([b[30], b[31]]), "{s}");
        }
    }

    #[test]
    fn rejeita_quadro_corrompido_ou_fora_de_sincronismo() {
        let mut b = hex(NORMAL);
        b[24] ^= 0x01;
        assert!(Quadro::de_bytes(&b).is_none());
        let mut c = hex(NORMAL);
        c[0] = 0xAA;
        assert!(Quadro::de_bytes(&c).is_none());
        assert!(Quadro::de_bytes(&hex(NORMAL)[..31]).is_none());
    }

    #[test]
    fn decodifica_rede_normal() {
        let (info, m) = decodificar(&quadro(NORMAL), 1.0);
        assert_eq!(info.nome, "Ragtech Quadri");
        assert!(
            perto(m.tensao_entrada_v, 131.0, 0.5),
            "{}",
            m.tensao_entrada_v
        );
        assert!(perto(m.tensao_saida_v, 113.7, 0.05), "{}", m.tensao_saida_v);
        assert!(
            perto(m.tensao_bateria_v, 14.1, 0.05),
            "{}",
            m.tensao_bateria_v
        );
        assert_eq!(m.temperatura_c, 39.0);
        assert_eq!(m.carga_saida_pct, 3.0); // 1 × 2,5 arredondado
        assert!(perto(m.frequencia_hz.unwrap(), 59.9, 0.05));
        assert_eq!(m.corrente_saida_a, None);
        assert_eq!(m.carga_bateria_pct, 100.0); // 14,1 V carregando passa de 100 %
    }

    #[test]
    fn decodifica_carga_e_bateria() {
        let (_, m) = decodificar(&quadro(CARGA_10), 1.0);
        assert_eq!(m.carga_saida_pct, 10.0);
        assert!(
            perto(m.tensao_bateria_v, 14.2, 0.05),
            "{}",
            m.tensao_bateria_v
        );
        let (_, r) = decodificar(&quadro(RECARGA), 1.0);
        assert!(
            perto(r.tensao_bateria_v, 13.4, 0.05),
            "{}",
            r.tensao_bateria_v
        );
        assert!(
            perto(r.carga_bateria_pct, 90.0, 1.0),
            "{}",
            r.carga_bateria_pct
        );
        assert_eq!(r.carga_saida_pct, 8.0); // 3 × 2,5
    }

    #[test]
    fn decodifica_queda_real() {
        let q = quadro(QUEDA);
        assert!(q.sem_rede());
        let (_, m) = decodificar(&q, 1.0);
        assert!(perto(m.tensao_saida_v, 114.9, 0.05), "{}", m.tensao_saida_v);
        assert!(
            perto(m.tensao_bateria_v, 12.8, 0.05),
            "{}",
            m.tensao_bateria_v
        );
        assert!(
            perto(m.carga_bateria_pct, 74.0, 1.0),
            "{}",
            m.carga_bateria_pct
        );
        assert_eq!(m.temperatura_c, 42.0);
        assert!(m.tensao_entrada_v < 10.0); // o nobreak manda ~7–9 sem rede
        assert_eq!(status_nut(&q, &m), "OB");
    }

    #[test]
    fn bit_de_rede_nas_transicoes() {
        assert!(!quadro(NORMAL).sem_rede());
        assert!(quadro(QUEDA_INICIO).sem_rede());
        assert!(!quadro(VOLTA).sem_rede());
        let (_, m) = decodificar(&quadro(RECARGA), 1.0);
        assert_eq!(status_nut(&quadro(RECARGA), &m), "OL CHRG");
        let (_, n) = decodificar(&quadro(NORMAL), 1.0);
        assert_eq!(status_nut(&quadro(NORMAL), &n), "OL");
    }

    #[test]
    fn fator_de_bateria_nao_muda_a_porcentagem() {
        let (_, a) = decodificar(&quadro(QUEDA), 1.0);
        let (_, b) = decodificar(&quadro(QUEDA), 2.0);
        assert!(perto(b.tensao_bateria_v, 2.0 * a.tensao_bateria_v, 0.01));
        assert_eq!(a.carga_bateria_pct, b.carga_bateria_pct);
    }

    #[test]
    fn extrai_quadros_do_fluxo() {
        // lixo antes, um `55 55` falso, quadro inteiro e o começo do próximo
        let mut buf = vec![0x00, 0x55, 0x55, 0x01];
        buf.extend(hex(NORMAL));
        buf.extend(&hex(QUEDA)[..10]);
        assert_eq!(extrair(&mut buf), Some(quadro(NORMAL)));
        assert_eq!(buf, hex(QUEDA)[..10].to_vec());
        assert_eq!(extrair(&mut buf), None); // incompleto: espera
        buf.extend(&hex(QUEDA)[10..]);
        assert_eq!(extrair(&mut buf), Some(quadro(QUEDA)));
        assert!(buf.is_empty());
    }

    #[test]
    fn extrair_descarta_lixo_sem_sincronismo() {
        let mut buf = vec![0x01, 0x02, 0x03, 0x55];
        assert_eq!(extrair(&mut buf), None);
        assert_eq!(buf, vec![0x55]);
        let mut corrompido = hex(NORMAL);
        corrompido[22] ^= 0xFF;
        let mut buf = corrompido.clone();
        buf.extend(hex(NORMAL));
        assert_eq!(extrair(&mut buf), Some(quadro(NORMAL)));
    }
}
