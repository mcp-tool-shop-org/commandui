<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.md">English</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

<p align="center">
  <a href="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml"><img src="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/mcp-tool-shop-org/commandui/releases/latest"><img src="https://img.shields.io/github/v/release/mcp-tool-shop-org/commandui?label=Release" alt="Release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue" alt="MIT License" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/"><img src="https://img.shields.io/badge/Landing_Page-live-blue" alt="Landing Page" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/handbook/"><img src="https://img.shields.io/badge/Handbook-read-blue" alt="Handbook" /></a>
</p>

Una interfaz para aquellos a quienes la terminal no da acceso. CommandUI explica cada resultado en lenguaje sencillo, permite solicitar un comando en lenguaje sencillo y nunca ejecuta un comando redactado hasta que lo haya visto y aprobado.

## Para quién es

- Personas que utilizan un lector de pantalla o que no utilizan un ratón
- Personas con baja visión, que necesitan texto más grande o un tema de alto contraste
- Personas a las que les resulta difícil seguir lo que ocurre en la terminal, incluidos principiantes y personas con discapacidades cognitivas o de aprendizaje
- Cualquiera que quiera leer un comando antes de que se ejecute

Aún así, obtienes una interfaz real, con tu propio perfil y más de una sesión. Escribir un comando funciona como siempre lo ha hecho.

## Instalación

- **Microsoft Store:** [CommandUI en Microsoft Store](https://apps.microsoft.com/detail/9NTN1GFQJ91M). Actualmente, la tienda tiene una versión anterior. La actualización descrita aquí está a la espera de pruebas de accesibilidad antes de que se envíe.
- **winget:** `winget install mcp-tool-shop.CommandUI` instala v1.0.0 desde [GitHub Releases](https://github.com/mcp-tool-shop-org/commandui/releases/latest).

Windows 10 o 11, x64. Ask necesita [Ollama](https://ollama.com) en el mismo ordenador con el modelo `qwen2.5:14b`. Todo lo demás funciona sin él.

## Qué hace

- **Cada resultado en una frase.** "Finalizado. 3 líneas de salida" o "No funcionó (código de salida 1). Un archivo o carpeta en ese comando no existe". Un fallo ofrece **Preguntar cómo solucionarlo** y **Volver a ejecutar**.
- **Preguntar en lenguaje sencillo.** Describe la tarea, y CommandUI redacta un comando, lo explica y espera. **Ejecutar plan** es la aprobación, y **Rechazar** no ejecuta nada. Cuando no puede explicar un comando, lo indica.
- **Un sí cuidadoso.** Un comando que elimina archivos o necesita permisos más altos espera hasta que escribas el nombre de la carpeta.
- **El comando sigue ejecutando lo que escribes.** Si una línea parece una solicitud, CommandUI ofrece preguntar en lugar de ejecutar la frase.
- **Flujos de trabajo que puedes crear.** Crea una lista de comandos, edítala, ejecútala y elimínala. Se puede deshacer una eliminación. El historial puede guardar los comandos que elijas.
- **Historial y memoria que controlas.** Busca lo que se ejecutó y lee o elimina lo que CommandUI ha notado.

## Diseñado para el teclado y para los lectores de pantalla

- Los resultados y los errores se anuncian una vez, sin mover el foco.
- **Salida** (Ctrl+Shift+O) muestra la salida de cada comando como texto sin formato, una región por comando, sin códigos de terminal.
- **F1** abre la ayuda del teclado. **Ctrl+Shift+R** salta al último resultado. **Ctrl+Shift+A** cambia entre Comando y Preguntar.
- Cada cuadro de diálogo mantiene el foco dentro de él, y Escape lo cierra y devuelve el foco a donde estabas.
- El tamaño del texto va del 100% al 200% en la configuración. Los paneles debajo de la terminal se pueden ocultar.
- Se respetan los temas de contraste de Windows y la configuración de movimiento reducido.

**Qué no se ha probado todavía:** Narrator, NVDA y los temas de contraste de Windows no han sido probados por personas en esta versión. Estas pruebas se realizarán antes de la actualización en la tienda. Hasta entonces, considera la lista anterior como lo que la aplicación está diseñada para hacer, no como una afirmación probada.

## Seguridad

CommandUI se ejecuta en tu máquina. Mantiene el historial, los planes, los flujos de trabajo, la memoria y la configuración localmente, y solo ejecuta los comandos de la terminal que apruebas. No envía ninguna telemetría. Ask se comunica con un modelo en este ordenador. Si ese modelo no está instalado, no se está ejecutando o no se ha descargado, Ask lo indica y no redacta un comando.

Consulta [SECURITY.md](SECURITY.md) para conocer el modelo de amenazas y cómo informar de una vulnerabilidad.

## Qué no es

- No es un chatbot, ni algo que ejecute un comando redactado por sí solo
- No es una afirmación de que los lectores de pantalla o los temas de contraste se hayan probado en esta versión (véase arriba)
- No es la consola. `apps/console` es una segunda interfaz en este repositorio y no forma parte de la aplicación que instalas

## Para desarrolladores

```bash
pnpm install
pnpm dev          # browser preview; does not run your shell
pnpm test         # all tests
pnpm typecheck

# Rust
cd apps/desktop/src-tauri
cargo test
```

Prepara el paquete para la carga en la tienda a partir de una versión de lanzamiento:

```powershell
./packaging/build-store-exe.ps1
./packaging/pack-msix.ps1
```

`pack-msix.ps1` escribe `release/CommandUI_<version>_x64.msix`. Mantiene el nombre del paquete, el editor y el ejecutable del producto existente de la tienda, y rechaza una versión que no sea superior a la última enviada. El archivo no está firmado; Partner Center lo firma.

```
commandui/
  apps/desktop/                 — the desktop app you install
  apps/console/                 — Rust terminal front end on the same runtime
  crates/runtime-core/          — shell sessions and events
  crates/runtime-persistence/   — local storage
  crates/runtime-planner/       — the local model Ask uses
  packages/                     — shared types, contracts, state, UI
  packaging/msix/               — Store manifest and logos
```

Más: [Handbook](https://mcp-tool-shop-org.github.io/commandui/handbook/) · [Developer Setup](docs/product/developer-setup.md) · [Known Limitations](docs/product/known-limitations.md) · [Release Checklist](docs/product/release-checklist.md)

## Estado

v1.0.2, aún no lanzado. La Microsoft Store tiene una versión anterior, y la versión pública de GitHub es v1.0.0.

Creado por [MCP Tool Shop](https://mcp-tool-shop.github.io/).
