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
    Verde,
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
            Tono::Verde => s.fg(Color::LightGreen).add_modifier(Modifier::BOLD),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Escena {
    /// «preparar»: un cerillo prende el carbón pieza por pieza
    Prender,
    /// «bajar discada»: llega la troca con la carga (los commits nuevos)
    Troca,
    /// se está actualizando
    Meneo,
    /// «instalar lo nuevo»: cada herramienta cae a la discada como ingrediente
    Ingredientes,
    /// quedó
    Lista,
    /// algo falló
    Apagada,
}

impl Escena {
    /// Cuadros que se queda aunque su paso acabe antes (si no, «preparar», que
    /// tarda una décima, ni se vería).
    pub fn minimo(self) -> usize {
        match self {
            Escena::Prender => 30,
            Escena::Troca => 44,
            _ => 0,
        }
    }
}

/// Lo que la escena necesita saber además del tiempo.
#[derive(Default, Clone)]
pub struct Extra {
    /// commits nuevos: la carga de la troca
    pub commits: usize,
    /// el último ingrediente: su nombre y hace cuántos cuadros cayó
    pub ingrediente: Option<(String, usize)>,
    /// la fiesta (el easter egg): hace cuántos cuadros empezó
    pub fiesta: Option<usize>,
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

/// Las piezas de carbón, de la derecha (donde llega el cerillo) a la izquierda.
const CARBONES: [usize; 8] = [26, 24, 22, 20, 18, 16, 14, 12];

/// «preparar»: llega un cerillo, raspa y prende el carbón pieza por pieza;
/// cuando agarra, salen las primeras llamas.
fn prender(g: &mut Rejilla, t: usize, frame: usize) {
    vapor(g, frame, Tono::Humo, (16, 26), [0, 2, 4, 6]);
    discada(g, frame, true);
    let prendidos = t.saturating_sub(10) / 2;
    for (k, &c) in CARBONES.iter().enumerate() {
        let tono = if k < prendidos {
            if ruido(c, frame).is_multiple_of(3) { Tono::Lumbre2 } else { Tono::Brasa }
        } else {
            Tono::Ceniza
        };
        g.put(11, c, '▄', tono);
        g.put(11, c + 1, '▄', tono);
        // ya agarró: llamitas encima de lo prendido
        if k < prendidos && t > 22 && !ruido(c, frame + 5).is_multiple_of(3) {
            g.put(10, c, ['^', '\'', ','][ruido(c, frame) % 3], Tono::Lumbre);
        }
    }
    // el cerillo: entra por el suelo desde la derecha, debajo de la discada,
    // hasta el primer carbón, y raspa
    if t < 14 {
        let c = (39 - t.min(7) * 2).max(29);
        g.put(11, c, if t >= 6 { '●' } else { '•' }, if t >= 6 { Tono::Lumbre2 } else { Tono::Carne });
        for k in 1..4 {
            g.put(11, c + k, '─', Tono::Grasa);
        }
        if (6..11).contains(&t) {
            g.put(10, c, ['*', '+', '·'][t % 3], Tono::Lumbre2);
            g.put(10, c - 1, ['·', '*', '\''][t % 3], Tono::Lumbre);
        }
    }
}

/// El Cerro de la Silla, al fondo (renglones 2 a 4).
fn cerro(g: &mut Rejilla, desde: usize) {
    for (r, l) in ["          /\\        /\\", "    _/\\  /  \\_/\\   /  \\__", " __/   \\/        \\_/      \\___"].iter().enumerate() {
        g.texto(desde + r, 4, l, Tono::Suelo);
    }
}

/// «bajar discada»: entra la troca con la carga, se para y baja el costal;
/// arriba, cuántos cambios trae.
fn troca(g: &mut Rejilla, t: usize, frame: usize, commits: usize) {
    cerro(g, 2);
    for c in 0..W {
        g.put(12, c, '─', Tono::Suelo);
    }
    const TROCA: [&str; 4] = ["  ______", " /|_||_\\`.__", "(   _    _ _\\", "=`-(_)--(_)-'"];
    // entra de la izquierda, un renglón por cuadro, y se para en la 11
    let x = (t as isize - 16).min(11);
    for (k, l) in TROCA.iter().enumerate() {
        for (j, ch) in l.chars().enumerate() {
            let c = x + j as isize;
            if ch != ' ' && c >= 0 {
                g.put(8 + k, c as usize, ch, Tono::Orilla);
            }
        }
    }
    let parada = t >= 27;
    // la carga en la caja, y el polvo atrás mientras va
    if !parada || t < 32 {
        for j in 8..11 {
            let c = x + j;
            if c >= 0 {
                g.put(8, c as usize, '▄', Tono::Ceniza);
            }
        }
    }
    if !parada {
        for k in 1..5 {
            let c = x - k;
            if c >= 0 && !ruido(c as usize, frame).is_multiple_of(3) {
                g.put(11, c as usize, ['.', ',', '\''][ruido(k as usize, frame) % 3], Tono::Humo);
            }
        }
    } else {
        // el costal cae junto a la troca
        let caida = (t - 32).min(3);
        if t >= 32 {
            g.texto(8 + caida, 27, "[▓▓]", Tono::Ceniza);
        }
        if t >= 34 {
            let txt = if commits == 0 { "al día".to_string() } else { format!("+{commits}") };
            g.texto(6, 27, &txt, Tono::Nota);
        }
    }
}

/// El ingrediente de una herramienta: siempre el mismo para el mismo nombre.
fn ingrediente_de(nombre: &str) -> (&'static str, Tono) {
    const TODOS: [(&str, Tono); 7] = [
        ("(@)", Tono::Orilla),   // cebolla
        (")>", Tono::Verde),     // jalapeño
        ("(o)", Tono::Carne),    // tomate
        ("=-=", Tono::Carne),    // chorizo
        ("~~~", Tono::Grasa),    // tocino
        ("<>", Tono::Lumbre2),   // pimiento
        ("[#]", Tono::Brasa),    // carne
    ];
    let h = nombre.bytes().fold(7usize, |a, b| a.wrapping_mul(31).wrapping_add(b as usize));
    TODOS[h % TODOS.len()]
}

/// «instalar lo nuevo»: la discada meneándose y cada herramienta cayendo como
/// ingrediente, con su nombre, hasta la carne (y salpica).
fn ingredientes(g: &mut Rejilla, frame: usize, ing: Option<&(String, usize)>) {
    vapor(g, frame, Tono::Vapor, (6, 34), [5, 8, 12, 16]);
    discada(g, frame, false);
    cuchara(g, frame);
    lumbre(g, frame);
    let Some((nombre, hace)) = ing else { return };
    let h = nombre.bytes().fold(3usize, |a, b| a.wrapping_mul(17).wrapping_add(b as usize));
    let c = 6 + h % 20;
    let (glifo, tono) = ingrediente_de(nombre);
    match *hace {
        0..=4 => {
            g.texto(*hace, c, glifo, tono);
            let etiqueta: String = format!("+{nombre}").chars().take(12).collect();
            g.texto(*hace, c + glifo.chars().count() + 1, &etiqueta, Tono::Nota);
        }
        // cae en la carne y salpica
        5..=7 => {
            g.texto(5, c, glifo, tono);
            for (dc, ch) in [(-1isize, '\''), (glifo.chars().count() as isize, '\''), (-2, ','), (glifo.chars().count() as isize + 1, ',')] {
                let col = c as isize + dc;
                if col >= 0 && *hace < 7 {
                    g.put(4, col as usize, ch, Tono::Grasa);
                }
            }
        }
        _ => {}
    }
}

/// El perro de la carne asada: cada tanto entra, huele (meneando la cola) y se
/// va. Va encima de la escena, del lado izquierdo, en el suelo.
fn perro(g: &mut Rejilla, frame: usize) {
    let p = (frame + 160) % 360;
    if p >= 72 {
        return;
    }
    let x: isize = match p {
        0..16 => p as isize / 2 - 8,
        16..56 => 0,
        _ => -((p as isize - 56) / 2),
    };
    let caminando = !(16..56).contains(&p);
    let cola = if (p / 3).is_multiple_of(2) { '/' } else { '\\' };
    let cuerpo = format!("o'')__{cola}");
    let patas = if caminando && (p / 2).is_multiple_of(2) { " /\\ /\\" } else { " |\\ |\\" };
    for (r, l) in [(10, cuerpo.as_str()), (11, patas)] {
        for (j, ch) in l.chars().enumerate() {
            let c = x + j as isize;
            if ch != ' ' && c >= 0 {
                g.put(r, c as usize, ch, Tono::Grasa);
            }
        }
    }
    // huele la discada: un corazoncito
    if (30..44).contains(&p) {
        g.put(9, 2, '♥', Tono::Nota);
    }
}

/// La fiesta (el easter egg): fuegos artificiales sobre el Cerro de la Silla.
fn fiesta(g: &mut Rejilla, s: usize) {
    // solo el cielo (renglones 0 a 3): la discada sigue abajo
    for r in 0..4 {
        for c in 0..W {
            g.put(r, c, ' ', Tono::Nada);
        }
    }
    cerro(g, 1);
    for (k, &(r0, c0)) in [(1usize, 6usize), (0, 20), (1, 33)].iter().enumerate() {
        let fase = (s + k * 5) % 15;
        let radio = fase / 3;
        if radio == 0 {
            g.put(r0, c0, '*', Tono::Lumbre2);
            continue;
        }
        let tono = [Tono::Nota, Tono::Lumbre2, Tono::Verde, Tono::Carne][(s / 15 + k) % 4];
        for (dr, dc) in [(-1isize, 0isize), (1, 0), (0, -2), (0, 2), (-1, -2), (-1, 2), (1, -2), (1, 2)] {
            let (r, c) = (r0 as isize + dr * (radio as isize / 2).min(1), c0 as isize + dc * radio as isize);
            if (0..H as isize).contains(&r) && (0..W as isize).contains(&c) {
                g.put(r as usize, c as usize, if radio > 3 { '·' } else { '*' }, tono);
            }
        }
    }
    if s > 6 {
        g.texto(0, 17, "¡Ajúa!", Tono::Nota);
    }
}

/// Un cuadro de la escena: `t` son los cuadros desde que empezó esta escena
/// y `frame` el reloj de siempre (para la lumbre, el perro…).
pub fn cuadro(escena: Escena, t: usize, frame: usize, extra: &Extra) -> Vec<Line<'static>> {
    let mut g = Rejilla::new();
    match escena {
        Escena::Prender => prender(&mut g, t, frame),
        Escena::Troca => troca(&mut g, t, frame, extra.commits),
        Escena::Meneo => {
            vapor(&mut g, frame, Tono::Vapor, (6, 34), [5, 8, 12, 16]);
            discada(&mut g, frame, false);
            cuchara(&mut g, frame);
            lumbre(&mut g, frame);
        }
        Escena::Ingredientes => ingredientes(&mut g, frame, extra.ingrediente.as_ref()),
        Escena::Apagada => {
            // humo gris, angosto, del centro
            vapor(&mut g, frame, Tono::Humo, (14, 26), [6, 10, 16, 22]);
            discada(&mut g, frame, true);
            apagada(&mut g, frame);
        }
        Escena::Lista => lista(&mut g, frame),
    }
    // el perro se asoma en las escenas con la discada en la lumbre
    if matches!(escena, Escena::Prender | Escena::Meneo | Escena::Ingredientes) {
        perro(&mut g, frame);
    }
    if escena == Escena::Lista {
        // con el vaquero, a la derecha, viéndolo y meneando la cola
        let cola = if (frame / 2).is_multiple_of(2) { '/' } else { '\\' };
        g.texto(10, 33, &format!("o'')__{cola}"), Tono::Grasa);
        g.texto(11, 33, " |\\ |\\", Tono::Grasa);
    }
    if let Some(s) = extra.fiesta {
        fiesta(&mut g, s);
    }
    g.lineas()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODAS: [Escena; 6] = [Escena::Prender, Escena::Troca, Escena::Meneo, Escena::Ingredientes, Escena::Lista, Escena::Apagada];

