<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.md">English</a> | <a href="README.pt-BR.md">Português (BR)</a>
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

Un'interfaccia per gli utenti che il terminale esclude. CommandUI spiega ogni risultato in modo chiaro, consente di richiedere un comando in linguaggio semplice e non esegue mai un comando preparato finché non lo si è visualizzato e approvato.

## A chi è rivolto

- Persone che utilizzano un lettore di schermo o che non utilizzano il mouse
- Persone con problemi di vista, che necessitano di testo più grande o di un tema ad alto contrasto
- Persone che trovano difficile seguire ciò che accade nel terminale, inclusi i principianti e le persone con disabilità cognitive o di apprendimento
- Chiunque voglia leggere un comando prima che venga eseguito

Si ottiene comunque una vera e propria shell, con il proprio profilo e più di una sessione. Digitare un comando funziona come sempre.

## Installazione

- **Microsoft Store:** [CommandUI sul Microsoft Store](https://apps.microsoft.com/detail/9NTN1GFQJ91M). Lo Store ha una versione precedente fino a quando non verrà pubblicata questa versione.
- **winget:** `winget install mcp-tool-shop.CommandUI` installa la v1.0.0 da [GitHub Releases](https://github.com/mcp-tool-shop-org/commandui/releases/latest).

Windows 10 o 11, x64. La funzione "Ask" richiede [Ollama](https://ollama.com) sullo stesso computer con il modello `qwen2.5:14b`. Tutto il resto funziona anche senza.

## Cosa fa

- **Ogni risultato in una frase.** "Completato. 3 righe di output" o "Non ha funzionato (codice di uscita 1). Un file o una cartella in quel comando non è presente". In caso di errore, offre la possibilità di **chiedere come risolverlo** e di **riprovare**.
- **Chiedere in linguaggio semplice.** Descrivere l'attività e CommandUI prepara un comando, lo spiega e attende. **Run Plan** è l'approvazione, e **Reject** non esegue nulla. Quando non può spiegare un comando, lo comunica.
- **Un'attenta conferma.** Un comando che elimina file o richiede autorizzazioni più elevate attende finché non si digita il nome della cartella.
- **Il comando esegue comunque ciò che si digita.** Se una riga sembra una richiesta, CommandUI offre di chiedere invece di eseguire la frase.
- **Flussi di lavoro che è possibile creare.** Creare un elenco di comandi, modificarlo, eseguirlo ed eliminarlo. Un'eliminazione può essere annullata. La cronologia può salvare i comandi selezionati.
- **Cronologia e memoria che si possono controllare.** Cercare cosa è stato eseguito e leggere o eliminare ciò che CommandUI ha rilevato.

## Progettato per la tastiera e per i lettori di schermo

- I risultati e gli errori vengono annunciati una sola volta, senza spostare il focus.
- **Output** (Ctrl+Shift+O) elenca l'output di ogni comando come testo semplice, una sezione per comando, senza codici del terminale.
- **F1** apre la guida per la tastiera. **Ctrl+Shift+R** passa all'ultimo risultato. **Ctrl+Shift+A** passa da Comando a Chiedi.
- Ogni finestra di dialogo mantiene il focus al suo interno e, premendo Esc, la finestra si chiude e il focus torna alla posizione precedente.
- La dimensione del testo varia dal 100% al 200% nelle impostazioni. I pannelli sotto il terminale possono essere nascosti.
- I temi di contrasto di Windows e le impostazioni di riduzione del movimento vengono rispettati.

**Cosa non è ancora stato testato:** Narrator, NVDA e i temi di contrasto di Windows non sono stati testati da persone che utilizzano questa versione. Questi test verranno eseguiti in un aggiornamento successivo. Fino ad allora, considerare l'elenco precedente come ciò per cui l'app è stata progettata, e non come un'affermazione testata.

## Sicurezza

CommandUI viene eseguito sulla propria macchina. Mantiene cronologia, piani, flussi di lavoro, memoria e impostazioni localmente ed esegue solo i comandi della shell che si approvano. Non invia dati di telemetria. La funzione "Ask" comunica con un modello su questo computer. Se tale modello non è installato, non è in esecuzione o non è stato scaricato, "Ask" lo comunica e non prepara un comando.

Consultare [SECURITY.md](SECURITY.md) per il modello di minaccia e per sapere come segnalare una vulnerabilità.

## Cosa non è

- Non è un chatbot e non esegue un comando preparato in autonomia
- Non è un'affermazione sul fatto che i lettori di schermo o i temi di contrasto siano stati testati in questa versione (vedere sopra)
- Non è la console. `apps/console` è un secondo frontend in questo repository e non fa parte dell'app che si installa

## Per gli sviluppatori

```bash
pnpm install
pnpm dev          # browser preview; does not run your shell
pnpm test         # all tests
pnpm typecheck

# Rust
cd apps/desktop/src-tauri
cargo test
```

Preparare il pacchetto per il caricamento sullo Store da una versione di rilascio:

```powershell
./packaging/build-store-exe.ps1
./packaging/pack-msix.ps1
```

`pack-msix.ps1` scrive `release/CommandUI_<version>_x64.msix`. Mantiene il nome del pacchetto, l'editore e l'eseguibile del prodotto esistente sullo Store e rifiuta una versione che non è successiva all'ultima inviata. Il file non è firmato; Partner Center lo firma.

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

Ulteriori informazioni: [Handbook](https://mcp-tool-shop.github.io/commandui/handbook/) · [Developer Setup](docs/product/developer-setup.md) · [Known Limitations](docs/product/known-limitations.md) · [Release Checklist](docs/product/release-checklist.md)

## Stato

v1.0.2, non ancora rilasciata. Il Microsoft Store ha una versione precedente e il rilascio pubblico su GitHub è la v1.0.0.

Realizzato da [MCP Tool Shop](https://mcp-tool-shop.github.io/).
