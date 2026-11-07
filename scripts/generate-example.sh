#!/usr/bin/env bash
#
# Generates the GeoJSON of the browser example: a synthetic landscape of 4000 × 2500 map units,
# so that no third-party data or licence comes into the tree and the data can be made again.
#
# It writes five files, one per layer of examples/web/sample.map:
#
#   landuse.geojson    parcels of field, cut into strips of three crops, forest, park, and
#                      wetland on a jittered grid, built-up areas about the larger settlements,
#                      and lakes (water);
#   rivers.geojson     a main river across the map, tributaries from the map's edges, and streams;
#   buildings.geojson  blocks between the streets of the towns, and houses about the villages
#                      and hamlets;
#   roads.geojson      motorways between the cities, primary roads from each town, secondary and
#                      minor roads and tracks over the grid of settlements, and streets in towns;
#   places.geojson     a few hundred named places, each with a population and a marker size.
#
# The landscape is drawn from one fixed seed through a Park–Miller generator, and computed with
# the four arithmetic operations, the remainder, and square roots, which IEEE 754 rounds exactly,
# so that every awk on every platform writes the same bytes. Coordinates are rounded to whole map
# units, which keeps the files small; a unit is 8 pixels at zoom 3, three levels in from zoom 0,
# where the page starts.
# scripts/test-generate-example.sh runs it twice and compares both runs with the checked-in files.
#
# Pure bash + POSIX awk (run with mawk, the awk of Debian and Ubuntu). Usage:
#     scripts/generate-example.sh [output directory]
# The directory defaults to examples/web/ of this repository.
# Exit code 0 when the files are written, 1 otherwise.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${1:-$ROOT/examples/web}"
mkdir -p "$OUT"

program="$(cat <<'AWK'
function rnd() {
  seed = (seed * 16807) % 2147483647
  return seed / 2147483647
}

# A coordinate rounded to a whole map unit.
function fmt(v,    s) {
  s = sprintf("%.0f", v)
  return s == "-0" ? "0" : s
}

function smooth(t) { return t * t * (3 - 2 * t) }

# Value noise on a lattice `spacing` apart, from 0 to 1.
function vnoise(x, y, spacing, lattice,    i, j, u, v, a, b, c, d) {
  i = int(x / spacing); j = int(y / spacing)
  u = smooth(x / spacing - i); v = smooth(y / spacing - j)
  a = lattice[i, j]; b = lattice[i + 1, j]; c = lattice[i, j + 1]; d = lattice[i + 1, j + 1]
  return a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
}

function noise(x, y) { return (2 * vnoise(x, y, 500, coarse) + vnoise(x, y, 250, fine)) / 3 }

function clamp(v, lo, hi) { return v < lo ? lo : (v > hi ? hi : v) }

function dist(x1, y1, x2, y2) { return sqrt((x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1)) }

function in_lake(x, y, margin,    l) {
  for (l = 1; l <= lakes; l++)
    if (dist(x, y, LX[l], LY[l]) < LR[l] * 1.25 + margin) return 1
  return 0
}

# The ring of a blob about (cx, cy) of radius r, 24 vertices whose radii vary smoothly, into
# BX, BY; rotated by 15° a vertex, whose cosine and sine are roots of 6 and 2.
function blob(cx, cy, r, vary,    k, c, s, ux, uy, t, rr, rs) {
  c = (sqrt(6) + sqrt(2)) / 4; s = (sqrt(6) - sqrt(2)) / 4
  for (k = 0; k < 24; k++) rr[k] = 1 - vary + 2 * vary * rnd()
  ux = 1; uy = 0
  for (k = 0; k < 24; k++) {
    rs = r * (rr[(k + 23) % 24] + 2 * rr[k] + rr[(k + 1) % 24]) / 4
    BX[k] = cx + rs * ux; BY[k] = cy + rs * uy
    t = ux * c - uy * s; uy = ux * s + uy * c; ux = t
  }
  BX[24] = BX[0]; BY[24] = BY[0]
}

function blob_ring(    k, s) {
  s = "[[" fmt(BX[0]) "," fmt(BY[0]) "]"
  for (k = 1; k <= 24; k++) s = s ",[" fmt(BX[k]) "," fmt(BY[k]) "]"
  return s "]"
}

