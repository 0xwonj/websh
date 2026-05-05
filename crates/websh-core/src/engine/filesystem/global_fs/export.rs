use crate::domain::{FsEntry, VirtualPath, runtime_state_root};
use crate::ports::{ScannedDirectory, ScannedSubtree};

use super::super::snapshot::{
    collect_scanned_directories, collect_scanned_files, has_manifest_metadata,
};
use super::GlobalFs;

impl GlobalFs {
    pub fn export_mount_snapshot(&self, mount_root: &VirtualPath) -> Option<ScannedSubtree> {
        let FsEntry::Directory { children, meta } = self.get_entry(mount_root)? else {
            return None;
        };
        let excluded_roots = self.export_excluded_roots(mount_root);

        let mut files = Vec::new();
        collect_scanned_files(mount_root, "", children, &excluded_roots, &mut files);
        files.sort_by(|a, b| a.path.cmp(&b.path));

        let mut directories = Vec::new();
        if has_manifest_metadata("", meta) {
            directories.push(ScannedDirectory {
                path: String::new(),
                meta: meta.clone(),
            });
        }
        collect_scanned_directories(mount_root, "", children, &excluded_roots, &mut directories);
        directories.sort_by(|a, b| a.path.cmp(&b.path));

        Some(ScannedSubtree { files, directories })
    }

    fn export_excluded_roots(&self, mount_root: &VirtualPath) -> Vec<VirtualPath> {
        let synthetic_state = runtime_state_root().clone();
        self.mount_points
            .iter()
            .filter(|path| *path != mount_root && path.starts_with(mount_root))
            .cloned()
            .chain(
                synthetic_state
                    .starts_with(mount_root)
                    .then_some(synthetic_state),
            )
            .collect()
    }
}
