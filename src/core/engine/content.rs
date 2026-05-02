use std::collections::BTreeMap;
use std::sync::Arc;

use crate::core::error::FetchError;
use crate::core::storage::{StorageBackend, StorageError};
use crate::models::VirtualPath;

use super::GlobalFs;

pub type BackendRegistry = BTreeMap<VirtualPath, Arc<dyn StorageBackend>>;

pub async fn read_text(
    fs: &GlobalFs,
    backends: &BackendRegistry,
    path: &VirtualPath,
) -> Result<String, FetchError> {
    if let Some(text) = fs.read_pending_text(path) {
        return Ok(text);
    }

    let (root, backend) = backend_for_path(backends, path)
        .ok_or_else(|| FetchError::NetworkError(format!("no backend for {}", path.as_str())))?;
    let rel_path = relative_backend_path(path, &root).ok_or_else(|| {
        FetchError::NetworkError(format!("path outside backend root: {}", path.as_str()))
    })?;

    backend
        .read_text(&rel_path)
        .await
        .map_err(map_storage_error)
}

pub async fn read_bytes(
    fs: &GlobalFs,
    backends: &BackendRegistry,
    path: &VirtualPath,
) -> Result<Vec<u8>, FetchError> {
    if let Some(text) = fs.read_pending_text(path) {
        return Ok(text.into_bytes());
    }

    let (root, backend) = backend_for_path(backends, path)
        .ok_or_else(|| FetchError::NetworkError(format!("no backend for {}", path.as_str())))?;
    let rel_path = relative_backend_path(path, &root).ok_or_else(|| {
        FetchError::NetworkError(format!("path outside backend root: {}", path.as_str()))
    })?;

    backend
        .read_bytes(&rel_path)
        .await
        .map_err(map_storage_error)
}

fn backend_for_path(
    backends: &BackendRegistry,
    path: &VirtualPath,
) -> Option<(VirtualPath, Arc<dyn StorageBackend>)> {
    backends
        .iter()
        .filter(|(root, _)| path.starts_with(root))
        .max_by_key(|(root, _)| root.as_str().len())
        .map(|(root, backend)| (root.clone(), backend.clone()))
}

fn relative_backend_path(path: &VirtualPath, root: &VirtualPath) -> Option<String> {
    let rel = path.strip_prefix(root)?;
    Some(rel.to_string())
}

fn map_storage_error(error: StorageError) -> FetchError {
    match error {
        StorageError::NotFound(_) => FetchError::HttpError(404),
        StorageError::ValidationFailed(message)
        | StorageError::NetworkError(message)
        | StorageError::BadRequest(message) => FetchError::NetworkError(message),
        StorageError::ServerError(status) => FetchError::HttpError(status),
        StorageError::RateLimited {
            retry_after: Some(seconds),
        } => FetchError::NetworkError(format!("rate limited; retry in {seconds}s")),
        StorageError::RateLimited { retry_after: None } => {
            FetchError::NetworkError("rate limited".to_string())
        }
        StorageError::AuthFailed => FetchError::NetworkError("authentication required".to_string()),
        StorageError::Conflict { remote_head } => {
            FetchError::NetworkError(format!("remote changed while reading (HEAD {remote_head})"))
        }
        StorageError::NoToken => FetchError::NetworkError("authentication required".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::models::{
        EntryExtensions, Fields, NodeKind, NodeMetadata, SCHEMA_VERSION, VirtualPath,
    };

    use super::*;

    struct StubBackend {
        reads: Mutex<Vec<String>>,
        text: String,
    }

    impl StorageBackend for StubBackend {
        fn backend_type(&self) -> &'static str {
            "stub"
        }

        fn scan(
            &self,
        ) -> crate::core::storage::BoxFuture<
            '_,
            crate::core::storage::StorageResult<crate::core::storage::ScannedSubtree>,
        > {
            Box::pin(async { Ok(crate::core::storage::ScannedSubtree::default()) })
        }

        fn read_text<'a>(
            &'a self,
            rel_path: &'a str,
        ) -> crate::core::storage::BoxFuture<'a, crate::core::storage::StorageResult<String>>
        {
            self.reads.lock().unwrap().push(rel_path.to_string());
            let text = self.text.clone();
            Box::pin(async move { Ok(text) })
        }

        fn read_bytes<'a>(
            &'a self,
            rel_path: &'a str,
        ) -> crate::core::storage::BoxFuture<'a, crate::core::storage::StorageResult<Vec<u8>>>
        {
            self.reads.lock().unwrap().push(rel_path.to_string());
            let text = self.text.clone();
            Box::pin(async move { Ok(text.into_bytes()) })
        }

        fn commit<'a>(
            &'a self,
            _request: &'a crate::core::storage::CommitRequest,
        ) -> crate::core::storage::BoxFuture<
            'a,
            crate::core::storage::StorageResult<crate::core::storage::CommitOutcome>,
        > {
            Box::pin(async {
                Err(crate::core::storage::StorageError::BadRequest(
                    "commit unused".to_string(),
                ))
            })
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn pending_content_wins_over_backend_reads() {
        let mut fs = GlobalFs::empty();
        let path = VirtualPath::from_absolute("/.websh/state/env/EDITOR").unwrap();
        fs.upsert_file(
            path.clone(),
            "vim".to_string(),
            NodeMetadata {
                schema: SCHEMA_VERSION,
                kind: NodeKind::Data,
                authored: Fields::default(),
                derived: Fields::default(),
            },
            EntryExtensions::default(),
        );

        let mut backends = BackendRegistry::new();
        backends.insert(
            VirtualPath::from_absolute("/.websh/state").unwrap(),
            Arc::new(StubBackend {
                reads: Mutex::new(Vec::new()),
                text: "nano".to_string(),
            }),
        );

        let text = read_text(&fs, &backends, &path).await.expect("text");
        assert_eq!(text, "vim");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn backend_reads_relative_path_under_mount_root() {
        let mut fs = GlobalFs::empty();
        fs.upsert_binary_placeholder(
            VirtualPath::from_absolute("/blog/post.md").unwrap(),
            NodeMetadata {
                schema: SCHEMA_VERSION,
                kind: NodeKind::Page,
                authored: Fields::default(),
                derived: Fields::default(),
            },
            EntryExtensions::default(),
        );

        let backend = Arc::new(StubBackend {
            reads: Mutex::new(Vec::new()),
            text: "hello".to_string(),
        });
        let mut backends = BackendRegistry::new();
        backends.insert(VirtualPath::root(), backend.clone());

        let text = read_text(
            &fs,
            &backends,
            &VirtualPath::from_absolute("/blog/post.md").unwrap(),
        )
        .await
        .expect("text");

        assert_eq!(text, "hello");
        assert_eq!(backend.reads.lock().unwrap().as_slice(), ["blog/post.md"]);
    }
}
