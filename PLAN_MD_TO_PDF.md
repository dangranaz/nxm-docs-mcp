# Piano: MD→PDF nativo con Typst

## Obiettivo
Sostituire l'HTML prodotto da `md_to_pdf` con un **PDF vero**, generato tramite il motore Typst embedded nel binario. Zero dipendenze esterne per l'utente.

## Architettura

```
Markdown → pulldown_cmark → Event stream → md_to_typst converter → Typst markup
                                                                    ↓
                                                    typst::compile() → PagedDocument
                                                                    ↓
                                                    typst_pdf::pdf() → Vec<u8> (PDF bytes)
```

## File da modificare/creare

| File | Azione | Contenuto |
|------|--------|-----------|
| `Cargo.toml` | Modifica | Aggiungere dipendenze `typst`, `typst-pdf`, `typst-library`, `typst-assets` |
| `src/convert.rs` | Modifica | Aggiungere `md_to_pdf()` che ritorna `Vec<u8>`, mantenere `md_to_html()` come fallback |
| `src/md_to_typst.rs` | **Nuovo** | Converter Markdown→Typst markup (~400-500 righe) |
| `src/world.rs` | **Nuovo** | Implementazione minimal di `typst::World` per fornire fonts e sorgente |
| `src/handler.rs` | Modifica | Aggiornare tool `md_to_pdf` per produrre PDF nativo, aggiornare description |
| `src/main.rs` | Modifica | Aggiungere `mod md_to_typst; mod world;` |
| `tests/` | Modifica | Aggiungere test per la conversione MD→PDF |

## Dipendenze da aggiungere

```toml
typst = "0.15"
typst-pdf = "0.15"
typst-library = "0.15"
typst-assets = "0.15"
```

## Componenti chiave

### 1. `md_to_typst.rs` — Converter Markdown→Typst
- Usa `pulldown_cmark` (già dipendenza) per parsare events
- Mapping:
  - `Heading(level)` → `= `, `== `, `=== ` etc.
  - `Paragraph` → testo separato da blank line
  - `CodeBlock(lang, code)` → `` ```lang ... ``` ``
  - `Table` → `#table(columns: N, ...)`
  - `List` → `- ` / `+ ` con indentazione
  - `BlockQuote` → `#quote[...]`
  - `Strong` → `#strong[...]`
  - `Emphasis` → `#emph[...]`
  - `Code` → `` `code` ``
  - `Link` → `#link("url")[text]`
  - `Image` → `#image("path")` (best-effort, skip se remote)

### 2. `world.rs` — Implementazione `typst::World`
- Fornisce il sorgente Typst generato
- Fornisce fonts: embedding di 1-2 font base + system fonts come fallback
- Methods: `source()`, `file()`, `font()`, `library()`

### 3. Aggiornamento `convert.rs`
```rust
pub fn md_to_pdf(markdown: &str) -> Result<Vec<u8>> {
    let typst_src = md_to_typst::convert(markdown);
    let world = TypstWorld::new(&typst_src);
    let document = typst::compile(&world).result.map_err(|e| anyhow!("typst error"))?;
    let pdf_bytes = typst_pdf::pdf(&document, &Default::default())?;
    Ok(pdf_bytes)
}
```

### 4. Aggiornamento `handler.rs`
- `md_to_pdf` ora produce PDF bytes
- Se `output_path` fornito → scrive `.pdf` (non più `.html`)
- Se inline → ritorna hex o base64 del PDF (o continue con HTML come fallback)
- Aggiornare `inputSchema`: `output_path` description → ".pdf"

## Font strategy
- **Embedded**: 1 font sans-serif via `typst-assets` o include_bytes
- **System fallback**: Typst cerca automaticamente nei system fonts
- **Size impact**: ~200-500KB aggiuntivi nel binario (font inclusi)

## Test
1. Unit test `md_to_typst::convert()` con vari input markdown
2. Integration test: MD file → PDF bytes → verifica che inizia con `%PDF-`
3. Test tool `md_to_pdf` via MCP stdio con `output_path`
4. Test errori: markdown vuoto, file non trovato

## Rollback plan
Mantenere `md_to_html()` come funzione pubblica. Se Typst causa problemi, si può sempre usare il path HTML.

## Rischio principale
- **Build time**: Typst ha ~15-20 crate dipendenti, la prima build sarà più lenta (~2-3 min)
- **Binario finale**: più grande (~5-8MB vs ~2MB attuali) per i font embedded
- **Compatibilità**: Typst 0.15 richiede Rust ≥1.75 (già soddisfatto)
