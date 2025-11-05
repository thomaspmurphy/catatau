use std::fs::File;
use std::io::Write;
use tempfile::TempDir;
use zip::{ZipWriter, write::FileOptions, CompressionMethod};
use catatau::{EpubReader, EpubError};

fn create_test_epub() -> (TempDir, std::path::PathBuf) {
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
    <dc:title>Test Book</dc:title>
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

    // Add chapter files
    zip.start_file("OEBPS/chapter1.xhtml", FileOptions::<()>::default()).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Chapter 1</title></head>
<body>
<h1>Chapter One</h1>
<p>This is the first chapter of our test book. It contains some sample text that we can search through and navigate.</p>
<p>Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.</p>
</body>
</html>"#).unwrap();

    zip.start_file("OEBPS/chapter2.xhtml", FileOptions::<()>::default()).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml">
<head><title>Chapter 2</title></head>
<body>
<h1>Chapter Two</h1>
<p>This is the second chapter. It has different content that we can use for testing search functionality.</p>
<p>Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.</p>
</body>
</html>"#).unwrap();

    zip.finish().unwrap();
    (temp_dir, epub_path)
}

#[test]
fn test_epub_parsing() {
    let (_temp_dir, epub_path) = create_test_epub();
    let epub = EpubReader::new(&epub_path).expect("Failed to parse test EPUB");

    assert_eq!(epub.title, "Test Book");
    assert_eq!(epub.author, "Test Author");
    assert_eq!(epub.chapter_count(), 2);

    let chapter0 = epub.get_chapter(0).expect("Failed to get chapter 0");
    assert_eq!(chapter0.title, "Chapter 1");
    assert!(chapter0.content.contains("Chapter One"));
    assert!(chapter0.content.contains("first chapter"));

    let chapter1 = epub.get_chapter(1).expect("Failed to get chapter 1");
    assert_eq!(chapter1.title, "Chapter 2");
    assert!(chapter1.content.contains("Chapter Two"));
    assert!(chapter1.content.contains("second chapter"));
}

#[test]
fn test_chapter_content_extraction() {
    let (_temp_dir, epub_path) = create_test_epub();
    let epub = EpubReader::new(&epub_path).expect("Failed to parse test EPUB");

    let chapter1 = epub.get_chapter(0).expect("Failed to get chapter 0");
    assert!(chapter1.content.contains("Chapter One"));
    assert!(chapter1.content.contains("Lorem ipsum"));

    assert!(!chapter1.content.contains("<h1>"));
    assert!(!chapter1.content.contains("<p>"));
}

#[test]
fn test_search_content() {
    let (_temp_dir, epub_path) = create_test_epub();
    let epub = EpubReader::new(&epub_path).expect("Failed to parse test EPUB");

    let search_results = epub.search("Lorem ipsum");
    assert!(!search_results.is_empty());
    assert_eq!(search_results[0].chapter_index, 0);
    assert!(search_results[0].context.contains("Lorem ipsum"));
    
    let search_results_2 = epub.search("second chapter");
    assert!(!search_results_2.is_empty());
    assert_eq!(search_results_2[0].chapter_index, 1);
}

#[test]
fn test_empty_search() {
    let (_temp_dir, epub_path) = create_test_epub();
    let epub = EpubReader::new(&epub_path).expect("Failed to parse test EPUB");
    
    let search_results = epub.search("nonexistent text");
    assert!(search_results.is_empty());
}

#[test]
fn test_case_insensitive_search() {
    let (_temp_dir, epub_path) = create_test_epub();
    let epub = EpubReader::new(&epub_path).expect("Failed to parse test EPUB");
    
    let search_results = epub.search("LOREM IPSUM");
    assert!(!search_results.is_empty());
    
    let search_results_2 = epub.search("chapter one");
    assert!(!search_results_2.is_empty());
}

#[test] 
fn test_missing_container_xml() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = temp_dir.path().join("no_container.epub");
    let file = File::create(&epub_path).unwrap();
    let mut zip = ZipWriter::new(file);
    
    // Add mimetype but no container.xml
    zip.start_file("mimetype", FileOptions::<()>::default().compression_method(CompressionMethod::Stored)).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();
    
    zip.finish().unwrap();
    
    let result = EpubReader::new(&epub_path);
    assert!(result.is_err());
    match result.unwrap_err() {
        EpubError::ContainerNotFound => {}
        other => panic!("Expected ContainerNotFound, got: {:?}", other),
    }
}

