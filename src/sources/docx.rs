//! Word .docx text extraction via `zip` + `quick-xml` (pure Rust, full whitespace control).
//!
//! A .docx is a ZIP whose body text lives in `word/document.xml`: paragraphs are `<w:p>`, runs of
//! text are `<w:t>`, with `<w:tab/>` and `<w:br/>` for tabs and line breaks.

use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result};
use quick_xml::events::Event;

/// Extract the plain text of a .docx file.
pub fn extract(path: &Path) -> Result<String> {
    let file =
        std::fs::File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file)
        .with_context(|| format!("{} is not a valid .docx (zip) file", path.display()))?;
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .context("missing word/document.xml — not a Word document?")?
        .read_to_string(&mut xml)
        .context("failed to read word/document.xml")?;
    extract_from_xml(&xml)
}

/// Pull text out of a WordprocessingML `document.xml` string.
fn extract_from_xml(xml: &str) -> Result<String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut out = String::new();
    let mut in_text = false;

    loop {
        match reader.read_event().context("malformed document.xml")? {
            Event::Eof => break,
            Event::Start(e) if e.name().as_ref() == b"w:t" => in_text = true,
            Event::End(e) => match e.name().as_ref() {
                b"w:t" => in_text = false,
                b"w:p" => out.push('\n'), // paragraph break
                _ => {}
            },
            Event::Empty(e) => match e.name().as_ref() {
                b"w:tab" => out.push('\t'),
                b"w:br" | b"w:cr" => out.push('\n'),
                _ => {}
            },
            Event::Text(e) if in_text => {
                out.push_str(&e.unescape().context("bad text encoding in document.xml")?);
            }
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn extracts_text_runs_and_paragraph_breaks() {
        let xml = r#"<?xml version="1.0"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
          <w:body>
            <w:p><w:r><w:t>Hello</w:t></w:r><w:r><w:t xml:space="preserve"> world</w:t></w:r></w:p>
            <w:p><w:r><w:t>second</w:t><w:tab/><w:t>line</w:t></w:r></w:p>
          </w:body>
        </w:document>"#;
        let text = extract_from_xml(xml).unwrap();
        assert!(text.contains("Hello world"));
        assert!(text.contains("second"));
        assert!(text.contains("line"));
    }

    #[test]
    fn extract_reads_a_real_zip_round_trip() {
        // Build a minimal .docx (a zip containing word/document.xml) and read it back.
        let path = std::env::temp_dir().join(format!("type-cli-test-{}.docx", std::process::id()));
        let xml = r#"<?xml version="1.0"?>
            <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
              <w:body><w:p><w:r><w:t>round trip works</w:t></w:r></w:p></w:body>
            </w:document>"#;
        {
            let f = std::fs::File::create(&path).unwrap();
            let mut zip = zip::ZipWriter::new(f);
            zip.start_file(
                "word/document.xml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(xml.as_bytes()).unwrap();
            zip.finish().unwrap();
        }
        let text = extract(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(text.contains("round trip works"));
    }
}
