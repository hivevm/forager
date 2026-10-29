// The `.dbf` header is checked as `msDBFOpenVirtualFile` of MapServer's src/mapxbase.c of the
// release pinned in scripts/fetch-msautotest.sh checks it.
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

//! The `.dbf` beside a shapefile: its header is checked when the shapefile opens, and its records
//! are read as the features' properties where a property is read (ADR-0026).

use std::borrow::Cow;

use forager_ports::{SourceError, Table, Value};

/// A field of the `.dbf` header: where its name lies in the file, its type, and where its value
/// lies in a record. Nothing of its name or value is copied.
#[derive(Debug)]
struct Field {
    descriptor: u16,
    kind: u8,
    offset: u32,
    size: u32,
}

/// The `.dbf` of a shapefile with its fields, which every feature of the layer shares. A feature
/// names its record; its values are read from the file when they are read.
pub(crate) struct Dbf {
    bytes: Vec<u8>,
    fields: Vec<Field>,
    header_length: usize,
    record_length: usize,
    records: u64,
}

/// Names the fields and records, never the file's bytes.
impl std::fmt::Debug for Dbf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Dbf")
            .field("fields", &self.fields.len())
            .field("records", &self.records)
            .finish_non_exhaustive()
    }
}

impl Dbf {
    /// Reads the header of a `.dbf` as `msDBFOpenVirtualFile` does. Of the 32-byte table header
    /// only the record count, the header length, and the record length are read: neither the
    /// version, the update date, nor the language driver.
    pub(crate) fn open(bytes: Vec<u8>) -> Result<Self, SourceError> {
        let header = bytes
            .get(..32)
            .ok_or_else(|| SourceError::new("the .dbf is shorter than its 32-byte header"))?;
        // A count whose last byte is 128 or more is read as no records.
        let records = if header[7] < 128 {
            u64::from(u32::from_le_bytes([
                header[4], header[5], header[6], header[7],
            ]))
        } else {
            0
        };
        let header_length = usize::from(u16::from_le_bytes([header[8], header[9]]));
        let record_length = usize::from(u16::from_le_bytes([header[10], header[11]]));
        if header_length <= 32 {
            return Err(SourceError::new(format!(
                "the .dbf header length {header_length} leaves no room for its fields"
            )));
        }
        let descriptors = bytes.get(32..header_length).ok_or_else(|| {
            SourceError::new(format!(
                "the .dbf is shorter than its header length {header_length}"
            ))
        })?;
        // One field per whole 32 bytes past the table header; the record starts with its deletion
        // flag. An N or F field takes one byte of size and one of decimals, any other two of size.
        let (chunks, _) = descriptors.as_chunks::<32>();
        let mut fields = Vec::with_capacity(chunks.len());
        // At most 2046 fields of at most 65535 bytes each: the sum fits a u32.
        let mut end = 1u32;
        for (descriptor, field) in (32u16..).step_by(32).zip(chunks) {
            let size = match field[11] {
                b'N' | b'F' => u32::from(field[16]),
                _ => u32::from(u16::from_le_bytes([field[16], field[17]])),
            };
            fields.push(Field {
                descriptor,
                kind: field[11],
                offset: end,
                size,
            });
            end += size;
        }
        if !fields.is_empty() && end as usize > record_length {
            return Err(SourceError::new(format!(
                "the .dbf fields take {end} bytes of a record of {record_length}"
            )));
        }
        Ok(Self {
            bytes,
            fields,
            header_length,
            record_length,
            records,
        })
    }

    /// Whether record `record` can be read: within the count the header states and whole in the
    /// file, as `msDBFReadAttribute` reads it. A shape past them carries no properties.
    pub(crate) fn has(&self, record: u64) -> bool {
        self.record(record).is_some()
    }

    fn record(&self, record: u64) -> Option<&[u8]> {
        if record >= self.records {
            return None;
        }
        let start = usize::try_from(record)
            .ok()?
            .checked_mul(self.record_length)?
            .checked_add(self.header_length)?;
        self.bytes
            .get(start..start.checked_add(self.record_length)?)
    }

    /// The name of a field as `msDBFGetFieldInfo` cuts it: the first 11 bytes of its descriptor up
    /// to a NUL, of which blanks at bytes 10 down to 1 are cut off one by one.
    fn name(&self, field: &Field) -> &[u8] {
        let at = usize::from(field.descriptor);
        let raw = &self.bytes[at..at + 11];
        // A NUL before byte 10 leaves a NUL there, and no blank is cut.
        let Some(end) = raw.iter().position(|&b| b == 0) else {
            let mut end = 11;
            while end > 1 && raw[end - 1] == b' ' {
                end -= 1;
            }
            return &raw[..end];
        };
        &raw[..end]
    }

    /// The value of `field` in `record` as `msDBFReadAttribute` reads it: up to its first NUL,
    /// trailing blanks cut, leading blanks cut from an `N`, `F`, or `D` field, and a null `N`,
    /// `F`, or `D` read as `0`; decoded as UTF-8, an invalid byte replaced (ADR-0024).
    fn value<'a>(&self, record: &'a [u8], field: &Field) -> Value<'a> {
        let (offset, size) = (field.offset as usize, field.size as usize);
        let raw = &record[offset..offset + size];
        let raw = &raw[..raw.iter().position(|&b| b == 0).unwrap_or(raw.len())];
        let raw = &raw[..raw.iter().rposition(|&b| b != b' ').map_or(0, |i| i + 1)];
        let numeric = matches!(field.kind, b'N' | b'F' | b'D');
        let raw = if numeric {
            &raw[raw.iter().position(|&b| b != b' ').unwrap_or(raw.len())..]
        } else {
            raw
        };
        let null = match field.kind {
            b'N' | b'F' => raw.first() == Some(&b'*'),
            b'D' => raw.starts_with(b"00000000"),
            _ => false,
        };
        if null {
            return Value::String(Cow::Borrowed("0"));
        }
        Value::String(String::from_utf8_lossy(raw))
    }
}

impl Table for Dbf {
    /// The field whose name equals `name` without regard to ASCII case, the first of several, as
    /// `msDBFGetItemIndex` finds it with `strcasecmp`: byte for byte, so that a name that is not
    /// UTF-8 is found by no name, though `each` gives it with its invalid bytes replaced.
    fn get(&self, record: u64, name: &str) -> Option<Value<'_>> {
        let bytes = self.record(record)?;
        let field = self
            .fields
            .iter()
            .find(|f| self.name(f).eq_ignore_ascii_case(name.as_bytes()))?;
        Some(self.value(bytes, field))
    }

    fn each(&self, record: u64, f: &mut dyn FnMut(&str, Value<'_>)) {
        let Some(bytes) = self.record(record) else {
            return;
        };
        for field in &self.fields {
            f(
                &String::from_utf8_lossy(self.name(field)),
                self.value(bytes, field),
            );
        }
    }
}
