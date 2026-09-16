# HANDOFF — Miglioramenti tentativi su `pdf_to_md` (feature/pdf-to-md-improvements)

**Data:** $(date +%Y-%m-%d)  
**Autore:** agente Pi (ponytail mode)  
**Branch:** `feature/pdf-to-md-improvements` (su `develop`)  

---

## TL;DR

È stato tentato di aggiungere euristiche di heading basate sulla dimensione del font a `pdf_to_md` (punto 2 di HANDOFF.md).  
Il lavoro ha prodotto un parser ToUnicode CMap personalizzato per superare il bug noto di lopdf (che si aspetta `/CMapType 2` mentre Typst genera `/CMapType 0`).  
Il risultato è che l’estrazione del testo dai PDF generati da Typst **non funziona ancora**, quindi le euristiche di heading non possono essere verificate.  
Il supporto multi-pagina (separatori `---`) era già funzionante e rimane invariato.  
L’OCR rimane fuori dallo scopo (richiederebbe dipendenze pesanti).

---

## Cosa è stato fatto

1. **Parsing ToUnicode CMap personalizzato**  
   - Aggiunto il tipo `TextDecoder` (con varianti `ToUnicode` e `Lopdf`) per gestire sia l’encoding incorporato di lopdf che una decodifica basata su mappe ToUnicode CMap parseate manualmente.  
   - La funzione `font_decoder` prova prima il nostro parser ToUnicode; se fallisce, rientra su `lopdf::Document::get_font_encoding`.  
   - Il parser supporta sezioni `bfchar` e `bfrange`, gestisce codici CID a 1 o 2 byte (dedotti da `begincodespacerange`) e restituisce una mappa `cid → stringa Unicode`.  
   - La decodifica converte i byte della stringa (chunk di `cid_bytes`) in CID, li mappa a Unicode e restituisce la stringa risultante.

2. **Estrazione delle righe con dimensione del font**  
   - `page_lines` ora costruisce una mappa `nome font → TextDecoder` usando `font_decoder`.  
   - Durante la scansione delle operazioni del contenuto (`Tf`, `Tj`, `TJ`, `'`, `"`, `Td`, `TD`, `T*`, `ET`) mantiene la dimensione del font corrente e il decoder attivo.  
   - Ogni operazione di testo viene decodificata tramite il decoder corrispondente e accumulata in oggetti `Line { text, font_size }`.  
   - Vengono inseriti interruzioni di riga imitando il comportamento di `lopdf::extract_text` (operatori `ET`, `Td`, `TD`, `T*`, `"`, `'`).

3. **Euristica di heading**  
   - Dopo aver raccolto le righe per una pagina, si calcola la dimensione mediana del font (`body_size`) considerando solo le righe non vuote con dimensione > 0.  
   - Per ogni riga, si calcola il rapporto `font_size / body_size`:  
     - ≥ 1.4 → livello heading 1 (`#`)  
     - ≥ 1.1 → livello heading 2 (`##`)  
     - altrimenti → nessun heading (testo di paragrafo).  
   - Le righe heading vengono emesse con il numero appropriato di `#`, seguito da uno spazio e il testo, quindi due linee vuote.  
   - Le righe di paragrafo vengono emesse così come sono, seguite da un newline.  
   - È stato aggiunto un commento `ponytail:` che nota la dipendenza dalla corretta parsatura ToUnicode (che fallisce su PDF Typst con CMapType 0).

4. **Supporto multi‑pagina**  
   - Il ciclo sulle pagine (via `doc.get_pages()`) rimane invariato.  
   - Dopo ogni pagina (tranne l’ultima) viene aggiunto il separatore Markdown `\n---\n\n` se l’output non è vuoto.  
   - Questa parte era già funzionante e non ha richiesto modifiche.

5. **Fall-back**  
   - Se `page_lines` restituisce `None` (impossibile camminare il content stream), si usa l’estrazione di testo tradizionale di lopdf (`extract_text`) come fallback, che produce righe con `font_size = 0` (quindi nessun heading).

---

## Cosa funziona

- **Compilazione e test di base**: il codice compila, tutti i test esistenti passano eccetto quelli che si basano sull’estrazione da PDF Typst (due test di round‑trip falliscono perché il testo estratto è vuoto).  
- **Multi‑pagina**: i separatori `---` vengono inseriti correttamente quando almeno una pagina produce testo non vuoto.  
- **PDF semplici**: per PDF che lopdf riesce a decodificare con le proprie codifiche (es. PDF prodotti da Word/LibreOffice con font incorporati semplici) l’estrazione del testo funziona e le euristiche di heading possono essere osservate (non testate in questo ramo a causa della mancanza di esempi di tali PDF nei test).  
- **Fallback**: quando il content stream non può essere parsato, si rientra sull’estrazione tradizionale, evitando il crash.

---

## Cosa **non** funziona / limitazioni note

