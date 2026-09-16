//! Document conversion core.
//!
//! Two directions, both dependency-light and offline:
//! - [`pdf_to_md`]: extract text from a PDF into Markdown (best-effort, text layer only).
//! - [`md_to_html`]: render Markdown to a standalone HTML document.
//!
//! This module has no knowledge of MCP or JSON-RPC — it is a plain library so it
//! can be unit-tested and reused independently.

use anyhow::{Context, Result};
use lopdf::content::Content;
use lopdf::Object;
use pulldown_cmark::{html, Options, Parser};
use std::collections::BTreeMap;
use std::path::Path;

use crate::md_to_typst;
use crate::world::NxmWorld;

/// A line of text extracted from a PDF page with the font size it was set in.
#[derive(Debug)]
struct Line {
    text: String,
    font_size: f32,
}

/// How to turn raw string bytes shown by a font into Unicode text.
/// Borrows font bytes from `doc`, hence the lifetime.
enum TextDecoder<'a> {
    /// ToUnicode CMap: cid (big-endian, `cid_bytes` wide) -> Unicode text.
    ToUnicode { map: BTreeMap<u32, String>, cid_bytes: u8 },
    /// lopdf's built-in encoding (simple fonts, WinAnsi, etc.).
    Lopdf(lopdf::Encoding<'a>),
}

impl<'a> TextDecoder<'a> {
    /// Decode raw bytes to text. On an undecodable font entry, returns `None`
    /// so callers can drop the segment rather than abort the whole page.
    fn decode(&self, bytes: &[u8]) -> Option<String> {
        match self {
            TextDecoder::ToUnicode { map, cid_bytes } => {
                let step = *cid_bytes as usize;
                let mut out = String::new();
                for chunk in bytes.chunks(step) {
                    let mut cid: u32 = 0;
                    for &b in chunk {
                        cid = (cid << 8) | b as u32;
                    }
                    if let Some(s) = map.get(&cid) {
                        out.push_str(s);
                    }
                }
                (!out.is_empty()).then_some(out)
            }
            TextDecoder::Lopdf(enc) => lopdf::Document::decode_text(enc, bytes).ok(),
        }
    }
}

/// Build a decoder for a font dictionary. Prefers a ToUnicode CMap when the
/// font has one (and we can parse it); otherwise falls back to lopdf's encoding.
///
/// lopdf's own `get_font_encoding` bails on ToUnicode CMaps that declare
/// `/CMapType 0` (which Typst emits), so we parse those ourselves — the
/// bfchar/bfrange sections are just hex pairs.
fn font_decoder<'a>(doc: &'a lopdf::Document, font: &'a lopdf::Dictionary) -> Option<TextDecoder<'a>> {
    // Prefer our own ToUnicode decode when a ToUnicode CMap is present and parses.
    if let Some(parsed) = try_tounicode(doc, font) {
        return Some(parsed);
    }
    // Simple fonts / no ToUnicode: let lopdf handle the base encoding.
    font.get_font_encoding(doc).ok().map(TextDecoder::Lopdf)
}

fn try_tounicode<'a>(doc: &'a lopdf::Document, font: &lopdf::Dictionary) -> Option<TextDecoder<'a>> {
    let id = font
        .get(b"ToUnicode")
        .and_then(|o| Object::as_reference(o))
        .ok()?;
    let Object::Stream(stream) = doc.get_object(id).ok()? else {
        return None;
    };
    let bytes = stream.decompressed_content().ok()?;
    let map = parse_tounicode(&bytes)?;
    Some(TextDecoder::ToUnicode { map, cid_bytes: 2 })
}

/// Parse a ToUnicode CMap's `beginbfchar`/`beginbfrange` sections into a
/// cid -> Unicode mapping. Returns `None` if nothing decodable is found.
fn parse_tounicode(cmap: &[u8]) -> Option<BTreeMap<u32, String>> {
    let mut map = BTreeMap::new();
    let text = std::str::from_utf8(cmap).ok()?;
    let mut started = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("endbfchar") || line.starts_with("endbfrange") {
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        // bfchar: <src> <dst>. bfrange: <lo> <hi> <dst>.
        if parts.len() >= 2 && parts[0].starts_with('<') {
            if let Some((h, m)) = parse_bf_entry(&parts) {
                if !started {
                    started = true;
                }
                map.extend(h);
                let _ = m;
            }
        }
    }
    if map.is_empty() { None } else { Some(map) }
}

