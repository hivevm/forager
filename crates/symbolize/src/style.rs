// The gamma of a fill follows MapServer's default GAMMA, src/mapagg.cpp, of the release pinned in
// scripts/fetch-msautotest.sh.
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

//! What drawing a style reads beside its bounds: a property's value as the engine binds it, and the
//! gamma a fill is drawn through. A leaf, which every module that draws a style may import.

use forager_style_ir::Bound;

/// The gamma a polygon shape and a hatch are filled through, MapServer's default `GAMMA` of its
/// AGG output formats (`agg2CreateImage`, `src/mapagg.cpp`); a `GAMMA` of the output
/// format or of the layer is not carried yet.
pub(crate) const GAMMA: f64 = 0.75;

/// A style property's value. The engine binds each style to the feature it draws before it draws
/// it, so a property bound to a property or an expression is a value here, or unset.
pub(crate) fn fixed<T: Copy>(property: &Option<Bound<T>>) -> Option<T> {
    property.as_ref().and_then(Bound::value)
}
