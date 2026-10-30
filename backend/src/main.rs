mod alarmes;
mod bateria;
mod fonte;
mod historico;
mod leitura;
mod longa;
mod modelos;
mod notificar;
mod protocolo;
mod quedas;

use alarmes::Alarme;
use fonte::Fonte;
use historico::Historico;
use leitura::{decodificar, status_nut, Brutos, Flags, InfoModelo, Medidas};
use longa::SerieLonga;
use notificar::{Destino, Detector, Leitura, Notificador};
use quedas::Quedas;
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
    /// Alarmes ligados agora (bits com significado conhecido).
    alarmes: Vec<Alarme>,
    /// Registradores 0x80..=0x9D em hexadecimal, para engenharia reversa.
    registradores_hex: Option<String>,
    calibracao_0xf3: Option<u8>,
    capacidade_0x136: Option<u8>,
    osc_0x202: Option<[u8; 2]>,
}

#[derive(Clone)]
struct Config {
    porta: String,
    escuta: String,
    intervalo: Duration,
    id: u8,
    simular: bool,
    arquivo_nut: Option<String>,
    fator_vbat: f32,
    arquivo_eventos: Option<String>,
    arquivo_quedas: Option<String>,
    /// Data de instalação da bateria (segundos Unix), de `NOBREAK_BATERIA_INSTALADA`.
    bateria_instalada: Option<u64>,
    historico_pontos: usize,
    historico_intervalo_s: u64,
    /// Arquivo da série longa (dias a semanas), de `NOBREAK_ARQUIVO_SERIE`.
    arquivo_serie: Option<String>,
    serie_dias: u64,
    serie_intervalo_s: u64,
    /// Webhook HTTP das notificações (`NOBREAK_WEBHOOK_URL`) e seus cabeçalhos extras.
    webhook_url: Option<String>,
    webhook_cabecalhos: Vec<String>,
    notificar_nome: String,
    notificar_eventos: String,
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
        arquivo_eventos: std::env::var("NOBREAK_ARQUIVO_EVENTOS")
            .ok()
            .filter(|v| !v.is_empty()),
        arquivo_quedas: std::env::var("NOBREAK_ARQUIVO_QUEDAS")
            .ok()
            .filter(|v| !v.is_empty()),
        bateria_instalada: bateria::parse_data(&env("NOBREAK_BATERIA_INSTALADA", "")),
        historico_pontos: env("NOBREAK_HISTORICO_PONTOS", "1440")
            .parse()
            .unwrap_or(1440),
        historico_intervalo_s: env("NOBREAK_HISTORICO_INTERVALO_S", "60")
            .parse()
            .unwrap_or(60),
        arquivo_serie: std::env::var("NOBREAK_ARQUIVO_SERIE")
            .ok()
            .filter(|v| !v.is_empty()),
        serie_dias: env("NOBREAK_SERIE_DIAS", "30").parse().unwrap_or(30),
        serie_intervalo_s: env("NOBREAK_SERIE_INTERVALO_S", "900")
            .parse()
            .unwrap_or(900),
        webhook_url: std::env::var("NOBREAK_WEBHOOK_URL")
            .ok()
            .filter(|v| !v.trim().is_empty()),
        webhook_cabecalhos: env("NOBREAK_WEBHOOK_CABECALHOS", "")
            .split('|')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect(),
        notificar_nome: env("NOBREAK_NOTIFICAR_NOME", "Nobreak"),
        notificar_eventos: env("NOBREAK_NOTIFICAR_EVENTOS", ""),
    }
}

/// Monta o destino do webhook a partir da configuração.
fn destino_webhook(cfg: &Config) -> Result<Option<Destino>, String> {
    let Some(u) = &cfg.webhook_url else {
        return Ok(None);
    };
    let url = notificar::parse_url(u).map_err(|e| format!("NOBREAK_WEBHOOK_URL: {e}"))?;
    let cabecalhos = cfg
        .webhook_cabecalhos
        .iter()
        .map(|c| notificar::parse_cabecalho(c))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("NOBREAK_WEBHOOK_CABECALHOS: {e}"))?;
    Ok(Some(Destino { url, cabecalhos }))
}

