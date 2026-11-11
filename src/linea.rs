//! De bytes de una terminal a renglones limpios: sin colores ANSI, con `\r`
//! que reescribe el renglón (las barras de progreso de pacman y git no se
//! amontonan) y el renglón a medias aparte: si se queda así un rato es que el
//! programa espera algo (una contraseña, un [S/n]).

#[derive(Default)]
pub struct Armador {
    /// el renglón que se está escribiendo
    actual: String,
    /// después de un `\r`: lo que siga reescribe el renglón
    regreso: bool,
    /// bytes de un caracter UTF-8 partido entre dos lecturas
    resto: Vec<u8>,
    /// a media secuencia de escape
    escape: Escape,
}

#[derive(Default, PartialEq)]
enum Escape {
    #[default]
    No,
    /// vio ESC
    Inicio,
    /// ESC [ … hasta una letra
    Csi,
    /// ESC ] … hasta BEL o ESC \
    Osc,
    OscEsc,
}

impl Armador {
    /// Mete bytes y devuelve los renglones que se cerraron.
    pub fn meter(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut datos = std::mem::take(&mut self.resto);
        datos.extend_from_slice(bytes);
        // si el último caracter quedó partido, para la siguiente
        let corte = match std::str::from_utf8(&datos) {
            Ok(_) => datos.len(),
            Err(e) if e.error_len().is_none() => e.valid_up_to(),
            Err(_) => datos.len(),
        };
        self.resto = datos[corte..].to_vec();
        let texto = String::from_utf8_lossy(&datos[..corte]).into_owned();
        let mut listos = Vec::new();
        for c in texto.chars() {
            match self.escape {
                Escape::Inicio => {
                    self.escape = match c {
                        '[' => Escape::Csi,
                        ']' => Escape::Osc,
                        _ => Escape::No,
                    };
                    continue;
                }
                Escape::Csi => {
                    if c.is_ascii_alphabetic() || c == '~' {
                        // ESC[K borra hasta el final: el renglón se reescribe
                        if c == 'K' && self.regreso {
                            self.actual.clear();
                            self.regreso = false;
                        }
                        self.escape = Escape::No;
                    }
                    continue;
                }
                Escape::Osc => {
                    match c {
                        '\x07' => self.escape = Escape::No,
                        '\x1b' => self.escape = Escape::OscEsc,
                        _ => {}
                    }
                    continue;
                }
                Escape::OscEsc => {
                    self.escape = Escape::No;
                    continue;
                }
                Escape::No => {}
            }
            match c {
                '\x1b' => self.escape = Escape::Inicio,
                '\n' => {
                    listos.push(std::mem::take(&mut self.actual).trim_end().to_string());
                    self.regreso = false;
                }
                '\r' => self.regreso = true,
                '\x08' => {
                    self.actual.pop();
                }
                '\t' => self.escribir(' '),
                c if c.is_control() => {}
                c => self.escribir(c),
            }
        }
        listos
    }

    fn escribir(&mut self, c: char) {
        if self.regreso {
            self.actual.clear();
            self.regreso = false;
        }
        self.actual.push(c);
    }

    /// El renglón a medias (lo que se ve sin que haya llegado el `\n`).
    pub fn a_medias(&self) -> &str {
        &self.actual
    }

    /// Lo que quedó al cerrar (un último renglón sin `\n`).
    pub fn cerrar(&mut self) -> Option<String> {
        let r = std::mem::take(&mut self.actual);
        (!r.trim().is_empty()).then(|| r.trim_end().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_colores_y_por_renglon() {
        let mut a = Armador::default();
        assert_eq!(a.meter(b"\x1b[1;35m::\x1b[0m hola\nmun"), [":: hola"]);
        assert_eq!(a.a_medias(), "mun");
        assert_eq!(a.meter(b"do\n"), ["mundo"]);
    }

    #[test]
    fn el_retorno_reescribe() {
        let mut a = Armador::default();
        // como una barra de progreso: 10%\r50%\r100%\n
        assert_eq!(a.meter(b"bajando 10%\rbajando 50%\rbajando 100%\n"), ["bajando 100%"]);
        // \r\n no borra el renglón
        assert_eq!(a.meter(b"listo\r\n"), ["listo"]);
    }

    #[test]
    fn utf8_partido_entre_lecturas() {
        let mut a = Armador::default();
        let s = "contraseña\n".as_bytes();
        let (x, y) = s.split_at(9); // a media ñ
        assert!(a.meter(x).is_empty());
        assert_eq!(a.meter(y), ["contraseña"]);
    }

    #[test]
    fn pregunta_a_medias() {
        let mut a = Armador::default();
        a.meter(b"[sudo] password for dany: ");
        assert_eq!(a.a_medias(), "[sudo] password for dany: ");
        assert_eq!(a.cerrar().as_deref(), Some("[sudo] password for dany:"));
    }

    #[test]
    fn osc_se_ignora() {
        let mut a = Armador::default();
        assert_eq!(a.meter(b"\x1b]0;titulo\x07ok\n"), ["ok"]);
    }
}
