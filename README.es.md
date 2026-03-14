<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.md">English</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

# CommandUI

Entorno de shell nativo de IA con revisión semántica de comandos.

## ¿Qué hace?

- Sesiones de shell PTY reales (no es un envoltorio, ni un chatbot).
- Dos vías de entrada: escritura directa en la terminal (libre) + editor (estructurado/registrado).
- Modo semántico: describe la intención → la IA genera el comando → usted lo revisa/edita/aprueba.
- Confirmación por niveles de riesgo: bajo (automático), medio (configurable), alto (requerido).
- Historial con opciones de reejecución, reapertura del plan y guardar en flujo de trabajo.
- Flujos de trabajo guardados: convierta cualquier comando en un flujo de trabajo reutilizable.
- Memoria específica del proyecto: aprende preferencias a partir de ediciones repetidas.
- Pestañas para múltiples sesiones con flujos de terminal por sesión.
- Persistencia SQLite local (historial, planes, flujos de trabajo, memoria, configuraciones).
- Modos clásico y guiado con diferencias reales en el comportamiento.

## Lo que NO es

- No es un chatbot ni un agente autónomo.
- No es un reemplazo de emulador de terminal.
- No está optimizado para producción (versión temprana v0).

## Distribución del espacio de trabajo

```
commandui/
  apps/desktop/         — Tauri v2 + React 19 desktop app
  packages/domain/      — Pure domain types
  packages/api-contract/ — Request/response contracts
  packages/state/       — Zustand stores
  packages/ui/          — Shared UI primitives (future)
```

## Inicio rápido

```bash
pnpm install
pnpm dev          # Vite dev server
pnpm test         # Run all tests
pnpm typecheck    # TypeScript check

# Rust backend
cd apps/desktop/src-tauri
cargo test
```

## Documentación

- [Configuración para desarrolladores](docs/product/developer-setup.md)
- [Limitaciones conocidas](docs/product/known-limitations.md)
- [Lista de verificación de pruebas básicas](docs/specs/smoke-test-checklist.md)
- [Lista de verificación de lanzamiento](docs/product/release-checklist.md)

## Estado actual

Versión temprana v0 con un núcleo de shell real. Un conjunto de 21 componentes que incluyen: sesiones PTY, bucle de revisión semántica, persistencia, memoria, flujos de trabajo, configuraciones de accesibilidad, pestañas para múltiples sesiones, terminal xterm.js, detección de finalización de comandos.
