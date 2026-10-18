//! Cliente JSON-RPC mínimo para el demonio aria2c (paridad con AriaRpc.cs).
//!
//! Lecciones verificadas contra aria2 1.37 (¡no cambiar sin probar!):
//! - Con --rpc-secret, en system.multicall el token va como PRIMER param de CADA
//!   sub-llamada y NINGUNO en el nivel exterior; el payload es params=[[specs]]
//!   (doble array).
//! - Cada elemento devuelto por multicall es [valor] (array directo) si OK, o
//!   {"code":…,"message":…} si error (NO {"result":[…]}).
//! - aria2.tellWaiting está roto dentro de multicall con rpc-secret: nunca usarlo.
//! - El error de gid desaparecido es literalmente "GID <hex> is not found".

use serde_json::{json, Value};
use std::fmt;
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
    url: String,
    secret: String,
    agent: ureq::Agent,
}

impl Rpc {
    pub fn new(port: u16, secret: &str) -> Self {
        Rpc {
            url: format!("http://127.0.0.1:{port}/jsonrpc"),
            secret: secret.to_string(),
            agent: ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(12))
                .build(),
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
        });
        let resp = self
            .agent
            .post(&self.url)
            .send_json(payload)
            .map_err(map_ureq_err)?;
        let body: Value = resp
            .json()
            .map_err(|e| AriaError { code: "parse".into(), message: e.to_string() })?;

        if let Some(err) = body.get("error") {
            return Err(AriaError {
                code: err.get("code").map(|c| c.to_string()).unwrap_or_default(),
                message: err
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("aria2 error")
                    .to_string(),
            });
        }
        Ok(body.get("result").cloned().unwrap_or(Value::Null))
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

fn map_ureq_err(e: ureq::Error) -> AriaError {
    AriaError {
        code: e.status().map(|s| s.to_string()).unwrap_or_else(|| "transport".into()),
        message: e.to_string(),
    }
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
