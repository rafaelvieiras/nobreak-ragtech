//! Série longa (dias a semanas): a série em memória guarda só 24 h, então aqui as leituras
//! viram baldes de 15 min com média, mínimo e máximo de cada medida. O mínimo e o máximo
//! importam: numa queda de 49 s a média de 15 min esconde a falta de energia, o mínimo não.
//! Cada balde fechado é anexado a um arquivo `.jsonl` opcional, para sobreviver a reinícios.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Medidas guardadas, na ordem de `Bucket::m`.
pub const MEDIDAS: usize = 6;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Bucket {
    /// Início do balde (segundos Unix).
    pub t: u64,
    /// Leituras no balde.
    pub n: u32,
    /// Por medida: média, mínimo e máximo.
    /// Ordem: entrada_v, saida_v, bateria_pct, bateria_v, carga_pct, temperatura_c.
    pub m: [[f32; 3]; MEDIDAS],
}

impl Bucket {
    /// Junta dois baldes do mesmo intervalo (o serviço reiniciou no meio dele).
    fn juntar(&mut self, o: &Bucket) {
        let (a, b) = (self.n as f32, o.n as f32);
        for i in 0..MEDIDAS {
            self.m[i][0] = (self.m[i][0] * a + o.m[i][0] * b) / (a + b).max(1.0);
            self.m[i][1] = self.m[i][1].min(o.m[i][1]);
            self.m[i][2] = self.m[i][2].max(o.m[i][2]);
        }
        self.n = self.n.saturating_add(o.n);
    }
}

#[derive(Default)]
struct Acumulado {
    soma: [f64; MEDIDAS],
    min: [f32; MEDIDAS],
    max: [f32; MEDIDAS],
    n: u32,
}

pub struct SerieLonga {
    baldes: VecDeque<Bucket>,
    intervalo_s: u64,
    max_baldes: usize,
    indice: u64,
    atual: Acumulado,
}

impl SerieLonga {
    pub fn nova(dias: u64, intervalo_s: u64) -> Self {
        let intervalo_s = intervalo_s.max(1);
        SerieLonga {
            baldes: VecDeque::new(),
            intervalo_s,
            max_baldes: ((dias.max(1) * 86_400) / intervalo_s).max(2) as usize,
            indice: 0,
            atual: Acumulado::default(),
        }
    }

    pub fn max_baldes(&self) -> usize {
        self.max_baldes
    }

    fn inserir(&mut self, b: Bucket) {
        match self.baldes.back_mut() {
            Some(u) if u.t == b.t => u.juntar(&b),
            _ => self.baldes.push_back(b),
        }
        while self.baldes.len() > self.max_baldes {
            self.baldes.pop_front();
        }
    }

    /// Baldes lidos do arquivo (mais antigos primeiro).
    pub fn restaurar(&mut self, v: Vec<Bucket>) {
        for b in v {
            // o arquivo pode ter dois baldes do mesmo intervalo (reinício): o último vale
            if self.baldes.back().map(|u| u.t) == Some(b.t) {
                self.baldes.pop_back();
            }
            self.inserir(b);
        }
    }

    fn fechar(&mut self) -> Option<Bucket> {
        let a = std::mem::take(&mut self.atual);
        if a.n == 0 {
            return None;
        }
        let mut m = [[0.0f32; 3]; MEDIDAS];
        for (i, x) in m.iter_mut().enumerate() {
            *x = [(a.soma[i] / a.n as f64) as f32, a.min[i], a.max[i]];
        }
        Some(Bucket {
            t: self.indice * self.intervalo_s,
            n: a.n,
            m,
        })
    }

    /// Soma uma leitura. Devolve o balde que acabou de fechar, para gravar no arquivo.
    pub fn amostrar(&mut self, t: u64, v: [f32; MEDIDAS]) -> Option<Bucket> {
        if v.iter().any(|x| !x.is_finite()) {
            return None;
        }
        let idx = t / self.intervalo_s;
        let mut fechado = None;
        if idx != self.indice {
            if let Some(b) = self.fechar() {
                self.inserir(b.clone());
                fechado = Some(b);
            }
            self.indice = idx;
        }
        let a = &mut self.atual;
        for (i, x) in v.iter().enumerate() {
            if a.n == 0 {
                a.min[i] = *x;
                a.max[i] = *x;
            } else {
                a.min[i] = a.min[i].min(*x);
                a.max[i] = a.max[i].max(*x);
            }
            a.soma[i] += *x as f64;
        }
        a.n += 1;
        fechado
    }

