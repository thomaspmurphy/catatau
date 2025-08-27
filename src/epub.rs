use crate::{
    constants::{
        HTML_TEXT_WIDTH, MIN_CONTENT_LENGTH, SEARCH_CONTEXT_AFTER_LINES, SEARCH_CONTEXT_LINES,
    },
    error::EpubError,
};
use quick_xml::{Reader, events::Event};
use std::{collections::HashMap, fs::File, io::Read, path::Path};
use zip::ZipArchive;

#[derive(Debug, Clone)]
pub struct Chapter {
    pub content: String,
    pub id: String,
    pub title: String,
}

impl std::ops::Deref for Chapter {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.id
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SearchResult {
    pub chapter_index: usize,
    pub context: String,
    pub line_number: usize,
    pub position: usize,
}

#[derive(Debug)]
struct OpfData {
    metadata: HashMap<String, String>,
    spine: Vec<String>,
    opf_path: String,
}

#[derive(Debug)]
pub struct EpubReader {
    pub chapters: Vec<Chapter>,
    pub title: String,
    pub author: String,
}

impl EpubReader {
    pub fn new(path: &Path) -> Result<Self, EpubError> {
        let file = File::open(path)?;
        let mut archive = ZipArchive::new(file)?;

        // Find and parse container.xml to get the OPF file path
        let opf_path = Self::find_opf_path(&mut archive)?;

        // Parse the OPF file to get metadata, spine, and opf path
        let opf_data = Self::parse_opf(&mut archive, &opf_path)?;

        // Extract chapter content
        let chapters = Self::extract_chapters(&mut archive, opf_data.spine, &opf_data.opf_path)?;

        Ok(EpubReader {
            chapters,
            title: opf_data
                .metadata
                .get("title")
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string()),
            author: opf_data
                .metadata
                .get("creator")
                .cloned()
                .unwrap_or_else(|| "Unknown".to_string()),
        })
    }