/// Parse one data line. `parts` is the whitespace-split line.
/// - bfchar (`<src> <dst>`): insert `src -> dst`.
/// - bfrange (`<lo> <hi> <dst>`): insert each cid in `[lo..=hi] -> dst`.
fn parse_bf_entry(parts: &[&str]) -> Option<(BTreeMap<u32, String>, u8)> {
    let mut map = BTreeMap::new();
    let src = hex_strip(parts[0])?;
    let src_cid = u32::from_str_radix(src, 16).ok()?;
    let cid_bytes = (src.len() / 2).clamp(1, 2) as u8;
    if parts.len() >= 3 {
        let hi_cid = u32::from_str_radix(hex_strip(parts[1])?, 16).ok()?;
        let dst = hex_to_utf16(hex_strip(parts[2])?);
        for c in src_cid..=hi_cid.saturating_add(0) {
            map.insert(c, dst.clone());
        }
    } else {
        let dst = hex_to_utf16(hex_strip(parts[1])?);
        map.insert(src_cid, dst);
    }
    Some((map, cid_bytes))
}

fn hex_strip(s: &str) -> Option<&str> {
    s.strip_prefix('<').and_then(|t| t.strip_suffix('>')).filter(|t| !t.contains('['))
}

/// Hex string -> UTF-16 code units -> Rust String (handles surrogate pairs).
fn hex_to_utf16(hex: &str) -> String {
    let units: Vec<u16> = (0..hex.len())
        .step_by(4)
        .filter_map(|k| u16::from_str_radix(&hex[k..k.min(k + 4)], 16).ok())
        .collect();
    let mut out = String::with_capacity(units.len());
    let mut it = units.iter();
    while let Some(&u) = it.next() {
        if (0xD800..0xDC00).contains(&u) {
            if let Some(&lo) = it.next() {
                if (0xDC00..0xE000).contains(&lo) {
                    if let Some(c) = char::from_u32(0x10000 + ((u as u32 - 0xD800) << 10) + (lo as u32 - 0xDC00)) {
                        out.push(c);
                        continue;
                    }
                }
            }
        }
        out.push(char::from_u32(u as u32).unwrap_or('\u{FFFD}'));
    }
    out
}



/// Extract the text layer of a PDF and return it as Markdown.
///
/// Best-effort heuristics:
/// - lines set in a font noticeably larger than the page's median body size are
///   promoted to Markdown headings (`#` / `##`);
/// - pages are separated by a horizontal rule.
///
/// It does **not** perform OCR (scanned/image-only PDFs will yield little or
/// no text) and does not attempt to reconstruct complex layout (tables,
/// multi-column).
pub fn pdf_to_md(pdf_bytes: &[u8]) -> Result<String> {
    let doc = lopdf::Document::load_mem(pdf_bytes)
        .context("failed to parse PDF (is the file a valid PDF?)")?;

    let mut out = String::new();
    // `get_pages` returns a map of page-number -> object id, ordered by page number.
    let pages: Vec<u32> = doc.get_pages().keys().copied().collect();
    let total = pages.len();

    for (idx, &page_number) in pages.iter().enumerate() {
        let lines = page_lines(&doc, page_number).unwrap_or_else(|| {
            // Fallback for pages whose content stream we can't walk.
            fallback_page_lines(&doc, page_number)
        });
        let body_size = median_font_size(&lines);
        eprintln!("page {}: body_size={}, lines={}", page_number, body_size, lines.len());
        for line in &lines {
            if line.text.is_empty() {
                out.push('\n');
                continue;
            }
            let level = heading_level(line.font_size, body_size);
            eprintln!("  line '{}' size={} -> level={:?}", line.text, line.font_size, level);
            match level {
                Some(level) => {
                    out.push('\n');
                    out.push_str(&"#".repeat(level));
                    out.push(' ');
                    out.push_str(&line.text);
                    out.push_str("\n\n");
                }
                None => {
                    out.push_str(&line.text);
                    out.push('\n');
                }
            }
        }

        // Separate pages with a horizontal rule, except after the last page.
        if idx + 1 < total && !out.trim_end().is_empty() {
            out.push_str("\n---\n\n");
        }
    }
    let result = out.trim_end().to_string() + "\n";
    eprintln!("pdf_to_md result: {result:?}");
    Ok(result)
}

/// Convenience wrapper: read a PDF file from disk and convert it to Markdown.
pub fn pdf_file_to_md(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read PDF file: {}", path.display()))?;
    pdf_to_md(&bytes)
}

