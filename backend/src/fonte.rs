//! Origem dos dados: porta serial real ou simulador (para desenvolvimento e CI).

use crate::leitura::{Brutos, BASE, TAM_BLOCO};
use crate::protocolo;
use std::time::{Duration, Instant};

pub enum Fonte {
    Serial {
        /// Caminho principal, de `NOBREAK_PORTA`.
        caminho: String,
        /// Caminhos extras, de `NOBREAK_PORTAS_ALTERNATIVAS` (vírgula).
        extras: Vec<String>,
        id: u8,
        porta: Option<Box<dyn serialport::SerialPort>>,
        /// Caminho que abriu por último (evita re-testar um caminho morto todo ciclo).
        ultimo_bom: Option<String>,
        /// `idVendor:idProduct` do USB, aprendido na primeira abertura boa. É o que
        /// permite reencontrar o aparelho quando ele troca de nome.
        identidade: Option<String>,
    },
    Simulada(u32),
}

impl Fonte {
    pub fn serial_com(caminho: String, extras: Vec<String>, id: u8) -> Self {
        Fonte::Serial {
            caminho,
            extras,
            id,
            porta: None,
            ultimo_bom: None,
            identidade: None,
        }
    }

    pub fn ler_tudo(&mut self) -> Result<Brutos, String> {
        match self {
            Fonte::Simulada(n) => {
                *n = n.wrapping_add(1);
                Ok(simulado(*n))
            }
            Fonte::Serial {
                caminho,
                extras,
                id,
                porta,
                ultimo_bom,
                identidade,
            } => {
                if porta.is_none() {
                    abrir(
                        caminho,
                        extras,
                        identidade,
                        ultimo_bom,
                        porta,
                    )?;
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

/// Abre a porta tentando, nesta ordem: o último caminho que funcionou, o caminho
/// configurado, as alternativas e — se este aparelho já foi aberto alguma vez — qualquer
/// serial com o mesmo `idVendor:idProduct` do USB.
fn abrir(
    caminho: &str,
    extras: &[String],
    identidade: &mut Option<String>,
    ultimo_bom: &mut Option<String>,
    porta: &mut Option<Box<dyn serialport::SerialPort>>,
) -> Result<(), String> {
    let ordem = ordem_de_abertura(ultimo_bom.as_deref(), caminho, extras, identidade.as_deref());
    let mut falhas = Vec::new();
    for p in &ordem {
        match serialport::new(p.as_str(), 2400)
            .timeout(Duration::from_millis(150))
            .open()
        {
            Ok(nova) => {
                if identidade.is_none() {
                    *identidade = identidade_usb(p);
                }
                if p != caminho {
                    // aparece em `docker logs`/journal: o nome do aparelho mudou
                    println!("porta {p} (o caminho configurado {caminho} não abriu)");
                }
                *ultimo_bom = Some(p.clone());
                *porta = Some(nova);
                return Ok(());
            }
            Err(e) => falhas.push(format!("{p}: {e}")),
        }
    }
    Err(mensagem_falha(caminho, &falhas, identidade.as_deref()))
}

/// Ordem de abertura. Sem identidade conhecida não adivinha: abrir o aparelho de outra
/// pessoa (um coordenador Zigbee, por exemplo) e escrever um quadro nele é pior que ficar
/// desconectado.
fn ordem_de_abertura(
    ultimo_bom: Option<&str>,
    principal: &str,
    extras: &[String],
    identidade: Option<&str>,
) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    if let Some(u) = ultimo_bom {
        empurrar(&mut v, u);
    }
    empurrar(&mut v, principal);
    for e in extras {
        empurrar(&mut v, e);
    }
    if let Some(id) = identidade {
        for p in dispositivos_seriais() {
            if identidade_usb(&p).as_deref() == Some(id) {
                empurrar(&mut v, &p);
            }
        }
    }
    v
}

fn empurrar(v: &mut Vec<String>, p: &str) {
    if !p.is_empty() && !v.iter().any(|x| x == p) {
        v.push(p.to_string());
    }
}

/// Seriais que o sistema mostra agora: os `by-id` (nome estável do aparelho) e os
/// ttyACM/ttyUSB. Serve para reencontrar o nobreak depois de uma reenumeração USB.
fn dispositivos_seriais() -> Vec<String> {
    let mut v = Vec::new();
    if let Ok(rd) = std::fs::read_dir("/dev/serial/by-id") {
        let mut nomes: Vec<String> = rd
            .flatten()
            .map(|e| e.path().to_string_lossy().into_owned())
            .collect();
        nomes.sort();
        v.extend(nomes);
    }
    for pre in ["/dev/ttyACM", "/dev/ttyUSB"] {
        for n in 0..8 {
            let p = format!("{pre}{n}");
            if std::path::Path::new(&p).exists() {
                v.push(p);
            }
        }
    }
    v
}

/// `idVendor:idProduct` do USB por trás de um caminho serial, lido do sysfs ("04d8:000a").
fn identidade_usb(caminho: &str) -> Option<String> {
    let tty = std::fs::canonicalize(caminho)
        .ok()?
        .file_name()?
        .to_str()?
        .to_owned();
    let base = format!("/sys/class/tty/{tty}/device");
    // ttyACM: o link `device` aponta para a interface (1-1.5:1.0) e o idVendor fica no pai.
    for sub in ["", "..", "../.."] {
        let d = if sub.is_empty() {
            base.clone()
        } else {
            format!("{base}/{sub}")
        };
        let ler = |n: &str| {
            std::fs::read_to_string(format!("{d}/{n}"))
                .ok()
                .map(|s| s.trim().to_string())
        };
        if let (Some(vid), Some(pid)) = (ler("idVendor"), ler("idProduct")) {
            return Some(format!("{vid}:{pid}"));
        }
    }
    None
}

/// Mensagem de erro com o motivo do caminho configurado e, quando existe, a pista de que
/// o aparelho pode ter trocado de nome (o que mais aconteceu em campo).
fn mensagem_falha(caminho: &str, falhas: &[String], identidade: Option<&str>) -> String {
    let prefixo = format!("{caminho}:");
    let motivo = falhas
        .iter()
        .find(|f| f.starts_with(&prefixo))
        .or_else(|| falhas.first())
        .map(String::as_str)
        .unwrap_or(caminho);
    let mut m = format!("não abriu {motivo}");
    if let Some(id) = identidade {
        m += &format!(" — o aparelho da última leitura (USB {id}) não respondeu em nenhum caminho");
    }
    let outros: Vec<String> = dispositivos_seriais()
        .into_iter()
        .filter(|p| p != caminho)
        .take(4)
        .collect();
    if !outros.is_empty() {
        m += &format!(
            " (existe: {} — veja NOBREAK_PORTA e NOBREAK_PORTAS_ALTERNATIVAS)",
            outros.join(", ")
        );
    }
    m
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

/// Simulador: a cada 60 ciclos, 12 ciclos de queda de energia (rede some, bateria desce,
/// bits de EVENTOS/FALHAS acendem e o NVOLATIL fica marcado depois). Só para testar o painel.
fn simulado(n: u32) -> Brutos {
    let hex = "09000c71027dc9ff00d1c576020231ac00000186c201a2320076060c00cf";
    let mut bloco = [0u8; TAM_BLOCO];
    for (i, b) in bloco.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
    }
    let fase = n % 60;
    let queda = (30..42).contains(&fase);
    bloco[0x8B - 0x80] = bloco[0x8B - 0x80].wrapping_add((n % 3) as u8);
    if queda {
        bloco[0x8B - 0x80] = 0; // sem rede
        bloco[0x90 - 0x80] |= 0x48; // sem rede + bit 6 (visto nas quedas reais)
        bloco[0x92 - 0x80] &= !0x01;
        if fase >= 38 {
            bloco[0x91 - 0x80] |= 0x01; // bateria baixa no fim da queda
        }
        bloco[0x87 - 0x80] = bloco[0x87 - 0x80].saturating_sub(((fase - 30) * 2) as u8);
    } else if fase >= 42 {
        bloco[0] |= 0x02; // marca que houve queda
    }
    Brutos {
        bloco,
        calibracao: 0x34,
        cap_bateria: 0xB4,
        osc53: 0x68,
        osc57: 0x46,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ordem_comeca_pelo_ultimo_bom_e_nao_repete() {
        let extras = vec!["/dev/ttyUSB0".to_string(), "/dev/ttyACM0".to_string()];
        let v = ordem_de_abertura(Some("/dev/ttyACM1"), "/dev/ttyACM0", &extras, None);
        assert_eq!(v, ["/dev/ttyACM1", "/dev/ttyACM0", "/dev/ttyUSB0"]);
    }

    #[test]
    fn sem_identidade_nao_vasculha_o_sistema() {
        // Sem ter aberto o aparelho nenhuma vez, a lista é só o que foi configurado:
        // não vale abrir um serial alheio (coordenador Zigbee, modem) e escrever nele.
        let v = ordem_de_abertura(None, "/dev/nao-existe", &[], None);
        assert_eq!(v, ["/dev/nao-existe"]);
    }

    #[test]
    fn identidade_de_caminho_inexistente_e_nula() {
        assert_eq!(identidade_usb("/dev/nao-existe-nobreak"), None);
    }

    #[test]
    fn mensagem_lembra_o_caminho_configurado_e_o_que_existe() {
        let falhas = vec![
            "/dev/ttyACM0: No such device or address".to_string(),
            "/dev/ttyUSB0: No such file or directory".to_string(),
        ];
        let m = mensagem_falha("/dev/ttyACM0", &falhas, Some("04d8:000a"));
        assert!(
            m.starts_with("não abriu /dev/ttyACM0: No such device or address"),
            "{m}"
        );
        assert!(m.contains("04d8:000a"), "{m}");
    }
}
