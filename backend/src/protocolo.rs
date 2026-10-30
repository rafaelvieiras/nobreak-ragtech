//! Quadro serial dos nobreaks Ragtech das famílias "10/13/15" (Easy Pro, Easy Way,
//! Easy Jet, NEP/Toro/Innergie, Save Digital). Descoberto por engenharia reversa
//! para fins de interoperabilidade; ver `docs/protocolo.md`.
//!
//! Pedido (6 bytes): `[A0|id] [cmd] [end_hi] [end_lo] [valor/tam] [cks]`
//! onde `cks = (end_hi + end_lo + valor) & 0xFF`.
//! Resposta: `[A0|id]` seguido dos bytes pedidos. Nibble alto `C` indica erro.

pub const CMD_LER: u8 = 0x04;

#[derive(Debug, PartialEq, Eq)]
pub enum ErroResposta {
    Curta { esperado: usize, recebido: usize },
    Recusada(u8),
    CabecalhoInvalido(u8),
}

impl std::fmt::Display for ErroResposta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Curta { esperado, recebido } => {
                write!(
                    f,
                    "resposta curta: esperava {esperado} bytes, recebi {recebido}"
                )
            }
            Self::Recusada(b) => write!(f, "nobreak recusou o comando (cabeçalho {b:#04x})"),
            Self::CabecalhoInvalido(b) => write!(f, "cabeçalho inesperado {b:#04x}"),
        }
    }
}

/// Soma de verificação do pedido: bytes 2..=4.
pub fn checksum(end_hi: u8, end_lo: u8, valor: u8) -> u8 {
    end_hi.wrapping_add(end_lo).wrapping_add(valor)
}

/// Monta um quadro de pedido de 6 bytes.
pub fn pedido(id: u8, cmd: u8, endereco: u16, valor: u8) -> [u8; 6] {
    let (hi, lo) = ((endereco >> 8) as u8, (endereco & 0xFF) as u8);
    [
        0xA0 | (id & 0x0F),
        cmd,
        hi,
        lo,
        valor,
        checksum(hi, lo, valor),
    ]
}

/// Pedido de leitura de `n` bytes a partir de `endereco`. Somente leitura.
pub fn pedido_leitura(id: u8, endereco: u16, n: u8) -> [u8; 6] {
    pedido(id, CMD_LER, endereco, n)
}

/// Valida a resposta e devolve os `n` bytes de dados (sem o cabeçalho).
pub fn dados_da_resposta(buf: &[u8], n: usize) -> Result<&[u8], ErroResposta> {
    let primeiro = *buf.first().ok_or(ErroResposta::Curta {
        esperado: n + 1,
        recebido: 0,
    })?;
    match primeiro & 0xF0 {
        0xC0 => return Err(ErroResposta::Recusada(primeiro)),
        0xA0 => {}
        _ => return Err(ErroResposta::CabecalhoInvalido(primeiro)),
    }
    if buf.len() < n + 1 {
        return Err(ErroResposta::Curta {
            esperado: n + 1,
            recebido: buf.len(),
        });
    }
    Ok(&buf[1..=n])
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn checksums_conferem_com_capturas_reais() {
        assert_eq!(
            pedido_leitura(0xA, 0x0080, 0x1E),
            [0xAA, 0x04, 0x00, 0x80, 0x1E, 0x9E]
        );
        assert_eq!(
            pedido_leitura(0xA, 0x00F3, 0x01),
            [0xAA, 0x04, 0x00, 0xF3, 0x01, 0xF4]
        );
    }

    #[test]
    fn resposta_valida() {
        let r = [0xAA, 1, 2, 3];
        assert_eq!(dados_da_resposta(&r, 3).unwrap(), &[1, 2, 3]);
    }

    #[test]
    fn resposta_recusada_e_curta() {
        assert!(matches!(
            dados_da_resposta(&[0xCA], 1),
            Err(ErroResposta::Recusada(_))
        ));
        assert!(matches!(
            dados_da_resposta(&[0xAA, 1], 3),
            Err(ErroResposta::Curta { .. })
        ));
        assert!(matches!(
            dados_da_resposta(&[0x55, 1], 1),
            Err(ErroResposta::CabecalhoInvalido(_))
        ));
        assert!(dados_da_resposta(&[], 1).is_err());
    }
}