/// Walk a page's content stream and collect text lines with their font sizes.
///
/// Mirrors lopdf's `extract_text`: text is decoded through each font's encoding
/// (handles ToUnicode CMaps in subset fonts, which is what Typst-produced PDFs
/// use). On top of that it captures the font size set by the matching `Tf` so
/// headings can be detected from relative text size. Returns `None` when the
/// page has no showable text, so callers can fall back.
fn page_lines(doc: &lopdf::Document, page_number: u32) -> Option<Vec<Line>> {
    let page_id = *doc.get_pages().get(&page_number)?;
    let content = doc.get_and_decode_page_content(page_id).ok()?;

    // Name(\.) -> text decoder. Only keep fonts we can actually decode.
    let decoders: BTreeMap<Vec<u8>, TextDecoder<'_>> = doc
        .get_page_fonts(page_id)
        .ok()?
        .into_iter()
        .filter_map(|(name, font)| font_decoder(doc, font).map(|it| (name, it)))
        .collect();

    let mut lines: Vec<Line> = Vec::new();
    let mut font_size = 0.0_f32;
    let mut current_decoder: Option<&TextDecoder<'_>> = None;

    for op in &content.operations {
        match op.operator.as_str() {
            "Tf" => {
                if let (Some(name), Some(size)) = (
                    op.operands.first().and_then(|o| Object::as_name(o).ok()),
                    op.operands.get(1).and_then(number),
                ) {
                    current_decoder = decoders.get(name);
                    font_size = size;
                }
            }
            "Tj" | "TJ" => {
                let Some(decoded) = current_decoder.and_then(|dec| decode_operands(dec, &op.operands))
                else {
                    continue;
                };
                if append_or_add(&mut lines, &decoded, font_size) {
                    push_break(&mut lines, font_size);
                }
            }
            "'" | "\"" => {
                // Move to the next line, then show text (last operand is the string).
                push_break(&mut lines, font_size);
                if let Some(decoded) = current_decoder
                    .and_then(|dec| op.operands.last().and_then(|o| decode_operands(dec, std::slice::from_ref(o))))
                {
                    append_or_add(&mut lines, &decoded, font_size);
                }
            }
            "Td" | "TD" | "T*" | "ET" => push_break(&mut lines, font_size),
            _ => {}
        }
    }

    if lines.iter().all(|l| l.text.is_empty()) {
        return None;
    }
    // Drop trailing blank lines.
    while matches!(lines.last(), Some(l) if l.text.is_empty()) {
        lines.pop();
    }
    Some(lines)
}

/// Decode the text-showing operands of `Tj`/`TJ` through a font decoder.
/// Returns `None` if no decodable string was produced.
fn decode_operands(decoder: &TextDecoder, operands: &[Object]) -> Option<String> {
    let mut text = String::new();
    for operand in operands {
        match operand {
            Object::String(bytes, _) => {
                if let Some(s) = decoder.decode(bytes) {
                    text.push_str(&s);
                }
            }
            Object::Array(arr) => {
                if let Some(s) = decode_operands(decoder, arr) {
                    text.push_str(&s);
                    text.push(' ');
                }
            }
            Object::Integer(i) => {
                if *i < -100 {
                    text.push(' ');
                }
            }
            _ => {}
        }
    }
    (!text.is_empty()).then_some(text)
}

/// Insert a blank-line boundary unless the previous line is already blank.
fn push_break(lines: &mut Vec<Line>, font_size: f32) {
    if !matches!(lines.last(), Some(l) if l.text.is_empty()) {
        lines.push(Line { text: String::new(), font_size });
    }
}

/// Last-resort extraction when the content stream can't be walked.
fn fallback_page_lines(doc: &lopdf::Document, page_number: u32) -> Vec<Line> {
    doc.extract_text(&[page_number])
        .unwrap_or_default()
        .lines()
        .map(|l| Line {
            text: normalize_extracted_text(l),
            font_size: 0.0,
        })
        .collect()
}

/// Append `text` to the current line, or start a new one. Returns true when the
/// caller should not add an extra line break afterwards (text was merged).
fn append_or_add(lines: &mut Vec<Line>, text: &str, font_size: f32) -> bool {
    let normalized: String = text.replace(char::is_control, "");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return false;
    }
    match lines.last_mut() {
        // Merge consecutive strings of the same font size on one visual line.
        Some(last) if !last.text.is_empty() && (last.font_size - font_size).abs() < 0.5 => {
            last.text.push_str(trimmed);
            true
        }
        _ => {
            lines.push(Line { text: trimmed.to_string(), font_size });
            false
        }
    }
}