# A river walked from (x, y) toward (tx, ty) in steps of 12 units, its heading turned at random
# and pulled toward the target; it ends at the target, or at the map's right edge for the main one.
function walk(r, x, y, tx, ty, pull, wander, main,    vx, vy, d, n, px, py, steps, t) {
  vx = tx - x; vy = ty - y; d = sqrt(vx * vx + vy * vy); vx /= d; vy /= d
  n = 1; RX[r, n] = x; RY[r, n] = y
  for (steps = 0; steps < 600; steps++) {
    d = rnd() - 0.5
    t = vx; vx += -vy * d * wander; vy += t * d * wander
    d = dist(x, y, tx, ty)
    if (!main && d < 14) break
    # The pull grows with the steps, so that a river never circles its target.
    vx += pull * (1 + steps / 12) * (tx - x) / d; vy += pull * (1 + steps / 12) * (ty - y) / d
    d = sqrt(vx * vx + vy * vy); vx /= d; vy /= d
    px = x; py = y
    x += 12 * vx; y = clamp(y + 12 * vy, 4, H - 4)
    if (main && x >= W) {
      n++; RX[r, n] = W; RY[r, n] = py + (W - px) * (y - py) / (x - px)
      RN[r] = n
      return
    }
    n++; RX[r, n] = x; RY[r, n] = y
  }
  n++; RX[r, n] = tx; RY[r, n] = ty
  RN[r] = n
}

# The river point nearest (x, y), among the rivers before `upto`, into NX_, NY_; its distance.
function nearest_river(x, y, upto,    r, k, d, best) {
  best = 1e9
  for (r = 1; r < upto; r++)
    for (k = 1; k <= RN[r]; k++) {
      d = dist(x, y, RX[r, k], RY[r, k])
      if (d < best) { best = d; NX_ = RX[r, k]; NY_ = RY[r, k] }
    }
  return best
}

function pick(list, n) { return list[1 + int(rnd() * n)] }

function name_for(hamlet,    name, tries) {
  for (tries = 0; tries < 50; tries++) {
    name = pick(PRE, npre) pick(SUF, nsuf)
    if (rnd() < 0.14) name = pick(QUAL, nqual) " " name
    if (hamlet && rnd() < 0.3) name = name " " pick(TAIL, ntail)
    if (!(name in used)) break
  }
  used[name] = 1
  return name
}

# A line from (x1, y1) to (x2, y2) bent by midpoint displacement into PX, PY, PN.
function bend(x1, y1, x2, y2, depth, rough,    o, mx, my) {
  if (depth == 0) { PN++; PX[PN] = x2; PY[PN] = y2; return }
  o = (rnd() - 0.5) * 2 * rough
  mx = (x1 + x2) / 2 - (y2 - y1) * o; my = (y1 + y2) / 2 + (x2 - x1) * o
  bend(x1, y1, mx, my, depth - 1, rough)
  bend(mx, my, x2, y2, depth - 1, rough)
}

function road(kind, x1, y1, x2, y2, depth, rough,    k, s) {
  PN = 1; PX[1] = x1; PY[1] = y1
  bend(x1, y1, x2, y2, depth, rough)
  s = "[" fmt(PX[1]) "," fmt(PY[1]) "]"
  for (k = 2; k <= PN; k++) s = s ",[" fmt(PX[k]) "," fmt(PY[k]) "]"
  ROADS[kind, ++ROADN[kind]] = s
}

# The point at fraction f along side s of the parcel in QX, QY, into SPX, SPY.
function side_point(s, f,    seg, t) {
  seg = int(f * NS); if (seg > NS - 1) seg = NS - 1
  t = f * NS - seg
  SPX = QX[s, seg] + (QX[s, seg + 1] - QX[s, seg]) * t
  SPY = QY[s, seg] + (QY[s, seg + 1] - QY[s, seg]) * t
}

function pt(x, y) { return "[" fmt(x) "," fmt(y) "]" }

# The ring of the strip of the parcel between fractions fa and fb of side a and fractions gb and
# ga of the side opposite, which runs the other way; at fraction 0 or 1 it takes in the side
# between.
function strip(a, fa, fb, ga, gb,    c, k, s, first) {
  c = (a + 2) % 4
  side_point(a, fa); first = pt(SPX, SPY); s = "[" first
  for (k = 1; k < NS; k++) if (k / NS > fa && k / NS < fb) s = s "," pt(QX[a, k], QY[a, k])
  side_point(a, fb); s = s "," pt(SPX, SPY)
  if (fb == 1) for (k = 1; k < NS; k++) s = s "," pt(QX[(a + 1) % 4, k], QY[(a + 1) % 4, k])
  side_point(c, gb); s = s "," pt(SPX, SPY)
  for (k = 1; k < NS; k++) if (k / NS > gb && k / NS < ga) s = s "," pt(QX[c, k], QY[c, k])
  side_point(c, ga); s = s "," pt(SPX, SPY)
  if (fa == 0) for (k = 1; k < NS; k++) s = s "," pt(QX[(a + 3) % 4, k], QY[(a + 3) % 4, k])
  return s "," first "]"
}

