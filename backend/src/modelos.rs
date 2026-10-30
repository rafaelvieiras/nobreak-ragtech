//! Tabela de modelos da família 10 (NEP/Toro/Innergie, Easy Pro etc.).
//! Os números são fatos do protocolo (escalas dos registradores), obtidos por engenharia reversa.

pub struct Modelo {
    pub nome: &'static str,
    /// volts por unidade do registrador 0x8A
    pub escala_vbat: f32,
    /// ampères por unidade do registrador 0x8C (antes de dividir pelo valor de calibração 0xF3)
    pub escala_isaida: f32,
    /// modelos "M2" são 220 V; "TI" são 115 V
    pub tensao_220: bool,
}

pub const FAMILIA_10: [Modelo; 20] = [
    Modelo {
        nome: "NEP/Toro/Innergie 600 TI",
        escala_vbat: 0.067,
        escala_isaida: 3.2,
        tensao_220: false,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 600 M2",
        escala_vbat: 0.067,
        escala_isaida: 1.7,
        tensao_220: true,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 700 TI",
        escala_vbat: 0.067,
        escala_isaida: 4.3,
        tensao_220: false,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 700 M2",
        escala_vbat: 0.067,
        escala_isaida: 2.3,
        tensao_220: true,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 900 TI",
        escala_vbat: 0.067,
        escala_isaida: 4.94,
        tensao_220: false,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 900 M2",
        escala_vbat: 0.067,
        escala_isaida: 2.64,
        tensao_220: true,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 1200 TI",
        escala_vbat: 0.067,
        escala_isaida: 6.37,
        tensao_220: false,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 1200 M2",
        escala_vbat: 0.067,
        escala_isaida: 3.41,
        tensao_220: true,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 1300 TI",
        escala_vbat: 0.134,
        escala_isaida: 9.56,
        tensao_220: false,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 1300 M2",
        escala_vbat: 0.134,
        escala_isaida: 5.11,
        tensao_220: true,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 1400 TI",
        escala_vbat: 0.134,
        escala_isaida: 11.15,
        tensao_220: false,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 1400 M2",
        escala_vbat: 0.134,
        escala_isaida: 5.96,
        tensao_220: true,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 1600 TI",
        escala_vbat: 0.134,
        escala_isaida: 12.8,
        tensao_220: false,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 1600 M2",
        escala_vbat: 0.134,
        escala_isaida: 6.8,
        tensao_220: true,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 1800 TI",
        escala_vbat: 0.134,
        escala_isaida: 14.4,
        tensao_220: false,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 1800 M2",
        escala_vbat: 0.134,
        escala_isaida: 7.65,
        tensao_220: true,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 2000 TI",
        escala_vbat: 0.134,
        escala_isaida: 15.93,
        tensao_220: false,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 2000 M2",
        escala_vbat: 0.134,
        escala_isaida: 8.52,
        tensao_220: true,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 2200 TI",
        escala_vbat: 0.134,
        escala_isaida: 17.52,
        tensao_220: false,
    },
    Modelo {
        nome: "NEP/Toro/Innergie 2200 M2",
        escala_vbat: 0.134,
        escala_isaida: 9.37,
        tensao_220: true,
    },
];

pub fn buscar(indice: u8) -> Option<&'static Modelo> {
    FAMILIA_10.get(indice as usize)
}
