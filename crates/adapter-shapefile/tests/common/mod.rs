//! Shapefiles built in memory for the tests, and a resource loader that hands them over.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::future::Future;
use std::task::{Context, Poll, Waker};

use forager_adapter_shapefile::ShapefileSource;
use forager_ports::{
    Feature, FeatureQuery, FeatureSource, ResourceLoader, ResourcePath, SourceError,
};
use forager_style_ir::Extent;

pub const NULL: i32 = 0;
pub const POINT: i32 = 1;
pub const ARC: i32 = 3;
pub const POLYGON: i32 = 5;
pub const MULTIPOINT: i32 = 8;

pub fn ready<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the shapefile source never waits"),
    }
}

/// The files of a resource loader, by path.
#[derive(Default)]
pub struct Loader(pub BTreeMap<String, Vec<u8>>);

impl ResourceLoader for Loader {
    async fn load(&self, path: &ResourcePath) -> Result<Vec<u8>, SourceError> {
        self.0
            .get(path.as_str())
            .cloned()
            .ok_or_else(|| SourceError::new(format!("{path} not found")))
    }
}

/// A shapefile as three files.
#[derive(Clone)]
pub struct Files {
    pub shp: Vec<u8>,
    pub shx: Vec<u8>,
    pub dbf: Vec<u8>,
}

impl Files {
    /// The three files of `content`'s records, one after another, in a file of `shape_type` and
    /// of the `.shx` bounds `bounds`.
    pub fn new(shape_type: i32, bounds: [f64; 4], content: &[Vec<u8>]) -> Self {
        let mut shp = header(shape_type, bounds);
        let mut entries = Vec::new();
        for (n, record) in (1i32..).zip(content) {
            entries.push((shp.len(), record.len()));
            shp.extend(n.to_be_bytes());
            shp.extend(words(record.len()).to_be_bytes());
            shp.extend(record);
        }
        let length = words(shp.len());
        shp[24..28].copy_from_slice(&length.to_be_bytes());
        let mut files = Self {
            shp,
            shx: Vec::new(),
            dbf: dbf(),
        };
        files.set_shx(shape_type, bounds, &entries);
        files
    }

    /// Replaces the `.shx` with one of these entries, offsets and sizes in bytes.
    pub fn set_shx(&mut self, shape_type: i32, bounds: [f64; 4], entries: &[(usize, usize)]) {
        let raw: Vec<(i32, i32)> = entries
            .iter()
            .map(|&(offset, size)| (words(offset), words(size)))
            .collect();
        self.set_shx_raw(shape_type, bounds, &raw);
    }

    /// Replaces the `.shx` with entries of these raw words.
    pub fn set_shx_raw(&mut self, shape_type: i32, bounds: [f64; 4], entries: &[(i32, i32)]) {
        let mut shx = header(shape_type, bounds);
        for (offset, size) in entries {
            shx.extend(offset.to_be_bytes());
            shx.extend(size.to_be_bytes());
        }
        let length = words(shx.len());
        shx[24..28].copy_from_slice(&length.to_be_bytes());
        self.shx = shx;
    }

    /// The total size of the three files.
    pub fn size(&self) -> usize {
        self.shp.len() + self.shx.len() + self.dbf.len()
    }

    pub fn loader(&self) -> Loader {
        self.loader_named("data/layer", "shp", "shx", "dbf")
    }

    pub fn loader_named(&self, base: &str, shp: &str, shx: &str, dbf: &str) -> Loader {
        let mut files = BTreeMap::new();
        files.insert(format!("{base}.{shp}"), self.shp.clone());
        files.insert(format!("{base}.{shx}"), self.shx.clone());
        files.insert(format!("{base}.{dbf}"), self.dbf.clone());
        Loader(files)
    }

    pub fn open(&self) -> Result<ShapefileSource, SourceError> {
        open(&self.loader())
    }

    /// The features an extent selects, by record number and geometry.
    pub fn query(&self, extent: [f64; 4]) -> Vec<Feature> {
        let source = self.open().expect("the shapefile opens");
        query(&source, extent)
    }

    /// The record numbers of the features that the whole plane selects.
    pub fn ids(&self) -> Vec<u64> {
        ids(&self.query(EVERYWHERE))
    }
}

pub const EVERYWHERE: [f64; 4] = [-1e9, -1e9, 1e9, 1e9];

pub fn open(loader: &Loader) -> Result<ShapefileSource, SourceError> {
    ready(ShapefileSource::open(
        loader,
        &ResourcePath::new("data/layer.shp").unwrap(),
    ))
}

