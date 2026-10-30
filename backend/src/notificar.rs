//! Notificações: detecta mudanças (queda de energia, volta da rede, alarmes novos, perda de
//! comunicação, avisos de saúde da bateria) e as envia a um webhook HTTP.
//!
//! O envio roda em outra thread, com fila curta e novas tentativas, e nunca trava a leitura do
//! nobreak. O cliente HTTP é mínimo (só `http://`, sem TLS) para não pesar no binário.

use crate::alarmes::{Alarme, Gravidade};
use crate::bateria::Aviso;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::time::Duration;

/// Eventos que o detector sabe gerar (valores aceitos em `NOBREAK_NOTIFICAR_EVENTOS`).
pub const EVENTOS: [&str; 6] = [
    "queda",
    "rede_voltou",
    "alarme",
    "desconectado",
    "reconectado",
    "saude",
];

const ESPERA_ALARME_S: u64 = 600;
const ESPERA_SAUDE_S: u64 = 6 * 3600;
const TENTATIVAS: [Duration; 2] = [Duration::from_secs(2), Duration::from_secs(10)];
const TEMPO_LIMITE: Duration = Duration::from_secs(5);

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Notif {
    pub evento: &'static str,
    pub gravidade: Gravidade,
    pub titulo: String,
    pub texto: String,
    /// Segundos Unix.
    pub t: u64,
}

// ---------------------------------------------------------------- detector

/// O que o detector precisa saber de uma leitura.
pub struct Leitura<'a> {
    pub em_queda: bool,
    pub bateria_pct: f32,
    pub alarmes: &'a [Alarme],
    /// Duração da última queda terminada, se houver.
    pub duracao_ultima_queda_s: Option<u64>,
}

pub struct Detector {
    nome: String,
    filtro: Option<HashSet<String>>,
    em_queda: bool,
    conectado: bool,
    alarmes: HashSet<&'static str>,
    avisos: HashSet<&'static str>,
    /// Último envio por chave (`alarme:codigo`, `saude:codigo`), para não repetir.
    ultimo: HashMap<String, u64>,
}

impl Detector {
    pub fn novo(nome: &str, filtro: Option<HashSet<String>>) -> Self {
        Detector {
            nome: nome.to_string(),
            filtro,
            em_queda: false,
            conectado: true,
            alarmes: HashSet::new(),
            avisos: HashSet::new(),
            ultimo: HashMap::new(),
        }
    }

    fn criar(
        &self,
        t: u64,
        evento: &'static str,
        g: Gravidade,
        titulo: &str,
        texto: String,
    ) -> Option<Notif> {
        if let Some(f) = &self.filtro {
            if !f.contains(evento) {
                return None;
            }
        }
        Some(Notif {
            evento,
            gravidade: g,
            titulo: format!("{}: {titulo}", self.nome),
            texto,
            t,
        })
    }

    /// Passou tempo suficiente desde o último envio com esta chave? Se sim, marca agora.
    fn livre(&mut self, chave: String, t: u64, espera: u64) -> bool {
        match self.ultimo.get(&chave) {
            Some(u) if t.saturating_sub(*u) < espera => false,
            _ => {
                self.ultimo.insert(chave, t);
                true
            }
        }
    }

    pub fn leitura(&mut self, t: u64, l: &Leitura) -> Vec<Notif> {
        let mut v = Vec::new();
        if l.em_queda && !self.em_queda {
            v.extend(self.criar(
                t,
                "queda",
                Gravidade::Aviso,
                "falta de energia",
                format!("O nobreak passou para a bateria ({:.0}%).", l.bateria_pct),
            ));
        } else if !l.em_queda && self.em_queda {
            let dur = match l.duracao_ultima_queda_s {
                Some(s) if s >= 90 => format!(" depois de {} min", s / 60),
                Some(s) => format!(" depois de {s} s"),
                None => String::new(),
            };
            v.extend(self.criar(
                t,
                "rede_voltou",
                Gravidade::Info,
                "energia voltou",
                format!(
                    "A rede elétrica voltou{dur}. Bateria em {:.0}%.",
                    l.bateria_pct
                ),
            ));
        }
        self.em_queda = l.em_queda;

        let agora: HashSet<&'static str> = l.alarmes.iter().map(|a| a.codigo).collect();
        for a in l.alarmes {
            // a falta de rede já tem o aviso de queda
            if a.codigo == "sem_rede" || self.alarmes.contains(a.codigo) {
                continue;
            }
            if !self.livre(format!("alarme:{}", a.codigo), t, ESPERA_ALARME_S) {
                continue;
            }
            v.extend(self.criar(
                t,
                "alarme",
                a.gravidade,
                a.texto,
                format!("{} (bateria {:.0}%).", a.texto, l.bateria_pct),
            ));
        }
        self.alarmes = agora;
        v
    }

