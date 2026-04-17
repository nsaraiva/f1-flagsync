use anyhow::{anyhow, Result};
use std::io::{self, Write};

pub enum MenuAction {
    Send(u16),
    Quit,
}

struct F1Preset {
    name: &'static str,
    value: u16,
}

const F1_BC_PRESETS: [F1Preset; 6] = [
    F1Preset {
        name: "Bandeira verde",
        value: 120,
    },
    F1Preset {
        name: "Bandeira amarela",
        value: 60,
    },
    F1Preset {
        name: "Bandeira vermelha",
        value: 0,
    },
    F1Preset {
        name: "Safety Car (SC)",
        value: 60,
    },
    F1Preset {
        name: "Virtual Safety Car (VSC)",
        value: 60,
    },
    F1Preset {
        name: "Checkered (quadriculada)",
        value: 0,
    },
];

pub fn read_preset_action() -> Result<MenuAction> {
    println!("\nPresets F1 (protocolo BC):");
    for (i, preset) in F1_BC_PRESETS.iter().enumerate() {
        println!(
            "[{}] {} -> {} (0x{:04X})",
            i + 1,
            preset.name,
            preset.value,
            preset.value
        );
    }
    println!("[q] Sair");
    print!("Preset: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if input.trim().eq_ignore_ascii_case("q") {
        return Ok(MenuAction::Quit);
    }

    let idx: usize = input
        .trim()
        .parse()
        .map_err(|_| anyhow!("Indice invalido"))?;

    if idx == 0 || idx > F1_BC_PRESETS.len() {
        return Err(anyhow!("Indice fora do intervalo de presets"));
    }

    let preset = &F1_BC_PRESETS[idx - 1];
    println!("Preset selecionado: {} (valor={})", preset.name, preset.value);
    Ok(MenuAction::Send(preset.value))
}
