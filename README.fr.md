<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.md">English</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
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

Une interface pour les utilisateurs que le terminal exclut. CommandUI explique chaque résultat en termes simples, vous permet de demander une commande en termes simples, et n’exécute jamais une commande préparée tant que vous ne l’avez pas vue et approuvée.

## À qui cela s’adresse

- Les personnes qui utilisent un lecteur d’écran, ou qui n’utilisent pas de souris
- Les personnes ayant une faible vision, qui ont besoin d’un texte plus grand ou d’un thème à contraste élevé
- Les personnes qui trouvent le terminal difficile à suivre, y compris les débutants et les personnes ayant des troubles cognitifs ou d’apprentissage
- Toute personne qui souhaite lire une commande avant qu’elle ne soit exécutée

Vous disposez toujours d’une interface en ligne de commande réelle, avec votre propre profil et plus d’une session. La saisie d’une commande fonctionne comme toujours.

## Installation

- **Microsoft Store :** [CommandUI sur le Microsoft Store](https://apps.microsoft.com/detail/9NTN1GFQJ91M). Le Store propose actuellement une version antérieure. La mise à jour décrite ici est en attente de tests d’accessibilité avant d’être soumise.
- **winget :** `winget install mcp-tool-shop.CommandUI` installe la version 1.0.0 à partir de [GitHub Releases](https://github.com/mcp-tool-shop-org/commandui/releases/latest).

Windows 10 ou 11, x64. La fonction « Ask » nécessite [Ollama](https://ollama.com) sur le même ordinateur avec le modèle `qwen2.5:14b`. Tout le reste fonctionne sans.

## Ce que cela fait

- **Chaque résultat dans une phrase.** « Terminé. 3 lignes de sortie. » ou « N’a pas fonctionné (code de sortie 1). Un fichier ou un dossier dans cette commande est introuvable. » En cas d’échec, l’application propose de **demander comment le corriger** et de **relancer**.
- **Demander en termes simples.** Décrivez la tâche, et CommandUI prépare une commande, l’explique et attend. « Exécuter le plan » est l’approbation, et « Refuser » ne lance rien. Lorsque l’application ne peut pas expliquer une commande, elle le signale.
- **Un « oui » prudent.** Une commande qui supprime des fichiers ou nécessite des autorisations plus élevées attend que vous saisissiez le nom du dossier.
- **La commande exécute toujours ce que vous tapez.** Si une ligne ressemble à une demande, CommandUI propose de demander plutôt que d’exécuter la phrase.
- **Des flux de travail que vous pouvez créer.** Créez une liste de commandes, modifiez-la, exécutez-la et supprimez-la. Une suppression peut être annulée. L’historique peut enregistrer les commandes que vous sélectionnez.
- **Historique et mémoire que vous contrôlez.** Recherchez ce qui a été exécuté, et lisez ou supprimez ce que CommandUI a remarqué.

## Conçu pour le clavier et les lecteurs d’écran

- Les résultats et les erreurs sont annoncés une seule fois, sans déplacer votre curseur.
- **Sortie** (Ctrl+Maj+O) affiche la sortie de chaque commande sous forme de texte brut, une région par commande, sans codes de terminal.
- **F1** ouvre l’aide du clavier. **Ctrl+Maj+R** permet d’accéder au dernier résultat. **Ctrl+Maj+A** permet de basculer entre la commande et la demande.
- Chaque boîte de dialogue conserve le focus à l’intérieur et la touche Échap la ferme et rétablit le focus à l’endroit où vous vous trouviez.
- La taille du texte varie de 100 % à 200 % dans les paramètres. Les panneaux situés sous le terminal peuvent être masqués.
- Les thèmes de contraste de Windows et les paramètres de réduction du mouvement sont respectés.

**Ce qui n’a pas encore été testé :** Narrateur, NVDA et les thèmes de contraste de Windows n’ont pas été testés par des utilisateurs dans cette version. Ces tests seront effectués avant la mise à jour du Store. Jusqu’alors, considérez la liste ci-dessus comme ce pour quoi l’application a été conçue, et non comme une affirmation testée.

## Sécurité

CommandUI s’exécute sur votre machine. Il conserve l’historique, les plans, les flux de travail, la mémoire et les paramètres localement, et n’exécute que les commandes de l’interface en ligne de commande que vous approuvez. Il n’envoie aucune télémétrie. La fonction « Ask » communique avec un modèle sur cet ordinateur. Si ce modèle n’est pas installé, en cours d’exécution ou téléchargé, la fonction « Ask » le signale et ne prépare pas de commande.

Consultez [SECURITY.md](SECURITY.md) pour connaître le modèle de menace et la manière de signaler une vulnérabilité.

## Ce que ce n’est pas

- Ce n’est pas un chatbot, et ce n’est pas un outil qui exécute une commande préparée de manière autonome
- Ce n’est pas une affirmation selon laquelle les lecteurs d’écran ou les thèmes de contraste ont été testés dans cette version (voir ci-dessus)
- Ce n’est pas la console. `apps/console` est une deuxième interface dans ce dépôt et ne fait pas partie de l’application que vous installez.

## Pour les développeurs

```bash
pnpm install
pnpm dev          # browser preview; does not run your shell
pnpm test         # all tests
pnpm typecheck

# Rust
cd apps/desktop/src-tauri
cargo test
```

Préparez le package pour le téléchargement sur le Store à partir d’une version de publication :

```powershell
./packaging/build-store-exe.ps1
./packaging/pack-msix.ps1
```

`pack-msix.ps1` écrit `release/CommandUI_<version>_x64.msix`. Il conserve le nom du package, l’éditeur et l’exécutable du produit existant du Store, et refuse une version qui n’est pas supérieure à la dernière version soumise. Le fichier n’est pas signé ; le Centre des partenaires le signe.

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

Plus d’informations : [Handbook](https://mcp-tool-shop-org.github.io/commandui/handbook/) · [Developer Setup](docs/product/developer-setup.md) · [Known Limitations](docs/product/known-limitations.md) · [Release Checklist](docs/product/release-checklist.md)

## État

v1.0.2, pas encore publiée. Le Microsoft Store propose une version antérieure, et la version publique sur GitHub est la v1.0.0.

Créé par [MCP Tool Shop](https://mcp-tool-shop.github.io/).
