// The reading of the `.shp` and `.shx` follows MapServer's src/mapshape.c of the release pinned in
// scripts/fetch-msautotest.sh, function by function as the comments name them.
// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// including without limitation the rights to use, copy, modify, merge, publish, distribute,
// sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions: The above copyright notice and this
// permission notice shall be included in all copies of this Software or works derived from this
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE
// AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

//! The ESRI shapefile feature source (ADR-0024): a `.shp` with its `.shx` and `.dbf`, read from
//! the bytes the resource loader hands over, with the records MapServer reads and drops, and in
//! memory that follows the files' size. `docs/semantics/shapefile.md` states each rule.
//!
//! Nothing is allocated from a count or a size the files state: a record is a view into the
//! `.shp`'s bytes, checked before its geometry is built, and the index is read where it lies.

mod dbf;
mod record;

use std::rc::Rc;

use forager_ports::{
    Feature, FeatureQuery, FeatureSource, Properties, ResourceLoader, ResourcePath, SourceError,
};
use forager_style_ir::Extent;

use dbf::Dbf;
use record::{Rect, Shape};

/// The length of the `.shp` and `.shx` headers.
const HEADER: usize = 100;
/// The entries of the `.shx` that `msSHXLoadPage` reads at once, and loses together.
const SHX_PAGE: usize = 1024;
/// The most records `msSHPOpenVirtualFile` accepts.
const MAX_RECORDS: i32 = 256_000_000;

/// The shapes of one shapefile, numbered by their record in the `.shx`.
pub struct ShapefileSource {
    shp: Vec<u8>,
    shx: Vec<u8>,
    /// The file's shape type: one byte of the `.shx` header.
    shape_type: u8,
    /// The entries of the `.shx` that can be read: the records its header states, less those on
    /// a page the file cuts short.
    entries: usize,
    /// The file's bounds, from the `.shx` header.
    bounds: Rect,
    /// The `.dbf`, whose records the features share as their properties (ADR-0026).
    dbf: Rc<Dbf>,
}

/// Names the file's shape, records, and bounds, never its bytes, so that a logged source does not
/// print whole files.
impl std::fmt::Debug for ShapefileSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShapefileSource")
            .field("shape_type", &self.shape_type)
            .field("entries", &self.entries)
            .field("shp_bytes", &self.shp.len())
            .field("bounds", &self.bounds)
            .field("dbf", &self.dbf)
            .finish_non_exhaustive()
    }
}

impl ShapefileSource {
    /// Opens the shapefile `data` names, as `msShapefileOpen` does: any extension of the name is
    /// replaced by `.shp`, `.shx`, and `.dbf`, each tried in upper case where the lower-case name
    /// cannot be loaded.
    pub async fn open(
        loader: &impl ResourceLoader,
        data: &ResourcePath,
    ) -> Result<Self, SourceError> {
        let base = basename(data.as_str());
        let shp = load(loader, base, "shp", "SHP").await?;
        let shx = load(loader, base, "shx", "SHX").await?;
        let (shape_type, entries, bounds) = Self::headers(&shp, &shx)?;
        let dbf = Rc::new(Dbf::open(load(loader, base, "dbf", "DBF").await?)?);
        Ok(Self {
            shp,
            shx,
            shape_type,
            entries,
            bounds,
            dbf,
        })
    }

    /// Reads the headers of the `.shp` and the `.shx` (`msSHPOpenVirtualFile`): the shape type,
    /// the entries that can be read, and the bounds.
    fn headers(shp: &[u8], shx: &[u8]) -> Result<(u8, usize, Rect), SourceError> {
        // Only the length of the .shp header is checked; its file code, its file length, and its
        // shape type are not read.
        if shp.len() < HEADER {
            return Err(SourceError::new(
                "the .shp is shorter than its 100-byte header",
            ));
        }
        let header = shx
            .get(..HEADER)
            .ok_or_else(|| SourceError::new("the .shx is shorter than its 100-byte header"))?;
        if header[..3] != [0, 0, 0x27] || !matches!(header[3], 0x0a | 0x0d) {
            return Err(SourceError::new("the .shx has no shapefile file code"));
        }
        // The file length, in 16-bit words, decides the record count.
        let half = record::be_i32(header, 24).unwrap_or(-1);
        let records = if half >= 50 { (half - 50) / 4 } else { -1 };
        if !(0..=MAX_RECORDS).contains(&records) {
            return Err(SourceError::new(format!(
                "the .shx header states a file length of {half} words"
            )));
        }
        let records = usize::try_from(records).unwrap_or(0);
        let bounds = Rect::read(header, 36).unwrap_or(Rect::EMPTY);
        let entries = readable_entries(shx.len(), records);
        Ok((header[32], entries, bounds))
    }

    /// The offset and the size in bytes of entry `i` of the `.shx`, each 0 where the entry holds
    /// none that doubles into a positive `int` (`msSHXLoadPage`).
    fn entry(&self, i: usize) -> (u64, u64) {
        let at = i.checked_mul(8).and_then(|b| b.checked_add(HEADER));
        let word = |delta: usize| {
            at.and_then(|at| at.checked_add(delta))
                .and_then(|at| record::be_i32(&self.shx, at))
                .filter(|&raw| raw > 0 && raw < i32::MAX / 2)
                .map_or(0, |raw| u64::from(raw.unsigned_abs()) * 2)
        };
        (word(0), word(4))
    }

