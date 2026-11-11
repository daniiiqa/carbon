//! carbon — el updater de discada: echarle carbón nuevo al asador.
//!
//!   carbon              actualizar discada (los pasos y su log a la
//!                       izquierda, la discada en la lumbre a la derecha)
//!   carbon prueba       la misma pantalla con pasos de mentira (no toca nada)
//!   carbon revisa REPO BASE
//!                       qué archivos tuyos toca la actualización (lo usa
//!                       install.sh --update; era UpdateCheck.py)

mod arte;
mod linea;
mod pasos;
mod revisa;
mod ui;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let es = pasos::spanish();
    let codigo = match args.first().map(String::as_str) {
        Some("revisa") => revisa::main(&args[1..]),
        Some("-h" | "--help") => {
            println!("carbon               {}", if es { "actualizar discada" } else { "update discada" });
            println!("carbon prueba        {}", if es { "la pantalla con pasos de mentira (no toca nada)" } else { "the screen with fake steps (touches nothing)" });
            println!("carbon revisa REPO BASE");
            0
        }
        Some("-V" | "--version") => {
            println!("carbon {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some("prueba") => {
            let plan = pasos::plan(&pasos::Modo::Prueba, es);
            if ui::correr(plan, es, true) { 0 } else { 1 }
        }
        None => match pasos::modo() {
            Some(m) => {
                let plan = pasos::plan(&m, es);
                if ui::correr(plan, es, false) { 0 } else { 1 }
            }
            None => {
                eprintln!(
                    "carbon: {}",
                    if es { "no encontré el repo de discada (~/.config ni ~/dotfiles)" } else { "couldn't find the discada repo (~/.config or ~/dotfiles)" }
                );
                1
            }
        },
        Some(otro) => {
            eprintln!("carbon: {otro}? (carbon --help)");
            2
        }
    };
    ExitCode::from(codigo as u8)
}