    pub fn conexao(&mut self, t: u64, conectado: bool, motivo: &str) -> Vec<Notif> {
        let mut v = Vec::new();
        if conectado != self.conectado {
            if conectado {
                v.extend(self.criar(
                    t,
                    "reconectado",
                    Gravidade::Info,
                    "comunicação restabelecida",
                    "O serviço voltou a ler o nobreak.".into(),
                ));
            } else {
                v.extend(self.criar(
                    t,
                    "desconectado",
                    Gravidade::Erro,
                    "sem comunicação",
                    format!("O serviço não consegue ler o nobreak: {motivo}"),
                ));
            }
        }
        self.conectado = conectado;
        v
    }

    pub fn saude(&mut self, t: u64, avisos: &[Aviso]) -> Vec<Notif> {
        let mut v = Vec::new();
        let agora: HashSet<&'static str> = avisos.iter().map(|a| a.codigo).collect();
        for a in avisos {
            if self.avisos.contains(a.codigo) {
                continue;
            }
            if !self.livre(format!("saude:{}", a.codigo), t, ESPERA_SAUDE_S) {
                continue;
            }
            v.extend(self.criar(t, "saude", a.gravidade, "saúde da bateria", a.texto.clone()));
        }
        self.avisos = agora;
        v
    }
}

// ------------------------------------------------------------- cliente HTTP

#[derive(Clone, Debug, PartialEq)]
pub struct Url {
    pub host_porta: String,
    pub caminho: String,
}

/// Aceita só `http://host[:porta][/caminho]`.
pub fn parse_url(s: &str) -> Result<Url, String> {
    let s = s.trim();
    if s.starts_with("https://") {
        return Err(
            "https não é suportado (o cliente não tem TLS); use http:// ou um proxy local".into(),
        );
    }
    let resto = s
        .strip_prefix("http://")
        .ok_or("a URL precisa começar com http://")?;
    let (hp, caminho) = match resto.find('/') {
        Some(i) => (&resto[..i], &resto[i..]),
        None => (resto, "/"),
    };
    if hp.is_empty() || hp.contains(char::is_whitespace) || caminho.contains(char::is_whitespace) {
        return Err("URL inválida".into());
    }
    let tem_porta = hp.rsplit(']').next().is_some_and(|x| x.contains(':'));
    let host_porta = if tem_porta {
        hp.to_string()
    } else {
        format!("{hp}:80")
    };
    Ok(Url {
        host_porta,
        caminho: caminho.to_string(),
    })
}

/// `Nome: valor`, sem quebras de linha (evita injeção de cabeçalho).
pub fn parse_cabecalho(s: &str) -> Result<(String, String), String> {
    let (n, v) = s.split_once(':').ok_or("use o formato \"Nome: valor\"")?;
    let (n, v) = (n.trim(), v.trim());
    let ok = |x: &str| !x.contains(['\r', '\n', '\0']);
    if n.is_empty() || !ok(n) || !ok(v) || n.contains(char::is_whitespace) {
        return Err("cabeçalho inválido".into());
    }
    Ok((n.to_string(), v.to_string()))
}

