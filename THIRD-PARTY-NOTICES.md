# Third-party notices

Forager is licensed under `MIT OR Apache-2.0`. Parts of it follow code of others, whose notices
must travel with every copy of Forager, in source and in binary form, the command-line tool and
the browser bindings included: a distribution of a binary carries this file.
`scripts/package.sh` builds both binary artefacts with this file and the licences, and holds
each to it.

## MapServer

Logic ported from MapServer's C code keeps MapServer's copyright and permission notice in the file
that follows it (`scripts/check-notice.sh` holds this):

```text
Copyright (c) 1996-2005 Regents of the University of Minnesota.

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute,
sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies of this
Software or works derived from this Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

## PROJ

The reprojection of `crates/proj` ports PROJ's formulas, as of PROJ 9.4.0. Each file that follows a
PROJ source names it and holds PROJ's notice (`scripts/check-notice.sh` holds this), from its file
`COPYING`, with the copyright lines of the PROJ sources it follows; the MIT licence each grants asks
that the notice is included in all copies:

```text
Copyright (c) 1995, Gerald Evenden
Copyright (c) 2000, Frank Warmerdam
Copyright (c) 2002, Frank Warmerdam <warmerdam@pobox.com>
Copyright (c) 2016, Thomas Knudsen / SDFE
Copyright (c) 2016, 2017 Thomas Knudsen/SDFE
Copyright (c) 2018, Thomas Knudsen / SDFE
```

```text
All source, data files and other contents of the PROJ package are available under the following
terms. Note that the PROJ 4.3 and earlier was "public domain" as is common with US government work,
but apparently this is not a well defined legal term in many countries. Frank Warmerdam placed
everything under the following MIT style license because he believed it is effectively the same as
public domain, allowing anyone to use the code as they wish, including making proprietary
derivatives.

Initial PROJ 4.3 public domain code was put as Frank Warmerdam as copyright holder, but he didn't
mean to imply he did the work. Essentially all work was done by Gerald Evenden.

Copyright information can be found in source files.

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute,
sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

## pnpoly, W. Randolph Franklin

The point-in-polygon tests of the label cache (`crates/label/src/cache.rs`) and of a layer's
`GEOMTRANSFORM` (`crates/symbolize/src/layer_transform.rs`) port MapServer's `msPointInPolygon`,
which `src/mapsearch.c` takes from W. Randolph Franklin's pnpoly under his notice. Its condition 2
asks that a redistribution in binary form reproduces it here:

```text
Copyright (c) 1970-2003, Wm. Randolph Franklin

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute,
sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

1. Redistributions of source code must retain the above copyright notice, this list of conditions
   and the following disclaimers.
2. Redistributions in binary form must reproduce the above copyright notice in the documentation
   and/or other materials provided with the distribution.
3. The name of W. Randolph Franklin may not be used to endorse or promote products derived from
   this Software without specific prior written permission.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```
