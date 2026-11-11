//! `carbon revisa REPO BASE`: qué archivos tuyos toca una actualización
//! (era hypr/scripts/UpdateCheck.py; install.sh --update lee su salida).
//!
//! Compara lo que hay en ~/.config con lo que instaló la versión anterior
//! (BASE, un commit del repo) y con lo que va a instalar la nueva (HEAD):
//!
//!   replace  RUTA   lo cambiaste y la versión nueva también: se reemplaza
//!                   (lo tuyo queda en el respaldo)
//!   keep     RUTA   lo cambiaste y la versión nueva no: se queda lo tuyo
//!
//! Cuenta los archivos del repo que van a ~/.config (CONFIG_DIRS de
//! install.sh, sin KEEP_ON_UPDATE, que nunca se pisan) y los vivos del look
//! actual (los que leña escribe desde hypr/looks/<look>/, con su fuente y el
//! tamaño de letra de FontSize.py).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// De la carpeta del look, lo que leña no copia.
const GENERADOS: [&str; 2] = ["look.toml", "README.md"];

fn git(repo: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let o = Command::new("git").arg("-C").arg(repo).args(args).output().ok()?;
    o.status.success().then_some(o.stdout)
}

fn show(repo: &Path, rev: &str, path: &str) -> Option<Vec<u8>> {
    git(repo, &["show", &format!("{rev}:{path}")])
}

fn rastreados(repo: &Path, rev: &str, dirs: &[String]) -> BTreeSet<String> {
    let mut args = vec!["ls-tree", "-r", "--name-only", "-z", rev, "--"];
    args.extend(dirs.iter().map(String::as_str));
    git(repo, &args)
        .unwrap_or_default()
        .split(|b| *b == 0)
        .filter(|p| !p.is_empty())
        .map(|p| String::from_utf8_lossy(p).into_owned())
        .collect()
}

/// `NOMBRE=(a b c)` de install.sh (puede abarcar varios renglones, con
/// comentarios).
pub fn lista_bash(texto: &str, nombre: &str) -> Vec<String> {
    let inicio = format!("{nombre}=(");
    let Some(i) = texto.lines().position(|l| l.starts_with(&inicio)) else { return Vec::new() };
    let mut cuerpo = String::new();
    for (k, l) in texto.lines().skip(i).enumerate() {
        let l = if k == 0 { &l[inicio.len()..] } else { l };
        let l = l.split('#').next().unwrap_or("");
        match l.find(')') {
            Some(j) => {
                cuerpo.push_str(&l[..j]);
                break;
            }
            None => {
                cuerpo.push_str(l);
                cuerpo.push(' ');
            }
        }
    }
    cuerpo.split_whitespace().map(String::from).collect()
}

/// `[font] family` y `display` de un look.toml (display: family si no hay).
pub fn fuentes(toml: &str) -> (String, String) {
    let mut dentro = false;
    let (mut fam, mut disp) = (String::new(), String::new());
    for l in toml.lines() {
        let l = l.split('#').next().unwrap_or("").trim();
        if l.starts_with('[') {
            dentro = l == "[font]";
            continue;
        }
        if let (true, Some((k, v))) = (dentro, l.split_once('=')) {
            let v = v.trim().trim_matches('"').to_string();
            match k.trim() {
                "family" => fam = v,
                "display" => disp = v,
                _ => {}
            }
        }
    }
    if disp.is_empty() {
        disp = fam.clone();
    }
    (fam, disp)
}

/// Lo mismo que FontSize.py le hace a lo vivo (kitty y swaync).
pub fn ajuste_letra(rel: &str, texto: &str, d: i64) -> String {
    if d == 0 {
        return texto.to_string();
    }
    let num = |x: f64, como: &str| if como.contains('.') || x.fract() != 0.0 { format!("{x:.1}") } else { format!("{}", x as i64) };
    let mover = |v: &str| -> Option<String> {
        let x: f64 = v.parse().ok()?;
        Some(num((x + d as f64).max(6.0), v))
    };
    let mut out = String::new();
    for (k, l) in texto.split_inclusive('\n').enumerate() {
        let _ = k;
        let nuevo = match rel {
            "kitty/kitty.conf" => l.strip_prefix("font_size").and_then(|resto| {
                let esp: String = resto.chars().take_while(|c| c.is_whitespace() && *c != '\n').collect();
                if esp.is_empty() {
                    return None;
                }
                let tras = &resto[esp.len()..];
                let v: String = tras.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                if v.is_empty() {
                    return None;
                }
                Some(format!("font_size{esp}{}{}", mover(&v)?, &tras[v.len()..]))
            }),
            "swaync/style.css" => {
                // cada font-size: Npx del renglón
                let mut r = String::new();
                let mut resto = l;
                let mut cambio = false;
                while let Some(i) = resto.find("font-size:") {
                    r.push_str(&resto[..i + 10]);
                    let tras = &resto[i + 10..];
                    let esp: String = tras.chars().take_while(|c| c.is_whitespace()).collect();
                    let t2 = &tras[esp.len()..];
                    let v: String = t2.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
                    match (v.is_empty(), t2[v.len()..].starts_with("px"), mover(&v)) {
                        (false, true, Some(n)) => {
                            r.push_str(&esp);
                            r.push_str(&n);
                            cambio = true;
                            resto = &t2[v.len()..];
                        }
                        _ => {
                            r.push_str(&esp);
                            resto = t2;
                        }
                    }
                }
                r.push_str(resto);
                cambio.then_some(r)
            }
            _ => None,
        };
        out.push_str(nuevo.as_deref().unwrap_or(l));
    }
    out
}

