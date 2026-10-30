//! Significado dos bits de EVENTOS (`0x90`), FALHAS (`0x91`) e USUARIO (`0x92`).
//! Obtido por engenharia reversa e conferido em quedas reais (ver `docs/protocolo.md`).
//! Só entram bits cujo significado é conhecido; os demais continuam aparecendo crus no painel.

use crate::leitura::Flags;
use serde::Serialize;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Gravidade {
    /// Só descreve o estado; não vira alarme.
    Info,
    Aviso,
    Erro,
}

pub struct Bit {
    pub registrador: u16,
    pub mascara: u8,
    pub codigo: &'static str,
    pub texto: &'static str,
    pub gravidade: Gravidade,
}

const fn bit(
    registrador: u16,
    mascara: u8,
    codigo: &'static str,
    texto: &'static str,
    gravidade: Gravidade,
) -> Bit {
    Bit {
        registrador,
        mascara,
        codigo,
        texto,
        gravidade,
    }
}

use Gravidade::{Aviso, Erro, Info};

pub const BITS: [Bit; 12] = [
    bit(0x91, 0x01, "bateria_baixa", "Bateria baixa", Aviso),
    bit(0x91, 0x02, "sobretemperatura", "Sobretemperatura", Erro),
    bit(0x91, 0x04, "fim_de_bateria", "Fim de bateria", Erro),
    bit(0x91, 0x08, "sobrecarga", "Sobrecarga na saída", Erro),
    bit(
        0x91,
        0x10,
        "tensao_saida_anormal",
        "Tensão de saída anormal",
        Erro,
    ),
    bit(
        0x91,
        0x20,
        "tensao_bateria_anormal",
        "Tensão de bateria anormal",
        Erro,
    ),
    bit(0x91, 0x40, "falha_inversor", "Falha do inversor", Erro),
    bit(
        0x90,
        0x04,
        "descarga_completa",
        "Descarga completa da bateria ativada",
        Aviso,
    ),
    bit(0x90, 0x08, "sem_rede", "Ausência de rede elétrica", Aviso),
    bit(0x90, 0x10, "rede_baixa", "Tensão de rede baixa", Aviso),
    bit(0x90, 0x20, "rede_alta", "Tensão de rede alta", Aviso),
    bit(
        0x92,
        0x02,
        "software_presente",
        "Software de monitoramento presente",
        Info,
    ),
];

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Alarme {
    pub codigo: &'static str,
    pub texto: &'static str,
    pub gravidade: Gravidade,
    pub registrador: u16,
    pub mascara: u8,
}

fn valor(f: &Flags, registrador: u16) -> u8 {
    match registrador {
        0x90 => f.eventos,
        0x91 => f.falhas,
        0x92 => f.usuario,
        _ => 0,
    }
}

/// Alarmes ligados agora (avisos e erros; bits só informativos ficam de fora).
pub fn ativos(f: &Flags) -> Vec<Alarme> {
    BITS.iter()
        .filter(|b| b.gravidade != Info && valor(f, b.registrador) & b.mascara != 0)
        .map(|b| Alarme {
            codigo: b.codigo,
            texto: b.texto,
            gravidade: b.gravidade,
            registrador: b.registrador,
            mascara: b.mascara,
        })
        .collect()
}

/// Texto para o log de mudanças: `+ Ausência de rede elétrica` (ligou), `− ...` (desligou).
/// Devolve `None` quando nenhum dos bits que mudaram tem significado conhecido.
pub fn descrever_mudanca(registrador: u16, de: u8, para: u8) -> Option<String> {
    let ligou = para & !de;
    let desligou = de & !para;
    let partes: Vec<String> = BITS
        .iter()
        .filter(|b| b.registrador == registrador)
        .filter_map(|b| {
            if ligou & b.mascara != 0 {
                Some(format!("+ {}", b.texto))
            } else if desligou & b.mascara != 0 {
                Some(format!("− {}", b.texto))
            } else {
                None
            }
        })
        .collect();
    if partes.is_empty() {
        None
    } else {
        Some(partes.join(", "))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn flags(eventos: u8, falhas: u8, usuario: u8) -> Flags {
        Flags {
            nvolatil: 0,
            eventos,
            falhas,
            usuario,
            gerais2: 0,
        }
    }

    #[test]
    fn sem_bits_sem_alarmes() {
        assert!(ativos(&flags(0, 0, 0)).is_empty());
        assert!(ativos(&flags(0, 0, 0x01)).is_empty());
    }

    #[test]
    fn queda_real_do_easy_pro() {
        // 0x48 foi o valor visto em EVENTOS nas quedas de 30/out/2026.
        let a = ativos(&flags(0x48, 0, 0));
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].codigo, "sem_rede");
    }

    #[test]
    fn fim_de_bateria_e_erro() {
        let a = ativos(&flags(0, 0x04, 0));
        assert_eq!(a[0].codigo, "fim_de_bateria");
        assert_eq!(a[0].gravidade, Gravidade::Erro);
    }

    #[test]
    fn software_presente_nao_e_alarme() {
        assert!(ativos(&flags(0, 0, 0x02)).is_empty());
    }

    #[test]
    fn descreve_mudancas() {
        assert_eq!(
            descrever_mudanca(0x90, 0x00, 0x48).as_deref(),
            Some("+ Ausência de rede elétrica")
        );
        assert_eq!(
            descrever_mudanca(0x90, 0x48, 0x00).as_deref(),
            Some("− Ausência de rede elétrica")
        );
        assert_eq!(descrever_mudanca(0x92, 0x01, 0x00), None);
        assert_eq!(descrever_mudanca(0x93, 0x00, 0xff), None);
    }

    #[test]
    fn nao_repete_mascara_por_registrador() {
        for (i, a) in BITS.iter().enumerate() {
            assert!(a.mascara.is_power_of_two());
            for b in &BITS[i + 1..] {
                assert!(
                    !(a.registrador == b.registrador && a.mascara == b.mascara),
                    "{} repetido",
                    a.codigo
                );
            }
        }
    }
}