    /// Série em colunas. O balde em andamento entra como último ponto.
    pub fn json(&self) -> String {
        let mut v: Vec<Bucket> = self.baldes.iter().cloned().collect();
        if self.atual.n > 0 {
            let a = &self.atual;
            let mut m = [[0.0f32; 3]; MEDIDAS];
            for (i, x) in m.iter_mut().enumerate() {
                *x = [(a.soma[i] / a.n as f64) as f32, a.min[i], a.max[i]];
            }
            let parcial = Bucket {
                t: self.indice * self.intervalo_s,
                n: a.n,
                m,
            };
            match v.last_mut() {
                Some(u) if u.t == parcial.t => u.juntar(&parcial),
                _ => v.push(parcial),
            }
        }
        // [medida][0=média,1=mín,2=máx] -> colunas
        let col = |i: usize, k: usize| -> Vec<f32> { v.iter().map(|b| b.m[i][k]).collect() };
        let nomes = [
            "entrada_v",
            "saida_v",
            "bateria_pct",
            "bateria_v",
            "carga_pct",
            "temperatura_c",
        ];
        let mut o = serde_json::Map::new();
        o.insert("intervalo_s".into(), self.intervalo_s.into());
        o.insert("t".into(), v.iter().map(|b| b.t).collect::<Vec<_>>().into());
        for (i, nome) in nomes.iter().enumerate() {
            o.insert(format!("{nome}_media"), col(i, 0).into());
            o.insert(format!("{nome}_min"), col(i, 1).into());
            o.insert(format!("{nome}_max"), col(i, 2).into());
        }
        serde_json::Value::Object(o).to_string()
    }
}

/// Extrai baldes de texto `.jsonl`, tolerando bytes nulos e linhas truncadas de queda de energia.
fn do_texto(bytes: &[u8]) -> Vec<Bucket> {
    String::from_utf8_lossy(bytes)
        .split('\n')
        .filter_map(|l| {
            serde_json::from_str(l.trim_matches(|c: char| c == '\0' || c.is_whitespace())).ok()
        })
        .collect()
}

/// Lê o arquivo e o reescreve limpo.
pub fn ler_arquivo(caminho: &str) -> Vec<Bucket> {
    let bytes = std::fs::read(caminho).unwrap_or_default();
    let v = do_texto(&bytes);
    let limpo: String = v
        .iter()
        .filter_map(|b| serde_json::to_string(b).ok())
        .map(|l| l + "\n")
        .collect();
    if !bytes.is_empty() && limpo.as_bytes() != bytes.as_slice() {
        let _ = std::fs::write(caminho, limpo);
    }
    v
}

