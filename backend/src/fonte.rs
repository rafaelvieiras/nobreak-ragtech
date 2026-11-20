//! Origem dos dados: porta serial real ou simulador (para desenvolvimento e CI).

use crate::leitura::{Bruto, Brutos, BASE, TAM_BLOCO};
use crate::protocolo;
use crate::quadri::{self, Quadro};
use std::time::{Duration, Instant};

/// Protocolo da porta serial, de `NOBREAK_PROTOCOLO`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocolo {
    /// Escuta sem enviar nada: se chegar quadro da Quadri, usa a Quadri; senão, família 10.
    Auto,
    /// Famílias 10/13/15 (Easy Pro, NEP...): pergunta e lê registradores.
    Familia10,
    /// Quadri: só escuta os quadros que o nobreak transmite sozinho.
    Quadri,
}

impl Protocolo {
    pub fn de_texto(s: &str) -> Option<Protocolo> {
        match s.trim().to_lowercase().as_str() {
            "" | "auto" => Some(Protocolo::Auto),
            "familia10" | "família10" | "10" => Some(Protocolo::Familia10),
            "quadri" => Some(Protocolo::Quadri),
            _ => None,
        }
    }

    pub fn nome(self) -> &'static str {
        match self {
            Protocolo::Auto => "auto",
            Protocolo::Familia10 => "familia10",
            Protocolo::Quadri => "quadri",
        }
    }
}

/// Quanto a detecção escuta antes de concluir que não é uma Quadri. Ela transmite um
/// quadro a cada ~1,33 s, então 3 s cobrem pelo menos dois quadros.
const ESCUTA_DETECCAO: Duration = Duration::from_millis(3000);
/// Prazo para chegar um quadro novo da Quadri a cada ciclo.
const ESCUTA_QUADRI: Duration = Duration::from_millis(3000);
/// Limite do que fica guardado esperando virar quadro.
const MAX_PENDENTE: usize = 4096;

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
        /// Protocolo configurado (`NOBREAK_PROTOCOLO`).
        protocolo: Protocolo,
        /// O que a escuta inicial descobriu (só em `Auto`). Refeito a cada reabertura.
        detectado: Option<Protocolo>,
        /// Bytes da Quadri que ainda não formaram um quadro.
        pendente: Vec<u8>,
    },
    /// Simulador da família 10.
    Simulada(u32),
    /// Simulador da Quadri (quadros reais, com uma queda a cada 60 ciclos).
    SimuladaQuadri(u32),
}

impl Fonte {
    pub fn serial_com(caminho: String, extras: Vec<String>, id: u8, protocolo: Protocolo) -> Self {
        Fonte::Serial {
            caminho,
            extras,
            id,
            porta: None,
            ultimo_bom: None,
            identidade: None,
            protocolo,
            detectado: None,
            pendente: Vec::new(),
        }
    }

    pub fn ler_tudo(&mut self) -> Result<Bruto, String> {
        match self {
            Fonte::Simulada(n) => {
                *n = n.wrapping_add(1);
                Ok(Bruto::Familia10(simulado(*n)))
            }
            Fonte::SimuladaQuadri(n) => {
                *n = n.wrapping_add(1);
                Ok(Bruto::Quadri(simulado_quadri(*n)))
            }
            Fonte::Serial {
                caminho,
                extras,
                id,
                porta,
                ultimo_bom,
                identidade,
                protocolo,
                detectado,
                pendente,
            } => {
                if porta.is_none() {
                    abrir(caminho, extras, identidade, ultimo_bom, porta)?;
                    pendente.clear();
                    *detectado = None;
                }
                let r = (|| {
                    let p = porta.as_mut().unwrap().as_mut();
                    let efetivo = match (*protocolo, *detectado) {
                        (Protocolo::Auto, Some(d)) => d,
                        (x, _) => x,
                    };
                    match efetivo {
                        Protocolo::Quadri => ler_quadri(p, pendente).map(Bruto::Quadri),
                        Protocolo::Familia10 => ler_familia10(p, *id).map(Bruto::Familia10),
                        // Detecção: só escuta. Uma Quadri nunca recebe o pedido da família 10.
                        Protocolo::Auto => match escutar_quadri(p, pendente, ESCUTA_DETECCAO)? {
                            Some(q) => {
                                *detectado = Some(Protocolo::Quadri);
                                println!("protocolo detectado: quadri (o nobreak transmite quadros sozinho)");
                                Ok(Bruto::Quadri(q))
                            }
                            None => {
                                *detectado = Some(Protocolo::Familia10);
                                println!("protocolo detectado: familia10 (nenhum quadro da Quadri em 3 s)");
                                ler_familia10(p, *id).map(Bruto::Familia10)
                            }
                        },
                    }
                })();
                if r.is_err() {
                    *porta = None; // reabre (e, em `Auto`, detecta de novo) no próximo ciclo
                }
                r
            }
        }
    }
}

