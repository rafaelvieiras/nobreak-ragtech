mod fonte;
mod leitura;
mod modelos;
mod protocolo;

use fonte::Fonte;
use leitura::{decodificar, status_nut, Brutos, Flags, InfoModelo, Medidas};
use rust_embed::RustEmbed;
use serde::Serialize;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tiny_http::{Header, Method, Response, Server};

#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
struct Painel;

#[derive(Serialize, Clone, Default)]
struct Estado {
    conectado: bool,
    ultimo_erro: Option<String>,
    atualizado_em: Option<u64>,
    falhas_seguidas: u32,
    leituras_ok: u64,
    modelo: Option<InfoModelo>,
    status: Option<String>,
    medidas: Option<Medidas>,
    flags: Option<Flags>,
    /// Registradores 0x80..=0x9D em hexadecimal, para engenharia reversa.
    registradores_hex: Option<String>,
    calibracao_0xf3: Option<u8>,
    capacidade_0x136: Option<u8>,
    osc_0x202: Option<[u8; 2]>,
}

struct Config {
    porta: String,
    escuta: String,
    intervalo: Duration,
    id: u8,
    simular: bool,
    arquivo_nut: Option<String>,
    fator_vbat: f32,
}

fn env(chave: &str, padrao: &str) -> String {
    std::env::var(chave)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| padrao.to_string())
}

fn config() -> Config {
    Config {
        porta: env("NOBREAK_PORTA", "/dev/ragtech"),
        escuta: env("NOBREAK_ESCUTA", "0.0.0.0:8080"),
        intervalo: Duration::from_secs(env("NOBREAK_INTERVALO_S", "5").parse().unwrap_or(5).max(1)),
        id: u8::from_str_radix(env("NOBREAK_ID", "A").trim_start_matches("0x"), 16).unwrap_or(0xA)
            & 0x0F,
        simular: env("NOBREAK_SIMULAR", "0") == "1",
        arquivo_nut: std::env::var("NOBREAK_ARQUIVO_NUT")
            .ok()
            .filter(|v| !v.is_empty()),
        fator_vbat: env("NOBREAK_FATOR_VBAT", "1")
            .replace(',', ".")
            .parse()
            .unwrap_or(1.0),
    }
}

fn agora() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn aplicar(e: &mut Estado, b: &Brutos, fator_vbat: f32) {
    let (info, medidas, flags) = decodificar(b, fator_vbat);
    e.status = Some(status_nut(&medidas).to_string());
    e.registradores_hex = Some(b.bloco.iter().map(|x| format!("{x:02x}")).collect());
    e.calibracao_0xf3 = Some(b.calibracao);
    e.capacidade_0x136 = Some(b.cap_bateria);
    e.osc_0x202 = Some([b.osc53, b.osc57]);
    e.modelo = Some(info);
    e.medidas = Some(medidas);
    e.flags = Some(flags);
    e.conectado = true;
    e.ultimo_erro = None;
    e.falhas_seguidas = 0;
    e.leituras_ok += 1;
    e.atualizado_em = Some(agora());
}

/// Formato do arquivo lido pelo driver `dummy-ups` do NUT (compatível com o script antigo).
fn arquivo_nut(e: &Estado) -> Option<String> {
    let (m, s) = (e.medidas.as_ref()?, e.status.as_ref()?);
    let l = |k: &str, v: String| format!("{k}: {v}\n");
    let pot = m.tensao_saida_v * m.corrente_saida_a;
    Some(
        l("battery.charge", format!("{:.0}", m.carga_bateria_pct))
            + &l("battery.voltage", format!("{:.2}", m.tensao_bateria_v))
            + &l("input.voltage", format!("{:.0}", m.tensao_entrada_v))
            + &l("output.voltage", format!("{:.0}", m.tensao_saida_v))
            + &l("output.current", format!("{:.2}", m.corrente_saida_a))
            + &l("output.apower", format!("{pot:.1}"))
            + &l("ups.temperature", format!("{:.0}", m.temperatura_c))
            + &l("ups.load", format!("{:.0}", m.carga_saida_pct))
            + &l("ups.status", s.clone()),
    )
}

/// Grava o arquivo do NUT. Tenta o caminho atômico (temporário + rename); se não der
/// (bind mount de arquivo único, raiz somente leitura), grava no próprio arquivo.
fn gravar_arquivo(caminho: &str, conteudo: &str) {
    let tmp = format!("{caminho}.tmp");
    if std::fs::write(&tmp, conteudo).is_ok() && std::fs::rename(&tmp, caminho).is_ok() {
        return;
    }
    let _ = std::fs::remove_file(&tmp);
    if let Err(e) = std::fs::write(caminho, conteudo) {
        eprintln!("não consegui gravar {caminho}: {e}");
    }
}

fn coletor(cfg: Config, estado: Arc<Mutex<Estado>>) {
    let mut fonte = if cfg.simular {
        Fonte::Simulada(0)
    } else {
        Fonte::serial(cfg.porta.clone(), cfg.id)
    };
    loop {
        let r = fonte.ler_tudo();
        {
            let mut e = estado.lock().unwrap();
            match r {
                Ok(b) => {
                    aplicar(&mut e, &b, cfg.fator_vbat);
                    if let (Some(p), Some(txt)) = (&cfg.arquivo_nut, arquivo_nut(&e)) {
                        gravar_arquivo(p, &txt);
                    }
                }
                Err(msg) => {
                    e.falhas_seguidas += 1;
                    e.ultimo_erro = Some(msg);
                    // tolera falhas isoladas; só marca desconectado após 3 seguidas
                    if e.falhas_seguidas >= 3 {
                        e.conectado = false;
                    }
                }
            }
        }
        std::thread::sleep(cfg.intervalo);
    }
}