/// Acrescenta um balde. Quando o arquivo passa de 2x o tamanho útil, reescreve só com os recentes.
pub fn anexar_arquivo(caminho: &str, b: &Bucket, max_baldes: usize) {
    use std::io::Write;
    let Ok(linha) = serde_json::to_string(b) else {
        return;
    };
    let grande = std::fs::metadata(caminho)
        .map(|m| m.len() > (max_baldes as u64) * 2 * (linha.len() as u64 + 20))
        .unwrap_or(false);
    if grande {
        let mut v = ler_arquivo(caminho);
        v.push(b.clone());
        let corte = v.len().saturating_sub(max_baldes);
        let txt: String = v[corte..]
            .iter()
            .filter_map(|b| serde_json::to_string(b).ok())
            .map(|l| l + "\n")
            .collect();
        let _ = std::fs::write(caminho, txt);
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(caminho)
    {
        let _ = writeln!(f, "{linha}");
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn leitura(entrada: f32) -> [f32; MEDIDAS] {
        [entrada, 220.0, 100.0, 13.3, 20.0, 50.0]
    }

    #[test]
    fn fecha_balde_com_media_minimo_e_maximo() {
        let mut s = SerieLonga::nova(30, 900);
        assert!(s.amostrar(0, leitura(220.0)).is_none());
        assert!(s.amostrar(60, leitura(0.0)).is_none()); // queda curta
        assert!(s.amostrar(120, leitura(230.0)).is_none());
        let b = s.amostrar(900, leitura(220.0)).expect("fechou");
        assert_eq!((b.t, b.n), (0, 3));
        assert!((b.m[0][0] - 150.0).abs() < 0.1);
        assert_eq!((b.m[0][1], b.m[0][2]), (0.0, 230.0));
    }

    #[test]
    fn respeita_o_limite_de_baldes() {
        let mut s = SerieLonga::nova(1, 3600); // 24 baldes
        assert_eq!(s.max_baldes(), 24);
        for h in 0..40u64 {
            s.amostrar(h * 3600, leitura(220.0));
        }
        let j: serde_json::Value = serde_json::from_str(&s.json()).unwrap();
        // 24 guardados + o balde em andamento
        assert_eq!(j["t"].as_array().unwrap().len(), 25);
        assert_eq!(j["t"][0], 15 * 3600);
    }

    #[test]
    fn json_inclui_balde_em_andamento() {
        let mut s = SerieLonga::nova(30, 900);
        s.amostrar(1000, leitura(220.0));
        let j: serde_json::Value = serde_json::from_str(&s.json()).unwrap();
        assert_eq!(j["t"].as_array().unwrap().len(), 1);
        assert_eq!(j["entrada_v_min"][0], 220.0);
    }

    #[test]
    fn leitura_invalida_e_ignorada() {
        let mut s = SerieLonga::nova(30, 900);
        s.amostrar(0, [f32::NAN, 1.0, 1.0, 1.0, 1.0, 1.0]);
        let j: serde_json::Value = serde_json::from_str(&s.json()).unwrap();
        assert!(j["t"].as_array().unwrap().is_empty());
    }

    #[test]
    fn reinicio_no_mesmo_intervalo_junta_os_baldes() {
        let mut antes = SerieLonga::nova(30, 900);
        antes.amostrar(0, leitura(220.0));
        antes.amostrar(60, leitura(220.0));
        let b1 = antes.fechar().unwrap();
        let mut s = SerieLonga::nova(30, 900);
        s.restaurar(vec![b1]);
        s.amostrar(300, leitura(100.0)); // mesmo balde (t=0) depois do reinício
        let j: serde_json::Value = serde_json::from_str(&s.json()).unwrap();
        assert_eq!(j["t"].as_array().unwrap().len(), 1);
        assert_eq!(j["entrada_v_min"][0], 100.0);
        assert_eq!(j["entrada_v_max"][0], 220.0);
        let media = j["entrada_v_media"][0].as_f64().unwrap();
        assert!((media - 180.0).abs() < 0.1);
    }

    #[test]
    fn arquivo_ida_e_volta_e_tolera_lixo() {
        let p = std::env::temp_dir().join(format!("longa-{}.jsonl", std::process::id()));
        let p = p.to_str().unwrap();
        let _ = std::fs::remove_file(p);
        let mut s = SerieLonga::nova(30, 900);
        s.amostrar(0, leitura(220.0));
        let b = s.amostrar(900, leitura(220.0)).unwrap();
        anexar_arquivo(p, &b, 100);
        // bloco de nulos que uma queda de energia deixa no fim, colado numa linha quebrada
        let mut dados = std::fs::read(p).unwrap();
        dados.extend_from_slice(&[0u8; 32]);
        dados.extend_from_slice(b"{\"t\":900,\"n\":");
        std::fs::write(p, dados).unwrap();
        let v = ler_arquivo(p);
        assert_eq!(v, vec![b]);
        // reescreveu limpo
        assert!(!std::fs::read(p).unwrap().contains(&0u8));
        let _ = std::fs::remove_file(p);
    }
}