fn ler_familia10(p: &mut dyn serialport::SerialPort, id: u8) -> Result<Brutos, String> {
    let bloco = ler(p, id, BASE, TAM_BLOCO as u8)?;
    let cal = ler(p, id, 0x00F3, 1)?;
    let cap = ler(p, id, 0x0136, 1)?;
    let osc = ler(p, id, 0x0202, 2)?;
    let mut b = [0u8; TAM_BLOCO];
    b.copy_from_slice(&bloco);
    Ok(Brutos {
        bloco: b,
        calibracao: cal[0],
        cap_bateria: cap[0],
        osc53: osc[0],
        osc57: osc[1],
    })
}

/// Lê até chegar um quadro válido da Quadri ou acabar o prazo. **Não escreve nada.**
fn escutar_quadri(
    p: &mut dyn serialport::SerialPort,
    pendente: &mut Vec<u8>,
    prazo: Duration,
) -> Result<Option<Quadro>, String> {
    let limite = Instant::now() + prazo;
    let mut tmp = [0u8; 128];
    loop {
        if let Some(q) = quadri::extrair(pendente) {
            return Ok(Some(q));
        }
        if Instant::now() >= limite {
            return Ok(None);
        }
        match p.read(&mut tmp) {
            Ok(0) => {}
            Ok(k) => {
                pendente.extend_from_slice(&tmp[..k]);
                if pendente.len() > MAX_PENDENTE {
                    let sobra = pendente.len() - MAX_PENDENTE;
                    pendente.drain(..sobra);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(e.to_string()),
        }
    }
}

/// Um quadro novo da Quadri: descarta o que acumulou desde o último ciclo e espera o próximo.
fn ler_quadri(
    p: &mut dyn serialport::SerialPort,
    pendente: &mut Vec<u8>,
) -> Result<Quadro, String> {
    let _ = p.clear(serialport::ClearBuffer::Input);
    pendente.clear();
    escutar_quadri(p, pendente, ESCUTA_QUADRI)?.ok_or_else(|| {
        format!(
            "nenhum quadro válido da Quadri em {} s",
            ESCUTA_QUADRI.as_secs()
        )
    })
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
    let ordem = ordem_de_abertura(
        ultimo_bom.as_deref(),
        caminho,
        extras,
        identidade.as_deref(),
    );
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

/// Pedido de leitura da família 10 e resposta. Só para a família 10: a Quadri não é perguntada.
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

/// Simulador da Quadri: quadros reais (rede normal e queda), com a bateria descendo na
/// queda e a entrada oscilando. A soma de verificação é refeita a cada quadro.
fn simulado_quadri(n: u32) -> Quadro {
    const NORMAL: &str = "5555ae0e09897263df6cac0027c2302710000004037bc101e400630f112708ee";
    const QUEDA: &str = "5555ae0e09897263df6cac00273028241100100c0307c301cf00630f112a07e5";
    let fase = n % 60;
    let queda = (30..42).contains(&fase);
    let hex = if queda { QUEDA } else { NORMAL };
    let mut b = [0u8; quadri::TAM_QUADRO];
    for (i, x) in b.iter_mut().enumerate() {
        *x = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap();
    }
    if queda {
        b[24] = b[24].saturating_sub(((fase - 30) * 2) as u8); // bateria descendo
    } else {
        b[21] = b[21].wrapping_add((n % 3) as u8); // entrada oscilando
    }
    let s = quadri::soma(&b).to_be_bytes();
    b[30] = s[0];
    b[31] = s[1];
    Quadro::de_bytes(&b).expect("soma refeita")
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn protocolo_de_texto() {
        assert_eq!(Protocolo::de_texto(""), Some(Protocolo::Auto));
        assert_eq!(Protocolo::de_texto("Quadri"), Some(Protocolo::Quadri));
        assert_eq!(Protocolo::de_texto("familia10"), Some(Protocolo::Familia10));
        assert_eq!(Protocolo::de_texto("10"), Some(Protocolo::Familia10));
        assert_eq!(Protocolo::de_texto("xyz"), None);
    }

    #[test]
    fn simulador_da_quadri_gera_quadros_validos_e_queda() {
        let mut fonte = Fonte::SimuladaQuadri(0);
        let mut viu_queda = false;
        for _ in 0..60 {
            match fonte.ler_tudo().unwrap() {
                Bruto::Quadri(q) => viu_queda |= q.sem_rede(),
                Bruto::Familia10(_) => panic!("simulador da Quadri deu família 10"),
            }
        }
        assert!(viu_queda);
    }

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