    fn texto(l: &[Line]) -> Vec<String> {
        l.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect()
    }

    fn extra(hace: usize, fiesta: Option<usize>) -> Extra {
        Extra { commits: 3, ingrediente: Some(("chapa".into(), hace)), fiesta }
    }

    #[test]
    fn cada_cuadro_cabe() {
        // todas las escenas, todo su tiempo, con perro, ingrediente y fiesta
        for e in TODAS {
            for f in (0..400).step_by(7) {
                for t in [0, 5, 12, 27, 33, 60] {
                    for x in [extra(t % 9, None), extra(3, Some(f % 60)), Extra::default()] {
                        let l = cuadro(e, t, f, &x);
                        assert_eq!(l.len(), H);
                        assert!(texto(&l).iter().all(|r| r.chars().count() == W), "{e:?} t={t} f={f}");
                    }
                }
            }
        }
    }

    #[test]
    fn se_mueve() {
        let x = Extra::default();
        assert_ne!(texto(&cuadro(Escena::Meneo, 0, 0, &x)), texto(&cuadro(Escena::Meneo, 1, 1, &x)));
        assert_ne!(texto(&cuadro(Escena::Lista, 0, 0, &x)), texto(&cuadro(Escena::Lista, 4, 4, &x)));
        // la troca avanza y luego se para
        assert_ne!(texto(&cuadro(Escena::Troca, 5, 5, &x)), texto(&cuadro(Escena::Troca, 15, 15, &x)));
        // el carbón se va prendiendo
        assert_ne!(texto(&cuadro(Escena::Prender, 2, 2, &x)), texto(&cuadro(Escena::Prender, 26, 2, &x)));
    }

