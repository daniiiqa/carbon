//! La pantalla: a la izquierda los pasos (con sus sub-pasos) y el log del
//! paso de ahora; a la derecha la discada en la lumbre, el avance y, al final,
//! el vaquero (o la lumbre apagada). Abajo, lo que el comando pregunta: se
//! contesta ahí mismo (la contraseña de sudo, el dedo, un [S/n]).

use std::io::IsTerminal;
use std::time::{Duration, Instant};

use disco::look::Look;
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use ratatui::crossterm::execute;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::{Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};
use ratatui::Frame;

use crate::arte::{self, Escena};
use crate::pasos::{Clase, Ev, Mando, Paso};

enum Edo {
    Espera,
    Corre(Instant),
    Bien(Duration),
    Mal(Duration),
}

struct PasoUi {
    titulo: String,
    edo: Edo,
    subs: Vec<String>,
}

struct App {
    look: Look,
    es: bool,
    pasos: Vec<PasoUi>,
    log: Vec<(String, Clase)>,
    /// renglones desde abajo (0: pegado al final)
    scroll: usize,
    medias: String,
    /// letras tecleadas en una pregunta secreta (se enseñan como puntos)
    secreto: usize,
    fin: Option<(bool, String)>,
    inicio: Instant,
    duro: Option<Duration>,
    frame: usize,
    cancelando: bool,
    nuevo: usize,
}

const GIRO: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

use crate::pasos::{pregunta, secreta};

fn reloj(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 60 { format!("{}m {:02}s", s / 60, s % 60) } else { format!("{:.1}s", d.as_secs_f32()) }
}

fn cortar(s: &str, w: usize) -> String {
    if s.chars().count() <= w {
        return s.to_string();
    }
    let mut out: String = s.chars().take(w.saturating_sub(1)).collect();
    out.push('…');
    out
}

impl App {
    fn t<'a>(&self, es: &'a str, en: &'a str) -> &'a str {
        if self.es { es } else { en }
    }

    fn evento(&mut self, ev: Ev) {
        match ev {
            Ev::Empieza(i) => {
                if let Some(p) = self.pasos.get_mut(i) {
                    p.edo = Edo::Corre(Instant::now());
                }
                self.scroll = 0;
            }
            Ev::Termina(i, ok) => {
                if let Some(p) = self.pasos.get_mut(i)
                    && let Edo::Corre(t) = p.edo
                {
                    p.edo = if ok { Edo::Bien(t.elapsed()) } else { Edo::Mal(t.elapsed()) };
                }
            }
            Ev::Sub(i, s) => {
                if let Some(p) = self.pasos.get_mut(i) {
                    p.subs.push(s);
                }
            }
            Ev::Log(l, c) => {
                if c == Clase::Nuevo {
                    self.nuevo += 1;
                }
                self.log.push((l, c));
                // si se está leyendo arriba, que no se mueva lo que se lee
                if self.scroll > 0 {
                    self.scroll += 1;
                }
            }
            Ev::Medias(m) => {
                // los puntos cuentan mientras siga la misma pregunta secreta
                if !secreta(&m) || !secreta(&self.medias) {
                    self.secreto = 0;
                }
                self.medias = m;
            }
            Ev::Fin(ok, msg) => {
                self.duro = Some(self.inicio.elapsed());
                self.fin = Some((ok, msg));
                self.medias.clear();
            }
        }
    }

    /// Una tecla: mientras corre va al comando (para contestarle); al final, salir.
    fn tecla(&mut self, k: event::KeyEvent, mando: &Mando) -> bool {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        match k.code {
            KeyCode::PageUp => self.scroll += 10,
            KeyCode::PageDown => self.scroll = self.scroll.saturating_sub(10),
            KeyCode::Up if self.fin.is_some() || self.medias.is_empty() => self.scroll += 1,
            KeyCode::Down if self.fin.is_some() || self.medias.is_empty() => self.scroll = self.scroll.saturating_sub(1),
            _ if self.fin.is_some() => return matches!(k.code, KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter),
            KeyCode::Char('c') if ctrl => {
                self.cancelando = true;
                mando.cancelar();
            }
            KeyCode::Enter => mando.teclear(b"\r"),
            KeyCode::Backspace => {
                self.secreto = self.secreto.saturating_sub(1);
                mando.teclear(b"\x7f");
            }
            KeyCode::Char(c) if !ctrl => {
                if secreta(&self.medias) {
                    self.secreto += 1;
                }
                let mut b = [0u8; 4];
                mando.teclear(c.encode_utf8(&mut b).as_bytes());
            }
            _ => {}
        }
        false
    }