/// Envia um POST e devolve o código de status.
pub fn postar(
    url: &Url,
    cabecalhos: &[(String, String)],
    tipo: &str,
    corpo: &[u8],
) -> Result<u16, String> {
    let addr = url
        .host_porta
        .to_socket_addrs()
        .map_err(|e| format!("não resolveu {}: {e}", url.host_porta))?
        .next()
        .ok_or("sem endereço")?;
    let mut s =
        TcpStream::connect_timeout(&addr, TEMPO_LIMITE).map_err(|e| format!("conexão: {e}"))?;
    let _ = s.set_read_timeout(Some(TEMPO_LIMITE));
    let _ = s.set_write_timeout(Some(TEMPO_LIMITE));
    let mut req = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: nobreak-ragtech\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\nConnection: close\r\n",
        url.caminho,
        url.host_porta,
        corpo.len()
    );
    for (n, v) in cabecalhos {
        req += &format!("{n}: {v}\r\n");
    }
    req += "\r\n";
    s.write_all(req.as_bytes())
        .and_then(|_| s.write_all(corpo))
        .and_then(|_| s.flush())
        .map_err(|e| format!("envio: {e}"))?;
    let mut linha = String::new();
    BufReader::new(s.take(4096))
        .read_line(&mut linha)
        .map_err(|e| format!("resposta: {e}"))?;
    let codigo: u16 = linha
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| format!("resposta inesperada: {}", linha.trim()))?;
    if (200..300).contains(&codigo) {
        Ok(codigo)
    } else {
        Err(format!("o destino respondeu HTTP {codigo}"))
    }
}

// ------------------------------------------------------------------ envio

/// Formato do corpo.
#[derive(Clone, Debug, PartialEq)]
pub enum Formato {
    /// JSON do próprio serviço (`Notif`).
    Json,
    /// Publicação JSON do ntfy (`POST /` com `topic`, `title`, `message`, `priority`, `tags`).
    Ntfy { topico: String },
}

/// Para onde e como enviar.
#[derive(Clone, Debug)]
pub struct Destino {
    pub nome: &'static str,
    pub url: Url,
    pub cabecalhos: Vec<(String, String)>,
    pub formato: Formato,
}

/// Prioridade do ntfy: 3 padrão, 4 alta, 5 urgente.
fn prioridade_ntfy(n: &Notif) -> u8 {
    match n.gravidade {
        Gravidade::Info => 3,
        Gravidade::Aviso => 4,
        Gravidade::Erro => 5,
    }
}

fn tags_ntfy(n: &Notif) -> Vec<&'static str> {
    match n.evento {
        "queda" => vec!["zap", "warning"],
        "rede_voltou" => vec!["electric_plug", "white_check_mark"],
        "alarme" => vec!["rotating_light"],
        "desconectado" => vec!["x"],
        "reconectado" => vec!["white_check_mark"],
        "saude" => vec!["battery"],
        _ => vec!["bell"],
    }
}

impl Destino {
    fn corpo(&self, n: &Notif) -> Result<(&'static str, Vec<u8>), String> {
        match &self.formato {
            Formato::Json => Ok((
                "application/json",
                serde_json::to_vec(n).map_err(|e| e.to_string())?,
            )),
            Formato::Ntfy { topico } => Ok((
                "application/json",
                serde_json::to_vec(&serde_json::json!({
                    "topic": topico,
                    "title": n.titulo,
                    "message": n.texto,
                    "priority": prioridade_ntfy(n),
                    "tags": tags_ntfy(n),
                }))
                .map_err(|e| e.to_string())?,
            )),
        }
    }

    pub fn enviar(&self, n: &Notif) -> Result<u16, String> {
        let (tipo, corpo) = self.corpo(n)?;
        postar(&self.url, &self.cabecalhos, tipo, &corpo)
    }
}

/// Envia com novas tentativas (2 s, depois 10 s).
fn enviar_com_tentativas(d: &Destino, n: &Notif, esperas: &[Duration]) -> Result<u16, String> {
    let mut r = d.enviar(n);
    for e in esperas {
        if r.is_ok() {
            break;
        }
        std::thread::sleep(*e);
        r = d.enviar(n);
    }
    r
}

fn laco(rx: Receiver<Notif>, d: Destino) {
    for n in rx {
        if let Err(e) = enviar_com_tentativas(&d, &n, &TENTATIVAS) {
            eprintln!("notificação '{}' ({}) não enviada: {e}", n.evento, d.nome);
        }
    }
}

/// Detector mais uma fila e uma thread por destino (um destino fora do ar não atrasa o outro).
pub struct Notificador {
    pub detector: Detector,
    filas: Vec<(&'static str, SyncSender<Notif>)>,
}

impl Notificador {
    pub fn iniciar(detector: Detector, destinos: Vec<Destino>) -> Self {
        let filas = destinos
            .into_iter()
            .map(|d| {
                let (tx, rx) = sync_channel(32);
                let nome = d.nome;
                std::thread::Builder::new()
                    .name(format!("notificar-{nome}"))
                    .stack_size(256 * 1024)
                    .spawn(move || laco(rx, d))
                    .expect("thread de notificação");
                (nome, tx)
            })
            .collect();
        Notificador { detector, filas }
    }

