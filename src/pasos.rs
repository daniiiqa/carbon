//! Lo que hace actualizar discada (era Update.sh), paso por paso, en un hilo
//! aparte. Cada comando corre en una terminal de mentira (pty): así install.sh
//! se cree en una terminal (sus preguntas, las de pacman y la de sudo siguen
//! funcionando) y su salida llega aquí renglón por renglón para la pantalla.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use portable_pty::{ChildKiller, CommandBuilder, PtySize, native_pty_system};

use crate::arte::Escena;
use crate::linea::Armador;

/// Qué clase de renglón es (la pantalla los pinta distinto).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Clase {
    Normal,
    /// `:: algo` de install.sh: un sub-paso
    Paso,
    /// `!! algo`
    Aviso,
    Error,
    /// sudo pide el dedo
    Huella,
    /// lo que nuevo trae discada (los commits)
    Nuevo,
}

#[derive(Debug)]
pub enum Ev {
    Empieza(usize),
    Termina(usize, bool),
    /// un sub-paso del paso de ahora
    Sub(usize, String),
    Log(String, Clase),
    /// el renglón que se está escribiendo (vacío: ninguno)
    Medias(String),
    /// se acabó: bien o no, y por qué
    Fin(bool, String),
}

/// Lo que la pantalla le pasa al trabajo: lo que se teclea (va a la pty del
/// comando de ahora) y para cancelarlo.
#[derive(Clone, Default)]
pub struct Mando {
    teclado: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    matar: Arc<Mutex<Option<Box<dyn ChildKiller + Send + Sync>>>>,
    /// columnas para las ptys (el ancho del log)
    pub columnas: Arc<Mutex<u16>>,
    /// lo que llegó por la entrada sin comando que lo leyera: espera al siguiente
    guardado: Arc<Mutex<Vec<u8>>>,
}

impl Mando {
    /// Una respuesta que llegó por la entrada (carbon sin pantalla): se guarda
    /// hasta que algún comando pregunte algo, aunque llegue mucho antes.
    pub fn encolar(&self, bytes: &[u8]) {
        if let Ok(mut g) = self.guardado.lock() {
            g.extend_from_slice(bytes);
        }
    }

    /// Hay una pregunta: le toca el siguiente renglón guardado (si hay).
    fn contestar_guardado(&self) {
        let renglon = self.guardado.lock().ok().and_then(|mut g| {
            let fin = g.iter().position(|b| *b == b'\r')?;
            Some(g.drain(..=fin).collect::<Vec<u8>>())
        });
        if let Some(r) = renglon {
            self.teclear(&r);
        }
    }

    /// Manda bytes al comando que está corriendo (si hay).
    pub fn teclear(&self, bytes: &[u8]) {
        if let Ok(mut t) = self.teclado.lock()
            && let Some(w) = t.as_mut()
        {
            let _ = w.write_all(bytes);
            let _ = w.flush();
        }
    }

    pub fn cancelar(&self) {
        if let Ok(mut k) = self.matar.lock()
            && let Some(k) = k.as_mut()
        {
            let _ = k.kill();
        }
    }
}

/// Lo que hace un paso (corre una vez, en el hilo del trabajo).
type Hacer = Box<dyn FnOnce(&mut Ctx) -> Result<(), String> + Send>;

pub struct Paso {
    pub titulo: String,
    /// lo que se ve a la derecha mientras corre
    pub escena: Escena,
    hacer: Hacer,
}

impl Paso {
    fn new(titulo: &str, hacer: impl FnOnce(&mut Ctx) -> Result<(), String> + Send + 'static) -> Paso {
        // cada paso con su escena: el carbón al preparar, la troca al bajar,
        // los ingredientes al instalar; lo demás, la discada meneándose
        let escena = match titulo {
            "preparar" | "prepare" => Escena::Prender,
            "bajar discada" | "download discada" => Escena::Troca,
            "instalar lo nuevo" | "install what's new" => Escena::Ingredientes,
            _ => Escena::Meneo,
        };
        Paso { titulo: titulo.to_string(), escena, hacer: Box::new(hacer) }
    }
}