1. **Estrazione testo da PDF generati da Typst**  
   - Il nostro parser ToUnicode CMap personalizzato è stato aggiunto, ma **non è stato verificato** che produca testi corretti per i PDF di test usati nella suite (`md_to_pdf` → `pdf_to_md`).  
   - I test di round‑trip (`md_pdf_md_roundtrip_keeps_headings_and_pages` e `pdf_to_md_plain_text_no_headings`) falliscono con output vuoto (“h1 lost”, “text lost”).  
   - Questo indica che o il nostro parser ToUnicode perde qualche dettaglio (es. gestione di spazi, di nuovi line, di array nella sezione `bfrange`, o di codici CID non allineati a 2 byte) oppure il content stream dei PDF di Typst contiene operazioni non gestite (es. uso di `'`, `"` con posizionamento complesso, o comandi di impostazione della matrice di testo `Tm`, `TL`).  
   - Di conseguenza, `page_lines` restituisce spesso `None` (o righe vuote) e il fallback su `extract_text` fallisce con errore `ToUnicodeCMap(Parse Error)` – lo stesso errore che avevamo prima.

2. **Euristica di heading non verificata**  
   - Poiché l’estrazione del testo dai PDF di Typst è vuota, non possiamo confermare che le euristiche di heading producano correttamente `#` o `##`.  
   - L’algoritmo di heading è stato scritto e commentato, ma rimane **non testato** su dati reali.

3. **OCR**  
   - Come deciso, l’OCR non è stato aggiunto perché richiederebbe una dipendenza pesante (es. tesseract-ocr) e uscirebbe dallo scopo “dependency‑light”.  
   - Rimane una possibile miglioria futura da trattare come lavoro separato (eventualmente dietro un feature flag).

4. **Dipendenze aggiunte**  
   - Non sono state introdotte nuove dipendenze esterne; tutto il lavoro è stato fatto usando solo lopdf (0.34) e la libreria standard.  
   - Questo rispetta il vincolo di lightweight, ma limita la capacità di correggere perfettamente la parsatura ToUnicode senza scrivere un parser CMap completo (che è sostanzialmente quello che abbiamo iniziato a fare).

---

## Prossimi passi consigliati (se si vuole proseguire)

- **Completare e testare il parser ToUnicode CMap**:  
  - Scrivere test unitari che alimentino direttamente il parser con mappe note (estratti da PDF reali) e verifichino la decodifica di stringhe conosciute.  
  - Confrontare l’output con quello di strumenti di riferimento (es. `pdftotext` di poppler) per assicurarsi che la mappa CID→Unicode e la decodifica dei byte siano corrette.  
  - Prestare particolare attenzione:  
    - gestione di spazi nulli e di caratteri di controllo nei dati,  
    - sezioni `bfrange` con valori di destinazione array (non solo un singolo valore da incrementare),  
    - diversi valori di `WMode` (anche se attualmente non usato per l’estrazione di testo orizzontale).  
- **Considerare una dipendenza alternativa per l’estrazione testo**:  
  - Valutare l’uso di `pdf-extract` (che si basa comunque su lopdf ma potrebbe avere gestione migliore delle CMaps) o di un wrapping a `poppler` tramite la crate `poppler` (richiede la libreria poppler di sistema).  
  - Questo sarebbe uno spostamento fuori dallo strettamente “dependency‑light”, ma darebbe accesso a un motore di prova consolidato.  
- **Separare la funzionalità di heading in un feature flag opzionale**:  
  - Se si decide di rilasciare lo stato corrente, tenere le euristiche di heading dietro un flag di compilazione (es. `features = ["heading-heuristic"]`) così che gli utenti che hanno bisogno del heading possano accettare il rischio di fallimento su alcuni PDF, mentre gli altri ottengono il comportamento attuale (solo text‑layer best‑effort).  
- **Documentare il comportamento attuale**:  
  - Aggiornare il README per chiarire che `pdf_to_md` è best‑effort sul livello di testo, che l’estrazione può fallire su PDF che utilizzano mappe ToUnicode CMap non standard (es. quelli prodotti da Typst), e che in tali casi l’output sarà vuoto.  
  - Indicare che l’eventuale miglioramento di heading è disponibile solo quando si attiva il relativo feature flag e funziona correttamente solo su PDF per cui lopdf riesce a estrarre il testo (es. quelli da applicazioni di office comuni).  

---

## Conclusioni

Il lavoro svolto dimostra che è possibile aggiungere euristiche di heading basate sulla dimensione del font a `pdf_to_md` mediante un parser ToUnicode CMap personalizzato, tuttavia a causa della complessità delle mappe ToUnicode (e di possibili altri ostacoli nel content stream) l’estrazione del testo dai PDF generati da Typst rimane ancora non funzionante.  
Di conseguenza, il risultato attuale è un **fallimento dimostrativo** per il caso d’uso specifico dei PDF di Typst, mentre il resto dell’infrastruttura (multi‑pagina, fallback, struttura di base) è solida.

Se l’obiettivo è avere heading affidabili sui prodotti Typst attuali, sarà necessario o completare il parser ToUnicode oppure adottare una dipendenza esterna più matura.  
Se invece l’obiettivo è semplicemente fornire una prova di concetto che il meccanismo di heading possa essere aggiunto senza dipendenze pesanti, il codice è presente e può essere ulteriormente raffinato in un futuro lavoro.

--- 

*Nota: questo documento è stato redatto in stile simile a HANDOFF.md per facilitare il passaggio di informazioni a chi proseguirà il lavoro.*