pub fn query(source: &ShapefileSource, [min_x, min_y, max_x, max_y]: [f64; 4]) -> Vec<Feature> {
    let query = FeatureQuery {
        extent: Extent {
            min_x,
            min_y,
            max_x,
            max_y,
        },
    };
    ready(source.features(&query)).expect("the source delivers")
}

pub fn ids(features: &[Feature]) -> Vec<u64> {
    features.iter().map(|f| f.id).collect()
}

/// A length in bytes as the 16-bit words the headers count.
fn words(bytes: usize) -> i32 {
    i32::try_from(bytes / 2).unwrap()
}

/// A 100-byte `.shp` or `.shx` header; its file length is set once the file is complete.
fn header(shape_type: i32, [min_x, min_y, max_x, max_y]: [f64; 4]) -> Vec<u8> {
    let mut h = Vec::with_capacity(100);
    h.extend(9994i32.to_be_bytes());
    h.extend([0; 20]);
    h.extend(0i32.to_be_bytes());
    h.extend(1000i32.to_le_bytes());
    h.extend(shape_type.to_le_bytes());
    for v in [min_x, min_y, max_x, max_y, 0.0, 0.0, 0.0, 0.0] {
        h.extend(v.to_le_bytes());
    }
    h
}

/// A `.dbf` of one character field ten bytes wide, and no records.
pub fn dbf() -> Vec<u8> {
    dbf_with(&[(b'C', 10, 0)], 11)
}

/// A `.dbf` of these fields — type, and the two size bytes — and this record length.
pub fn dbf_with(fields: &[(u8, u8, u8)], record_length: u16) -> Vec<u8> {
    let header_length = u16::try_from(32 + 32 * fields.len() + 1).unwrap();
    let mut d = vec![0x03, 126, 1, 1, 0, 0, 0, 0];
    d.extend(header_length.to_le_bytes());
    d.extend(record_length.to_le_bytes());
    d.extend([0; 20]);
    for (n, &(kind, size, second)) in fields.iter().enumerate() {
        let mut field = [0u8; 32];
        field[0] = b'A' + u8::try_from(n % 26).unwrap();
        field[11] = kind;
        field[16] = size;
        field[17] = second;
        d.extend(field);
    }
    d.push(0x0d);
    d
}

/// The content of a point record.
pub fn point(x: f64, y: f64) -> Vec<u8> {
    let mut r = POINT.to_le_bytes().to_vec();
    r.extend(x.to_le_bytes());
    r.extend(y.to_le_bytes());
    r
}

/// The content of a null record.
pub fn null() -> Vec<u8> {
    NULL.to_le_bytes().to_vec()
}

/// The box of these points.
pub fn bbox(points: &[(f64, f64)]) -> [f64; 4] {
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for &(x, y) in points {
        b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
    }
    b
}

/// The content of a multipoint record.
pub fn multipoint(points: &[(f64, f64)]) -> Vec<u8> {
    let mut r = MULTIPOINT.to_le_bytes().to_vec();
    for v in bbox(points) {
        r.extend(v.to_le_bytes());
    }
    r.extend(i32::try_from(points.len()).unwrap().to_le_bytes());
    for &(x, y) in points {
        r.extend(x.to_le_bytes());
        r.extend(y.to_le_bytes());
    }
    r
}

/// The content of an arc or polygon record of these parts.
pub fn lines(shape_type: i32, parts: &[&[(f64, f64)]]) -> Vec<u8> {
    let points: Vec<(f64, f64)> = parts.iter().flat_map(|p| p.iter().copied()).collect();
    let mut starts = Vec::new();
    let mut start = 0;
    for part in parts {
        starts.push(start);
        start += i32::try_from(part.len()).unwrap();
    }
    raw_lines(shape_type, bbox(&points), &starts, &points)
}

/// The content of an arc or polygon record with exactly these part starts and points.
pub fn raw_lines(
    shape_type: i32,
    bbox: [f64; 4],
    starts: &[i32],
    points: &[(f64, f64)],
) -> Vec<u8> {
    let mut r = shape_type.to_le_bytes().to_vec();
    for v in bbox {
        r.extend(v.to_le_bytes());
    }
    r.extend(i32::try_from(starts.len()).unwrap().to_le_bytes());
    r.extend(i32::try_from(points.len()).unwrap().to_le_bytes());
    for s in starts {
        r.extend(s.to_le_bytes());
    }
    for &(x, y) in points {
        r.extend(x.to_le_bytes());
        r.extend(y.to_le_bytes());
    }
    r
}