# The polygon of a square about (x, y), `half` from its centre to a side.
function square(x, y, half) {
  return "[[" pt(x - half, y - half) "," pt(x + half, y - half) "," pt(x + half, y + half) "," \
    pt(x - half, y + half) "," pt(x - half, y - half) "]]"
}

# How far a town's streets reach from its centre.
function town_radius(pop) { return sqrt(pop) * 0.12 + 15 }

function feature(file, properties, type, coordinates) {
  printf "%s{\"type\":\"Feature\",\"properties\":{%s},\"geometry\":{\"type\":\"%s\",\"coordinates\":%s}}",
    (written[file]++ ? ",\n" : "{\"type\":\"FeatureCollection\",\"features\":[\n"),
    properties, type, coordinates > file
}

function finish(file) {
  printf "%s]}\n", (written[file] ? "\n" : "{\"type\":\"FeatureCollection\",\"features\":[") > file
  close(file)
}

BEGIN {
  W = 4000; H = 2500
  seed = 20261107

  # The noise that decides forest and field.
  for (i = 0; i <= 9; i++) for (j = 0; j <= 6; j++) coarse[i, j] = rnd()
  for (i = 0; i <= 17; i++) for (j = 0; j <= 11; j++) fine[i, j] = rnd()

  # Lakes, apart from one another.
  lakes = 0
  for (tries = 0; lakes < 6 && tries < 200; tries++) {
    x = 300 + 3400 * rnd(); y = 300 + 1900 * rnd(); r = 50 + 70 * rnd()
    if (in_lake(x, y, r * 1.25 + 150)) continue
    lakes++; LX[lakes] = x; LY[lakes] = y; LR[lakes] = r
  }

  # The rivers: the main one from the left edge to the right, tributaries from the top and the
  # bottom edge into it, and streams into either; ORDER is 3, 2, and 1.
  rivers = 1; ORDER[1] = 3
  walk(1, 0, 900 + 700 * rnd(), W + 400, 900 + 700 * rnd(), 0.08, 0.5, 1)
  for (t = 0; t < 4; t++) {
    x = 500 + 3000 * rnd(); y = (t % 2) ? H - 4 : 4
    nearest_river(x + 500 * (rnd() - 0.5), H / 2, 2)
    rivers++; ORDER[rivers] = 2
    walk(rivers, x, y, NX_, NY_, 0.06, 0.5, 0)
  }
  for (tries = 0; rivers < 21 && tries < 500; tries++) {
    x = 100 + 3800 * rnd(); y = 100 + 2300 * rnd()
    if (in_lake(x, y, 20)) continue
    d = nearest_river(x, y, rivers + 1)
    if (d < 80 || d > 450) continue
    rivers++; ORDER[rivers] = 1
    walk(rivers, x, y, NX_, NY_, 0.1, 0.6, 0)
  }

  # The settlements on a jittered grid of 20 × 12, leaving out those in a lake; a population
  # drawn as 100 / r², so that few are large; a city near an earlier one shrinks to a town.
  places = 0
  for (c = 0; c < 20; c++) for (rr = 0; rr < 12; rr++) {
    x = (c + 0.5) * 200 + 120 * (rnd() - 0.5); y = (rr + 0.5) * H / 12 + 120 * (rnd() - 0.5)
    r = rnd()
    if (in_lake(x, y, 15)) continue
    places++; SX[places] = x; SY[places] = y; GRID[c, rr] = places
    if (r < 0.0001) r = 0.0001
    pop = int(100 / (r * r))
    if (pop > 500000) pop = 120000 + int(380000 * rnd())
    if (pop < 40) pop = 40 + int(160 * rnd())
    for (k = 1; k < places && pop >= 100000; k++)
      if (POP[k] >= 100000 && dist(x, y, SX[k], SY[k]) < 700) pop = int(pop / 10)
    POP[places] = pop
  }
  gridded = places
  # Hamlets between them.
  for (tries = 0; places < gridded + 70 && tries < 1000; tries++) {
    x = 30 + 3940 * rnd(); y = 30 + 2440 * rnd()
    if (in_lake(x, y, 15)) continue
    near = 0
    for (k = 1; k <= places; k++) if (dist(x, y, SX[k], SY[k]) < 60) near = 1
    if (near) continue
    places++; SX[places] = x; SY[places] = y; POP[places] = 20 + int(300 * rnd())
  }

  npre = split("Ash Bram Brook Cal Dun Elm Fern Gold Hart Hol Kings Lang Mar Mill Nor Oak Pen Red Ross Stan Thorn Wey Whit Wil", PRE, " ")
  nsuf = split("bury by field ford ham hurst ley mere stead stow ton well wick worth", SUF, " ")
  nqual = split("Upper Lower Great Little North South East West", QUAL, " ")
  ntail = split("Green End Cross Common Farm Heath", TAIL, " ")
  for (k = 1; k <= places; k++) NAME[k] = name_for(POP[k] < 1000)

  # The parcels: a grid of 32 × 20 cells of 125 units, its inner vertices jittered, and each
  # inner edge bent at a point, NS - 1, shared by the parcels on either side.
  NXC = 32; NYC = 20; cell = 125; NS = 2
  for (i = 0; i <= NXC; i++) for (j = 0; j <= NYC; j++) {
    VX[i, j] = i * cell + ((i > 0 && i < NXC) ? 0.6 * cell * (rnd() - 0.5) : 0)
    VY[i, j] = j * cell + ((j > 0 && j < NYC) ? 0.6 * cell * (rnd() - 0.5) : 0)
  }
  for (i = 0; i <= NXC; i++) for (j = 0; j <= NYC; j++) {
    if (i < NXC) for (k = 1; k < NS; k++) {
      o = (j > 0 && j < NYC) ? 0.3 * (rnd() - 0.5) : 0
      dx = VX[i + 1, j] - VX[i, j]; dy = VY[i + 1, j] - VY[i, j]
      EHX[i, j, k] = VX[i, j] + dx * k / NS - dy * o; EHY[i, j, k] = VY[i, j] + dy * k / NS + dx * o
    }
    if (j < NYC) for (k = 1; k < NS; k++) {
      o = (i > 0 && i < NXC) ? 0.3 * (rnd() - 0.5) : 0
      dx = VX[i, j + 1] - VX[i, j]; dy = VY[i, j + 1] - VY[i, j]
      EVX[i, j, k] = VX[i, j] + dx * k / NS - dy * o; EVY[i, j, k] = VY[i, j] + dy * k / NS + dx * o
    }
  }
  landuse = OUT "/landuse.geojson"
  for (j = 0; j < NYC; j++) for (i = 0; i < NXC; i++) {
    # The parcel's four sides in the order of its ring, each of NS + 1 points: bottom, right, top,
    # and left, the last two run backwards.
    for (k = 0; k <= NS; k++) {
      QX[0, k] = k == 0 ? VX[i, j] : (k == NS ? VX[i + 1, j] : EHX[i, j, k])
      QY[0, k] = k == 0 ? VY[i, j] : (k == NS ? VY[i + 1, j] : EHY[i, j, k])
      QX[1, k] = k == 0 ? VX[i + 1, j] : (k == NS ? VX[i + 1, j + 1] : EVX[i + 1, j, k])
      QY[1, k] = k == 0 ? VY[i + 1, j] : (k == NS ? VY[i + 1, j + 1] : EVY[i + 1, j, k])
      QX[2, k] = k == 0 ? VX[i + 1, j + 1] : (k == NS ? VX[i, j + 1] : EHX[i, j + 1, NS - k])
      QY[2, k] = k == 0 ? VY[i + 1, j + 1] : (k == NS ? VY[i, j + 1] : EHY[i, j + 1, NS - k])
      QX[3, k] = k == 0 ? VX[i, j + 1] : (k == NS ? VX[i, j] : EVX[i, j, NS - k])
      QY[3, k] = k == 0 ? VY[i, j + 1] : (k == NS ? VY[i, j] : EVY[i, j, NS - k])
    }
    cx = (VX[i, j] + VX[i + 1, j] + VX[i, j + 1] + VX[i + 1, j + 1]) / 4
    cy = (VY[i, j] + VY[i + 1, j] + VY[i, j + 1] + VY[i + 1, j + 1]) / 4
    n = noise(cx, cy); u = rnd()
    town = 0
    for (k = 1; k <= gridded; k++)
      if (POP[k] >= 10000 && dist(cx, cy, SX[k], SY[k]) < town_radius(POP[k]) + 90) town = 1
    if (town && u < 0.35) kind = "park"
    else if (n < 0.5 && u < 0.6 && nearest_river(cx, cy, rivers + 1) < 80) kind = "wetland"
    else if (n > 0.58 || u > 0.95) kind = "forest"
    else kind = "field"
    if (kind != "field") {
      feature(landuse, "\"kind\":\"" kind "\"", "Polygon", "[" strip(0, 0, 1, 1, 0) "]")
      continue
    }
    # A field parcel is cut into one to three strips across one pair of its sides, each strip
    # with a crop of its own.
    a = rnd() < 0.5 ? 0 : 1; strips = 1 + int(2.4 * rnd())
    fa = 0; ga = 1
    for (m = 1; m <= strips; m++) {
      if (m == strips) { fb = 1; gb = 0 }
      else { fb = (m + 0.3 * (rnd() - 0.5)) / strips; gb = 1 - fb + 0.1 * (rnd() - 0.5) }
      feature(landuse, "\"kind\":\"field\",\"crop\":" (1 + int(3 * rnd())), "Polygon",
              "[" strip(a, fa, fb, ga, gb) "]")
      fa = fb; ga = gb
    }
  }
  # Built-up areas about the towns and villages, then the lakes over all.
  for (k = 1; k <= gridded; k++) if (POP[k] >= 1000) {
    blob(SX[k], SY[k], POP[k] >= 10000 ? town_radius(POP[k]) * 1.15 : sqrt(POP[k]) * 0.4, 0.25)
    feature(landuse, "\"kind\":\"residential\"", "Polygon", "[" blob_ring() "]")
  }
  for (l = 1; l <= lakes; l++) {
    blob(LX[l], LY[l], LR[l], 0.3)
    feature(landuse, "\"kind\":\"water\"", "Polygon", "[" blob_ring() "]")
  }
  finish(landuse)

  file = OUT "/rivers.geojson"
  for (r = 1; r <= rivers; r++) {
    s = "[" fmt(RX[r, 1]) "," fmt(RY[r, 1]) "]"
    for (k = 2; k <= RN[r]; k++) s = s ",[" fmt(RX[r, k]) "," fmt(RY[r, k]) "]"
    feature(file, "\"order\":" ORDER[r], "LineString", "[" s "]")
  }
  finish(file)

  # Motorways join the cities from west to east; each town has a primary road to the nearest
  # larger place; the grid's neighbours are joined by a secondary road, a minor road, or a track,
  # by the smaller population; each hamlet between has a track to the nearest place on the grid;
  # and the towns have a grid of streets, turned at random.
  cities = 0
  for (k = 1; k <= gridded; k++) if (POP[k] >= 100000) CITY[++cities] = k
  for (a = 1; a <= cities; a++) for (b = a + 1; b <= cities; b++)
    if (SX[CITY[b]] < SX[CITY[a]]) { t = CITY[a]; CITY[a] = CITY[b]; CITY[b] = t }
  for (a = 1; a < cities; a++)
    road("motorway", SX[CITY[a]], SY[CITY[a]], SX[CITY[a + 1]], SY[CITY[a + 1]], 3, 0.06)
  for (k = 1; k <= gridded; k++) if (POP[k] >= 10000 && POP[k] < 100000) {
    best = 0
    for (m = 1; m <= gridded; m++)
      if (POP[m] > POP[k] && (!best || dist(SX[k], SY[k], SX[m], SY[m]) < d)) {
        best = m; d = dist(SX[k], SY[k], SX[m], SY[m])
      }
    if (best) road("primary", SX[k], SY[k], SX[best], SY[best], 3, 0.08)
  }
  for (c = 0; c < 20; c++) for (rr = 0; rr < 12; rr++) {
    a = GRID[c, rr]
    if (!a) continue
    for (t = 0; t < 2; t++) {
      b = t ? GRID[c, rr + 1] : GRID[c + 1, rr]
      if (!b || in_lake((SX[a] + SX[b]) / 2, (SY[a] + SY[b]) / 2, 0)) continue
      m = POP[a] < POP[b] ? POP[a] : POP[b]
      kind = m >= 1000 ? "secondary" : (m >= 150 ? "minor" : "track")
      road(kind, SX[a], SY[a], SX[b], SY[b], 2, 0.1)
    }
  }
  for (k = gridded + 1; k <= places; k++) {
    best = 0
    for (m = 1; m <= gridded; m++)
      if (!best || dist(SX[k], SY[k], SX[m], SY[m]) < d) { best = m; d = dist(SX[k], SY[k], SX[m], SY[m]) }
    road("track", SX[k], SY[k], SX[best], SY[best], 2, 0.1)
  }
  # A town's streets, 16 apart across a disc about it, and in most squares between them, short
  # of the disc's edge, a block of buildings; one feature of each per town.
  for (k = 1; k <= gridded; k++) if (POP[k] >= 10000) {
    R = town_radius(POP[k]); x = int(SX[k]); y = int(SY[k]); M = int(R / 16)
    s = ""
    for (m = -M; m <= M; m++) {
      w = R * R - m * 16 * m * 16
      if (w < 100) continue
      w = int(sqrt(w))
      s = s (s == "" ? "" : ",") "[" pt(x - w, y + m * 16) "," pt(x + w, y + m * 16) "]"
      s = s ",[" pt(x + m * 16, y - w) "," pt(x + m * 16, y + w) "]"
    }
    STREETS[++STREETN] = "[" s "]"
    s = ""
    for (m = -M; m < M; m++) for (q = -M; q < M; q++) {
      ox = m * 16 + 8; oy = q * 16 + 8
      if (ox * ox + oy * oy > R * R * 0.64 || rnd() < 0.12) continue
      if (nearest_river(x + ox, y + oy, rivers + 1) < 14) continue
      s = s (s == "" ? "" : ",") square(x + ox, y + oy, 5)
    }
    if (s != "") BUILDINGS[++NB] = "\"kind\":\"block\"\t[" s "]"
  }
  # Houses about the villages and hamlets, clear of the place's marker; one feature per place.
  for (k = 1; k <= places; k++) if (POP[k] < 10000) {
    count = POP[k] >= 1000 ? 4 + int(5 * rnd()) : 1 + int(3 * rnd())
    R = POP[k] >= 1000 ? 6 + sqrt(POP[k]) * 0.2 : 10
    s = ""
    for (m = 0; m < count; m++) {
      do { ox = int((2 * rnd() - 1) * R); oy = int((2 * rnd() - 1) * R) } \
        while (ox * ox + oy * oy > R * R || ox * ox + oy * oy < 16)
      if (nearest_river(SX[k] + ox, SY[k] + oy, rivers + 1) < 8) continue
      s = s (s == "" ? "" : ",") square(int(SX[k]) + ox, int(SY[k]) + oy, 1 + int(2 * rnd()))
    }
    BUILDINGS[++NB] = "\"kind\":\"house\"\t[" s "]"
  }
  file = OUT "/buildings.geojson"
  for (k = 1; k <= NB; k++) {
    split(BUILDINGS[k], parts, "\t")
    feature(file, parts[1], "MultiPolygon", parts[2])
  }
  finish(file)
  file = OUT "/roads.geojson"
  for (k = 1; k <= STREETN; k++) feature(file, "\"kind\":\"street\"", "MultiLineString", STREETS[k])
  nkinds = split("track minor secondary primary motorway", KINDS, " ")
  for (t = 1; t <= nkinds; t++)
    for (k = 1; k <= ROADN[KINDS[t]]; k++)
      feature(file, "\"kind\":\"" KINDS[t] "\"", "LineString", "[" ROADS[KINDS[t], k] "]")
  finish(file)

  # The places, the larger first, each with the size of its marker.
  file = OUT "/places.geojson"
  for (k = 1; k <= places; k++) ORDERED[k] = k
  for (a = 2; a <= places; a++) {
    t = ORDERED[a]
    for (b = a - 1; b >= 1 && POP[ORDERED[b]] < POP[t]; b--) ORDERED[b + 1] = ORDERED[b]
    ORDERED[b + 1] = t
  }
  for (a = 1; a <= places; a++) {
    k = ORDERED[a]
    size = POP[k] >= 100000 ? 10 : (POP[k] >= 10000 ? 7 : (POP[k] >= 1000 ? 5 : 3))
    feature(file, "\"name\":\"" NAME[k] "\",\"population\":" POP[k] ",\"size\":" size,
            "Point", "[" fmt(SX[k]) "," fmt(SY[k]) "]")
  }
  finish(file)
}
AWK
)"

LC_ALL=C awk -v OUT="$OUT" "$program" </dev/null