#[test]
fn test_missing_opf_file() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = temp_dir.path().join("no_opf.epub");
    let file = File::create(&epub_path).unwrap();
    let mut zip = ZipWriter::new(file);
    
    // Add mimetype
    zip.start_file("mimetype", FileOptions::<()>::default().compression_method(CompressionMethod::Stored)).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();
    
    // Add container.xml pointing to non-existent OPF
    zip.start_file("META-INF/container.xml", FileOptions::<()>::default()).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="missing.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>"#).unwrap();
    
    zip.finish().unwrap();
    
    let result = EpubReader::new(&epub_path);
    assert!(result.is_err());
    match result.unwrap_err() {
        EpubError::OpfNotFound => {}
        other => panic!("Expected OpfNotFound, got: {:?}", other),
    }
}

#[test] 
fn test_invalid_opf_structure() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = temp_dir.path().join("invalid_opf.epub");
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
    
    // Add OPF file with no spine (invalid structure)
    zip.start_file("content.opf", FileOptions::<()>::default()).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="uuid_id" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Test Book</dc:title>
    <dc:creator>Test Author</dc:creator>
  </metadata>
  <manifest>
    <item id="chapter1" href="chapter1.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
</package>"#).unwrap();
    
    zip.finish().unwrap();
    
    let result = EpubReader::new(&epub_path);
    assert!(result.is_err());
    match result.unwrap_err() {
        EpubError::InvalidOpfStructure => {}
        other => panic!("Expected InvalidOpfStructure, got: {:?}", other),
    }
}

#[test]
fn test_chapter_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let epub_path = temp_dir.path().join("missing_chapters.epub");
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
    
    // Add OPF file referencing missing chapter files
    zip.start_file("content.opf", FileOptions::<()>::default()).unwrap();
    zip.write_all(br#"<?xml version="1.0" encoding="UTF-8"?>
<package xmlns="http://www.idpf.org/2007/opf" unique-identifier="uuid_id" version="2.0">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>Test Book</dc:title>
    <dc:creator>Test Author</dc:creator>
  </metadata>
  <manifest>
    <item id="chapter1" href="missing_chapter.xhtml" media-type="application/xhtml+xml"/>
  </manifest>
  <spine>
    <itemref idref="chapter1"/>
  </spine>
</package>"#).unwrap();
    
    zip.finish().unwrap();

    let result = EpubReader::new(&epub_path);
    assert!(result.is_ok());
    let epub = result.unwrap();
    assert_eq!(epub.chapter_count(), 0);
}

#[test]
fn test_lazy_loading() {
    let (_temp_dir, epub_path) = create_test_epub();
    let epub = EpubReader::new(&epub_path).expect("Failed to parse test EPUB");

    assert_eq!(epub.chapter_count(), 2);

    let chapter0 = epub.get_chapter(0).expect("Failed to load chapter 0");
    assert_eq!(chapter0.title, "Chapter 1");

    let chapter0_again = epub.get_chapter(0).expect("Failed to load chapter 0 from cache");
    assert_eq!(chapter0_again.title, "Chapter 1");
    assert_eq!(chapter0.content, chapter0_again.content);
}

#[test]
fn test_invalid_chapter_index() {
    let (_temp_dir, epub_path) = create_test_epub();
    let epub = EpubReader::new(&epub_path).expect("Failed to parse test EPUB");

    let result = epub.get_chapter(999);
    assert!(result.is_err());
    match result.unwrap_err() {
        catatau::EpubError::InvalidChapterIndex(idx) => assert_eq!(idx, 999),
        other => panic!("Expected InvalidChapterIndex, got: {:?}", other),
    }
}

#[test]
fn test_chapter_caching() {
    let (_temp_dir, epub_path) = create_test_epub();
    let epub = EpubReader::new(&epub_path).expect("Failed to parse test EPUB");

    for _ in 0..3 {
        let chapter = epub.get_chapter(0).expect("Failed to load chapter");
        assert!(chapter.content.contains("Chapter One"));
    }

    for _ in 0..3 {
        let chapter = epub.get_chapter(1).expect("Failed to load chapter");
        assert!(chapter.content.contains("Chapter Two"));
    }
}