/// Lo que un paso tiene a la mano.
pub struct Ctx {
    i: usize,
    tx: Sender<Ev>,
    mando: Mando,
    pub es: bool,
    /// al salir, `hyprctl reload` (como el trap de Update.sh); no si chocó
    recargar_al_salir: bool,
    /// lo nuevo que trajo (commits), para el final
    nuevo: Vec<String>,
}

impl Ctx {
    pub fn t(&self, es: &str, en: &str) -> String {
        if self.es { es.to_string() } else { en.to_string() }
    }

    pub fn log(&self, s: impl Into<String>, c: Clase) {
        let _ = self.tx.send(Ev::Log(s.into(), c));
    }

    /// Corre `argv` en una pty, con su salida a la pantalla. Ok si salió con 0.
    pub fn correr(&mut self, argv: &[&str], dir: Option<&Path>) -> Result<(), String> {
        let cols = self.mando.columnas.lock().map(|c| *c).unwrap_or(100).max(40);
        let pty = native_pty_system();
        let par = pty.openpty(PtySize { rows: 40, cols, pixel_width: 0, pixel_height: 0 }).map_err(|e| e.to_string())?;
        let mut cmd = CommandBuilder::new(argv[0]);
        for a in &argv[1..] {
            cmd.arg(a);
        }
        if let Some(d) = dir {
            cmd.cwd(d);
        }
        // los hijos no ponen su propio arte: el de carbon ya está a la derecha
        cmd.env("DISCADA_SIN_ARTE", "1");
        cmd.env("TERM", "xterm-256color");
        let mut hijo = par.slave.spawn_command(cmd).map_err(|e| format!("{}: {e}", argv[0]))?;
        drop(par.slave);
        let mut lector = par.master.try_clone_reader().map_err(|e| e.to_string())?;
        if let Ok(mut t) = self.mando.teclado.lock() {
            *t = par.master.take_writer().ok();
        }
        if let Ok(mut k) = self.mando.matar.lock() {
            *k = Some(hijo.clone_killer());
        }
        let (btx, brx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = channel();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = lector.read(&mut buf) {
                if n == 0 || btx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        let mut arm = Armador::default();
        let mut medias = String::new();
        loop {
            match brx.recv_timeout(Duration::from_millis(100)) {
                Ok(bytes) => {
                    for l in arm.meter(&bytes) {
                        self.renglon(&l);
                    }
                }
                // la pty se cerró: el comando acabó (y sus hijos soltaron la terminal)
                Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }
            if arm.a_medias() != medias {
                medias = arm.a_medias().to_string();
                let _ = self.tx.send(Ev::Medias(medias.clone()));
                // una pregunta y una respuesta guardada (de la entrada): un renglón
                if pregunta(&medias) {
                    self.mando.contestar_guardado();
                }
            }
        }
        if let Some(l) = arm.cerrar() {
            self.renglon(&l);
        }
        let _ = self.tx.send(Ev::Medias(String::new()));
        if let Ok(mut t) = self.mando.teclado.lock() {
            *t = None;
        }
        if let Ok(mut k) = self.mando.matar.lock() {
            *k = None;
        }
        let st = hijo.wait().map_err(|e| e.to_string())?;
        if st.success() { Ok(()) } else { Err(format!("{} → {}", argv.join(" "), st.exit_code())) }
    }

    /// `sh -c script` en una pty.
    pub fn sh(&mut self, script: &str) -> Result<(), String> {
        self.correr(&["sh", "-c", script], None)
    }

    /// Un renglón de la salida: los `::` de install.sh son sub-pasos.
    fn renglon(&self, l: &str) {
        let t = l.trim();
        if t.is_empty() {
            return;
        }
        let clase = if let Some(s) = t.strip_prefix(":: ") {
            let _ = self.tx.send(Ev::Sub(self.i, s.to_string()));
            Clase::Paso
        } else if t.starts_with("!!") {
            Clase::Aviso
        } else if t.contains("finger") || t.contains("dedo") || t.contains("huella") {
            Clase::Huella
        } else if t.starts_with("error") || t.starts_with("Error") || t.starts_with("fatal:") {
            Clase::Error
        } else {
            Clase::Normal
        };
        self.log(l.trim_end(), clase);
    }
}

/// ¿Pide algo que no se debe ver? (la contraseña de sudo y compañía)
pub fn secreta(p: &str) -> bool {
    let p = p.to_lowercase();
    ["password", "contraseña", "passphrase", "[sudo]"].iter().any(|k| p.contains(k))
}

/// ¿El renglón a medias es una pregunta (espera respuesta)?
pub fn pregunta(p: &str) -> bool {
    let t = p.trim_end();
    !t.is_empty() && (secreta(t) || t.ends_with(':') || t.ends_with('?') || t.ends_with(']') || t.ends_with('>'))
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

fn rev(repo: &Path) -> Option<String> {
    let o = Command::new("git").arg("-C").arg(repo).args(["rev-parse", "HEAD"]).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// `git log --oneline antes..después`: lo nuevo que trae.
fn commits(repo: &Path, antes: &str, despues: &str) -> Vec<String> {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["log", "--oneline", "--no-decorate", &format!("{antes}..{despues}")])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(String::from).collect())
        .unwrap_or_default()
}

/// Idioma de discada (hypr/discada.conf → LANGUAGE=es|en).
pub fn spanish() -> bool {
    std::fs::read_to_string(home().join(".config/hypr/discada.conf"))
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("LANGUAGE=").map(|v| v.trim_start().starts_with("es"))))
        .unwrap_or(false)
}

