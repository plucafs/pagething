use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};

/// Extract just the title from an EPUB's metadata (no chapters or images).
pub fn extract_title(path: &Path) -> Option<String> {
    let epub = rbook::Epub::open(path).ok()?;
    epub.metadata().title().map(|t| t.value().to_string())
}

enum Message {
    Done {
        path: PathBuf,
        title: Option<String>,
    },
}

/// Loads recent-book titles on a background thread so the UI never blocks.
pub struct TitleLoader {
    request_tx: Sender<PathBuf>,
    rx: Receiver<Message>,
    titles: HashMap<PathBuf, Option<String>>,
    queued: HashSet<PathBuf>,
}

impl TitleLoader {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<PathBuf>();
        let (tx, rx) = mpsc::channel::<Message>();
        std::thread::spawn(move || {
            while let Ok(path) = request_rx.recv() {
                let title = extract_title(&path);
                if tx.send(Message::Done { path, title }).is_err() {
                    break;
                }
            }
        });
        Self {
            request_tx,
            rx,
            titles: HashMap::new(),
            queued: HashSet::new(),
        }
    }

    /// Queue any paths whose title has not been loaded yet.
    pub fn ensure(&mut self, paths: &[PathBuf]) {
        for path in paths {
            if !self.titles.contains_key(path) && self.queued.insert(path.clone()) {
                let _ = self.request_tx.send(path.clone());
            }
        }
    }

    /// Drain finished extractions. Non-blocking.
    pub fn poll(&mut self) {
        while let Ok(Message::Done { path, title }) = self.rx.try_recv() {
            self.queued.remove(&path);
            self.titles.insert(path, title);
        }
    }

    /// The loaded title, or `None` while pending or if extraction failed.
    pub fn title(&self, path: &Path) -> Option<&str> {
        self.titles.get(path).and_then(|t| t.as_deref())
    }

    /// Number of extractions still in flight.
    pub fn pending(&self) -> usize {
        self.queued.len()
    }
}

impl Default for TitleLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn pg1342() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/books/pg1342.epub")
    }

    #[test]
    fn extracts_title_from_pg1342() {
        assert_eq!(
            extract_title(&pg1342()).as_deref(),
            Some("Pride and Prejudice")
        );
    }

    #[test]
    fn missing_file_has_no_title() {
        assert_eq!(extract_title(Path::new("/nonexistent/nope.epub")), None);
    }

    #[test]
    fn loader_resolves_queued_titles() {
        let path = pg1342();
        let mut loader = TitleLoader::new();
        loader.ensure(std::slice::from_ref(&path));
        loader.ensure(std::slice::from_ref(&path));
        let deadline = Instant::now() + Duration::from_secs(5);
        while loader.pending() > 0 && Instant::now() < deadline {
            loader.poll();
            std::thread::sleep(Duration::from_millis(10));
        }
        loader.poll();
        assert_eq!(loader.pending(), 0);
        assert_eq!(loader.title(&path), Some("Pride and Prejudice"));
    }
}
