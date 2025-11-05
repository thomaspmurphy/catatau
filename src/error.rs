use std::fmt;

#[derive(Debug)]
pub enum EpubError {
    Io(std::io::Error),
    Zip(zip::result::ZipError),
    Xml(quick_xml::Error),
    Utf8(std::str::Utf8Error),
    ContainerNotFound,
    OpfNotFound,
    InvalidOpfStructure,
    ChapterNotFound(String),
    FileTooLarge { size: u64, max: u64 },
    ChapterTooLarge { size: usize, max: usize },
    DecompressionBomb { compressed: u64, decompressed: u64, ratio: usize },
    InvalidChapterIndex(usize),
    CacheLockError,
}

impl fmt::Display for EpubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EpubError::Io(err) => write!(f, "IO error: {}", err),
            EpubError::Zip(err) => write!(f, "ZIP error: {}", err),
            EpubError::Xml(err) => write!(f, "XML parsing error: {}", err),
            EpubError::Utf8(err) => write!(f, "UTF-8 encoding error: {}", err),
            EpubError::ContainerNotFound => write!(f, "container.xml not found in EPUB"),
            EpubError::OpfNotFound => write!(f, "OPF file not found"),
            EpubError::InvalidOpfStructure => write!(f, "Invalid OPF file structure"),
            EpubError::ChapterNotFound(path) => write!(f, "Chapter file not found: {}", path),
            EpubError::FileTooLarge { size, max } => {
                write!(f, "EPUB file too large: {} bytes (max: {} bytes)", size, max)
            }
            EpubError::ChapterTooLarge { size, max } => {
                write!(f, "Chapter too large: {} bytes (max: {} bytes)", size, max)
            }
            EpubError::DecompressionBomb { compressed, decompressed, ratio } => {
                write!(
                    f,
                    "Potential decompression bomb detected: {}x ratio (compressed: {}, decompressed: {})",
                    ratio, compressed, decompressed
                )
            }
            EpubError::InvalidChapterIndex(idx) => {
                write!(f, "Invalid chapter index: {}", idx)
            }
            EpubError::CacheLockError => {
                write!(f, "Failed to acquire cache lock")
            }
        }
    }
}

impl std::error::Error for EpubError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EpubError::Io(err) => Some(err),
            EpubError::Zip(err) => Some(err),
            EpubError::Xml(err) => Some(err),
            EpubError::Utf8(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for EpubError {
    fn from(err: std::io::Error) -> Self {
        EpubError::Io(err)
    }
}

impl From<zip::result::ZipError> for EpubError {
    fn from(err: zip::result::ZipError) -> Self {
        EpubError::Zip(err)
    }
}

impl From<quick_xml::Error> for EpubError {
    fn from(err: quick_xml::Error) -> Self {
        EpubError::Xml(err)
    }
}

impl From<std::str::Utf8Error> for EpubError {
    fn from(err: std::str::Utf8Error) -> Self {
        EpubError::Utf8(err)
    }
}

impl From<std::string::FromUtf8Error> for EpubError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        EpubError::Utf8(err.utf8_error())
    }
}

impl From<quick_xml::events::attributes::AttrError> for EpubError {
    fn from(err: quick_xml::events::attributes::AttrError) -> Self {
        EpubError::Xml(quick_xml::Error::InvalidAttr(err))
    }
}

#[derive(Debug)]
pub enum UiError {
    Terminal(Box<dyn std::error::Error + Send + Sync>),
    Epub(EpubError),
}

impl fmt::Display for UiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UiError::Terminal(err) => write!(f, "Terminal error: {}", err),
            UiError::Epub(err) => write!(f, "EPUB error: {}", err),
        }
    }
}

impl std::error::Error for UiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            UiError::Terminal(err) => Some(err.as_ref()),
            UiError::Epub(err) => Some(err),
        }
    }
}

impl From<EpubError> for UiError {
    fn from(err: EpubError) -> Self {
        UiError::Epub(err)
    }
}

impl From<std::io::Error> for UiError {
    fn from(err: std::io::Error) -> Self {
        UiError::Terminal(Box::new(err))
    }
}

