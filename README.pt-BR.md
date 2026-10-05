<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.md">English</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

<p align="center">
  <a href="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml"><img src="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://codecov.io/gh/mcp-tool-shop-org/commandui"><img src="https://codecov.io/gh/mcp-tool-shop-org/commandui/graph/badge.svg" alt="Coverage" /></a>
  <a href="https://github.com/mcp-tool-shop-org/commandui/releases/latest"><img src="https://img.shields.io/github/v/release/mcp-tool-shop-org/commandui?label=Release" alt="Release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue" alt="MIT License" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/"><img src="https://img.shields.io/badge/Landing_Page-live-blue" alt="Landing Page" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/handbook/"><img src="https://img.shields.io/badge/Handbook-read-blue" alt="Handbook" /></a>
</p>

Um ambiente para aqueles que o terminal exclui. O CommandUI explica cada resultado em linguagem simples, permite que você solicite um comando em linguagem simples e nunca executa um comando salvo até que você o tenha visto e aprovado.

## Para quem é

- Pessoas que usam um leitor de tela ou que não usam um mouse
- Pessoas com baixa visão, que precisam de texto maior ou de um tema de alto contraste
- Pessoas que acham o terminal difícil de usar, incluindo iniciantes e pessoas com deficiências cognitivas ou de aprendizado
- Qualquer pessoa que queira ler um comando antes de executá-lo

Você ainda tem um shell real, com seu próprio perfil e mais de uma sessão. Digitar um comando funciona da mesma forma que sempre funcionou.

## Instalação

- **Microsoft Store:** [CommandUI na Microsoft Store](https://apps.microsoft.com/detail/9NTN1GFQJ91M). A Store tem uma versão anterior até que esta atualização seja publicada lá.
- **winget:** `winget install mcp-tool-shop.CommandUI` instala a v1.0.0 a partir de [GitHub Releases](https://github.com/mcp-tool-shop-org/commandui/releases/latest).

Windows 10 ou 11, x64. O recurso "Ask" requer o [Ollama](https://ollama.com) no mesmo computador com o modelo `qwen2.5:14b`. Tudo o mais funciona sem ele.

## O que ele faz

- **Cada resultado em uma frase.** "Concluído. 3 linhas de saída." ou "Não funcionou (código de saída 1). Um arquivo ou pasta nesse comando não existe." Uma falha oferece **Perguntar como corrigir** e **Executar novamente**.
- **Perguntar em linguagem simples.** Descreva a tarefa e o CommandUI cria um comando, explica-o e aguarda. **Executar Plano** é a aprovação, e **Rejeitar** não executa nada. Quando não consegue explicar um comando, ele informa isso.
- **Um "sim" cuidadoso.** Um comando que exclui arquivos ou precisa de permissões mais altas aguarda até que você digite o nome da pasta.
- **O comando ainda executa o que você digita.** Se uma linha parecer um pedido, o CommandUI oferece perguntar em vez de executar a frase.
- **Fluxos de trabalho que você pode criar.** Crie uma lista de comandos, edite-a, execute-a e exclua-a. Uma exclusão pode ser desfeita. O histórico pode salvar os comandos que você escolher.
- **Histórico e memória que você controla.** Pesquise o que foi executado e leia ou exclua o que o CommandUI registrou.

## Criado para o teclado e para leitores de tela

- Os resultados e os erros são anunciados uma vez, sem alterar o foco.
- **Saída** (Ctrl+Shift+O) lista a saída de cada comando como texto simples, uma região por comando, sem códigos de terminal.
- **F1** abre a ajuda do teclado. **Ctrl+Shift+R** vai para o último resultado. **Ctrl+Shift+A** alterna entre Comando e Perguntar.
- Cada diálogo mantém o foco dentro dele, e Esc fecha-o e retorna o foco para onde você estava.
- O tamanho do texto varia de 100% a 200% nas Configurações. Os painéis abaixo do terminal podem ser ocultados.
- Os temas de contraste do Windows e as configurações de movimento reduzido são respeitados.

**O que ainda não foi testado:** O Narrator, o NVDA e os temas de contraste do Windows não foram testados por pessoas nesta versão. Esses testes serão realizados em uma atualização posterior. Até então, trate a lista acima como o que o aplicativo foi projetado para fazer, e não como uma afirmação testada.

## Segurança

O CommandUI é executado em sua máquina. Ele mantém o histórico, os planos, os fluxos de trabalho, a memória e as configurações localmente e executa apenas os comandos do shell que você aprova. Ele não envia nenhuma telemetria. O recurso "Ask" se comunica com um modelo neste computador. Se esse modelo não estiver instalado, não estiver em execução ou não tiver sido baixado, o "Ask" informa isso e não cria um comando.

Consulte [SECURITY.md](SECURITY.md) para obter o modelo de ameaças e como relatar uma vulnerabilidade.

## O que não é

- Não é um chatbot e não é algo que executa um comando salvo por conta própria
- Não é uma afirmação de que os leitores de tela ou os temas de contraste foram testados nesta versão (veja acima)
- Não é o console. `apps/console` é uma segunda interface frontal neste repositório e não faz parte do aplicativo que você instala

## Para desenvolvedores

```bash
pnpm install
pnpm dev          # browser preview; does not run your shell
pnpm test         # all tests
pnpm typecheck

# Rust
cd apps/desktop/src-tauri
cargo test
```

Prepare o pacote para o upload na Store a partir de uma versão de lançamento:

```powershell
./packaging/build-store-exe.ps1
./packaging/pack-msix.ps1
```

`pack-msix.ps1` grava `release/CommandUI_<version>_x64.msix`. Ele mantém o nome do pacote, o editor e o executável do produto existente da Store e rejeita uma versão que não seja posterior à última enviada. O arquivo não é assinado; o Partner Center o assina.

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

Mais: [Handbook](https://mcp-tool-shop-org.github.io/commandui/handbook/) · [Developer Setup](docs/product/developer-setup.md) · [Known Limitations](docs/product/known-limitations.md) · [Release Checklist](docs/product/release-checklist.md)

## Status

v1.0.2, ainda não lançado. A Microsoft Store tem uma versão anterior, e a versão pública do GitHub é a v1.0.0.

Criado por [MCP Tool Shop](https://mcp-tool-shop.github.io/).