    #[test]
    fn el_mismo_ingrediente_para_el_mismo_nombre() {
        assert_eq!(ingrediente_de("rola").0, ingrediente_de("rola").0);
        let cae = texto(&cuadro(Escena::Ingredientes, 0, 0, &extra(2, None))).join("\n");
        assert!(cae.contains("+chapa"));
    }

    #[test]
    fn el_perro_viene_y_se_va() {
        let x = Extra::default();
        let con = (0..360).filter(|f| texto(&cuadro(Escena::Meneo, 0, *f, &x)).join("").contains("o''")).count();
        assert!(con > 30 && con < 120, "{con}");
    }

    /// `cargo test vistazo -- --ignored --nocapture`: los cuadros en texto.
    #[test]
    #[ignore]
    fn vistazo() {
        for (e, t, f, x) in [
            (Escena::Prender, 4, 4, Extra::default()),
            (Escena::Prender, 28, 28, Extra::default()),
            (Escena::Troca, 12, 12, Extra::default()),
            (Escena::Troca, 40, 40, extra(0, None)),
            (Escena::Ingredientes, 0, 9, extra(2, None)),
            (Escena::Meneo, 0, 200 + 20, Extra::default()),
            (Escena::Lista, 0, 5, Extra::default()),
            (Escena::Meneo, 0, 9, extra(0, Some(10))),
        ] {
            println!("── {e:?} t={t}");
            for r in texto(&cuadro(e, t, f, &x)) {
                println!("{r}");
            }
        }
    }
}