    /// Põe na fila de cada destino. Se a fila encheu (destino fora do ar por muito tempo), descarta.
    pub fn enviar(&self, v: Vec<Notif>) {
        for n in v {
            for (nome, tx) in &self.filas {
                if tx.try_send(n.clone()).is_err() {
                    eprintln!("fila de notificações ({nome}) cheia: descartando");
                }
            }
        }
    }
}

/// Lê `NOBREAK_NOTIFICAR_EVENTOS` (lista separada por vírgula). Vazio = todos.
pub fn parse_filtro(s: &str) -> Result<Option<HashSet<String>>, String> {
    let v: Vec<String> = s
        .split(',')
        .map(|x| x.trim().to_lowercase())
        .filter(|x| !x.is_empty())
        .collect();
    if v.is_empty() {
        return Ok(None);
    }
    if let Some(x) = v.iter().find(|x| !EVENTOS.contains(&x.as_str())) {
        return Err(format!(
            "evento desconhecido '{x}' (válidos: {})",
            EVENTOS.join(", ")
        ));
    }
    Ok(Some(v.into_iter().collect()))
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::net::TcpListener;

    fn alarme(codigo: &'static str, g: Gravidade) -> Alarme {
        Alarme {
            codigo,
            texto: "Texto",
            gravidade: g,
            registrador: 0x91,
            mascara: 1,
        }
    }

    fn l<'a>(em_queda: bool, pct: f32, alarmes: &'a [Alarme]) -> Leitura<'a> {
        Leitura {
            em_queda,
            bateria_pct: pct,
            alarmes,
            duracao_ultima_queda_s: Some(300),
        }
    }

    fn evs(v: &[Notif]) -> Vec<&'static str> {
        v.iter().map(|n| n.evento).collect()
    }

    #[test]
    fn queda_e_volta_da_rede_uma_vez_cada() {
        let mut d = Detector::novo("Nobreak", None);
        assert!(d.leitura(0, &l(false, 100.0, &[])).is_empty());
        assert_eq!(evs(&d.leitura(10, &l(true, 98.0, &[]))), ["queda"]);
        assert!(d.leitura(15, &l(true, 95.0, &[])).is_empty());
        let v = d.leitura(300, &l(false, 70.0, &[]));
        assert_eq!(evs(&v), ["rede_voltou"]);
        assert!(v[0].texto.contains("5 min"));
        assert!(v[0].titulo.starts_with("Nobreak: "));
        assert!(d.leitura(305, &l(false, 70.0, &[])).is_empty());
    }

    #[test]
    fn alarme_novo_avisa_uma_vez_e_respeita_a_espera() {
        let mut d = Detector::novo("N", None);
        let bat = [alarme("bateria_baixa", Gravidade::Aviso)];
        assert_eq!(
            evs(&d.leitura(0, &l(true, 20.0, &bat))),
            ["queda", "alarme"]
        );
        assert!(d.leitura(5, &l(true, 19.0, &bat)).is_empty());
        // some e volta em 1 min: dentro da espera, não repete
        assert!(d.leitura(10, &l(true, 19.0, &[])).is_empty());
        assert!(d.leitura(60, &l(true, 18.0, &bat)).is_empty());
        // volta depois da espera: avisa
        assert!(d.leitura(70, &l(true, 18.0, &[])).is_empty());
        assert_eq!(evs(&d.leitura(700, &l(true, 17.0, &bat))), ["alarme"]);
    }

    #[test]
    fn sem_rede_nao_duplica_o_aviso_de_queda() {
        let mut d = Detector::novo("N", None);
        let a = [alarme("sem_rede", Gravidade::Aviso)];
        assert_eq!(evs(&d.leitura(0, &l(true, 100.0, &a))), ["queda"]);
    }

    #[test]
    fn conexao_perdida_e_restabelecida() {
        let mut d = Detector::novo("N", None);
        assert!(d.conexao(0, true, "").is_empty());
        let v = d.conexao(10, false, "timeout");
        assert_eq!(evs(&v), ["desconectado"]);
        assert!(v[0].texto.contains("timeout"));
        assert_eq!(v[0].gravidade, Gravidade::Erro);
        assert!(d.conexao(15, false, "timeout").is_empty());
        assert_eq!(evs(&d.conexao(20, true, "")), ["reconectado"]);
    }

    #[test]
    fn saude_avisa_so_o_que_apareceu() {
        let mut d = Detector::novo("N", None);
        let a = Aviso {
            codigo: "autonomia_curta",
            texto: "Autonomia de 5 min".into(),
            gravidade: Gravidade::Aviso,
        };
        assert_eq!(evs(&d.saude(0, std::slice::from_ref(&a))), ["saude"]);
        assert!(d.saude(60, std::slice::from_ref(&a)).is_empty());
        // some e volta dentro de 6 h: não repete
        assert!(d.saude(120, &[]).is_empty());
        assert!(d.saude(180, std::slice::from_ref(&a)).is_empty());
        assert!(d.saude(120, &[]).is_empty());
        assert_eq!(evs(&d.saude(7 * 3600, std::slice::from_ref(&a))), ["saude"]);
    }

    #[test]
    fn filtro_de_eventos() {
        let f = parse_filtro("queda, Rede_Voltou").unwrap();
        let mut d = Detector::novo("N", f);
        let bat = [alarme("bateria_baixa", Gravidade::Aviso)];
        assert_eq!(evs(&d.leitura(0, &l(true, 50.0, &bat))), ["queda"]);
        assert!(d.conexao(1, false, "x").is_empty());
        assert_eq!(parse_filtro(""), Ok(None));
        assert!(parse_filtro("queda,xyz").is_err());
    }

    #[test]
    fn url_e_cabecalho() {
        let u = parse_url("http://ha.local:8123/api/webhook/abc").unwrap();
        assert_eq!(
            (u.host_porta.as_str(), u.caminho.as_str()),
            ("ha.local:8123", "/api/webhook/abc")
        );
        let u = parse_url("http://10.0.0.5").unwrap();
        assert_eq!(
            (u.host_porta.as_str(), u.caminho.as_str()),
            ("10.0.0.5:80", "/")
        );
        assert_eq!(parse_url("http://[::1]/x").unwrap().host_porta, "[::1]:80");
        assert_eq!(parse_url("http://[::1]:9/x").unwrap().host_porta, "[::1]:9");
        assert!(parse_url("https://x.com/a").unwrap_err().contains("TLS"));
        assert!(parse_url("ftp://x").is_err());
        assert!(parse_url("http://").is_err());
        assert_eq!(
            parse_cabecalho("Authorization: Bearer abc").unwrap(),
            ("Authorization".into(), "Bearer abc".into())
        );
        assert!(parse_cabecalho("sem dois pontos").is_err());
        assert!(parse_cabecalho("X: a\r\nEvil: 1").is_err());
    }

    /// Servidor de um pedido só: devolve o pedido bruto e responde com `status`.
    fn servidor(status: &'static str) -> (String, std::thread::JoinHandle<String>) {
        let ln = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/hook?x=1", ln.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let (mut s, _) = ln.accept().unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            let fim = loop {
                let n = s.read(&mut tmp).unwrap();
                buf.extend_from_slice(&tmp[..n]);
                let txt = String::from_utf8_lossy(&buf).to_string();
                if let Some(i) = txt.find("\r\n\r\n") {
                    let cl: usize = txt
                        .lines()
                        .find_map(|l| l.strip_prefix("Content-Length: "))
                        .and_then(|x| x.trim().parse().ok())
                        .unwrap_or(0);
                    if buf.len() >= i + 4 + cl {
                        break txt;
                    }
                }
                if n == 0 {
                    break txt;
                }
            };
            let _ =
                s.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\n\r\n").as_bytes());
            fim
        });
        (url, h)
    }

    fn exemplo() -> Notif {
        Notif {
            evento: "queda",
            gravidade: Gravidade::Aviso,
            titulo: "N: falta de energia".into(),
            texto: "Passou para a bateria.".into(),
            t: 1_700_000_000,
        }
    }

    #[test]
    fn webhook_envia_json_com_cabecalho() {
        let (url, h) = servidor("200 OK");
        let d = Destino {
            nome: "webhook",
            url: parse_url(&url).unwrap(),
            cabecalhos: vec![("X-Token".into(), "segredo".into())],
            formato: Formato::Json,
        };
        assert_eq!(d.enviar(&exemplo()), Ok(200));
        let req = h.join().unwrap();
        assert!(req.starts_with("POST /hook?x=1 HTTP/1.1\r\n"));
        assert!(req.contains("Content-Type: application/json"));
        assert!(req.contains("X-Token: segredo"));
        let corpo: serde_json::Value =
            serde_json::from_str(req.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(corpo["evento"], "queda");
        assert_eq!(corpo["gravidade"], "aviso");
        assert_eq!(corpo["t"], 1_700_000_000u64);
    }

    #[test]
    fn ntfy_envia_json_de_publicacao_com_token() {
        let (url, h) = servidor("200 OK");
        let raiz = url.split("/hook").next().unwrap().to_string();
        let d = Destino {
            nome: "ntfy",
            url: parse_url(&format!("{raiz}/")).unwrap(),
            cabecalhos: vec![("Authorization".into(), "Bearer tk_x".into())],
            formato: Formato::Ntfy {
                topico: "nobreak".into(),
            },
        };
        let mut n = exemplo();
        n.titulo = "Moon: falta de energia ⚡".into();
        assert_eq!(d.enviar(&n), Ok(200));
        let req = h.join().unwrap();
        assert!(req.starts_with("POST / HTTP/1.1\r\n"));
        assert!(req.contains("Authorization: Bearer tk_x"));
        let c: serde_json::Value =
            serde_json::from_str(req.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(c["topic"], "nobreak");
        assert_eq!(c["title"], "Moon: falta de energia ⚡");
        assert_eq!(c["message"], "Passou para a bateria.");
        assert_eq!(c["priority"], 4);
        assert_eq!(c["tags"], serde_json::json!(["zap", "warning"]));
    }

    #[test]
    fn prioridade_segue_a_gravidade() {
        let mut n = exemplo();
        n.gravidade = Gravidade::Info;
        assert_eq!(prioridade_ntfy(&n), 3);
        n.gravidade = Gravidade::Aviso;
        assert_eq!(prioridade_ntfy(&n), 4);
        n.gravidade = Gravidade::Erro;
        assert_eq!(prioridade_ntfy(&n), 5);
        for e in EVENTOS {
            n.evento = e;
            assert!(!tags_ntfy(&n).is_empty());
        }
    }

    #[test]
    fn webhook_status_de_erro_e_destino_fora_do_ar() {
        let (url, h) = servidor("500 Internal Server Error");
        let d = Destino {
            nome: "webhook",
            url: parse_url(&url).unwrap(),
            cabecalhos: vec![],
            formato: Formato::Json,
        };
        assert!(d.enviar(&exemplo()).unwrap_err().contains("500"));
        h.join().unwrap();
        // porta fechada
        let ln = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", ln.local_addr().unwrap());
        drop(ln);
        let d = Destino {
            nome: "webhook",
            url: parse_url(&url).unwrap(),
            cabecalhos: vec![],
            formato: Formato::Json,
        };
        assert!(d.enviar(&exemplo()).is_err());
    }

    #[test]
    fn tenta_de_novo_ate_funcionar() {
        // primeiro destino falha (porta fechada), depois sobe um servidor na mesma porta
        let ln = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = ln.local_addr().unwrap();
        drop(ln);
        let d = Destino {
            nome: "webhook",
            url: parse_url(&format!("http://{addr}/")).unwrap(),
            cabecalhos: vec![],
            formato: Formato::Json,
        };
        let sobe = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            let ln = TcpListener::bind(addr).unwrap();
            let (mut s, _) = ln.accept().unwrap();
            let mut b = [0u8; 2048];
            let _ = s.read(&mut b);
            let _ = s.write_all(b"HTTP/1.1 204 No Content\r\n\r\n");
        });
        let r = enviar_com_tentativas(
            &d,
            &exemplo(),
            &[Duration::from_millis(300), Duration::from_millis(600)],
        );
        sobe.join().unwrap();
        assert_eq!(r, Ok(204));
    }
}