    fn find_opf_path(archive: &mut ZipArchive<File>) -> Result<String, EpubError> {
        let mut container_file = match archive.by_name("META-INF/container.xml") {
            Ok(file) => file,
            Err(zip::result::ZipError::FileNotFound) => return Err(EpubError::ContainerNotFound),
            Err(e) => return Err(EpubError::Zip(e)),
        };

        let mut container_content = String::new();
        container_file.read_to_string(&mut container_content)?;

        let mut reader = Reader::from_str(&container_content);
        reader.config_mut().trim_text(true);

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(e) | Event::Empty(e) if e.name().as_ref() == b"rootfile" => {
                    for attr in e.attributes() {
                        let attr = attr?;
                        if attr.key.as_ref() == b"full-path" {
                            return Ok(String::from_utf8(attr.value.to_vec())?);
                        }
                    }
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Err(EpubError::OpfNotFound)
    }

    fn parse_opf(archive: &mut ZipArchive<File>, opf_path: &str) -> Result<OpfData, EpubError> {
        let mut opf_file = match archive.by_name(opf_path) {
            Ok(file) => file,
            Err(zip::result::ZipError::FileNotFound) => return Err(EpubError::OpfNotFound),
            Err(e) => return Err(EpubError::Zip(e)),
        };
        let mut opf_content = String::new();
        opf_file.read_to_string(&mut opf_content)?;

        let mut reader = Reader::from_str(&opf_content);
        reader.config_mut().trim_text(true);

        let mut metadata = HashMap::new();
        let mut manifest = HashMap::new();
        let mut spine = Vec::new();
        let mut buf = Vec::new();
        let mut current_section = String::new();

        loop {
            match reader.read_event_into(&mut buf)? {
                Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                    b"metadata" => {
                        current_section = "metadata".to_string();
                    }
                    b"manifest" => {
                        current_section = "manifest".to_string();
                    }
                    b"spine" => {
                        current_section = "spine".to_string();
                    }
                    b"item" if current_section == "manifest" => {
                        let mut id = String::new();
                        let mut href = String::new();
                        for attr in e.attributes() {
                            let attr = attr?;
                            match attr.key.as_ref() {
                                b"id" => id = String::from_utf8(attr.value.to_vec())?,
                                b"href" => href = String::from_utf8(attr.value.to_vec())?,
                                _ => {}
                            }
                        }
                        if !id.is_empty() && !href.is_empty() {
                            manifest.insert(id, href);
                        }
                    }
                    b"itemref" if current_section == "spine" => {
                        for attr in e.attributes() {
                            let attr = attr?;
                            if attr.key.as_ref() == b"idref" {
                                let idref = String::from_utf8(attr.value.to_vec())?;
                                if let Some(href) = manifest.get(&idref) {
                                    spine.push(href.clone());
                                }
                            }
                        }
                    }
                    b"dc:title" if current_section == "metadata" => {
                        if let Ok(Event::Text(text)) = reader.read_event_into(&mut buf) {
                            metadata.insert("title".to_string(), text.unescape()?.to_string());
                        }
                    }
                    b"dc:creator" if current_section == "metadata" => {
                        if let Ok(Event::Text(text)) = reader.read_event_into(&mut buf) {
                            metadata.insert("creator".to_string(), text.unescape()?.to_string());
                        }
                    }
                    _ => {}
                },
                Event::End(e) => match e.name().as_ref() {
                    b"metadata" | b"manifest" | b"spine" => {
                        current_section.clear();
                    }
                    _ => {}
                },
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        // Validate that we have the bare essentials of a valid EPUB
        if spine.is_empty() {
            return Err(EpubError::InvalidOpfStructure);
        }

        Ok(OpfData {
            metadata,
            spine,
            opf_path: opf_path.to_string(),
        })
    }

    fn extract_chapters(
        archive: &mut ZipArchive<File>,
        spine: Vec<String>,
        opf_path: &str,
    ) -> Result<Vec<Chapter>, EpubError> {
        let mut chapters = Vec::new();

        for href in spine.iter() {
            let content = match Self::resolve_and_read_file(archive, href, opf_path) {
                Ok(content) => content,
                Err(_) => {
                    eprintln!("Warning: Could not find file: {}", href);
                    continue;
                }
            };

            // Convert HTML to plain text
            let text_content = html2text::from_read(content.as_bytes(), HTML_TEXT_WIDTH);

            // Skip if content is too short (likely not actual content)
            if text_content.trim().len() < MIN_CONTENT_LENGTH {
                continue;
            }

            // Extract chapter title from content or HTML
            let chapter_title =
                Self::extract_chapter_title(&content, &text_content, chapters.len() + 1);

            chapters.push(Chapter {
                title: chapter_title,
                content: text_content,
                id: href.clone(),
            });
        }

        Ok(chapters)
    }

    fn resolve_and_read_file(
        archive: &mut ZipArchive<File>,
        href: &str,
        opf_path: &str,
    ) -> Result<String, EpubError> {
        // Get the OPF directory as the base path
        let opf_dir = Path::new(opf_path).parent().unwrap_or(Path::new(""));

        // Resolve href relative to OPF directory
        let resolved_path = opf_dir.join(href);
        let resolved_path_str = resolved_path.to_string_lossy();

        // Try the properly resolved path first
        if let Ok(mut file) = archive.by_name(&resolved_path_str) {
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            return Ok(content);
        }

        // Fallback for malformed EPUBs: try the original href as-is
        if let Ok(mut file) = archive.by_name(href) {
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            return Ok(content);
        }

        // Final fallback: try possible EPUB directory structures
        let fallback_paths = Self::generate_fallback_paths(href);
        for path in fallback_paths {
            if let Ok(mut file) = archive.by_name(&path) {
                let mut content = String::new();
                file.read_to_string(&mut content)?;
                return Ok(content);
            }
        }

        Err(EpubError::ChapterNotFound(href.to_string()))
    }

    fn generate_fallback_paths(href: &str) -> Vec<String> {
        // Fallback paths for malformed EPUBs that don't follow spec (many exist)
        vec![
            format!("OEBPS/{}", href),
            format!("OPS/{}", href),
            format!("Text/{}", href),
            format!("EPUB/{}", href),
            format!("content/{}", href),
        ]
    }

    fn extract_chapter_title(
        html_content: &str,
        text_content: &str,
        fallback_number: usize,
    ) -> String {
        // Try to extract title from HTML tags first
        if let Some(title) = Self::extract_title_from_html(html_content) {
            return title;
        }

        // Try to extract title from the first line of text content
        if let Some(title) = Self::extract_title_from_text(text_content) {
            return title;
        }

        // Fallback to generic chapter numbering
        format!("Chapter {}", fallback_number)
    }

    fn extract_title_from_html(html_content: &str) -> Option<String> {
        // Look for title in common HTML tags
        // TODO: Seems like this could be improved with a more robust HTML parser.
        let title_patterns = [
            r"<title[^>]*>([^<]+)</title>",
            r"<h1[^>]*>([^<]+)</h1>",
            r"<h2[^>]*>([^<]+)</h2>",
        ];

        for pattern in &title_patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(captures) = re.captures(html_content) {
                    if let Some(title) = captures.get(1) {
                        let title_text = title.as_str().trim();
                        if !title_text.is_empty() && title_text.len() < 100 {
                            return Some(
                                html2text::from_read(title_text.as_bytes(), 200)
                                    .trim()
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_title_from_text(text_content: &str) -> Option<String> {
        // Try to find a title-like first line
        let first_line = text_content.lines().next()?.trim();

        // Check if the first line looks like a title
        if !first_line.is_empty()
            && first_line.len() < 100
            && first_line.len() > 3
            && !first_line.ends_with('.')
            && first_line.chars().any(|c| c.is_alphabetic())
        {
            Some(first_line.to_string())
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();
        let query_lower = query.to_lowercase();

        for (chapter_index, chapter) in self.chapters.iter().enumerate() {
            let lines: Vec<&str> = chapter.content.lines().collect();

            for (line_index, line) in lines.iter().enumerate() {
                let line_lower = line.to_lowercase();
                if line_lower.contains(&query_lower) {
                    // Calculate the position within the chapter
                    let position: usize = lines[..line_index]
                        .iter()
                        .map(|l| l.len() + 1) // +1 for newline
                        .sum();

                    // Create context around the match
                    let start = line_index.saturating_sub(SEARCH_CONTEXT_LINES);
                    let end = std::cmp::min(line_index + SEARCH_CONTEXT_AFTER_LINES, lines.len());
                    let context_lines = &lines[start..end];
                    let context = context_lines.join("\n");

                    results.push(SearchResult {
                        chapter_index,
                        line_number: line_index,
                        context: context.to_string(),
                        position,
                    });
                }
            }
        }

        results
    }

    #[allow(dead_code)]
    pub fn get_chapter_line_count(&self, chapter_index: usize) -> usize {
        if let Some(chapter) = self.chapters.get(chapter_index) {
            chapter.content.lines().count()
        } else {
            0
        }
    }
}