/// Recargar todo con lo nuevo (como reload_all de Update.sh).
const RECARGAR: &str = r#"
s="$HOME/.config/hypr/scripts"
"$s/FontSize.py" apply --no-reload
"$HOME/.local/bin/lena" apps >/dev/null 2>&1
hyprctl reload >/dev/null
pkill -x waybar; pkill -x empalme; sleep 0.3
setsid "$s/Bar.sh" >/dev/null 2>&1 < /dev/null &
if [ -x "$HOME/.local/bin/picha" ]; then
  pkill -f '(^|/)picha recibe'
  setsid "$HOME/.local/bin/picha" recibe >/dev/null 2>&1 < /dev/null &
fi
pkill -USR1 -x kitty
echo "barra, picha y kitty: con lo nuevo"
"#;

pub enum Modo {
    /// ~/.config es el repo (la máquina donde se desarrolla)
    Repo,
    /// instalado con install.sh, el repo en ~/dotfiles
    Dotfiles,
    /// `carbon prueba`: pasos de mentira para ver la pantalla sin tocar nada
    Prueba,
}

pub fn modo() -> Option<Modo> {
    let cfg = home().join(".config");
    let es_repo = Command::new("git").arg("-C").arg(&cfg).args(["rev-parse", "--is-inside-work-tree"]).output().is_ok_and(|o| o.status.success());
    if es_repo && cfg.join("hypr/looks").is_dir() {
        Some(Modo::Repo)
    } else if home().join("dotfiles/.git").is_dir() {
        Some(Modo::Dotfiles)
    } else {
        None
    }
}

/// leña por su ruta (en la pty no hace falta que esté en el PATH)
fn lena() -> String {
    home().join(".local/bin/lena").to_string_lossy().into_owned()
}

fn scripts() -> PathBuf {
    home().join(".config/hypr/scripts")
}

fn look_actual() -> Option<String> {
    let cur = std::fs::read_to_string(home().join(".config/hypr/looks/.current")).ok()?.trim().to_string();
    home().join(".config/hypr/looks").join(&cur).is_dir().then_some(cur)
}

