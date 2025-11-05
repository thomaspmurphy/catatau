use std::fs::File;
use std::io::Write;
use tempfile::TempDir;
use zip::{ZipWriter, write::FileOptions, CompressionMethod};
use catatau::{EpubReader, App};

fn create_test_epub_with_content() -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = temp_dir.path().join("test.epub");
    let file = File::create(&epub_path).unwrap();
    let mut zip = ZipWriter::new(file);

    // Add mimetype
    zip.start_file("mimetype", FileOptions::<()>::default().compression_method(CompressionMethod::Stored)).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();

    // Add container.xml
    zip.start_file("META-INF/container.xml", FileOptions::<()>::default()).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#).unwrap();

    // Add OPF file
    zip.start_file("content.opf", FileOptions::<()>::default()).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="uuid_id" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Search Test Book</dc:title>
    <dc:creator>Test Author</dc:creator>
  </metadata>
  <manifest>
    <item id="chapter1" href="OEBPS/chapter1.xhtml" media-type="application/xhtml+xml"/>
    <item id="chapter2" href="OEBPS/chapter2.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
    <itemref idref="chapter2"/>
  </spine>
</package>"#).unwrap();

    // Add chapter files with searchable content
    zip.start_file("OEBPS/chapter1.xhtml", FileOptions::<()>::default()).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Chapter 1</title></head>
<body>
<h1>The Beginning</h1>
<p>This is the opening chapter of our test book. It contains a unique phrase: "magic crystal".</p>
<p>The protagonist discovers something extraordinary in the forest.</p>
<p>Lorem ipsum dolor sit amet, consectetur adipiscing elit.</p>
<p>More content to make this chapter longer and more searchable.</p>
<p>The adventure begins with a mysterious sound in the distance.</p>
</body>
</html>"#).unwrap();

    zip.start_file("OEBPS/chapter2.xhtml", FileOptions::<()>::default()).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Chapter 2</title></head>
<body>
<h1>The Journey Continues</h1>
<p>In this second chapter, the story develops further.</p>
<p>Our hero encounters the ancient guardian who speaks of the "magic crystal" once more.</p>
<p>The path ahead is treacherous but necessary.</p>
<p>Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.</p>
<p>The final destination becomes clear as they reach the mountaintop.</p>
</body>
</html>"#).unwrap();

    zip.finish().unwrap();
    (temp_dir, epub_path)
}

#[test]
fn test_app_initialization() {
    let (_temp_dir, epub_path) = create_test_epub_with_content();
    let epub = EpubReader::new(&epub_path).expect("Failed to parse test EPUB");
    let app = App::new(epub);

    assert_eq!(app.current_chapter(), 0);
    assert_eq!(app.scroll_offset(), 0);
    assert_eq!(app.epub().chapter_count(), 2);
}

#[test]
fn test_search_functionality() {
    let (_temp_dir, epub_path) = create_test_epub_with_content();
    let epub = EpubReader::new(&epub_path).expect("Failed to parse test EPUB");

    let results = epub.search("magic crystal");
    assert!(!results.is_empty());
    assert_eq!(results.len(), 2);

    assert_eq!(results[0].chapter_index, 0);
    assert!(results[0].context.contains("magic crystal"));

    assert_eq!(results[1].chapter_index, 1);
    assert!(results[1].context.contains("magic crystal"));
}

#[test]
fn test_search_case_insensitive() {
    let (_temp_dir, epub_path) = create_test_epub_with_content();
    let epub = EpubReader::new(&epub_path).expect("Failed to parse test EPUB");
    
    let results_lower = epub.search("magic crystal");
    let results_upper = epub.search("MAGIC CRYSTAL");
    let results_mixed = epub.search("Magic Crystal");
    
    assert_eq!(results_lower.len(), results_upper.len());
    assert_eq!(results_lower.len(), results_mixed.len());
    assert!(!results_lower.is_empty());
}

#[test]
fn test_search_unique_content() {
    let (_temp_dir, epub_path) = create_test_epub_with_content();
    let epub = EpubReader::new(&epub_path).expect("Failed to parse test EPUB");

    let results = epub.search("extraordinary in the forest");
    assert!(!results.is_empty());
    assert_eq!(results[0].chapter_index, 0);

    let results = epub.search("ancient guardian");
    assert!(!results.is_empty());
    assert_eq!(results[0].chapter_index, 1);
}

#[test]
fn test_chapter_line_count() {
    let (_temp_dir, epub_path) = create_test_epub_with_content();
    let epub = EpubReader::new(&epub_path).expect("Failed to parse test EPUB");
    
    let chapter1_lines = epub.get_chapter_line_count(0);
    let chapter2_lines = epub.get_chapter_line_count(1);
    let invalid_chapter_lines = epub.get_chapter_line_count(999);
    
    assert!(chapter1_lines > 0);
    assert!(chapter2_lines > 0);
    assert_eq!(invalid_chapter_lines, 0);
}

#[test]
fn test_search_with_line_numbers() {
    let (_temp_dir, epub_path) = create_test_epub_with_content();
    let epub = EpubReader::new(&epub_path).expect("Failed to parse test EPUB");
    
    let results = epub.search("Beginning");
    assert!(!results.is_empty());
    
    let first_result = &results[0];
    assert_eq!(first_result.chapter_index, 0);
    assert!(first_result.line_number < epub.get_chapter_line_count(0));
    let chapter0 = epub.get_chapter(0).expect("Failed to get chapter 0");
    assert!(first_result.position < chapter0.content.len());
}