fn json(corpo: String, codigo: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(corpo)
        .with_status_code(codigo)
        .with_header(Header::from_bytes("Content-Type", "application/json; charset=utf-8").unwrap())
        .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
}

fn metricas(e: &Estado) -> String {
    let mut s = String::new();
    let mut g = |nome: &str, ajuda: &str, v: f64| {
        s += &format!(
            "# HELP nobreak_{nome} {ajuda}\n# TYPE nobreak_{nome} gauge\nnobreak_{nome} {v}\n"
        );
    };
    g(
        "conectado",
        "1 se a última leitura funcionou",
        if e.conectado { 1.0 } else { 0.0 },
    );
    if let Some(m) = &e.medidas {
        g(
            "bateria_pct",
            "carga da bateria em %",
            m.carga_bateria_pct as f64,
        );
        g(
            "entrada_volts",
            "tensão de entrada",
            m.tensao_entrada_v as f64,
        );
        g("saida_volts", "tensão de saída", m.tensao_saida_v as f64);
        g(
            "saida_amperes",
            "corrente de saída",
            m.corrente_saida_a as f64,
        );
        g("carga_pct", "carga na saída em %", m.carga_saida_pct as f64);
        g(
            "temperatura_celsius",
            "temperatura do inversor",
            m.temperatura_c as f64,
        );
    }
    s
}

fn servir(
    estado: &Arc<Mutex<Estado>>,
    metodo: &Method,
    url: &str,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let caminho = url.split('?').next().unwrap_or("/");
    if *metodo != Method::Get && *metodo != Method::Head {
        return json("{\"erro\":\"método não permitido\"}".into(), 405);
    }
    match caminho {
        "/api/estado" => json(
            serde_json::to_string(&*estado.lock().unwrap()).unwrap(),
            200,
        ),
        "/api/saude" => {
            let e = estado.lock().unwrap();
            let obsoleto = e
                .atualizado_em
                .map(|t| agora().saturating_sub(t) > 60)
                .unwrap_or(true);
            let ok = e.conectado && !obsoleto;
            json(format!("{{\"ok\":{ok}}}"), if ok { 200 } else { 503 })
        }
        "/metrics" => Response::from_string(metricas(&estado.lock().unwrap()))
            .with_header(Header::from_bytes("Content-Type", "text/plain; version=0.0.4").unwrap()),
        _ => {
            let nome = caminho.trim_start_matches('/');
            let nome = if nome.is_empty() { "index.html" } else { nome };
            let (arq, nome) = match Painel::get(nome) {
                Some(a) => (a, nome),
                None => match Painel::get("index.html") {
                    Some(a) => (a, "index.html"),
                    None => {
                        return Response::from_string("painel não compilado").with_status_code(404)
                    }
                },
            };
            let tipo = mime_guess::from_path(nome)
                .first_or_octet_stream()
                .to_string();
            let cache = if nome.starts_with("assets/") {
                "public, max-age=31536000, immutable"
            } else {
                "no-cache"
            };
            Response::from_data(arq.data.into_owned())
                .with_header(Header::from_bytes("Content-Type", tipo).unwrap())
                .with_header(Header::from_bytes("Cache-Control", cache).unwrap())
        }
    }
}

fn main() {
    if std::env::args().any(|a| a == "--saude") {
        // usado pelo HEALTHCHECK do Docker (a imagem não tem curl)
        let porta = env("NOBREAK_ESCUTA", "0.0.0.0:8080");
        let porta = porta.rsplit(':').next().unwrap_or("8080").to_string();
        let ok = std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{porta}").parse().unwrap(),
            Duration::from_secs(2),
        )
        .and_then(|mut s| {
            s.write_all(b"GET /api/saude HTTP/1.0\r\n\r\n")?;
            let mut r = String::new();
            s.read_to_string(&mut r)?;
            Ok(r.starts_with("HTTP/1.0 200") || r.starts_with("HTTP/1.1 200"))
        })
        .unwrap_or(false);
        std::process::exit(if ok { 0 } else { 1 });
    }

    let cfg = config();
    let escuta = cfg.escuta.clone();
    eprintln!(
        "nobreak-ragtech {} | porta={} escuta={} intervalo={}s simular={} fator_vbat={}",
        env!("CARGO_PKG_VERSION"),
        cfg.porta,
        cfg.escuta,
        cfg.intervalo.as_secs(),
        cfg.simular,
        cfg.fator_vbat
    );
    let estado = Arc::new(Mutex::new(Estado::default()));
    {
        let estado = estado.clone();
        std::thread::Builder::new()
            .name("coletor".into())
            .stack_size(256 * 1024)
            .spawn(move || coletor(cfg, estado))
            .unwrap();
    }
    let servidor = Server::http(&escuta).unwrap_or_else(|e| {
        eprintln!("não consegui escutar em {escuta}: {e}");
        std::process::exit(1);
    });
    for req in servidor.incoming_requests() {
        let resp = servir(&estado, req.method(), req.url());
        let _ = req.respond(resp);
    }
}