/// Los pasos de cada modo.
pub fn plan(modo: &Modo, es: bool) -> Vec<Paso> {
    let t = |a: &'static str, b: &'static str| if es { a } else { b };
    let preparar = Paso::new(t("preparar", "prepare"), |c| {
        // sin autoreload mientras cambian los archivos (Hyprland se caía
        // recargando a media copia); se recarga una vez al final
        let hypr = scripts().join("Hypr.sh");
        let _ = Command::new(hypr).args(["set", "misc:disable_autoreload", "true"]).output();
        c.log(c.t("autoreload de Hyprland apagado mientras tanto", "Hyprland autoreload off meanwhile"), Clase::Normal);
        Ok(())
    });
    match modo {
        Modo::Dotfiles => vec![
            preparar,
            Paso::new(t("bajar discada", "download discada"), |c| {
                let repo = home().join("dotfiles");
                let antes = rev(&repo).unwrap_or_default();
                c.correr(&["git", "-C", &repo.to_string_lossy(), "pull", "--ff-only"], None)
                    .map_err(|_| c.t("git pull falló en ~/dotfiles", "git pull failed in ~/dotfiles"))?;
                let despues = rev(&repo).unwrap_or_default();
                c.nuevo = commits(&repo, &antes, &despues);
                for l in &c.nuevo {
                    c.log(l.clone(), Clase::Nuevo);
                }
                Ok(())
            }),
            Paso::new(t("instalar lo nuevo", "install what's new"), |c| {
                let sh = home().join("dotfiles/install.sh");
                c.correr(&["bash", &sh.to_string_lossy(), "--update"], None).map_err(|_| c.t("install.sh --update falló", "install.sh --update failed"))
            }),
            Paso::new(t("recargar", "reload"), |c| {
                c.sh(RECARGAR)?;
                let _ = Command::new(scripts().join("Refresh.sh")).output();
                Ok(())
            }),
        ],
        Modo::Repo => {
            let cur = look_actual();
            let cur2 = cur.clone();
            vec![
                preparar,
                Paso::new(t("guardar tu look", "save your look"), move |c| {
                    // lo vivo al look (como antes de cambiar de look), sin el tamaño de letra
                    c.correr(&[&scripts().join("FontSize.py").to_string_lossy(), "unapply"], None)?;
                    if let Some(l) = &cur {
                        c.correr(&[&lena(), "save", l], None)?;
                    }
                    Ok(())
                }),
                Paso::new(t("bajar discada", "download discada"), |c| {
                    let cfg = home().join(".config");
                    let antes = rev(&cfg).unwrap_or_default();
                    let fs = scripts().join("FontSize.py");
                    if c.correr(&["git", "-C", &cfg.to_string_lossy(), "pull", "--rebase", "--autostash"], None).is_err() {
                        let _ = c.correr(&[&fs.to_string_lossy(), "apply", "--no-reload"], None);
                        return Err(c.t("git pull falló: revisa el repo en ~/.config", "git pull failed: check the repo in ~/.config"));
                    }
                    // el autostash puede chocar con lo nuevo y git salir bien igual,
                    // dejando <<<<<<< en los archivos: recargar así tumba Hyprland.
                    // Se para aquí sin recargar (Hyprland sigue con lo que tenía) y
                    // tus cambios siguen a salvo en el stash.
                    let choque = Command::new("git").arg("-C").arg(&cfg).args(["diff", "--name-only", "--diff-filter=U"]).output();
                    let choque = choque.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
                    if !choque.is_empty() {
                        c.recargar_al_salir = false;
                        let _ = c.correr(&[&fs.to_string_lossy(), "apply", "--no-reload"], None);
                        for f in choque.lines() {
                            c.log(format!("   {f}"), Clase::Error);
                        }
                        return Err(c.t(
                            "Tus cambios chocaron con la actualización (arriba los archivos). Arréglalos (quita los <<<<<<< / >>>>>>>), luego: git -C ~/.config stash drop y hyprctl reload",
                            "Your changes conflicted with the update (files above). Fix them (remove the <<<<<<< / >>>>>>>), then: git -C ~/.config stash drop and hyprctl reload",
                        ));
                    }
                    let despues = rev(&cfg).unwrap_or_default();
                    c.nuevo = commits(&cfg, &antes, &despues);
                    for l in &c.nuevo {
                        c.log(l.clone(), Clase::Nuevo);
                    }
                    Ok(())
                }),
                Paso::new(t("poner tu look", "apply your look"), move |c| match &cur2 {
                    Some(l) => c.correr(&[&lena(), "apply", l], None),
                    None => Ok(()),
                }),
                Paso::new(t("recargar", "reload"), |c| c.sh(RECARGAR)),
            ]
        }
        Modo::Prueba => vec![
            Paso::new(t("preparar", "prepare"), |c| c.sh("echo preparando; sleep 1")),
            Paso::new(t("bajar discada", "download discada"), |c| {
                c.sh("for i in 1 2 3 4 5; do printf 'bajando %d0%%\\r' $((i*2)); sleep 0.4; done; echo; echo 'Updating 3e1f2a..9c0b7d'; echo ' hypr/scripts/LockScreen.sh | 12 ++--'")?;
                c.nuevo = vec!["9c0b7d carbon: el updater con su lumbre".into(), "4e5f6a chapa: el estilo de cada look".into()];
                for l in c.nuevo.clone() {
                    c.log(l, Clase::Nuevo);
                }
                Ok(())
            }),
            Paso::new(t("instalar lo nuevo", "install what's new"), |c| {
                c.sh(concat!(
                    "printf '\\033[1;35m::\\033[0m paquetes\\n'; sleep 0.8; echo ' nada que hacer'; ",
                    "printf '\\033[1;35m::\\033[0m rola\\n'; sleep 0.6; echo '  rola: al día'; ",
                    "printf '\\033[1;35m::\\033[0m chapa\\n'; sleep 1.2; echo '  chapa: listo (ya compilado) d662321'; ",
                    "printf '\\033[1;33m!!\\033[0m un aviso de mentira\\n'; ",
                    "read -r -p '   ¿Seguir con la actualización? [S/n] ' r; echo \"   contestaste: ${r:-S}\"; ",
                    "printf '\\033[1;35m::\\033[0m carbon\\n'; sleep 1; echo '  carbon: listo'"
                ))
            }),
            Paso::new(t("recargar", "reload"), |c| c.sh("echo recargando; sleep 1; echo 'barra, picha y kitty: con lo nuevo'")),
        ],
    }
}