fn cfg() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config")
}

/// Las líneas `keep\truta` / `replace\truta` (ya en orden).
pub fn revisar(repo: &Path, base: &str) -> Vec<(bool, String)> {
    let install = String::from_utf8_lossy(&show(repo, "HEAD", "install.sh").unwrap_or_default()).into_owned();
    let dirs = lista_bash(&install, "CONFIG_DIRS");
    let guardar: BTreeSet<String> = lista_bash(&install, "KEEP_ON_UPDATE").into_iter().collect();
    let look = std::fs::read_to_string(cfg().join("hypr/looks/.current")).map(|s| s.trim().to_string()).unwrap_or_default();
    let aplicado: i64 = std::fs::read_to_string(cfg().join("hypr/looks/.fontsize")).ok().and_then(|s| s.trim().parse().ok()).unwrap_or(0);

    let vivos = |rev: &str| -> BTreeSet<String> {
        if look.is_empty() {
            return BTreeSet::new();
        }
        let pre = format!("hypr/looks/{look}/");
        rastreados(repo, rev, std::slice::from_ref(&pre))
            .into_iter()
            .filter_map(|p| p.strip_prefix(&pre).map(String::from))
            .filter(|r| !GENERADOS.contains(&r.as_str()))
            .collect()
    };
    // lo que la versión `rev` deja en ~/.config/rel (None: nada)
    let esperado = |rev: &str, rel: &str, vivo: bool| -> Option<Vec<u8>> {
        if !vivo {
            return show(repo, rev, rel);
        }
        let crudo = show(repo, rev, &format!("hypr/looks/{look}/{rel}"));
        let toml = show(repo, rev, &format!("hypr/looks/{look}/look.toml"));
        let (Some(crudo), Some(toml)) = (crudo, toml) else { return show(repo, rev, &format!("hypr/looks/{look}/{rel}")) };
        let Ok(texto) = String::from_utf8(crudo.clone()) else { return Some(crudo) };
        let (fam, disp) = fuentes(&String::from_utf8_lossy(&toml));
        // como el render de leña: los dos marcadores, cada uno con lo suyo
        let texto = texto.replace("{{font}}", &fam).replace("{{font_display}}", &disp);
        Some(ajuste_letra(rel, &texto, aplicado).into_bytes())
    };

    let vivo: BTreeSet<String> = vivos(base).union(&vivos("HEAD")).cloned().collect();
    let archivos: BTreeSet<String> = rastreados(repo, base, &dirs).union(&rastreados(repo, "HEAD", &dirs)).filter(|r| !guardar.contains(*r)).cloned().collect();
    let mut out = Vec::new();
    for rel in archivos.union(&vivo) {
        if guardar.contains(rel) {
            continue;
        }
        let Ok(nuestro) = std::fs::read(cfg().join(rel)) else { continue };
        let es_vivo = vivo.contains(rel);
        let viejo = esperado(base, rel, es_vivo);
        match &viejo {
            None => continue,
            Some(v) if *v == nuestro => continue,
            _ => {}
        }
        let nuevo = esperado("HEAD", rel, es_vivo);
        // igual a lo nuevo, o ya no viene en discada (no se toca ni se usa)
        match &nuevo {
            None => continue,
            Some(n) if *n == nuestro => continue,
            _ => {}
        }
        // sin cambios en discada: no se toca
        out.push((nuevo == viejo, rel.clone()));
    }
    out
}

pub fn main(args: &[String]) -> i32 {
    let (Some(repo), Some(base)) = (args.first(), args.get(1)) else {
        eprintln!("uso: carbon revisa REPO BASE");
        return 2;
    };
    for (guardar, rel) in revisar(Path::new(repo), base) {
        println!("{}\t{rel}", if guardar { "keep" } else { "replace" });
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listas_de_bash() {
        let s = "X=1\nKEEP_ON_UPDATE=(hypr/a.conf hypr/b.lua   # nota\n                hypr/c.conf)\nCONFIG_DIRS=(hypr jale kitty)\n";
        assert_eq!(lista_bash(s, "KEEP_ON_UPDATE"), ["hypr/a.conf", "hypr/b.lua", "hypr/c.conf"]);
        assert_eq!(lista_bash(s, "CONFIG_DIRS"), ["hypr", "jale", "kitty"]);
        assert!(lista_bash(s, "NADA").is_empty());
    }

    #[test]
    fn tamano_de_letra_como_fontsize() {
        assert_eq!(ajuste_letra("kitty/kitty.conf", "font_size 11\nfoo 1\n", 2), "font_size 13\nfoo 1\n");
        assert_eq!(ajuste_letra("kitty/kitty.conf", "font_size  10.5\n", 1), "font_size  11.5\n");
        assert_eq!(ajuste_letra("kitty/kitty.conf", "font_size 7\n", -4), "font_size 6\n");
        assert_eq!(ajuste_letra("swaync/style.css", "a { font-size: 12px; } b { font-size:9px }\n", 1), "a { font-size: 13px; } b { font-size:10px }\n");
        assert_eq!(ajuste_letra("otro.conf", "font_size 11\n", 2), "font_size 11\n");
    }

    #[test]
    fn fuentes_del_look() {
        let (f, d) = fuentes("[font]\nfamily = \"Mono\"\ndisplay = \"Serif\"  # x\n[colors]\nfamily = \"no\"\n");
        assert_eq!((f.as_str(), d.as_str()), ("Mono", "Serif"));
    }
}
