mod acoes;
mod alarmes;
mod bateria;
mod fonte;
mod historico;
mod leitura;
mod longa;
mod modelos;
mod notificar;
mod politica;
mod protocolo;
mod quedas;

use alarmes::{Alarme, Gravidade};
use fonte::Fonte;
use historico::Historico;
use leitura::{decodificar, status_nut, Brutos, Flags, InfoModelo, Medidas};
use longa::SerieLonga;
use notificar::{Destino, Detector, Leitura, Notif, Notificador};
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
    /// Quando começou a sequência atual de falhas de leitura (segundos Unix). `null`
    /// quando está lendo. Diz há quanto tempo o nobreak está mudo.
    sem_leitura_desde: Option<u64>,
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
    /// Estado da política de desligamento por bateria baixa.
    desligamento: Option<politica::Situacao>,
    /// Situação do religa-mento do nashome (WOL).
    ligar_nashome: Option<acoes::Situacao>,
    /// Autonomia estimada agora, em segundos: projeção ao vivo durante a queda,
    /// autonomia medida reescalada pela carga atual com a rede presente.
    autonomia_s: Option<u64>,
}

#[derive(Clone)]
struct Config {
    porta: String,
    /// Caminhos extras de serial (`NOBREAK_PORTAS_ALTERNATIVAS`, separados por vírgula).
    portas_alternativas: Vec<String>,
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
    /// ntfy: `NOBREAK_NTFY_URL` (servidor, ex.: `http://host:2586`), tópico e token.
    ntfy_url: Option<String>,
    ntfy_topico: String,
    ntfy_token: Option<String>,
    notificar_nome: String,
    notificar_eventos: String,
    /// Desligamento do host por bateria baixa (`NOBREAK_DESLIGAR*`).
    politica: politica::Config,
    /// Minutos de autonomia projetada a partir dos quais o NUT reporta `LB`
    /// (`NOBREAK_LB_PROJETADO_MIN`, 0 desliga).
    lb_projetado_min: f32,
    /// Religar o nashome por Wake-on-LAN (`NOBREAK_LIGAR_*`).
    acoes: acoes::Config,
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
        portas_alternativas: env("NOBREAK_PORTAS_ALTERNATIVAS", "")
            .split(',')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty())
            .collect(),
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
        ntfy_url: std::env::var("NOBREAK_NTFY_URL")
            .ok()
            .filter(|v| !v.trim().is_empty()),
        ntfy_topico: env("NOBREAK_NTFY_TOPICO", "nobreak"),
        ntfy_token: std::env::var("NOBREAK_NTFY_TOKEN")
            .ok()
            .filter(|v| !v.trim().is_empty()),
        notificar_nome: env("NOBREAK_NOTIFICAR_NOME", "Nobreak"),
        notificar_eventos: env("NOBREAK_NOTIFICAR_EVENTOS", ""),
        lb_projetado_min: env("NOBREAK_LB_PROJETADO_MIN", "5")
            .replace(',', ".")
            .parse()
            .unwrap_or(5.0),
        acoes: acoes::Config {
            ligada: env("NOBREAK_LIGAR_NASHOME", "0") == "1",
            estavel_s: env("NOBREAK_LIGAR_ESTAVEL_S", "900").parse().unwrap_or(900),
            bateria_pct: env("NOBREAK_LIGAR_BATERIA_PCT", "85")
                .replace(',', ".")
                .parse()
                .unwrap_or(85.0),
            janela_s: env("NOBREAK_LIGAR_JANELA_H", "6")
                .parse::<u64>()
                .unwrap_or(6)
                * 3600,
            arquivo: std::env::var("NOBREAK_ARQUIVO_LIGAR_NASHOME")
                .ok()
                .filter(|v| !v.trim().is_empty()),
        },
        politica: politica::Config {
            ligada: env("NOBREAK_DESLIGAR", "0") == "1",
            margem_s: env("NOBREAK_DESLIGAR_MARGEM_S", "120")
                .parse()
                .unwrap_or(120),
            bateria_minima_pct: env("NOBREAK_DESLIGAR_BATERIA_PCT", "40")
                .replace(',', ".")
                .parse()
                .unwrap_or(40.0),
            confirma_s: env("NOBREAK_DESLIGAR_CONFIRMA_S", "10")
                .parse()
                .unwrap_or(10),
            tempo_maximo_s: env("NOBREAK_DESLIGAR_TEMPO_S", "180")
                .parse()
                .unwrap_or(180),
            aviso_s: env("NOBREAK_DESLIGAR_AVISO_S", "20").parse().unwrap_or(20),
            piso_pct: env("NOBREAK_DESLIGAR_PISO_PCT", "15")
                .replace(',', ".")
                .parse()
                .unwrap_or(15.0),
            dados_minimos_s: env("NOBREAK_DESLIGAR_DADOS_MINIMOS_S", "20")
                .parse()
                .unwrap_or(20),
            arquivo: std::env::var("NOBREAK_ARQUIVO_DESLIGAR")
                .ok()
                .filter(|v| !v.trim().is_empty()),
        },
    }
}

