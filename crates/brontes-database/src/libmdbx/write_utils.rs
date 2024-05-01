use brontes_types::db::traits::DBWriter;

use super::LibmdbxReadWriter;

///
pub struct LibmdbxBatchWriter {
    db: &'static LibmdbxReadWriter,
}
