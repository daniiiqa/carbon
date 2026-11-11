//! La columna de la derecha: la discada en la lumbre mientras se actualiza
//! (meneándose con la cuchara, el vapor subiendo, la lumbre viva), el vaquero
//! bailando cuando queda lista, y la lumbre apagada si algo falló.
//!
//! Era `farula meneo` / `farula lista`, más chico y en ANSI; aquí es una
//! rejilla de celdas con los 16 colores de la terminal (los del look).

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Ancho y alto del dibujo, en celdas.
pub const W: usize = 40;
pub const H: usize = 13;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tono {
    Nada,
    Vapor,
    Humo,
    Cuchara,
    Orilla,
    Carne,
    Grasa,
    Lumbre,
    Lumbre2,
    Brasa,
    Ceniza,
    Suelo,
    Nota,
}

impl Tono {
    fn estilo(self) -> Style {
        let s = Style::new();
        match self {
            Tono::Nada => s,
            Tono::Vapor => s.fg(Color::Gray).add_modifier(Modifier::DIM),
            Tono::Humo => s.fg(Color::DarkGray),
            Tono::Cuchara => s.fg(Color::LightYellow).add_modifier(Modifier::BOLD),
            Tono::Orilla => s.fg(Color::White).add_modifier(Modifier::BOLD),
            Tono::Carne => s.fg(Color::Red).add_modifier(Modifier::BOLD),
            Tono::Grasa => s.fg(Color::Yellow),
            Tono::Lumbre => s.fg(Color::LightRed).add_modifier(Modifier::BOLD),
            Tono::Lumbre2 => s.fg(Color::LightYellow).add_modifier(Modifier::BOLD),
            Tono::Brasa => s.fg(Color::Red),
            Tono::Ceniza => s.fg(Color::DarkGray),
            Tono::Suelo => s.fg(Color::DarkGray),
            Tono::Nota => s.fg(Color::LightMagenta).add_modifier(Modifier::BOLD),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Escena {
    /// se está actualizando
    Meneo,
    /// quedó
    Lista,
    /// algo falló
    Apagada,
}

struct Rejilla(Vec<Vec<(char, Tono)>>);

impl Rejilla {
    fn new() -> Rejilla {
        Rejilla(vec![vec![(' ', Tono::Nada); W]; H])
    }
    fn put(&mut self, r: usize, c: usize, ch: char, t: Tono) {
        if r < H && c < W {
            self.0[r][c] = (ch, t);
        }
    }
    fn texto(&mut self, r: usize, c: usize, s: &str, t: Tono) {
        for (k, ch) in s.chars().enumerate() {
            if ch != ' ' {
                self.put(r, c + k, ch, t);
            }
        }
    }
    fn lineas(&self) -> Vec<Line<'static>> {
        self.0
            .iter()
            .map(|fila| {
                let mut spans: Vec<Span<'static>> = Vec::new();
                let mut buf = String::new();
                let mut tono = Tono::Nada;
                for &(ch, t) in fila {
                    if t != tono && !buf.is_empty() {
                        spans.push(Span::styled(std::mem::take(&mut buf), tono.estilo()));
                    }
                    tono = t;
                    buf.push(ch);
                }
                spans.push(Span::styled(buf, tono.estilo()));
                Line::from(spans)
            })
            .collect()
    }
}

/// Ruido que no se repite a la vista pero es el mismo para (a, b): las llamas,
/// el vapor y las chispas no van en loop mecánico.
fn ruido(a: usize, b: usize) -> usize {
    let mut x = a.wrapping_mul(2_654_435_761) ^ b.wrapping_mul(40_503).wrapping_add(0x9e37);
    x ^= x >> 13;
    x = x.wrapping_mul(0x5bd1_e995);
    x ^ (x >> 15)
}

/// La discada (orilla, dos capas de carne que giran en sentidos contrarios, el
/// fondo, el soporte) del renglón 4 al 9.
fn discada(g: &mut Rejilla, frame: usize, quieta: bool) {
    for c in 4..36 {
        g.put(4, c, '_', Tono::Orilla);
    }
    g.put(5, 3, '\\', Tono::Orilla);
    g.put(5, 36, '/', Tono::Orilla);
    g.put(6, 4, '\\', Tono::Orilla);
    g.put(6, 35, '/', Tono::Orilla);
    g.put(7, 5, '\\', Tono::Orilla);
    g.put(7, 34, '/', Tono::Orilla);
    for c in 6..34 {
        g.put(7, c, '_', Tono::Orilla);
    }
    let carne: Vec<char> = "°~·~*~°~·~ ~°·~*~ ·~°~·~*~".chars().collect();
    let n = carne.len();
    let paso = if quieta { 0 } else { frame };
    for (fila, (desde, hasta), dir) in [(5usize, (4usize, 36usize), 1isize), (6, (5, 35), -1)] {
        for c in desde..hasta {
            let i = (c as isize + dir * paso as isize).rem_euclid(n as isize) as usize;
            let ch = carne[(i + fila * 3) % n];
            if ch != ' ' {
                g.put(fila, c, ch, if matches!(ch, '°' | '*') { Tono::Carne } else { Tono::Grasa });
            }
        }
    }
    // el soporte: patas que abren hacia abajo
    for (r, izq, der) in [(8usize, 9usize, 30usize), (9, 8, 31), (10, 7, 32)] {
        g.put(r, izq, if r == 8 { '|' } else { '/' }, Tono::Orilla);
        g.put(r, der, if r == 8 { '|' } else { '\\' }, Tono::Orilla);
    }
    // el suelo
    for c in 4..36 {
        g.put(12, c, '─', Tono::Suelo);
    }
}

/// La cuchara: la punta en la carne yendo y viniendo, el mango hacia arriba a
/// la derecha (cruza el vapor).
fn cuchara(g: &mut Rejilla, frame: usize) {
    const RUTA: [usize; 14] = [8, 10, 12, 14, 16, 18, 20, 22, 24, 22, 20, 18, 14, 10];
    let punta = RUTA[frame % RUTA.len()];
    g.put(5, punta, 'U', Tono::Cuchara);
    for k in 1..5 {
        g.put(5 - k, punta + k, '/', Tono::Cuchara);
    }
    g.put(0, punta + 5, '▪', Tono::Cuchara);
}

/// Lo que sube de la discada: vapor que sube un renglón cada dos cuadros
/// (lo de arriba es lo que estuvo abajo), más ralo mientras más alto.
fn vapor(g: &mut Rejilla, frame: usize, tono: Tono, ancho: (usize, usize), densidad: [usize; 4]) {
    const CH: [char; 5] = ['(', ')', '\'', '.', '~'];
    for (r, &dens) in densidad.iter().enumerate() {
        let t = frame / 2 + (3 - r);
        for c in ancho.0..ancho.1 {
            let v = ruido(c, t);
            if v % 100 < dens {
                g.put(r, c, CH[(v / 100) % CH.len()], tono);
            }
        }
    }
}

/// La lumbre: llamas que parpadean, brasas abajo y alguna chispa que sube.
fn lumbre(g: &mut Rejilla, frame: usize) {
    for c in 11..29 {
        // las llamas, más altas al centro
        let centro = 10 - (c as isize - 19).unsigned_abs().min(9);
        let v = ruido(c, frame);
        if v % 12 < centro {
            let ch = ['^', 'ʌ', '^', '"'][v % 4];
            g.put(10, c, ch, if (v / 7).is_multiple_of(2) { Tono::Lumbre } else { Tono::Lumbre2 });
        }
        if v % 20 < centro / 2 {
            g.put(9, c, ['\'', ',', '`'][v % 3], Tono::Lumbre2);
        }
        let b = ruido(c, frame / 2 + 777);
        g.put(11, c, ['▁', '▂', '▃', '▂'][b % 4], if b.is_multiple_of(3) { Tono::Lumbre } else { Tono::Brasa });
    }
    // una chispa de vez en cuando, subiendo por un lado de la discada
    let k = frame % 8;
    let col = 11 + ruido(frame / 8, 3) % 18;
    if k < 4 {
        let lado = if col < 20 { 2 + col / 4 } else { 37 - (col - 20) / 4 };
        g.put(9 - k * 2, lado.min(W - 1), '·', Tono::Lumbre2);
    }
}

fn apagada(g: &mut Rejilla, frame: usize) {
    for c in 11..29 {
        let v = ruido(c, 99);
        if v.is_multiple_of(3) {
            g.put(11, c, '▁', Tono::Ceniza);
        }
        if v.is_multiple_of(7) {
            g.put(10, c, '.', Tono::Ceniza);
        }
    }
    // una brasa que todavía no se apaga del todo
    if frame % 6 < 3 {
        g.put(11, 19, '▂', Tono::Brasa);
    }
}

const BAILE: [[&str; 3]; 4] = [
    ["\\(^o^)/", "   |   ", "  / \\  "],
    [" (^o^) ", "  /|\\  ", "  / \\  "],
    [" (^o^)/", "  /|   ", "  / >  "],
    ["\\(^o^) ", "   |\\  ", "  < \\  "],
];

/// El vaquero (sombrero y todo) bailando, la discada servida a un lado y las
/// notas subiendo.
fn lista(g: &mut Rejilla, frame: usize) {
    let b = BAILE[(frame / 2) % BAILE.len()];
    // con los pies en el suelo (renglón 12), meciéndose de lado a lado
    let x = 9 + if (frame / 4).is_multiple_of(2) { 0 } else { 1 };
    g.texto(7, x + 1, " _n_ ", Tono::Cuchara);
    g.texto(8, x, "/_|_\\", Tono::Cuchara);
    for (k, l) in b.iter().enumerate() {
        g.texto(9 + k, x - 1, l, Tono::Orilla);
    }
    // la discada servida en el suelo, humeando
    g.texto(10, 24, "\\~°~*~°/", Tono::Carne);
    g.texto(11, 25, "\\____/", Tono::Orilla);
    for r in 7..10 {
        let v = ruido(r, frame / 2 + 12 - r);
        if !v.is_multiple_of(3) {
            g.put(r, 25 + v % 6, ['(', ')', '\''][v % 3], Tono::Vapor);
        }
    }
    // las notas suben por encima de él y se van (renglones 0 a 5)
    for n in 0..3 {
        let t = frame / 2 + n * 5;
        let r = 5 - (t % 6).min(5);
        let c = 3 + (ruido(t / 6, n) % 30);
        g.put(r, c, ['♪', '♫', '♬'][n], Tono::Nota);
    }
    for c in 4..36 {
        g.put(12, c, '─', Tono::Suelo);
    }
}

/// Un cuadro de la escena.
pub fn cuadro(escena: Escena, frame: usize) -> Vec<Line<'static>> {
    let mut g = Rejilla::new();
    match escena {
        Escena::Meneo => {
            vapor(&mut g, frame, Tono::Vapor, (6, 34), [5, 8, 12, 16]);
            discada(&mut g, frame, false);
            cuchara(&mut g, frame);
            lumbre(&mut g, frame);
        }
        Escena::Apagada => {
            // humo gris, angosto, del centro
            vapor(&mut g, frame, Tono::Humo, (14, 26), [6, 10, 16, 22]);
            discada(&mut g, frame, true);
            apagada(&mut g, frame);
        }
        Escena::Lista => lista(&mut g, frame),
    }
    g.lineas()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texto(l: &[Line]) -> Vec<String> {
        l.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect()
    }

    #[test]
    fn cada_cuadro_cabe() {
        for e in [Escena::Meneo, Escena::Lista, Escena::Apagada] {
            for f in 0..64 {
                let l = cuadro(e, f);
                assert_eq!(l.len(), H);
                assert!(texto(&l).iter().all(|r| r.chars().count() == W), "{e:?} {f}");
            }
        }
    }

    #[test]
    fn se_mueve() {
        assert_ne!(texto(&cuadro(Escena::Meneo, 0)), texto(&cuadro(Escena::Meneo, 1)));
        assert_ne!(texto(&cuadro(Escena::Lista, 0)), texto(&cuadro(Escena::Lista, 4)));
    }

    /// `cargo test vistazo -- --ignored --nocapture`: los cuadros en texto.
    #[test]
    #[ignore]
    fn vistazo() {
        for e in [Escena::Meneo, Escena::Apagada, Escena::Lista] {
            println!("── {e:?}");
            for r in texto(&cuadro(e, 5)) {
                println!("{r}");
            }
        }
    }
}