    /// Calls `f` with each record the extent selects and its shape, in record order, the records
    /// read as no shape skipped (`msShapefileWhichShapes`, `msSHPLayerNextShape`).
    fn each_shape<'a>(&'a self, extent: &Rect, mut f: impl FnMut(usize, Shape<'a>)) {
        // The file's bounds outside the extent select none; inside it, all; across its edge,
        // each record by its own box.
        if !self.bounds.overlaps(extent) {
            return;
        }
        let by_box = !self.bounds.contained_in(extent);
        // The records read from the .shp add up to no more bytes than it holds (ADR-0024).
        let mut budget = self.shp.len() as u64;
        for i in 0..self.entries {
            if by_box && !self.box_overlaps(i, extent) {
                continue;
            }
            if let Some(shape) = self.shape(i, &mut budget) {
                f(i, shape);
            }
        }
    }

    /// Whether record `i`'s stored box overlaps the extent (`msSHPReadBounds`). A record whose
    /// box cannot be read is no shape: MapServer skips it where the `.shx` size is 4, and fails
    /// the layer otherwise, where the adapter reads on (ADR-0024).
    fn box_overlaps(&self, i: usize, extent: &Rect) -> bool {
        let (offset, size) = self.entry(i);
        if size <= 4 || offset == 0 || offset >= (i32::MAX - 12) as u64 {
            return false;
        }
        let Ok(at) = usize::try_from(offset + 12) else {
            return false;
        };
        let stored = if record::is_point(self.shape_type) {
            record::coord(&self.shp, at).map(|c| Rect {
                min_x: c.x,
                min_y: c.y,
                max_x: c.x,
                max_y: c.y,
            })
        } else {
            Rect::read(&self.shp, at).filter(|r| !r.min_x.is_nan())
        };
        stored.is_some_and(|r| r.overlaps(extent))
    }

    /// Record `i` as `msSHPReadShape` reads it, or `None` for no shape.
    fn shape(&self, i: usize, budget: &mut u64) -> Option<Shape<'_>> {
        let (offset, size) = self.entry(i);
        // A size of 4 is a null shape; under 4, a broken one.
        if size <= 4 || offset == 0 {
            return None;
        }
        // The record's 8-byte header is read with it, though nothing of it is used.
        let length = size + 8;
        let start = usize::try_from(offset).ok()?;
        let end = usize::try_from(offset.checked_add(length)?).ok()?;
        let bytes = self.shp.get(start..end)?;
        *budget = budget.checked_sub(length)?;
        record::parse(self.shape_type, bytes)
    }
}

impl FeatureSource for ShapefileSource {
    async fn features(&self, query: &FeatureQuery) -> Result<Vec<Feature>, SourceError> {
        let extent = Rect::from(&query.extent);
        // Counted first, so that the list takes no more memory than its features.
        let mut count = 0;
        self.each_shape(&extent, |_, _| count += 1);
        let mut features = Vec::with_capacity(count);
        self.each_shape(&extent, |i, shape| {
            // A shape past the .dbf's records carries no properties, as MapServer's carries no
            // values there (msSHPLayerNextShape).
            let record = i as u64;
            let properties = if self.dbf.has(record) {
                Properties::record(self.dbf.clone(), record)
            } else {
                Properties::default()
            };
            features.push(Feature {
                id: record,
                geometry: shape.geometry(),
                properties,
            });
        });
        Ok(features)
    }
}

impl From<&Extent> for Rect {
    fn from(e: &Extent) -> Self {
        Rect {
            min_x: e.min_x,
            min_y: e.min_y,
            max_x: e.max_x,
            max_y: e.max_y,
        }
    }
}

/// The entries of a `.shx` of `len` bytes that `msSHXLoadPage` reads, of the `records` its
/// header states: a page of 1024 entries that the file cuts short is lost as a whole, and so is
/// every page after it.
fn readable_entries(len: usize, records: usize) -> usize {
    let present = len.saturating_sub(HEADER) / 8;
    if present >= records {
        records
    } else {
        present / SHX_PAGE * SHX_PAGE
    }
}

/// The name without its extension, as `msSHPOpen` cuts it: from the last `.` after the last
/// separator, a `.` that starts the name included. [`ShapefileSource::open`] asks for the files
/// of this name, so a host that looks for them first, or that opens one shapefile once for every
/// layer naming it, cuts the name here.
pub fn basename(name: &str) -> &str {
    let bytes = name.as_bytes();
    let mut i = bytes.len().saturating_sub(1);
    while i > 0 && !matches!(bytes[i], b'.' | b'/' | b'\\') {
        i -= 1;
    }
    if bytes.get(i) == Some(&b'.') {
        &name[..i]
    } else {
        name
    }
}

/// Loads `base.lower`, or `base.upper` where that cannot be loaded.
async fn load(
    loader: &impl ResourceLoader,
    base: &str,
    lower: &str,
    upper: &str,
) -> Result<Vec<u8>, SourceError> {
    let path = |ext: &str| {
        ResourcePath::new(&format!("{base}.{ext}")).map_err(|e| {
            SourceError::new(format!("the shapefile {base}.{ext} cannot be named: {e}"))
        })
    };
    let first = path(lower)?;
    match loader.load(&first).await {
        Ok(bytes) => Ok(bytes),
        Err(error) => loader.load(&path(upper)?).await.map_err(|_| {
            SourceError::new(format!(
                "the shapefile file {first} cannot be read: {error}"
            ))
        }),
    }
}