    fn escena(&self) -> Escena {
        match &self.fin {
            Some((true, _)) => Escena::Lista,
            Some((false, _)) => Escena::Apagada,
            None => Escena::Meneo,
        }
    }

    fn dibujar(&mut self, f: &mut Frame, mando: &Mando) {
        let t = self.look.theme;
        if !self.look.transparent {
            f.render_widget(Block::new().style(Style::new().bg(t.base)), f.area());
        }
        let area = f.area().inner(Margin { horizontal: 1, vertical: 0 });
        let [cabeza, cuerpo, pie] = Layout::vertical([Constraint::Length(1), Constraint::Fill(1), Constraint::Length(1)]).areas(area);

        // arriba: carbon y lo que hace; a la derecha el tiempo
        let dur = self.duro.unwrap_or_else(|| self.inicio.elapsed());
        let titulo = Line::from(vec![
            Span::styled(format!(" {} ", self.look.tx("carbon")), Style::new().fg(t.base).bg(t.primary).bold()),
            Span::styled(format!("  {}", self.look.tx(self.t("actualizar discada", "update discada"))), Style::new().fg(t.fg).bold()),
        ]);
        f.render_widget(Paragraph::new(titulo), cabeza);
        f.render_widget(Paragraph::new(Span::styled(reloj(dur), Style::new().fg(t.muted))).alignment(Alignment::Right), cabeza);

        // la discada necesita su ancho: en una terminal angosta, solo la izquierda
        let ancho_arte = arte::W as u16 + 4;
        let con_arte = cuerpo.width >= ancho_arte + 46;
        let [izq, der] = if con_arte {
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(ancho_arte)]).areas(cuerpo)
        } else {
            [cuerpo, Rect::default()]
        };
        self.izquierda(f, izq, mando);
        if con_arte {
            self.derecha(f, der);
        }

        let ayuda = match &self.fin {
            None if self.cancelando => self.t("cancelando…", "cancelling…").to_string(),
            None if pregunta(&self.medias) => self.t("escribe y enter · ctrl+c cancela", "type and enter · ctrl+c cancels").to_string(),
            None => self.t("pgup/pgdn el log · ctrl+c cancela", "pgup/pgdn the log · ctrl+c cancels").to_string(),
            Some(_) => self.t("enter o q para salir · pgup/pgdn el log", "enter or q to quit · pgup/pgdn the log").to_string(),
        };
        f.render_widget(Paragraph::new(Span::styled(self.look.tx(&ayuda), Style::new().fg(t.muted))), pie);
    }

    fn izquierda(&mut self, f: &mut Frame, area: Rect, mando: &Mando) {
        let t = self.look.theme;
        // los pasos: los sub-pasos del que corre (los últimos), los demás contados
        let mut filas: Vec<Line> = Vec::new();
        for p in &self.pasos {
            let (marca, estilo, tiempo) = match p.edo {
                Edo::Espera => ("○", Style::new().fg(t.muted), String::new()),
                Edo::Corre(i) => (GIRO[self.frame % GIRO.len()], Style::new().fg(t.primary).bold(), reloj(i.elapsed())),
                Edo::Bien(d) => ("✓", Style::new().fg(t.green), reloj(d)),
                Edo::Mal(d) => ("✗", Style::new().fg(t.red).bold(), reloj(d)),
            };
            let mut l = vec![
                Span::styled(format!(" {marca} "), estilo),
                Span::styled(self.look.tx(&p.titulo), if matches!(p.edo, Edo::Espera) { Style::new().fg(t.muted) } else { Style::new().fg(t.fg) }),
            ];
            if !p.subs.is_empty() && !matches!(p.edo, Edo::Corre(_)) {
                l.push(Span::styled(format!("  ({})", p.subs.len()), Style::new().fg(t.muted)));
            }
            // el tiempo, alineado a la derecha como columna
            let usado: usize = l.iter().map(|s| s.content.chars().count()).sum();
            let ancho = area.width.saturating_sub(3) as usize;
            let hueco = ancho.saturating_sub(usado + tiempo.chars().count()).max(2);
            l.push(Span::raw(" ".repeat(hueco)));
            l.push(Span::styled(tiempo, Style::new().fg(t.muted)));
            filas.push(Line::from(l));
            if matches!(p.edo, Edo::Corre(_)) {
                let n = p.subs.len();
                for (k, s) in p.subs.iter().enumerate().skip(n.saturating_sub(4)) {
                    let ultimo = k + 1 == n;
                    filas.push(Line::from(vec![
                        Span::styled(if ultimo { "   └ " } else { "   ├ " }, Style::new().fg(t.border)),
                        Span::styled(s.clone(), if ultimo { Style::new().fg(t.accent) } else { Style::new().fg(t.muted) }),
                    ]));
                }
            }
        }
        let alto_pasos = filas.len() as u16 + 2;
        let preguntando = !self.medias.is_empty();
        let [arriba, log, abajo] =
            Layout::vertical([Constraint::Length(alto_pasos), Constraint::Fill(1), Constraint::Length(if preguntando { 3 } else { 0 })]).areas(area);
        let b = self.look.block(false).title(Span::styled(format!(" {} ", self.look.tx(self.t("pasos", "steps"))), Style::new().fg(t.muted)));
        f.render_widget(Paragraph::new(filas).block(b), arriba);

        // el log: lo último abajo; pgup sube
        let b = self.look.block(false).title(Span::styled(format!(" {} ", self.look.tx("log")), Style::new().fg(t.muted)));
        let dentro = b.inner(log);
        if let Ok(mut c) = mando.columnas.lock() {
            *c = dentro.width.saturating_sub(1).max(40);
        }
        let alto = dentro.height as usize;
        let total = self.log.len();
        self.scroll = self.scroll.min(total.saturating_sub(alto));
        let hasta = total - self.scroll;
        let desde = hasta.saturating_sub(alto);
        let w = dentro.width as usize;
        let renglones: Vec<Line> = self.log[desde..hasta]
            .iter()
            .map(|(l, c)| {
                let st = match c {
                    Clase::Paso => Style::new().fg(t.primary).bold(),
                    Clase::Aviso => Style::new().fg(t.accent),
                    Clase::Error => Style::new().fg(t.red),
                    Clase::Huella => Style::new().fg(t.accent).add_modifier(Modifier::BOLD),
                    Clase::Nuevo => Style::new().fg(t.green),
                    Clase::Normal => Style::new().fg(t.muted),
                };
                let texto = if *c == Clase::Nuevo { format!("+ {l}") } else { l.clone() };
                Line::from(Span::styled(format!(" {}", cortar(&texto, w.saturating_sub(2))), st))
            })
            .collect();
        let titulo_scroll = if self.scroll > 0 { format!(" ↑{} ", self.scroll) } else { String::new() };
        f.render_widget(Paragraph::new(renglones).block(b.title_bottom(Span::styled(titulo_scroll, Style::new().fg(t.accent)))), log);

        // lo que el comando pregunta (o está escribiendo)
        if preguntando {
            let q = pregunta(&self.medias);
            let mut l = vec![Span::styled(if q { " › " } else { "   " }, Style::new().fg(t.accent).bold())];
            l.push(Span::styled(cortar(self.medias.trim_start(), w.saturating_sub(12)), if q { Style::new().fg(t.fg).bold() } else { Style::new().fg(t.muted) }));
            if secreta(&self.medias) {
                l.push(Span::styled(format!(" {}", "•".repeat(self.secreto.min(24))), Style::new().fg(t.accent)));
            }
            if q && self.frame % 8 < 5 {
                l.push(Span::styled("▏", Style::new().fg(t.accent)));
            }
            let b = self.look.block(q);
            f.render_widget(Paragraph::new(Line::from(l)).block(b), abajo);
        }
    }

    fn derecha(&self, f: &mut Frame, area: Rect) {
        let t = self.look.theme;
        let b = self.look.block(false);
        let dentro = b.inner(area);
        f.render_widget(b, area);
        let alto_arte = arte::H as u16;
        let [_, dibujo, _, info] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(alto_arte), Constraint::Length(1), Constraint::Fill(1)]).areas(dentro);
        f.render_widget(Paragraph::new(arte::cuadro(self.escena(), self.frame)).alignment(Alignment::Center), dibujo);

        let mut lineas: Vec<Line> = Vec::new();
        match &self.fin {
            None => {
                let hechos = self.pasos.iter().filter(|p| matches!(p.edo, Edo::Bien(_))).count();
                let total = self.pasos.len().max(1);
                let ahora = self.pasos.iter().find(|p| matches!(p.edo, Edo::Corre(_)));
                let etiqueta = ahora.map(|p| p.subs.last().cloned().unwrap_or_else(|| p.titulo.clone())).unwrap_or_default();
                lineas.push(Line::from(Span::styled(
                    self.look.tx(&format!("{} {}", self.t("meneando:", "stirring:"), cortar(&etiqueta, 24))),
                    Style::new().fg(t.fg),
                )));
                lineas.push(Line::default());
                let ancho = dentro.width.saturating_sub(10) as usize;
                let (hecho, falta) = self.look.bar(hechos as f64 / total as f64, ancho);
                lineas.push(Line::from(vec![
                    Span::styled(hecho, Style::new().fg(t.primary)),
                    Span::styled(falta, Style::new().fg(t.border)),
                    Span::styled(format!("  {hechos}/{total}"), Style::new().fg(t.muted)),
                ]));
            }
            Some((true, msg)) => {
                lineas.push(Line::from(Span::styled(self.look.tx(self.t("¡discada lista!", "discada is ready!")), Style::new().fg(t.accent).bold())));
                lineas.push(Line::default());
                lineas.push(Line::from(Span::styled(msg.clone(), Style::new().fg(t.fg))));
            }
            Some((false, msg)) => {
                lineas.push(Line::from(Span::styled(self.look.tx(self.t("se apagó la lumbre", "the fire went out")), Style::new().fg(t.red).bold())));
                lineas.push(Line::default());
                lineas.push(Line::from(Span::styled(msg.clone(), Style::new().fg(t.fg))));
            }
        }
        let p = Paragraph::new(lineas).alignment(Alignment::Center).wrap(Wrap { trim: true });
        f.render_widget(p, info.inner(Margin { horizontal: 1, vertical: 0 }));
    }
}

