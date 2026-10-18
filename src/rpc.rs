//! Cliente JSON-RPC mínimo para el demonio aria2c.
//! Transporte: HTTP/1.1 sobre TcpStream (solo localhost) — sin dependencias HTTP
//! sujetas a deriva de versiones.
//!
//! Lecciones verificadas contra aria2 1.37 (¡no cambiar sin probar!):
//! - Con --rpc-secret, en system.multicall el token va como PRIMER param de CADA
//!   sub-llamada y NINGUNO en el nivel exterior; el payload es params=[[specs]]
//!   (doble array).
//! - Cada elemento devuelto por multicall es [valor] (array directo) si OK, o
//!   {"code":…, "message":…} si error (NO {"result":[…]}).
//! - aria2.tellWaiting está roto dentro de multicall con rpc-secret: nunca usarlo.
//! - El error de gid desaparecido es literalmente "GID <hex> is not found".

use serde_json::{json, Value};
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

#[derive(Debug)]
pub struct AriaError {
    pub code: String,
    pub message: String,
}

impl fmt::Display for AriaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl AriaError {
    /// El download ya no existe en el demonio (demonio muerto/reiniciado).
    pub fn is_unknown_gid(&self) -> bool {
        let m = self.message.to_lowercase();
        m.contains("is not found") || m.contains("no such") || self.code == "RESOURCE_NOT_FOUND"
    }
}

pub struct Rpc {
    addr: String,
    secret: String,
}

impl Rpc {
    pub fn new(port: u16, secret: &str) -> Self {
        Rpc {
            addr: format!("127.0.0.1:{port}"),
            secret: secret.to_string(),
        }
    }

    /// Llamada con el token añadido como primer param.
    pub fn call(&self, method: &str, params: &[Value]) -> Result<Value, AriaError> {
        let mut prms = vec![json!(format!("token:{}", self.secret))];
        prms.extend(params.iter().cloned());
        self.call_raw(method, prms)
    }

    /// system.multicall: token por sub-llamada, sin token exterior.
    pub fn multicall(&self, calls: &[(&str, Vec<Value>)]) -> Result<Value, AriaError> {
        let specs: Vec<Value> = calls
            .iter()
            .map(|(m, args)| {
                let mut sub = vec![json!(format!("token:{}", self.secret))];
                sub.extend(args.iter().cloned());
                json!({ "methodName": m, "params": sub })
            })
            .collect();
        self.call_raw("system.multicall", vec![json!(specs)])
    }

    fn call_raw(&self, method: &str, params: Vec<Value>) -> Result<Value, AriaError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": "1",
            "method": method,
            "params": params,
        })
        .to_string();

        let request = format!(
            "POST /jsonrpc HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\n\
             Content-Length: {}\r\nConnection: close\r\n\r\n{}",
            self.addr,
            payload.len(),
            payload
        );

        let mut stream = TcpStream::connect(&self.addr).map_err(io_err)?;
        stream.set_write_timeout(Some(Duration::from_secs(12))).map_err(io_err)?;
        stream.set_read_timeout(Some(Duration::from_secs(15))).map_err(io_err)?;
        stream.write_all(request.as_bytes()).map_err(io_err)?;
        stream.flush().map_err(io_err)?;

        // Connection: close ⇒ leemos hasta el final de la conexión.
        let mut buf = Vec::with_capacity(8192);
        stream.read_to_end(&mut buf).map_err(io_err)?;

        // separar cabeceras/cuerpo
        let split = find_headers_end(&buf).ok_or_else(|| AriaError {
            code: "proto".into(),
            message: "respuesta HTTP sin fin de cabeceras".into(),
        })?;
        let body_txt = String::from_utf8_lossy(&buf[split..]);

        let parsed: Value = serde_json::from_str(body_txt.trim_start()).map_err(|e| AriaError {
            code: "parse".into(),
            message: format!("respuesta no-JSON del demonio: {e}"),
        })?;

        if let Some(err) = parsed.get("error") {
            return Err(AriaError {
                code: err.get("code").map(|c| c.to_string()).unwrap_or_default(),
                message: err
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("aria2 error")
                    .to_string(),
            });
        }
        Ok(parsed.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Versión sin fallos (para probes de arranque).
    pub fn try_get_version(&self) -> Option<String> {
        self.call("aria2.getVersion", &[])
            .ok()?
            .get("version")
            .and_then(|v| v.as_str())
            .map(String::from)
    }
}

fn io_err(e: std::io::Error) -> AriaError {
    AriaError { code: "transport".into(), message: e.to_string() }
}

fn find_headers_end(buf: &[u8]) -> Option<usize> {
    if buf.len() < 4 {
        return None;
    }
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
}

// ----- parseo de elementos de multicall -----

/// Éxito => [valor]; tolera también {"result":[valor]}.
pub fn mc_get<'a>(el: &'a Value) -> Option<&'a Value> {
    match el {
        Value::Array(a) => a.first(),
        Value::Object(_) => match el.get("result") {
            Some(Value::Array(a)) => a.first(),
            _ => None,
        },
        _ => None,
    }
}

pub fn mc_err(el: &Value) -> Option<&str> {
    el.get("message")
        .and_then(|m| m.as_str())
        .or_else(|| {
            el
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
        })
}

/// tellStatus devuelve los números como cadenas: aceptar ambos.
pub fn get_i64(st: &Value, key: &str) -> i64 {
    match st.get(key) {
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        _ => 0,
    }
}

pub fn get_f64(st: &Value, key: &str) -> f64 {
    match st.get(key) {
        Some(Value::String(s)) => s.parse().unwrap_or(0.0),
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        _ => 0.0,
    }
}