/// Corre el plan en un hilo; los eventos llegan por el canal.
pub fn arrancar(pasos: Vec<Paso>, es: bool, prueba: bool, mando: Mando) -> Receiver<Ev> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let mut c = Ctx { i: 0, tx: tx.clone(), mando, es, recargar_al_salir: !prueba, nuevo: Vec::new() };
        let mut fallo = None;
        for (i, p) in pasos.into_iter().enumerate() {
            c.i = i;
            let _ = tx.send(Ev::Empieza(i));
            let r = (p.hacer)(&mut c);
            let _ = tx.send(Ev::Termina(i, r.is_ok()));
            if let Err(e) = r {
                fallo = Some(e);
                break;
            }
        }
        // como el trap de Update.sh: una recarga al final (sí o sí, salvo choque)
        if c.recargar_al_salir {
            let _ = Command::new("hyprctl").arg("reload").output();
        }
        let ok = fallo.is_none();
        let msg = match fallo {
            Some(e) => e,
            None if c.nuevo.is_empty() => c.t("Ya estabas al día (las herramientas, revisadas).", "Already up to date (tools checked)."),
            None => c.t(&format!("discada actualizada: {} cambios nuevos.", c.nuevo.len()), &format!("discada updated: {} new changes.", c.nuevo.len())),
        };
        if !prueba {
            let (urg, txt) = if ok { ("low", c.t("Actualizada", "Updated")) } else { ("critical", msg.clone()) };
            let _ = Command::new("notify-send").args(["-e", "-u", urg, "discada", &txt]).output();
        }
        let _ = tx.send(Ev::Fin(ok, msg));
    });
    rx
}