fn number(obj: &Object) -> Option<f32> {
    match obj {
        Object::Integer(i) => Some(*i as f32),
        Object::Real(f) => Some(*f),
        _ => None,
    }
}

/// Median font size of non-blank lines — the document's body text size.
fn median_font_size(lines: &[Line]) -> f32 {
    let mut sizes: Vec<f32> = lines
        .iter()
        .filter(|l| !l.text.is_empty() && l.font_size > 0.0)
        .map(|l| l.font_size)
        .collect();
    if sizes.is_empty() {
        return 0.0;
    }
    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sizes[sizes.len() / 2]
}

/// Decide the Markdown heading level from a font size relative to body size.
/// ponytail: two fixed bands, no per-section analysis — tune ratios if output
/// quality on real documents demands it.
fn heading_level(size: f32, body: f32) -> Option<usize> {
    if size <= 0.0 || body <= 0.0 {
        return None;
    }
    let ratio = size / body;
    if ratio >= 1.4 {
        Some(1)
    } else if ratio >= 1.1 {
        Some(2)
    } else {
        None
    }
}

/// Collapse the noisy whitespace that PDF text extraction often produces.
fn normalize_extracted_text(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        // Collapse runs of internal whitespace to single spaces.
        let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
        lines.push(collapsed);
    }
    // Drop leading/trailing empty lines but keep single blank lines between blocks.
    let joined = lines.join("\n");
    joined.trim().to_string()
}

/// Render a Markdown string to a standalone HTML document.
///
/// A true Markdown→PDF conversion requires a PDF rendering engine (layout,
/// fonts, pagination) which is intentionally out of scope for this small,
/// dependency-light tool. Producing clean HTML lets any browser or `wkhtmltopdf`
/// / "Print to PDF" step complete the last mile without pulling a heavy native
/// rendering stack into this crate.
#[allow(dead_code)]
pub fn md_to_html(markdown: &str, title: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options);
    let mut body = String::new();
    html::push_html(&mut body, parser);

    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<title>{title}</title>\n\