/// Corre el plan con la pantalla. Devuelve si salió bien.
pub fn correr(pasos: Vec<Paso>, es: bool, prueba: bool) -> bool {
    let mando = Mando::default();
    let titulos: Vec<PasoUi> = pasos.iter().map(|p| PasoUi { titulo: p.titulo.clone(), edo: Edo::Espera, subs: Vec::new() }).collect();
    if !std::io::stdout().is_terminal() {
        return sin_pantalla(pasos, es, prueba, mando);
    }
    let rx = crate::pasos::arrancar(pasos, es, prueba, mando.clone());
    let mut app = App {
        look: Look::load("carbon"),
        es,
        pasos: titulos,
        log: Vec::new(),
        scroll: 0,
        medias: String::new(),
        secreto: 0,
        fin: None,
        inicio: Instant::now(),
        duro: None,
        frame: 0,
        cancelando: false,
        nuevo: 0,
    };
    let mut term = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableMouseCapture);
    let mut ultimo_cuadro = Instant::now();
    let mut ultimo_look = Instant::now();
    loop {
        while let Ok(ev) = rx.try_recv() {
            app.evento(ev);
        }
        let _ = term.draw(|f| app.dibujar(f, &mando));
        if event::poll(Duration::from_millis(40)).unwrap_or(false) {
            match event::read() {
                Ok(Event::Key(k)) if k.kind == KeyEventKind::Press => {
                    if app.tecla(k, &mando) {
                        break;
                    }
                }
                Ok(Event::Mouse(m)) => match m.kind {
                    MouseEventKind::ScrollUp => app.scroll += 3,
                    MouseEventKind::ScrollDown => app.scroll = app.scroll.saturating_sub(3),
                    _ => {}
                },
                _ => {}
            }
        }
        // 8 cuadros por segundo; el vaquero sigue bailando hasta que salgas
        if ultimo_cuadro.elapsed() >= Duration::from_millis(125) {
            app.frame += 1;
            ultimo_cuadro = Instant::now();
        }
        if ultimo_look.elapsed() >= Duration::from_secs(1) {
            app.look.refresh();
            ultimo_look = Instant::now();
        }
    }
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    // lo que pasó se queda en la terminal
    let (ok, msg) = app.fin.clone().unwrap_or((false, String::new()));
    println!("{} {msg}", if ok { "\x1b[1;35m::\x1b[0m" } else { "\x1b[1;31m!!\x1b[0m" });
    for (l, c) in &app.log {
        if *c == Clase::Nuevo {
            println!("   + {l}");
        }
    }
    ok
}