/// Monta os destinos (webhook e ntfy) a partir da configuração.
fn destinos(cfg: &Config) -> Result<Vec<Destino>, String> {
    let mut v = Vec::new();
    if let Some(u) = &cfg.webhook_url {
        let url = notificar::parse_url(u).map_err(|e| format!("NOBREAK_WEBHOOK_URL: {e}"))?;
        let cabecalhos = cfg
            .webhook_cabecalhos
            .iter()
            .map(|c| notificar::parse_cabecalho(c))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("NOBREAK_WEBHOOK_CABECALHOS: {e}"))?;
        v.push(Destino {
            nome: "webhook",
            url,
            cabecalhos,
            formato: notificar::Formato::Json,
        });
    }
    if let Some(u) = &cfg.ntfy_url {
        let mut url = notificar::parse_url(u).map_err(|e| format!("NOBREAK_NTFY_URL: {e}"))?;
        // a publicação em JSON vai sempre para a raiz do servidor
        url.caminho = "/".into();
        let topico = cfg.ntfy_topico.trim().to_string();
        if topico.is_empty() || topico.contains(|c: char| c == '/' || c.is_whitespace()) {
            return Err("NOBREAK_NTFY_TOPICO: tópico inválido".into());
        }
        let cabecalhos = match &cfg.ntfy_token {
            Some(t) => {
                vec![
                    notificar::parse_cabecalho(&format!("Authorization: Bearer {}", t.trim()))
                        .map_err(|e| format!("NOBREAK_NTFY_TOKEN: {e}"))?,
                ]
            }
            None => vec![],
        };
        v.push(Destino {
            nome: "ntfy",
            url,
            cabecalhos,
            formato: notificar::Formato::Ntfy { topico },
        });
    }
    Ok(v)
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

/// Troca acentos por ASCII: o `dummy-ups` não preserva byte não-ASCII e o texto chega
/// embaralhado a quem lê (`Ausncia de rede eltrica`, visto em 13/nov/2026).
fn sem_acentos(t: &str) -> String {
    t.chars()
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'A',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'Í' | 'Ì' | 'Î' | 'Ï' => 'I',
            'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'Ú' | 'Ù' | 'Û' | 'Ü' => 'U',
            'Ç' => 'C',
            c if c.is_ascii() => c,
            _ => '?',
        })
        .collect()
}

/// Status para o NUT, com LB derivado quando a autonomia projetada acaba antes de o
/// nobreak sinalizar bateria fraca. É o que faz o `upsmon` de outra máquina (o Unraid do
/// nashome) desligar a tempo: esperar o LB do próprio nobreak seria esperar o fim.
fn status_para_nut(e: &Estado, lb_apos_s: Option<u64>) -> String {
    let mut st = e.status.clone().unwrap_or_default();
    if let Some(limite) = lb_apos_s {
        if st.starts_with("OB") && !st.contains("LB") {
            if let Some(a) = e.autonomia_s {
                if a <= limite {
                    st.push_str(" LB");
                }
            }
        }
    }
    st
}