<style>body{{max-width:48rem;margin:2rem auto;padding:0 1rem;\
font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;\
line-height:1.6}}pre{{background:#f5f5f5;padding:1rem;overflow:auto}}\
code{{font-family:ui-monospace,monospace}}table{{border-collapse:collapse}}\
td,th{{border:1px solid #ddd;padding:.4rem .6rem}}</style>\n\
</head>\n<body>\n{body}</body>\n</html>\n",
        title = escape_html(title),
        body = body,
    )
}

/// Convenience wrapper: read a Markdown file from disk and render it to HTML.
#[allow(dead_code)]
pub fn md_file_to_html(path: &Path) -> Result<String> {
    let markdown = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read Markdown file: {}", path.display()))?;
    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    Ok(md_to_html(&markdown, title))
}

/// Minimal HTML escaping for text placed into element content / title.
#[allow(dead_code)]
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Convert a Markdown string to a native PDF document.
///
/// Uses Typst as the typesetting engine. The pipeline is:
/// Markdown → Typst markup → Typst compilation → PDF export.
pub fn md_to_pdf(markdown: &str) -> Result<Vec<u8>> {
    let typst_src = md_to_typst::convert(markdown);
    let world = NxmWorld::new(&typst_src);
    let warned = typst::compile(&world);
    let document = warned.output.map_err(|errors| {
        let msgs: Vec<String> = errors.iter().map(|e| format!("{:?}", e)).collect();
        anyhow::anyhow!("Typst compilation failed:\n{}", msgs.join("\n"))
    })?;
    let pdf_bytes = typst_pdf::pdf(&document, &Default::default())
        .map_err(|errors| {
        let msgs: Vec<String> = errors.iter().map(|e| format!("{:?}", e)).collect();
            anyhow::anyhow!("PDF export failed:\n{}", msgs.join("\n"))
        })?;
    Ok(pdf_bytes)
}

/// Convenience wrapper: read a Markdown file from disk and convert it to PDF.
pub fn md_file_to_pdf(path: &Path) -> Result<Vec<u8>> {
    let markdown = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read Markdown file: {}", path.display()))?;
    md_to_pdf(&markdown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md_to_html_renders_heading_and_paragraph() {
        let html = md_to_html("# Title\n\nHello **world**.", "doc");
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("<strong>world</strong>"));
        assert!(html.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn md_to_html_supports_tables() {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |";
        let html = md_to_html(md, "t");
        assert!(html.contains("<table>"));
        assert!(html.contains("<td>1</td>"));
    }

    #[test]
    fn md_to_html_escapes_title() {
        let html = md_to_html("x", "a<b>&c");
        assert!(html.contains("<title>a&lt;b&gt;&amp;c</title>"));
    }

    #[test]
    fn normalize_collapses_whitespace() {
        let got = normalize_extracted_text("  hello    world  \n\n  foo \t bar ");
        assert_eq!(got, "hello world\n\nfoo bar");
    }

    #[test]
    fn pdf_to_md_rejects_garbage() {
        let err = pdf_to_md(b"not a pdf at all").unwrap_err();
        assert!(err.to_string().contains("PDF"));
    }

    /// Round-trip: Markdown → native PDF (Typst) → Markdown. Headings and
    /// multi-page separation must survive.
    #[test]
    fn md_pdf_md_roundtrip_keeps_headings_and_pages() {
        let md = "# Big Title\n\nSome body text here.\n\n## A Subheading\n\n\\pagebreak More text on page two.\n";
        let pdf = md_to_pdf(md).expect("md_to_pdf");
        let back = pdf_to_md(&pdf).expect("pdf_to_md");

        assert!(back.contains("# Big Title"), "h1 lost:\n{back}");
        assert!(back.contains("## A Subheading"), "h2 lost:\n{back}");
        assert!(back.contains("Some body text here"), "body lost:\n{back}");
        assert!(back.contains("More text on page two"), "page 2 lost:\n{back}");
        assert!(back.contains("---"), "page separator missing:\n{back}");
        // Body text must NOT be promoted to a heading.
        assert!(!back.contains("# Some body text"), "body promoted:\n{back}");
    }

    /// A plain-text-only document (no headings) must not gain spurious headings.
    #[test]
    fn pdf_to_md_plain_text_no_headings() {
        let md = "Just a paragraph of plain text with no headings at all.\n";
        let pdf = md_to_pdf(md).expect("md_to_pdf");
        let back = pdf_to_md(&pdf).expect("pdf_to_md");
        assert!(!back.contains("# "), "spurious heading:\n{back}");
        assert!(back.contains("Just a paragraph"), "text lost:\n{back}");
    }

    #[test]
    fn heading_level_thresholds() {
        assert_eq!(heading_level(24.0, 12.0), Some(1)); // 2.0x
        assert_eq!(heading_level(15.0, 12.0), Some(2)); // 1.25x
        assert_eq!(heading_level(13.0, 12.0), None);    // ~1.08x
        assert_eq!(heading_level(12.0, 0.0), None);     // unknown body size
    }
}


#[cfg(test)]
#[ignore]
#[test]
fn dbg2() {
    let md = "# Big Title\n\nSome body text here.\n";
    let pdf = md_to_pdf(md).unwrap();
    let doc = lopdf::Document::load_mem(&pdf).unwrap();
    let pid = *doc.get_pages().get(&1).unwrap();
    let fonts = doc.get_page_fonts(pid).unwrap();
    for (name, font) in &fonts {
        if let Some(TextDecoder::ToUnicode { map, cid_bytes }) = font_decoder(&doc, font) {
            eprintln!("font {name:?} cid_bytes={cid_bytes} entries={}", map.len());
            for (k,v) in map.iter().take(8) { eprintln!("   {k:#06x} -> {v:?}"); }
        }
    }
    let content = doc.get_and_decode_page_content(pid).unwrap();
    for (i,op) in content.operations.iter().enumerate() {
        if op.operator == "Tj" || op.operator=="TJ" {
            if let Some(lopdf::Object::Array(parts)) = op.operands.first() {
                for p in parts {
                    if let lopdf::Object::String(b,_) = p { eprintln!("op{i} bytes = {:02x?}", b); }
                }
            }
        }
    }
    let ln = page_lines(&doc, 1);
    let pdf = md_to_pdf(md).unwrap();
    let doc = lopdf::Document::load_mem(&pdf).unwrap();
    let pid = *doc.get_pages().get(&1).unwrap();
    let fonts = doc.get_page_fonts(pid).unwrap();
    for (name, font) in &fonts {
        let dec = font_decoder(&doc, font);
        eprintln!("font {name:?} -> decoder present: {}", dec.is_some());
    }
    let ln = page_lines(&doc, 1);
    eprintln!("page_lines: {}\n{ln:?}", ln.is_some());
    eprintln!("extract_text: {:?}", doc.extract_text(&[1]));
}