fn agora() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn aplicar(e: &mut Estado, b: &Brutos, fator_vbat: f32) {
    let (info, medidas, flags) = decodificar(b, fator_vbat);
    e.status = Some(status_nut(&medidas, &flags));
    e.alarmes = alarmes::ativos(&flags);
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
            + &l("ups.status", s.clone())
            + &if e.alarmes.is_empty() {
                String::new()
            } else {
                l(
                    "ups.alarm",
                    e.alarmes
                        .iter()
                        .map(|a| a.texto)
                        .collect::<Vec<_>>()
                        .join("/"),
                )
            },
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

fn coletor(
    cfg: Config,
    estado: Arc<Mutex<Estado>>,
    hist: Arc<Mutex<Historico>>,
    quedas: Arc<Mutex<Quedas>>,
    longa: Arc<Mutex<SerieLonga>>,
    mut notif: Option<Notificador>,
) {
    let mut fonte = if cfg.simular {
        Fonte::Simulada(0)
    } else {
        Fonte::serial(cfg.porta.clone(), cfg.id)
    };
    loop {
        let r = fonte.ler_tudo();
        {
            let mut e = estado.lock().unwrap();
            let mut novos = Vec::new();
            match r {
                Ok(b) => {
                    aplicar(&mut e, &b, cfg.fator_vbat);
                    {
                        let mut h = hist.lock().unwrap();
                        novos.extend(h.conexao(agora(), true, ""));
                        if let (Some(m), Some(st)) = (&e.medidas, &e.status) {
                            novos.extend(h.registrar(agora(), &b, m, st));
                        }
                    }
                    if let (Some(p), Some(txt)) = (&cfg.arquivo_nut, arquivo_nut(&e)) {
                        gravar_arquivo(p, &txt);
                    }
                    if let (Some(m), Some(st)) = (&e.medidas, &e.status) {
                        let fechado = longa.lock().unwrap().amostrar(
                            agora(),
                            [
                                m.tensao_entrada_v,
                                m.tensao_saida_v,
                                m.carga_bateria_pct,
                                m.tensao_bateria_v,
                                m.carga_saida_pct,
                                m.temperatura_c,
                            ],
                        );
                        if let (Some(p), Some(b)) = (&cfg.arquivo_serie, fechado) {
                            longa::anexar_arquivo(p, &b, longa.lock().unwrap().max_baldes());
                        }
                        let mut q = quedas.lock().unwrap();
                        q.atualizar(
                            agora(),
                            st.starts_with("OB"),
                            m.carga_bateria_pct,
                            m.tensao_entrada_v,
                        );
                        q.detalhar(
                            agora(),
                            quedas::Detalhe {
                                bateria_pct: m.carga_bateria_pct,
                                bateria_v: m.tensao_bateria_v,
                                carga_pct: m.carga_saida_pct,
                            },
                        );
                        if let Some(p) = &cfg.arquivo_quedas {
                            quedas::anexar_arquivo(p, &q.drenar());
                        } else {
                            q.drenar();
                        }
                        if let Some(n) = notif.as_mut() {
                            let t = agora();
                            let mut v = n.detector.conexao(t, true, "");
                            v.extend(n.detector.leitura(
                                t,
                                &Leitura {
                                    em_queda: st.starts_with("OB"),
                                    bateria_pct: m.carga_bateria_pct,
                                    alarmes: &e.alarmes,
                                    duracao_ultima_queda_s:
                                        q.lista().back().and_then(|x| x.duracao_s),
                                },
                            ));
                            let saude = bateria::avaliar(
                                &q,
                                Some(m.temperatura_c),
                                cfg.bateria_instalada,
                                t,
                            );
                            v.extend(n.detector.saude(t, &saude.avisos));
                            n.enviar(v);
                        }
                    }
                }
                Err(msg) => {
                    e.falhas_seguidas += 1;
                    e.ultimo_erro = Some(msg);
                    // tolera falhas isoladas; só marca desconectado após 3 seguidas
                    if e.falhas_seguidas >= 3 {
                        e.conectado = false;
                        let motivo = e.ultimo_erro.clone().unwrap_or_default();
                        novos.extend(hist.lock().unwrap().conexao(agora(), false, &motivo));
                        if let Some(n) = notif.as_mut() {
                            let v = n.detector.conexao(agora(), false, &motivo);
                            n.enviar(v);
                        }
                    }
                }
            }
            if let Some(p) = &cfg.arquivo_eventos {
                historico::anexar_arquivo(p, &novos);
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

fn metricas(e: &Estado, q: &Quedas, cfg: &Config) -> String {
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
        g(
            "contagem_regressiva_segundos",
            "contador regressivo de desligamento (0 = parado)",
            m.contagem_regressiva_s as f64,
        );
    }
    if let Some(st) = &e.status {
        g(
            "em_bateria",
            "1 se o nobreak está na bateria (sem rede)",
            if st.starts_with("OB") { 1.0 } else { 0.0 },
        );
    }
    g(
        "alarmes_ativos",
        "quantidade de alarmes ligados agora",
        e.alarmes.len() as f64,
    );
    let saude = bateria::avaliar(
        q,
        e.medidas.as_ref().map(|m| m.temperatura_c),
        cfg.bateria_instalada,
        agora(),
    );
    g(
        "bateria_avisos",
        "quantidade de avisos de saúde da bateria",
        saude.avisos.len() as f64,
    );
    if let Some(v) = saude.tensao_repouso_v {
        g(
            "bateria_tensao_repouso_volts",
            "tensão média da bateria com a rede ligada e a carga completa",
            v as f64,
        );
    }
    if let Some(a) = saude
        .ultima_queda
        .as_ref()
        .and_then(|u| u.autonomia_projetada_min)
    {
        g(
            "bateria_autonomia_projetada_minutos",
            "autonomia de 100% a 0% projetada pela última queda",
            a as f64,
        );
    }
    if let Some(d) = saude.idade_dias {
        g("bateria_idade_dias", "idade da bateria informada", d as f64);
    }
    let r = q.resumo(agora(), 86_400);
    g(
        "quedas_24h",
        "quedas de energia nas últimas 24 h",
        r.quedas as f64,
    );
    g(
        "segundos_em_bateria_24h",
        "tempo na bateria nas últimas 24 h",
        r.segundos_em_bateria as f64,
    );
    s
}

fn servir(
    cfg: &Config,
    estado: &Arc<Mutex<Estado>>,
    hist: &Arc<Mutex<Historico>>,
    quedas: &Arc<Mutex<Quedas>>,
    longa: &Arc<Mutex<SerieLonga>>,
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
        "/api/eventos" => json(hist.lock().unwrap().eventos_json(), 200),
        "/api/historico" => json(hist.lock().unwrap().serie_json(), 200),
        "/api/historico/longo" => json(longa.lock().unwrap().json(), 200),
        "/api/quedas" => {
            let q = quedas.lock().unwrap();
            let t = agora();
            json(
                serde_json::json!({
                    "em_curso": q.em_curso(),
                    "ultimas_24h": q.resumo(t, 86_400),
                    "ultimos_7_dias": q.resumo(t, 7 * 86_400),
                    "ultimos_30_dias": q.resumo(t, 30 * 86_400),
                    "lista": q.lista_json(t),
                })
                .to_string(),
                200,
            )
        }
        "/api/bateria" => {
            let temp = estado
                .lock()
                .unwrap()
                .medidas
                .as_ref()
                .map(|m| m.temperatura_c);
            let s = bateria::avaliar(
                &quedas.lock().unwrap(),
                temp,
                cfg.bateria_instalada,
                agora(),
            );
            json(serde_json::to_string(&s).unwrap(), 200)
        }
        "/api/saude" => {
            let e = estado.lock().unwrap();
            let obsoleto = e
                .atualizado_em
                .map(|t| agora().saturating_sub(t) > 60)
                .unwrap_or(true);
            let ok = e.conectado && !obsoleto;
            json(format!("{{\"ok\":{ok}}}"), if ok { 200 } else { 503 })
        }
        "/metrics" => Response::from_string(metricas(
            &estado.lock().unwrap(),
            &quedas.lock().unwrap(),
            cfg,
        ))
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
    if std::env::args().any(|a| a == "--testar-webhook") {
        // envia uma notificação de teste e sai (0 = o destino aceitou)
        let r = destino_webhook(&cfg).and_then(|d| {
            let d = d.ok_or("defina NOBREAK_WEBHOOK_URL")?;
            d.enviar(&notificar::Notif {
                evento: "teste",
                gravidade: alarmes::Gravidade::Info,
                titulo: format!("{}: teste", cfg.notificar_nome),
                texto: "Notificação de teste do nobreak-ragtech.".into(),
                t: agora(),
            })
        });
        match r {
            Ok(c) => println!("webhook respondeu HTTP {c}"),
            Err(e) => {
                eprintln!("falhou: {e}");
                std::process::exit(1);
            }
        }
        return;
    }
    let notificador = match destino_webhook(&cfg)
        .and_then(|d| Ok((d, notificar::parse_filtro(&cfg.notificar_eventos)?)))
    {
        Ok((Some(d), filtro)) => Some(Notificador::iniciar(
            Detector::novo(&cfg.notificar_nome, filtro),
            d,
        )),
        Ok((None, _)) => None,
        Err(e) => {
            eprintln!("notificações desligadas: {e}");
            None
        }
    };
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
    let hist = Arc::new(Mutex::new(Historico::novo(
        cfg.historico_pontos,
        cfg.historico_intervalo_s,
    )));
    if let Some(p) = &cfg.arquivo_eventos {
        hist.lock().unwrap().restaurar(historico::ler_arquivo(p));
    }
    let quedas = Arc::new(Mutex::new(Quedas::default()));
    if let Some(p) = &cfg.arquivo_quedas {
        quedas.lock().unwrap().restaurar(&quedas::ler_arquivo(p));
    }
    let longa = Arc::new(Mutex::new(SerieLonga::nova(
        cfg.serie_dias,
        cfg.serie_intervalo_s,
    )));
    if let Some(p) = &cfg.arquivo_serie {
        longa.lock().unwrap().restaurar(longa::ler_arquivo(p));
    }
    {
        let longa = longa.clone();
        let estado = estado.clone();
        let hist = hist.clone();
        let quedas = quedas.clone();
        let cfg_coletor = cfg.clone();
        std::thread::Builder::new()
            .name("coletor".into())
            .stack_size(256 * 1024)
            .spawn(move || coletor(cfg_coletor, estado, hist, quedas, longa, notificador))
            .unwrap();
    }
    let servidor = Server::http(&escuta).unwrap_or_else(|e| {
        eprintln!("não consegui escutar em {escuta}: {e}");
        std::process::exit(1);
    });
    for req in servidor.incoming_requests() {
        let resp = servir(
            &cfg,
            &estado,
            &hist,
            &quedas,
            &longa,
            req.method(),
            req.url(),
        );
        let _ = req.respond(resp);
    }
}