/// Formato do arquivo lido pelo driver `dummy-ups` do NUT (compatível com o script antigo).
fn arquivo_nut(e: &Estado, lb_apos_s: Option<u64>) -> Option<String> {
    let m = e.medidas.as_ref()?;
    e.status.as_ref()?;
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
            + &l("ups.status", status_para_nut(e, lb_apos_s))
            + &e.autonomia_s
                .map(|a| l("battery.runtime", a.to_string()))
                .unwrap_or_default()
            // `ups.alarm` é escrito sempre: o driver `dummy-ups` mantém uma variável que
            // desaparece do arquivo, então uma linha vazia é o que limpa o alarme antigo.
            + &l(
                "ups.alarm",
                sem_acentos(
                    &e.alarmes
                        .iter()
                        .map(|a| a.texto)
                        .collect::<Vec<_>>()
                        .join("/"),
                ),
            ),
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
        Fonte::serial_com(cfg.porta.clone(), cfg.portas_alternativas.clone(), cfg.id)
    };
    let mut pol = politica::Politica::nova(cfg.politica.clone());
    let mut ac = acoes::Acoes::nova(cfg.acoes.clone());
    loop {
        let r = fonte.ler_tudo();
        let mut em_bateria_s = 0u64;
        let mut bateria_inicio_pct = 100.0f32;
        {
            let mut e = estado.lock().unwrap();
            let mut novos = Vec::new();
            match r {
                Ok(b) => {
                    aplicar(&mut e, &b, cfg.fator_vbat);
                    e.sem_leitura_desde = None;
                    {
                        let mut h = hist.lock().unwrap();
                        novos.extend(h.conexao(agora(), true, ""));
                        if let (Some(m), Some(st)) = (&e.medidas, &e.status) {
                            novos.extend(h.registrar(agora(), &b, m, st));
                        }
                    }
                    let lb_apos_s = if cfg.lb_projetado_min > 0.0 {
                        Some((cfg.lb_projetado_min * 60.0) as u64)
                    } else {
                        None
                    };
                    if let (Some(p), Some(txt)) = (&cfg.arquivo_nut, arquivo_nut(&e, lb_apos_s)) {
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
                        if q.em_curso() {
                            if let Some(x) = q.lista().back() {
                                em_bateria_s = agora().saturating_sub(x.inicio);
                                bateria_inicio_pct = x.bateria_inicio_pct;
                            }
                        } else {
                            em_bateria_s = 0;
                        }
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
                                Some(m.carga_saida_pct),
                            );
                            if let Some(m) = saude.autonomia_na_carga_atual_min {
                                e.autonomia_s = Some((m * 60.0) as u64);
                            }
                            v.extend(n.detector.saude(t, &saude.avisos));
                            n.enviar(v);
                        }
                    }
                    // Política de desligamento por bateria baixa. Só roda com leitura boa:
                    // leitura velha nunca desliga máquina.
                    let t = agora();
                    let evento = pol.avaliar(
                        t,
                        politica::Leitura {
                            em_queda: e
                                .status
                                .as_deref()
                                .map(|x| x.starts_with("OB"))
                                .unwrap_or(false),
                            bateria_pct: e
                                .medidas
                                .as_ref()
                                .map(|m| m.carga_bateria_pct)
                                .unwrap_or(100.0),
                            bateria_inicio_pct,
                            em_bateria_s,
                        },
                    );
                    e.desligamento = Some(pol.situacao(t));
                    if let Some(a) = e.desligamento.as_ref().and_then(|d| d.autonomia_s) {
                        e.autonomia_s = Some(a);
                    }
                    // Religa-mento do nashome: só depois de uma queda, com a rede estável
                    // e a bateria recuperada (o WOL é feito pelo host, via wolbridge).
                    let em_queda = e
                        .status
                        .as_deref()
                        .map(|x| x.starts_with("OB"))
                        .unwrap_or(false);
                    let fim_ultima = {
                        let q = quedas.lock().unwrap();
                        q.lista().back().and_then(|x| x.fim)
                    };
                    let evento_ligar = ac.avaliar(
                        t,
                        em_queda,
                        e.medidas
                            .as_ref()
                            .map(|m| m.carga_bateria_pct)
                            .unwrap_or(0.0),
                        fim_ultima,
                    );
                    e.ligar_nashome = Some(ac.situacao(t, em_queda));
                    if let Some(acoes::Evento::Ligar { motivo, erro }) = evento_ligar {
                        match &erro {
                            None => {
                                println!("ligar nashome: pedido gravado — {motivo}");
                                if let Some(n) = notif.as_mut() {
                                    n.enviar(vec![Notif {
                                        evento: "ligar_nashome",
                                        gravidade: Gravidade::Info,
                                        titulo: "Ligando o nashome".to_string(),
                                        texto: motivo,
                                        t,
                                    }]);
                                }
                            }
                            Some(e) => {
                                println!("ligar nashome: NÃO gravado — {motivo}. {e}");
                                if let Some(n) = notif.as_mut() {
                                    n.enviar(vec![Notif {
                                        evento: "ligar_nashome",
                                        gravidade: Gravidade::Erro,
                                        titulo: "Nashome NÃO será ligado".to_string(),
                                        texto: format!("{motivo}. {e}"),
                                        t,
                                    }]);
                                }
                            }
                        }
                    }
                    if let Some(evento) = evento {
                        let (titulo, texto, gravidade) = match &evento {
                            politica::Evento::Aviso { restante_s, motivo } => (
                                "Vai desligar".to_string(),
                                format!(
                                    "{motivo}. Desligando em {restante_s}s se a rede não voltar."
                                ),
                                Gravidade::Erro,
                            ),
                            politica::Evento::Pedido { motivo, erro } => match erro {
                                None => (
                                    "Desligamento pedido".to_string(),
                                    format!("{motivo}. Pedido de desligamento gravado."),
                                    Gravidade::Erro,
                                ),
                                Some(e) => (
                                    "Desligamento NÃO gravado".to_string(),
                                    format!("{motivo}. {e}"),
                                    Gravidade::Erro,
                                ),
                            },
                            politica::Evento::Cancelado { motivo } => (
                                "Desligamento cancelado".to_string(),
                                motivo.clone(),
                                Gravidade::Info,
                            ),
                        };
                        println!("desligamento: {titulo} — {texto}");
                        if let Some(n) = notif.as_mut() {
                            n.enviar(vec![Notif {
                                evento: "desligamento",
                                gravidade,
                                titulo,
                                texto,
                                t,
                            }]);
                        }
                    }
                }
                Err(msg) => {
                    if e.falhas_seguidas == 0 {
                        e.sem_leitura_desde = Some(agora());
                    }
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
    if let Some(t) = e.sem_leitura_desde {
        g(
            "segundos_sem_leitura",
            "segundos desde a última leitura boa (0 = lendo agora)",
            agora().saturating_sub(t) as f64,
        );
    } else {
        g(
            "segundos_sem_leitura",
            "segundos desde a última leitura boa (0 = lendo agora)",
            0.0,
        );
    }
    if let Some(a) = e.autonomia_s {
        g(
            "autonomia_s",
            "autonomia estimada agora, em segundos (o que vai no battery.runtime do NUT)",
            a as f64,
        );
    }
    if let Some(l) = &e.ligar_nashome {
        g(
            "ligar_nashome_estavel_segundos",
            "segundos de rede estável desde a última queda (0 = na bateria)",
            l.estavel_s.unwrap_or(0) as f64,
        );
    }
    if let Some(d) = &e.desligamento {
        let fase = match d.fase {
            politica::Fase::Normal => 0.0,
            politica::Fase::Aviso => 1.0,
            politica::Fase::Pedido => 2.0,
        };
        g(
            "desligamento_armado",
            "1 se a política de desligar por bateria baixa está ligada",
            if d.armado { 1.0 } else { 0.0 },
        );
        g(
            "desligamento_fase",
            "0 normal, 1 aviso (contagem), 2 pedido gravado",
            fase,
        );
        g(
            "desligamento_restante_segundos",
            "segundos até o pedido quando em aviso (0 fora do aviso)",
            d.restante_s.unwrap_or(0) as f64,
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
        e.medidas.as_ref().map(|m| m.carga_saida_pct),
    );
    g(
        "bateria_avisos",
        "quantidade de avisos de saúde da bateria",
        saude.avisos.len() as f64,
    );
    if let Some(m) = saude
        .ultima_queda
        .as_ref()
        .and_then(|x| x.autonomia_projetada_min)
    {
        g(
            "autonomia_medida_min",
            "autonomia projetada pela última queda medida, na carga daquela queda",
            m as f64,
        );
    }
    if let Some(m) = saude.autonomia_na_carga_atual_min {
        g(
            "autonomia_carga_atual_min",
            "autonomia estimada com a carga ligada agora",
            m as f64,
        );
    }
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
                estado
                    .lock()
                    .unwrap()
                    .medidas
                    .as_ref()
                    .map(|m| m.carga_saida_pct),
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
    if std::env::args().any(|a| a == "--testar-notificacao" || a == "--testar-webhook") {
        // envia uma notificação de teste a cada destino e sai (0 = todos aceitaram)
        let d = destinos(&cfg).unwrap_or_else(|e| {
            eprintln!("falhou: {e}");
            std::process::exit(1);
        });
        if d.is_empty() {
            eprintln!("falhou: defina NOBREAK_WEBHOOK_URL e/ou NOBREAK_NTFY_URL");
            std::process::exit(1);
        }
        let mut ok = true;
        for d in &d {
            let n = notificar::Notif {
                evento: "teste",
                gravidade: alarmes::Gravidade::Info,
                titulo: format!("{}: teste", cfg.notificar_nome),
                texto: "Notificação de teste do nobreak-ragtech.".into(),
                t: agora(),
            };
            match d.enviar(&n) {
                Ok(c) => println!("{}: respondeu HTTP {c}", d.nome),
                Err(e) => {
                    eprintln!("{}: falhou: {e}", d.nome);
                    ok = false;
                }
            }
        }
        std::process::exit(if ok { 0 } else { 1 });
    }
    let notificador = match destinos(&cfg)
        .and_then(|d| Ok((d, notificar::parse_filtro(&cfg.notificar_eventos)?)))
    {
        Ok((d, filtro)) if !d.is_empty() => Some(Notificador::iniciar(
            Detector::novo(&cfg.notificar_nome, filtro),
            d,
        )),
        Ok(_) => None,
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

#[cfg(test)]
mod testes_nut {
    use super::*;

    #[test]
    fn sem_acentos_limpa_texto() {
        assert_eq!(
            sem_acentos("Bateria baixa/Falta de rede elétrica"),
            "Bateria baixa/Falta de rede eletrica"
        );
        assert_eq!(sem_acentos("tensão de saída"), "tensao de saida");
    }

    #[test]
    fn lb_derivado_entra_quando_a_autonomia_acaba() {
        let mut e = Estado {
            status: Some("OB".to_string()),
            autonomia_s: Some(120),
            ..Default::default()
        };
        assert_eq!(status_para_nut(&e, Some(300)), "OB LB"); // 2 min < 5 min
        assert_eq!(status_para_nut(&e, Some(60)), "OB"); // 2 min > 1 min
        assert_eq!(status_para_nut(&e, None), "OB"); // recurso desligado
        e.status = Some("OL".to_string());
        assert_eq!(status_para_nut(&e, Some(300)), "OL"); // com rede nunca inventa LB
        e.status = Some("OB LB".to_string());
        assert_eq!(status_para_nut(&e, Some(300)), "OB LB"); // não duplica
    }
}