/// Sin terminal (lo corrió otro programa): los renglones tal cual.
fn sin_pantalla(pasos: Vec<Paso>, es: bool, prueba: bool, mando: Mando) -> bool {
    let titulos: Vec<String> = pasos.iter().map(|p| p.titulo.clone()).collect();
    // lo que llegue por la entrada va al comando (para contestar sus preguntas)
    let m = mando.clone();
    std::thread::spawn(move || {
        for l in std::io::stdin().lines().map_while(Result::ok) {
            m.encolar(format!("{l}\r").as_bytes());
        }
    });
    let rx = crate::pasos::arrancar(pasos, es, prueba, mando);
    for ev in rx {
        match ev {
            Ev::Empieza(i) => println!(":: {}", titulos.get(i).map(String::as_str).unwrap_or("")),
            Ev::Log(l, _) => println!("   {l}"),
            // una pregunta: que se vea (la respuesta llega por la entrada)
            Ev::Medias(m) if pregunta(&m) => {
                print!("   {m}");
                let _ = std::io::Write::flush(&mut std::io::stdout());
            }
            Ev::Fin(ok, msg) => {
                println!("{} {msg}", if ok { "::" } else { "!!" });
                return ok;
            }
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn app(n: usize) -> App {
        App {
            look: Look::load("carbon"),
            es: true,
            pasos: ["preparar", "bajar discada", "instalar lo nuevo", "recargar"]
                .iter()
                .take(n)
                .map(|t| PasoUi { titulo: t.to_string(), edo: Edo::Espera, subs: Vec::new() })
                .collect(),
            log: Vec::new(),
            scroll: 0,
            medias: String::new(),
            secreto: 0,
            fin: None,
            inicio: Instant::now(),
            duro: None,
            frame: 3,
            cancelando: false,
            nuevo: 0,
        }
    }

    fn pantalla(a: &mut App) -> String {
        let mut term = Terminal::new(TestBackend::new(120, 30)).unwrap();
        let m = Mando::default();
        term.draw(|f| a.dibujar(f, &m)).unwrap();
        let buf = term.backend().buffer();
        (0..buf.area.height).map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect::<String>().trim_end().to_string()).collect::<Vec<_>>().join("\n")
    }

    fn a_media_carrera() -> App {
        let mut a = app(4);
        for ev in [
            Ev::Empieza(0),
            Ev::Log("autoreload de Hyprland apagado mientras tanto".into(), Clase::Normal),
            Ev::Termina(0, true),
            Ev::Empieza(1),
            Ev::Log("Updating 3e1f2a..9c0b7d".into(), Clase::Normal),
            Ev::Log("9c0b7d carbon: el updater con su lumbre".into(), Clase::Nuevo),
            Ev::Termina(1, true),
            Ev::Empieza(2),
            Ev::Sub(2, "paquetes".into()),
            Ev::Log(":: paquetes".into(), Clase::Paso),
            Ev::Log(" nada que hacer".into(), Clase::Normal),
            Ev::Sub(2, "rola".into()),
            Ev::Log("  rola: al día".into(), Clase::Normal),
            Ev::Sub(2, "chapa".into()),
            Ev::Log("!! un aviso".into(), Clase::Aviso),
            Ev::Medias("[sudo] password for dany: ".into()),
        ] {
            a.evento(ev);
        }
        a.secreto = 5;
        a
    }

    #[test]
    fn la_pregunta_se_ve_y_los_pasos_tambien() {
        let s = pantalla(&mut a_media_carrera());
        assert!(s.contains("instalar lo nuevo") && s.contains("└ chapa"));
        assert!(s.contains("[sudo] password for dany:") && s.contains("•••••"));
        assert!(s.contains("+ 9c0b7d carbon"));
    }

    #[test]
    fn al_final_el_mensaje() {
        let mut a = a_media_carrera();
        a.evento(Ev::Medias(String::new()));
        a.evento(Ev::Termina(2, true));
        a.evento(Ev::Fin(true, "discada actualizada: 1 cambios nuevos.".into()));
        let s = pantalla(&mut a);
        assert!(s.contains("lista") && s.contains("1 cambios nuevos"));
    }

    /// `cargo test vistazo -- --ignored --nocapture`: la pantalla en texto.
    #[test]
    #[ignore]
    fn vistazo() {
        let mut a = a_media_carrera();
        println!("{}", pantalla(&mut a));
        a.evento(Ev::Medias(String::new()));
        a.evento(Ev::Termina(2, false));
        a.evento(Ev::Fin(false, "install.sh --update falló".into()));
        println!("{}", pantalla(&mut a));
    }
}
