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

//! The `.dbf` beside a shapefile: its header is checked when the shapefile opens. Its records do
//! not reach the feature source port yet.

use forager_ports::SourceError;

/// Checks the header of a `.dbf` as `msDBFOpenVirtualFile` does. Of the 32-byte table header only
/// the header length and the record length are read: neither the version, the update date, nor the
/// language driver.
pub(crate) fn check(dbf: &[u8]) -> Result<(), SourceError> {
    let header = dbf
        .get(..32)
        .ok_or_else(|| SourceError::new("the .dbf is shorter than its 32-byte header"))?;
    let header_length = usize::from(u16::from_le_bytes([header[8], header[9]]));
    let record_length = u32::from(u16::from_le_bytes([header[10], header[11]]));
    if header_length <= 32 {
        return Err(SourceError::new(format!(
            "the .dbf header length {header_length} leaves no room for its fields"
        )));
    }
    let descriptors = dbf.get(32..header_length).ok_or_else(|| {
        SourceError::new(format!(
            "the .dbf is shorter than its header length {header_length}"
        ))
    })?;
    // One field per whole 32 bytes past the table header; the record starts with its deletion
    // flag. An N or F field takes one byte of size and one of decimals, any other two of size.
    let (fields, _) = descriptors.as_chunks::<32>();
    let end = fields.iter().fold(1u32, |end, field| {
        let size = match field[11] {
            b'N' | b'F' => u32::from(field[16]),
            _ => u32::from(u16::from_le_bytes([field[16], field[17]])),
        };
        end + size
    });
    if !fields.is_empty() && end > record_length {
        return Err(SourceError::new(format!(
            "the .dbf fields take {end} bytes of a record of {record_length}"
        )));
    }
    Ok(())
}
