# Aria-XTRDownloader — edición Arch Linux (GTK4)

> **⚡ Proyecto creado con IA.** Parte del código es generado por IA y curado/verificado por el autor.

Acelerador de descargas sobre **aria2** con interfaz **nativa GTK4**: varias descargas a la vez
con hasta **16 hilos por descarga**, pausa/reanudación que **sobrevive al cierre de la app**
(continúa donde estaba usando los archivos de control `.aria2`) y recuperación automática si el
motor cae.

## Stack (rama `arch`)

| Capa | Tecnología |
|---|---|
| UI | **Rust + gtk4-rs** (GTK ≥ 4.10, CSS oscuro propio) |
| Motor | **aria2c** en modo demonio JSON-RPC (puertos 6800-6804) |
| HTTP-RPC | `ureq` (solo localhost) |
| Persistencia | `registry.json` en `~/.local/share/Aria-XTRDownloader/` |

## Compilar y ejecutar en Arch

```bash
sudo pacman -S --needed base-devel rust aria2 gtk4
git clone -b arch https://github.com/xatornet/XTRDownloader.git
cd XTRDownloader
cargo build --release
./target/release/aria-xtrdownloader
```

## Variables de entorno (opcionales)

| Variable | Efecto |
|---|---|
| `ARIA_XTR_DATA` | Redirige el directorio de datos (registry, logs, sesión) |
| `ARIA_XTR_ENGINE` | Ruta a un binario `aria2c` alternativo |

## Ficheros de datos

- `~/.local/share/Aria-XTRDownloader/registry.json` — lista de descargas (fuente de verdad del resume)
- `~/.local/share/Aria-XTRDownloader/aria2c.log` — log del motor
- `~/.local/share/Aria-XTRDownloader/ui.log` — trazas de la app (diagnóstico)

## Estado

Desarrollo en curso (UI GTK4 + motor). La rama `windows` (Avalonia/.NET) está **terminada y estable**:
esta rama porta la misma lógica de motor, incluidos todos los bugs conocidos de aria2 ya resueltos
(token por sub-llamada en `system.multicall`, `tellWaiting` inutilizable con rpc-secret,
`file-allocation=prealloc` en Linux, re-añadido automático con `continue=true` cuando el demonio
pierde los GIDs).

## Licencia

MIT
