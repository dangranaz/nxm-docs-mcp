# HANDOFF — nxm-docs-mcp e strategia pubblico/privato

**Data:** 2026-09-11 (sera)
**Autore sessione:** Kiro
**Per:** sessione di domani (io stesso o altro agente)

---

## TL;DR

Oggi abbiamo (1) deciso la strategia pubblico/privato della suite `nxm-tools`
e (2) creato il primo server MCP pubblico, `nxm-docs-mcp` (PDF↔MD), che **compila
e passa tutti i test**. Manca solo pubblicarlo su GitHub.

> **DA RIPRENDERE DOMANI CON L'UTENTE:** le due domande aperte in fondo
> (#2 obiettivo della pubblicazione, #3 nome dell'ombrello della suite MCP)
> sono decisioni dell'utente, non ancora prese. Vanno chieste a inizio sessione
> — influenzano licenza, comunicazione e naming dei prossimi repo.

---

## PIANO DI PUBBLICAZIONE + LINKEDIN (deciso 2026-09-11 sera)

Obiettivo utente: pubblicare e postare su LinkedIn, in sequenza.
- **Domenica** — un MCP nuovo (candidato: `nxm-docs-mcp`, gia pronto).
- **Mercoledi** — `engine-mlx`.
- **Anche** `nxm-tui` da rendere pubblica (commodity, molto mostrabile).

### Ordine consigliato (rischio decrescente, mostrabilita crescente)
1. **Domenica — `nxm-docs-mcp`**: PRONTO, zero deps private, 11 test verdi.
   Solo `git init` + gitleaks + push `dangranaz` + rifinire README. Rodaggio processo.
2. **Meta settimana — `nxm-tui`**: quasi pronta e MOLTO mostrabile (gira sul
   laptop di chiunque -> GIF/asciinema per LinkedIn). Miglior post visivo.
3. **Mercoledi — `engine-mlx`**: il piu tecnico; richiede prima di pubblicare le
   3 dipendenze (vedi blocker sotto).

### Framing LinkedIn suggerito
Narrazione unica, non post scollegati: prima il piccolo MCP utile (scalda il
pubblico, condivisibile), poi l'engine Rust (mostra profondita). Coerente con
open-core: mostri competenza, NON il differenziatore (`engine-metal` resta
privato — dirlo aumenta la percezione di valore).

### BLOCKER TRASVERSALE — deps git private
`engine-mlx` e `nxm-tui` dipendono da crate via git privati/tag. Un repo pubblico
che **non compila per un estraneo** fa piu danno che non pubblicare.
- `engine-mlx` -> dipende da `nxm-core`, `nxm-shared`, `nxm-sampler` (git privati).
  Vanno pubblicati INSIEME (sono gia "PUBBLICO" in tabella). Test: clone pulito +
  `cargo build`.
- `nxm-tui` -> dipende da `nxm-tools` git pubblico `dangranaz/nxm-tools` tag `v0.1.0`.
  **VERIFICARE** che quel repo+tag esistano davvero pubblici (il `nxm-public/nxm-tools`
  locale e solo un guscio `src/lib.rs`). Senza, la TUI non builda per gli altri.
- **`gitleaks` sulla history** di ogni repo con storia vera (engine-mlx, tui) prima del push.

### Stato reale nxm-tui (esaminato oggi)
`~/Projects/nxm-public/nxm-tui` — **piu pronta del previsto**, non un abbozzo:
- Licenza MIT, repo `dangranaz/nxm-tui`, README curato (install one-liner, provider, features).
- Feature reali: streaming SSE, tool calls agentici, markdown, session save/load,
  overlay reasoning/tool/metrics, tool approval con risk badges, autocomplete.
- 7 file di test (overlay su TestBackend senza TTY, approval, roundtrip, metrics).
- Dep `nxm-tools` gia gestita bene (tag pubblico + `.cargo/config.toml` gitignored per dev).
- Consiglio: pubblicare come **v0.1 / active development** (non "WIP incompleto" —
  e funzionante). NON ha bisogno di sminuirsi.
- `nxm-tui-docs` (analisi competitor, wayfinder) resta SEPARATO/privato.

### Benchmark REALI engine-mlx (correzione — i doc engine erano vecchi)
Fonte: `engine-bench/results/engine-mlx/2026-09-10_15-20-20.md` (commit `1a0c5e2`).
**Qwen3-1.7B-MLX-4bit**: factual ~38 t/s, sustained 41-43 t/s (degr 4%),
length-ramp 43->35 t/s (drop 18%), coherence 40 t/s. => **~40 t/s stabili**,
molto meglio dei ~20 t/s dei doc engine (05-09 set, obsoleti).
- Verdetto "overall FAIL" e per il solo test `instruction` (reply DONE) =
  **limite del MODELLO, non dell'engine** (regola engine-vs-modello, AGENT_TESTING §6).
  Salute engine (latenza, degradazione, API) PASSA. Su LinkedIn spiegare o mostrare
  solo i numeri buoni con framing corretto.
- **1.7B-8bit ROTTO** (-87% sustained, ~8 t/s): NON mostrare finche non fixato.
  Solo 4bit nei materiali pubblici.
- Output ripetitivo visibile ("...Paris. ...Paris. ...Paris."): menzionare o mitigare col sampling.

### engine-metal — stato (esaminato oggi; PRIVATO, non si pubblica)
`~/Projects/nexum/engine-metal` — e il DIFFERENZIATORE, resta privato. Stato:
- STATUS.md fermo al **2026-08-21**: "10/10 crates compilano". Ma "Cosa manca"
  elenca errori di porting objc2 (ops 8, prefill 6, kvcache 5, serve 19) -> i doc
  si contraddicono (probabilmente risolti dopo, vedi CHANGELOG).
- CHANGELOG piu recente: loader safetensors->GPU byte-exact, crate condiviso
  `nxm-model`, kernel fused (SwiGLU Q4/FP32, Norm+LoRA, fused attention, autotuner).
- **NON ancora testato end-to-end** (utente: "ancora non ho iniziato i test").
  Nessun forward pass E2E confermato come in engine-mlx; loader validato standalone
  (`tests/loader.rs` GPU byte-exact `--ignored`), ma il forward-decode non e cablato
  (`engine.rs` "non promette piu il passo Load weights").
- **Conclusione:** engine-metal e a uno stadio piu indietro di engine-mlx (che ha
  E2E + token-exact + benchmark). PRIMA testarlo E2E, POI valutarne le performance.
  Resta privato in ogni caso. Prossimo lavoro tecnico: forward pass funzionante e
  benchmarkabile.

---

## Stato attuale (fatti verificati)

### Progetto creato: `~/Projects/nxm-public/nxm-docs-mcp`
Server MCP in Rust, stdio, dual-license MIT/Apache-2.0.

- `src/convert.rs` — core: `pdf_to_md` (via `lopdf`), `md_to_html` (via `pulldown-cmark`).
- `src/protocol.rs` — tipi JSON-RPC 2.0 minimali (scritti a mano).
- `src/handler.rs` — `initialize`, `tools/list`, `tools/call`; tool `pdf_to_md` e `md_to_pdf`.
- `src/main.rs` — loop stdio (una richiesta JSON per riga).
- `tests/mcp_stdio.rs` — integration test end-to-end sul binario.
- `README.md`, `LICENSE-MIT`, `LICENSE-APACHE`, `.gitignore`.

**Verifiche eseguite oggi (output reale):**
- `cargo build` + `cargo build --release`: OK, zero warning.
- `cargo test`: **11 passati** (9 unit + 2 integration stdio).
- Smoke test manuale `initialize`/`tools/list`: risponde correttamente.

**Scelte di design (non regressioni, decisioni volute):**
- `md_to_pdf` produce **HTML** (pronto per "Print to PDF"), non PDF nativo —
  evita di trascinare un motore di rendering pesante. Documentato nel README.
- `pdf_to_md` è best-effort sul text-layer (no OCR).
- Il layer MCP è scritto a mano (niente macro `rmcp`) per **coerenza col
  `nxm_mcp` esistente**, che usa lo stesso approccio JSON-RPC manuale.

---

## COSA FARE DOMANI (in ordine)

### 1. Pubblicare `nxm-docs-mcp` su GitHub  ← priorità
Richiede conferma utente (operazione verso remote). Passi:
```bash
cd ~/Projects/nxm-public/nxm-docs-mcp
git init && git add -A
git commit -m "Initial commit: PDF<->MD MCP server"
# repo su GitHub: dangranaz/nxm-docs-mcp  (owner CONFERMATO: sempre dangranaz)
git remote add origin https://github.com/dangranaz/nxm-docs-mcp.git
git push -u origin main
```
- ⚠️ Prima del push: **scan `gitleaks`** (anche se il repo è nuovo e pulito, è la
  regola che ci siamo dati per ogni repo pubblico).
- ✅ Owner deciso: **`dangranaz`** (account personale) per tutti i repo pubblici.
  Il Cargo.toml già punta a `github.com/dangranaz/nxm-docs-mcp`.

### 2. Migliorare `pdf_to_md` (opzionale, se c'è tempo)
- Testare su un PDF reale multi-pagina e verificare la qualità dell'estrazione.
- Valutare euristiche di heading (dimensione font) — MA ricordare: se diventa
  "avanzato" è un potenziale pezzo da tenere privato (vedi doc divisione).

### 3. Secondo server pubblico: **web article → MD**
- Nuovo repo pubblico `nxm-web-mcp` (o simile), stesso pattern di `nxm-docs-mcp`.
- ⚠️ **Differenza chiave:** fa **rete in uscita** → trattare contenuto scaricato
  come **untrusted** (rischio prompt-injection via MD prodotto). Estrarre
  testo/struttura, non propagare HTML grezzo. `reqwest` con timeout/limite size.
- Vedi note in `~/Projects/nxm-private/nxm-tools/docs/DIVISIONE_PUBBLICO_PRIVATO.md`.

### 4. Risolvere il conflitto di nomi "nxm-tools" (prima o poi, importante)
Oggi "nxm-tools" indica TRE cose:
1. workspace in `nxm-private/nxm-tools` = di fatto **`nxm-memory`**.
2. crate `nxm_tools` = tool interni della TUI (web_search, read_file…).
3. l'idea di **ombrello per i server MCP**.
→ Scegliere un nome definitivo per l'ombrello e rinominare, prima di aggiungere
altri server.

---

## Decisioni strategiche già prese (contesto)

Modello **open-core**, divisione decisa **per singolo componente**:

| Componente | Pubblico/Privato | Motivo |
|-----------|------------------|--------|
| `engine-mlx` | PUBBLICO | baseline/reference, non è il differenziatore |
| `engine-metal` / `engine-metal-ssd` | PRIVATO | il salto di performance (Metal nativo, MoE spill) |
| `nxm-memory` (server MCP) | PRIVATO (o solo interfaccia/binario) | know-how reale; ha già LICENSE + EULA |
| `nxm-docs-mcp` (PDF↔MD) | PUBBLICO | commodity, vetrina, porta d'ingresso |
| `web→MD` (futuro) | PUBBLICO | commodity |

Principio: pubblico ciò che dimostra competenza e attira; privato il
differenziatore difficile da replicare. I pezzi pubblici fanno da marketing per
il memory commerciale.

**Documenti di riferimento:**
- `~/Projects/nexum/VALUTAZIONE_PUBBLICO_PRIVATO.md` — strategia engine.
- `~/Projects/nxm-private/nxm-tools/docs/DIVISIONE_PUBBLICO_PRIVATO.md` — strategia
  suite MCP (interfaccia vs motore, tabella per-server, opzioni A/B/C).

---

## Igiene segreti — promemoria (dai check di oggi)

- `.env` root di `nexum` contiene chiavi ma NON è tracciato (root non è git). OK.
- `nxm-core` ha `target/` committato → da ripulire (`git rm -r --cached target/`).
- WORKSPACE.md segnala "token hardcoded nei remote" GitLab → da verificare.
- Regola: **`gitleaks` sulla history** prima di rendere pubblico QUALSIASI repo
  (irreversibile).

---

## Domande aperte per l'utente

1. ✅ RISOLTA — Owner GitHub dei repo pubblici: **`dangranaz`** (sempre, account personale).
2. Obiettivo primario della pubblicazione: portfolio / contributori / prodotto?
   (determina quanto spingere sull'open-core e la comunicazione)
3. Nome definitivo dell'ombrello della suite MCP.
