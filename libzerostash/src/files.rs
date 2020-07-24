use crate::chunks::ChunkPointer;
use crate::meta::{FieldReader, FieldWriter, MetaObjectField};

use dashmap::DashMap;

use std::error::Error;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

type DashSet<T> = DashMap<T, ()>;

#[derive(Hash, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub unix_secs: u64,
    pub unix_nanos: u32,
    pub unix_perm: u32,
    pub unix_uid: u32,
    pub unix_gid: u32,

    pub size: u64,
    pub readonly: bool,
    pub name: String,

    pub chunks: Vec<(u64, Arc<ChunkPointer>)>,
}

impl Entry {
    #[cfg(windows)]
    pub fn from_file(file: &fs::File, path: impl AsRef<Path>) -> Result<Entry, Box<dyn Error>> {
        let path = path.as_ref();
        let metadata = file.metadata()?;
        let (unix_secs, unix_nanos) = to_unix_mtime(&metadata)?;

        Ok(File {
            unix_secs,
            unix_nanos,
            unix_perm: 0,
            unix_uid: 0,
            unix_gid: 0,

            size: metadata.len(),
            readonly: metadata.permissions().readonly(),
            name: path.as_ref().to_str().unwrap().to_string(),

            chunks: Vec::new(),
        })
    }

    #[cfg(unix)]
    pub fn from_file(file: &fs::File, path: impl AsRef<Path>) -> Result<Entry, Box<dyn Error>> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let metadata = file.metadata()?;
        let perms = metadata.permissions();
        let (unix_secs, unix_nanos) = to_unix_mtime(&metadata)?;

        Ok(Entry {
            unix_secs,
            unix_nanos,
            unix_perm: perms.mode(),
            unix_uid: metadata.uid(),
            unix_gid: metadata.gid(),

            size: metadata.len(),
            readonly: metadata.permissions().readonly(),
            name: path.as_ref().to_str().unwrap().to_string(),

            chunks: Vec::new(),
        })
    }
}

fn to_unix_mtime(m: &fs::Metadata) -> Result<(u64, u32), Box<dyn Error>> {
    let mtime = m.modified()?.duration_since(UNIX_EPOCH)?;
    Ok((mtime.as_secs(), mtime.subsec_nanos()))
}

pub type FileIndex = DashSet<Arc<Entry>>;

#[derive(Clone, Default)]
pub struct FileStore(Arc<FileIndex>);

impl FileStore {
    pub fn index(&self) -> &FileIndex {
        &self.0
    }

    pub fn has_changed(&self, file: &Entry) -> bool {
        !self.0.contains_key(file)
    }

    pub fn push(&mut self, file: Entry) {
        self.0.insert(Arc::new(file), ());
    }
}

impl MetaObjectField for FileStore {
    type Item = Entry;

    fn key() -> String {
        "files".to_string()
    }

    fn serialize(&self, mw: &mut impl FieldWriter) {
        for f in self.0.iter() {
            mw.write_next(f.key());
        }
    }

    fn deserialize(&self, mw: &mut impl FieldReader<Self::Item>) {
        while let Ok(file) = mw.read_next() {
            self.0.insert(Arc::new(file), ());
        }
    }
}
