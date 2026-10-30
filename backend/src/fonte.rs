//! Origem dos dados: porta serial real ou simulador (para desenvolvimento e CI).

use crate::leitura::{Brutos, BASE, TAM_BLOCO};
use crate::protocolo;
use std::time::{Duration, Instant};

pub enum Fonte {
    Serial {
        caminho: String,
        id: u8,
        porta: Option<Box<dyn serialport::SerialPort>>,
    },
    Simulada(u32),
}

impl Fonte {
    pub fn serial(caminho: String, id: u8) -> Self {
        Fonte::Serial {
            caminho,
            id,
            porta: None,
        }
    }

    pub fn ler_tudo(&mut self) -> Result<Brutos, String> {
        match self {
            Fonte::Simulada(n) => {
                *n = n.wrapping_add(1);
                Ok(simulado(*n))
            }
            Fonte::Serial { caminho, id, porta } => {
                if porta.is_none() {
                    let p = serialport::new(caminho.as_str(), 2400)
                        .timeout(Duration::from_millis(150))
                        .open()
                        .map_err(|e| format!("não abriu {caminho}: {e}"))?;
                    *porta = Some(p);
                }
                let r = (|| {
                    let p = porta.as_mut().unwrap();
                    let bloco = ler(p.as_mut(), *id, BASE, TAM_BLOCO as u8)?;
                    let cal = ler(p.as_mut(), *id, 0x00F3, 1)?;
                    let cap = ler(p.as_mut(), *id, 0x0136, 1)?;
                    let osc = ler(p.as_mut(), *id, 0x0202, 2)?;
                    let mut b = [0u8; TAM_BLOCO];
                    b.copy_from_slice(&bloco);
                    Ok(Brutos {
                        bloco: b,
                        calibracao: cal[0],
                        cap_bateria: cap[0],
                        osc53: osc[0],
                        osc57: osc[1],
                    })
                })();
                if r.is_err() {
                    *porta = None; // reabre no próximo ciclo
                }
                r
            }
        }
    }
}

fn ler(p: &mut dyn serialport::SerialPort, id: u8, end: u16, n: u8) -> Result<Vec<u8>, String> {
    let _ = p.clear(serialport::ClearBuffer::Input);
    p.write_all(&protocolo::pedido_leitura(id, end, n))
        .map_err(|e| e.to_string())?;
    p.flush().map_err(|e| e.to_string())?;
    std::thread::sleep(Duration::from_millis(50));
    let esperado = n as usize + 1;
    let mut buf = Vec::with_capacity(esperado);
    let limite = Instant::now() + Duration::from_millis(900);
    let mut tmp = [0u8; 64];
    while buf.len() < esperado && Instant::now() < limite {
        match p.read(&mut tmp) {
            Ok(0) => {}
            Ok(k) => buf.extend_from_slice(&tmp[..k]),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    protocolo::dados_da_resposta(&buf, n as usize)
        .map(|d| d.to_vec())
        .map_err(|e| e.to_string())
}

fn simulado(n: u32) -> Brutos {
    let hex = "09000c71027dc9ff00d1c576020231ac00000186c201a2320076060c00cf";
    let mut bloco = [0u8; TAM_BLOCO];
    for (i, b) in bloco.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
    }
    bloco[0x8B - 0x80] = bloco[0x8B - 0x80].wrapping_add((n % 3) as u8);
    Brutos {
        bloco,
        calibracao: 0x34,
        cap_bateria: 0xB4,
        osc53: 0x68,
        osc57: 0x46,
    }
}
