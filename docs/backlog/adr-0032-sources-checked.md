# ADR-0032 cites two sources as recalled and still to be checked

Observed: ADR-0032's Sources name Ericson, *Real-Time Collision Detection*, chapter 7, and Teschner
et al., *Optimized Spatial Hashing for Collision Detection of Deformable Objects* (VMV 2003), each
"recalled, to be checked". Both were checked on 2026-11-14:

- Ericson's book (Morgan Kaufmann, 2005) has, in chapter 7 *Spatial Partitioning*, section 7.1.3
  *Hashed Storage and Infinite Grids* under 7.1 *Uniform Grids*, as its table of contents at
  realtimecollisiondetection.net/books/rtcd/toc shows.
- Teschner, Heidelberger, Müller, Pomeranets, and Gross, VMV 2003 (Munich), pages 47–54: a hash
  function maps a potentially infinite regular grid onto a table of fixed size, as cited.

The maintainer chose on 2026-11-14 to drop the two "to be checked" notes at the next normalization
rather than supersede ADR-0032 now.

Where: docs/adr/0032-label-cache.md (Sources / Prior art).
