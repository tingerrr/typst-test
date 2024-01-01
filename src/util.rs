pub mod fs {
    use std::fs::DirEntry;
    use std::path::Path;
    use std::{fs, io};

    pub fn collect_dir_entries(path: &Path) -> io::Result<Vec<DirEntry>> {
        fs::read_dir(path)?.collect::<io::Result<Vec<DirEntry>>>()
    }
}
