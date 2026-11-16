//! Tests of the reprojection against the worked examples of IOGP's Guidance Note 7-2 and the
//! projections the cases in scope name.

use super::*;
use forager_style_ir::Extent;

const LONGLAT_WGS84: &str = "+proj=longlat +datum=WGS84 +no_defs";
/// EPSG:3978 as the suite's `wxs/data/epsg2` defines it, with its stray `no_defs`.
const EPSG_3978: &str = "+proj=lcc +lat_1=49 +lat_2=77 +lat_0=49 +lon_0=-95 +x_0=0 +y_0=0 \
                         +ellps=GRS80 +datum=NAD83 +units=m +no_defs no_defs";
/// EPSG:3857 as `adapter-epsg` builds it in.
const EPSG_3857: &str = "+proj=merc +a=6378137 +b=6378137 +lat_ts=0 +lon_0=0 +x_0=0 +y_0=0 +k=1 \
                         +units=m +nadgrids=@null +wktext +no_defs";

fn way(from: &str, to: &str) -> Reprojection {
    Reprojection::new(
        Projection::parse(from).unwrap(),
        Projection::parse(to).unwrap(),
    )
}

fn at(x: f64, y: f64) -> Coord {
    Coord { x, y }
}

fn degrees(d: f64, m: f64) -> f64 {
    d + m / 60.0
}

#[test]
fn the_lambert_conic_of_two_parallels_matches_iogps_worked_example() {
    // Verifies: ADR-0037
    // Guidance Note 7-2, Lambert Conic Conformal (2SP): NAD27 / Texas South Central on Clarke
    // 1866; 28°30'N 96°00'W is E 2963503.91 ftUS, N 254759.80 ftUS.
    let geographic = "+proj=longlat +a=6378206.4 +rf=294.9786982";
    let texas = format!(
        "+proj=lcc +a=6378206.4 +rf=294.9786982 +lat_1={:.12} +lat_2={:.12} +lat_0={:.12} \
         +lon_0=-99 +x_0=609601.2192 +y_0=0 +to_meter=1200/3937",
        degrees(28.0, 23.0),
        degrees(30.0, 17.0),
        degrees(27.0, 50.0)
    );
    let projected = way(geographic, &texas)
        .point(at(-96.0, degrees(28.0, 30.0)))
        .unwrap();
    assert!((projected.x - 2_963_503.91).abs() < 0.01, "{projected:?}");
    assert!((projected.y - 254_759.80).abs() < 0.01, "{projected:?}");
    let back = way(&texas, geographic).point(projected).unwrap();
    assert!(
        (back.x + 96.0).abs() < 1e-9 && (back.y - 28.5).abs() < 1e-9,
        "{back:?}"
    );
}

#[test]
fn the_mercator_of_an_ellipsoid_matches_iogps_worked_example() {
    // Verifies: ADR-0037
    // Guidance Note 7-2, Mercator (variant A): Makassar / NEIEZ on Bessel 1841; 3°S 120°E is
    // E 5009726.58 m, N 569150.82 m.
    let geographic = "+proj=longlat +a=6377397.155 +rf=299.1528128";
    let neiez = "+proj=merc +a=6377397.155 +rf=299.1528128 +lon_0=110 +k=0.997 +x_0=3900000 \
                 +y_0=900000";
    let projected = way(geographic, neiez).point(at(120.0, -3.0)).unwrap();
    assert!((projected.x - 5_009_726.58).abs() < 0.01, "{projected:?}");
    assert!((projected.y - 569_150.82).abs() < 0.01, "{projected:?}");
    let back = way(neiez, geographic).point(projected).unwrap();
    assert!(
        (back.x - 120.0).abs() < 1e-9 && (back.y + 3.0).abs() < 1e-9,
        "{back:?}"
    );
}

#[test]
fn the_transverse_mercator_matches_iogps_worked_example() {
    // Verifies: ADR-0037
    // Guidance Note 7-2, Transverse Mercator: OSGB 1936 / British National Grid on Airy 1830;
    // 50°30'N 0°30'E is E 577274.99 m, N 69740.50 m.
    let geographic = "+proj=longlat +a=6377563.396 +rf=299.3249646";
    let grid = "+proj=tmerc +a=6377563.396 +rf=299.3249646 +lat_0=49 +lon_0=-2 \
                +k=0.9996012717 +x_0=400000 +y_0=-100000";
    let projected = way(geographic, grid).point(at(0.5, 50.5)).unwrap();
    assert!((projected.x - 577_274.99).abs() < 0.01, "{projected:?}");
    assert!((projected.y - 69_740.50).abs() < 0.01, "{projected:?}");
    let back = way(grid, geographic).point(projected).unwrap();
    assert!(
        (back.x - 0.5).abs() < 1e-9 && (back.y - 50.5).abs() < 1e-9,
        "{back:?}"
    );
}

#[test]
fn a_utm_zone_as_adapter_epsg_builds_it_in_projects_as_utm_does() {
    // Verifies: ADR-0037
    let zone_32 = "+proj=utm +zone=32 +datum=WGS84 +units=m +no_defs";
    let to_utm = way(LONGLAT_WGS84, zone_32);
    // On the central meridian at the equator: the false easting, northing 0.
    let origin = to_utm.point(at(9.0, 0.0)).unwrap();
    assert!(
        (origin.x - 500_000.0).abs() < 1e-6 && origin.y.abs() < 1e-6,
        "{origin:?}"
    );
    // 50°N 10°E is E 571666.45, N 5539109.82 in zone 32, as online UTM converters give it (no
    // primary source); the record of tests/reference/ holds zone 32 to MapServer's PROJ.
    let point = to_utm.point(at(10.0, 50.0)).unwrap();
    assert!((point.x - 571_666.45).abs() < 0.01, "{point:?}");
    assert!((point.y - 5_539_109.82).abs() < 0.01, "{point:?}");
    let back = to_utm.reversed().point(point).unwrap();
    assert!(
        (back.x - 10.0).abs() < 1e-9 && (back.y - 50.0).abs() < 1e-9,
        "{back:?}"
    );
    // South of the equator the false northing is 10 000 km: the mirror of the north.
    let south = "+proj=utm +zone=32 +south +datum=WGS84 +units=m +no_defs";
    let north = to_utm.point(at(9.5, 10.0)).unwrap();
    let mirrored = way(LONGLAT_WGS84, south).point(at(9.5, -10.0)).unwrap();
    assert!(
        (mirrored.x - north.x).abs() < 1e-6,
        "{mirrored:?} {north:?}"
    );
    assert!(
        (mirrored.y - (10_000_000.0 - north.y)).abs() < 1e-6,
        "{mirrored:?} {north:?}"
    );
}

#[test]
fn web_mercator_takes_wgs84_longitude_and_latitude_on_its_sphere_without_a_shift() {
    // Verifies: ADR-0037
    let projected = way(LONGLAT_WGS84, EPSG_3857).point(at(10.0, 50.0)).unwrap();
    assert!(
        (projected.x - 1_113_194.907_932_735_7).abs() < 1e-6,
        "{projected:?}"
    );
    assert!(
        (projected.y - 6_446_275.841_017_158).abs() < 1e-6,
        "{projected:?}"
    );
}

#[test]
fn the_canadian_lambert_conic_of_the_suite_has_its_origin_at_49n_95w() {
    // Verifies: ADR-0037
    // A zero `towgs84` shifts nothing, as MapServer's call of PROJ does not.
    let projected = way(LONGLAT_WGS84, EPSG_3978)
        .point(at(-95.0, 49.0))
        .unwrap();
    assert!(
        projected.x.abs() < 1e-6 && projected.y.abs() < 1e-6,
        "{projected:?}"
    );
    // Prince Edward Island, where misc/quoting.map draws, lies east and south of the origin.
    let island = way(LONGLAT_WGS84, EPSG_3978)
        .point(at(-63.5, 46.4))
        .unwrap();
    assert!(
        island.x > 2_000_000.0 && island.x < 2_600_000.0,
        "{island:?}"
    );
    assert!(island.y > -100_000.0 && island.y < 600_000.0, "{island:?}");
    let back = way(EPSG_3978, LONGLAT_WGS84).point(island).unwrap();
    assert!(
        (back.x + 63.5).abs() < 1e-9 && (back.y - 46.4).abs() < 1e-9,
        "{back:?}"
    );
}

#[test]
fn a_seven_parameter_shift_moves_a_point_by_some_metres_and_back() {
    // Verifies: ADR-0037
    // OSGB36 on Airy 1830, as PROJ's table of datums defines it.
    let osgb36 = "+proj=longlat +a=6377563.396 +rf=299.3249646 \
                  +towgs84=446.448,-125.157,542.060,0.1502,0.2470,0.8421,-20.4894";
    let shifted = way(osgb36, LONGLAT_WGS84).point(at(-1.0, 52.0)).unwrap();
    // OSGB36 lies about 100 m from WGS84 in England: some thousandths of a degree.
    let moved = (shifted.x + 1.0).abs() + (shifted.y - 52.0).abs();
    assert!(moved > 1e-4 && moved < 1e-2, "{shifted:?}");
    // The way back starts at height 0, where the way there ended some metres off it, as PROJ's
    // two-dimensional transform drops the height: it returns within millimetres.
    let back = way(LONGLAT_WGS84, osgb36).point(shifted).unwrap();
    assert!(
        (back.x + 1.0).abs() < 1e-7 && (back.y - 52.0).abs() < 1e-7,
        "{back:?}"
    );
}

#[test]
fn a_named_datum_shifts_nothing_and_an_explicit_zero_towgs84_changes_the_ellipsoid() {
    // Verifies: ADR-0037
    // +datum=NAD83 is read as its EPSG datum, which MapServer's call of PROJ does not shift, as
    // the record of tests/reference/ shows; an explicit zero towgs84 on GRS80 is a bound CRS
    // whose null Helmert step still changes the ellipsoid, by about a tenth of a millimetre.
    let named = way(LONGLAT_WGS84, "+proj=longlat +datum=NAD83")
        .point(at(-63.0, 46.0))
        .unwrap();
    assert_eq!((named.x, named.y), (-63.0, 46.0));
    let bound = way(LONGLAT_WGS84, "+proj=longlat +ellps=GRS80 +towgs84=0,0,0")
        .point(at(-63.0, 46.0))
        .unwrap();
    let moved = (bound.y - 46.0).abs();
    assert!(moved > 0.0 && moved < 1e-8, "{bound:?}");
    // On WGS84's own ellipsoid a zero towgs84 shifts nothing either.
    let wgs84 = way(LONGLAT_WGS84, "+proj=longlat +ellps=WGS84 +towgs84=0,0,0")
        .point(at(-63.0, 46.0))
        .unwrap();
    assert_eq!((wgs84.x, wgs84.y), (-63.0, 46.0));
}

#[test]
fn an_unknown_projection_or_parameter_is_an_error_naming_it() {
    // Verifies: ADR-0037
    let error = |text: &str| Projection::parse(text).unwrap_err();
    assert_eq!(
        error("+proj=omerc +lat_0=4"),
        Error::Unsupported("+proj=omerc".to_owned())
    );
    assert_eq!(
        error("+proj=tmerc +R=6370997"),
        Error::Unsupported("+proj=tmerc on a sphere".to_owned())
    );
    assert_eq!(
        error("+proj=tmerc +ellps=GRS80 +approx"),
        Error::Unsupported("+approx".to_owned())
    );
    assert!(matches!(
        error("+proj=utm +zone=61 +ellps=GRS80"),
        Error::Invalid(_)
    ));
    assert_eq!(
        error("+proj=lcc +lat_1=49 +lon_wrap=180"),
        Error::Unsupported("+lon_wrap".to_owned())
    );
    assert_eq!(
        error("+proj=longlat +nadgrids=conus"),
        Error::Unsupported("+nadgrids=conus".to_owned())
    );
    assert_eq!(
        error("+proj=longlat +datum=NAD27"),
        Error::Unsupported("+datum=NAD27".to_owned())
    );
    // A datum, an ellipsoid, or a unit no case in scope names is unsupported until one does.
    assert_eq!(
        error("+proj=longlat +datum=OSGB36"),
        Error::Unsupported("+datum=OSGB36".to_owned())
    );
    assert_eq!(
        error("+proj=longlat +ellps=bessel"),
        Error::Unsupported("+ellps=bessel".to_owned())
    );
    assert_eq!(
        error("+proj=lcc +lat_1=30 +units=us-ft"),
        Error::Unsupported("+units=us-ft".to_owned())
    );
    assert_eq!(
        error("+proj=longlat +ellps=WGS84 +lon_0=10"),
        Error::Unsupported("+lon_0 of +proj=longlat".to_owned())
    );
    assert!(matches!(
        error("+proj=lcc +lat_1=0 +lat_2=0"),
        Error::Invalid(_)
    ));
    assert!(matches!(error("+proj=merc +x_0=east"), Error::Invalid(_)));
    assert!(matches!(error("+ellps=WGS84"), Error::Invalid(_)));
    assert_eq!(
        error("+proj=longlat +units=furlong").to_string(),
        "+units=furlong is not supported"
    );
}

#[test]
fn a_point_that_does_not_transform_is_an_error_not_a_panic() {
    // Verifies: ADR-0037, Q-3
    let to_lcc = way(LONGLAT_WGS84, EPSG_3978);
    // The pole the cone opens away from has no place on it.
    assert_eq!(to_lcc.point(at(0.0, -90.0)), Err(NotTransformed));
    // A latitude past the pole.
    assert_eq!(to_lcc.point(at(0.0, 95.0)), Err(NotTransformed));
    // A coordinate that is no finite number, which PROJ refuses as HUGE_VAL.
    for hostile in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(to_lcc.point(at(hostile, 10.0)), Err(NotTransformed));
        assert_eq!(
            to_lcc.reversed().point(at(hostile, 0.0)),
            Err(NotTransformed)
        );
        assert_eq!(
            to_lcc.reversed().point(at(0.0, hostile)),
            Err(NotTransformed)
        );
    }
    // The largest numbers transform as PROJ's formulas take them, or not, without a panic.
    for huge in [f64::MAX, -f64::MAX] {
        let _ = to_lcc.point(at(huge, huge));
        let _ = to_lcc.reversed().point(at(huge, huge));
    }
}

#[test]
fn the_angles_of_proj_are_those_of_its_literals() {
    // Verifies: ADR-0037
    assert_eq!(DEG_TO_RAD, ".017453292519943296".parse::<f64>().unwrap());
    assert_eq!(RAD_TO_DEG, "57.295779513082321".parse::<f64>().unwrap());
}

#[test]
fn projections_differ_as_mapserver_compares_their_parameters() {
    // Verifies: ADR-0037
    assert!(!differ("", LONGLAT_WGS84), "a layer without parameters");
    assert!(!differ(LONGLAT_WGS84, "  "), "a map without parameters");
    assert!(!differ(
        LONGLAT_WGS84,
        "+proj=longlat  +datum=WGS84 +no_defs"
    ));
    assert!(differ(LONGLAT_WGS84, "+proj=longlat +no_defs +datum=WGS84"));
    assert!(differ(EPSG_3978, LONGLAT_WGS84));
}

#[test]
fn an_extent_reprojects_to_a_rectangle_that_holds_its_reprojected_points() {
    // Verifies: ADR-0037
    // misc/quoting.map's extent, in longitude and latitude, asked of its layer in EPSG:3978.
    let map = Extent {
        min_x: -67.5725,
        min_y: 42.0,
        max_x: -58.9275,
        max_y: 48.5,
    };
    let way = way(LONGLAT_WGS84, EPSG_3978);
    let asked = way.extent(&map).unwrap();
    for corner in [
        at(map.min_x, map.min_y),
        at(map.min_x, map.max_y),
        at(map.max_x, map.min_y),
        at(map.max_x, map.max_y),
        at(-63.0, 48.5),
    ] {
        let p = way.point(corner).unwrap();
        assert!(
            asked.min_x <= p.x && p.x <= asked.max_x && asked.min_y <= p.y && p.y <= asked.max_y,
            "{corner:?} at {p:?} lies outside {asked:?}"
        );
    }
    // No point of a side reprojects: no rectangle.
    let beyond = Extent {
        min_x: 0.0,
        min_y: 100.0,
        max_x: 1.0,
        max_y: 120.0,
    };
    assert_eq!(way.extent(&beyond), None);
}

#[test]
fn a_rectangle_of_the_whole_world_or_of_no_size_takes_mapserver_s_shortcuts() {
    // Verifies: ADR-0037
    let world = Extent {
        min_x: -180.0,
        min_y: -90.0,
        max_x: 180.0,
        max_y: 90.0,
    };
    let edge = 20_037_508.342_789_2;
    let web = way(LONGLAT_WGS84, EPSG_3857).extent(&world).unwrap();
    assert_eq!(
        (web.min_x, web.min_y, web.max_x, web.max_y),
        (-edge, -edge, edge, edge)
    );
    let conic = way(LONGLAT_WGS84, EPSG_3978).extent(&world).unwrap();
    assert_eq!((conic.min_x, conic.max_y), (-1e15, 1e15));
    let point = Extent {
        min_x: -63.0,
        min_y: 45.0,
        max_x: -63.0,
        max_y: 45.0,
    };
    let to = way(LONGLAT_WGS84, EPSG_3978);
    let p = to.point(at(-63.0, 45.0)).unwrap();
    assert_eq!(
        to.extent(&point),
        Some(Extent {
            min_x: p.x,
            min_y: p.y,
            max_x: p.x,
            max_y: p.y
        })
    );
}

#[test]
fn a_side_that_leaves_the_projection_ends_at_its_horizon() {
    // Verifies: ADR-0037
    // The conic opens away from the south pole: the side along it does not reproject, and the
    // rectangle reaches as far south as the halving of the sides to it finds points that do.
    let to = way(LONGLAT_WGS84, EPSG_3978);
    let rect = to
        .extent(&Extent {
            min_x: -120.0,
            min_y: -90.0,
            max_x: -60.0,
            max_y: 60.0,
        })
        .unwrap();
    let near_pole = to.point(at(-90.0, -89.9)).unwrap();
    assert!(rect.min_y < near_pole.y, "{rect:?}");
}

#[test]
fn over_keeps_a_longitude_past_the_antimeridian_on_the_way_in_and_out() {
    // Verifies: ADR-0037
    let over = "+proj=latlong +datum=WGS84 +over";
    let merc_over = "+proj=merc +datum=WGS84 +over";
    let east = way(over, merc_over).point(at(190.0, 10.0)).unwrap();
    let west = way(LONGLAT_WGS84, "+proj=merc +datum=WGS84")
        .point(at(-170.0, 10.0))
        .unwrap();
    assert!(
        (east.x - (west.x + 2.0 * 20_037_508.342_789_244)).abs() < 1e-6,
        "{east:?}"
    );
    assert_eq!(east.y, west.y);
    let back = way(merc_over, over).point(east).unwrap();
    assert!(
        (back.x - 190.0).abs() < 1e-9 && (back.y - 10.0).abs() < 1e-9,
        "{back:?}"
    );
    // Without it the longitude comes back within ±180°.
    let wrapped = way("+proj=merc +datum=WGS84", LONGLAT_WGS84)
        .point(east)
        .unwrap();
    assert!((wrapped.x + 170.0).abs() < 1e-9, "{wrapped:?}");
}

/// The extent of `misc/rot_out.map` grown to its image of 400 × 300, as `msAdjustExtent` grows
/// it.
const ROT_OUT: Extent = Extent {
    min_x: 478_153.478_260_869_57,
    min_y: 4_762_880.0,
    max_x: 481_796.521_739_130_43,
    max_y: 4_765_610.0,
};

fn rot_out() -> Geotransform {
    Geotransform::rotated_map(&ROT_OUT, 400, 300, 30.0).unwrap()
}

#[test]
fn a_rotated_maps_geotransform_is_mapservers_to_the_bit() {
    // Verifies: ADR-0045
    // misc/rot_out.map, ANGLE 30, as msMapComputeGeotransform and msMapSetFakedExtent compute it
    // in IEEE doubles.
    let map = geotransform::map_geotransform(&ROT_OUT, 400, 300, 30.0).unwrap();
    assert_eq!(
        map,
        [
            477_708.779_697_437_1,
            7.907_188_469_336_169,
            4.565_217_391_304_347,
            4_764_518.034_792_14,
            4.565_217_391_304_341,
            -7.907_188_469_336_179
        ]
    );
    let flipped = rot_out();
    let terms: Vec<f64> = (0..6).map(|i| flipped.term(i)).collect();
    assert_eq!(
        terms,
        [
            479_078.344_914_828_37,
            7.907_188_469_336_169,
            -4.565_217_391_304_347,
            4_762_145.878_251_339,
            4.565_217_391_304_341,
            7.907_188_469_336_179
        ]
    );
    let inverse: Vec<f64> = (0..6).map(|i| flipped.inverse_term(i)).collect();
    assert_eq!(
        inverse,
        [
            -306_224.952_348_443_4,
            0.094_850_401_366_867_25,
            0.054_761_904_761_904_83,
            -425_456.205_221_994_66,
            -0.054_761_904_761_904_76,
            0.094_850_401_366_867_11
        ]
    );
    // MapServer computes none for an image one pixel wide or an extent of no width.
    assert_eq!(Geotransform::rotated_map(&ROT_OUT, 1, 300, 30.0), None);
    let flat = Extent {
        max_x: ROT_OUT.min_x,
        ..ROT_OUT
    };
    assert_eq!(Geotransform::rotated_map(&flat, 400, 300, 30.0), None);
}

#[test]
fn a_point_in_the_maps_projection_lands_where_the_geotransform_puts_it() {
    // Verifies: ADR-0045
    // The extent's centre lands on (200, 150.00000000017462) of the frame, y up, as the C's
    // formulas give it; an unrotated map puts it on (199.5, 149.5).
    let way = Reprojection::identity(false, false).onto_rotated_map(rot_out());
    let centre = way.point(at(479_975.0, 4_764_245.0)).unwrap();
    assert_eq!(centre.x, 200.0);
    assert_eq!(300.0 - centre.y, 150.000_000_000_174_62);
    let corner = way.point(at(478_315.531_25, 4_762_880.5)).unwrap();
    assert!(
        (corner.x + 32.123_896_040_895_49).abs() < 1e-9,
        "{corner:?}"
    );
    assert!(
        (300.0 - corner.y - 188.547_703_022_370_1).abs() < 1e-9,
        "{corner:?}"
    );
    // The way back moves the frame by the geotransform alone.
    let back = way.reversed().point(at(0.0, 0.0)).unwrap();
    assert_eq!(back, at(479_078.344_914_828_37, 4_762_145.878_251_339));
}

#[test]
fn the_area_asked_of_a_rotated_map_holds_the_corners_of_its_view() {
    // Verifies: ADR-0045
    // msProjectRect samples the faked extent 0, 0, 400, 300 and moves each sample by the map's
    // geotransform; the PROJ step from the CRS into itself runs no formula.
    let asked = Reprojection::identity(false, false)
        .onto_rotated_map(rot_out())
        .reversed()
        .extent(&Extent {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 400.0,
            max_y: 300.0,
        })
        .unwrap();
    assert_eq!(
        asked,
        Extent {
            min_x: 477_708.779_697_437_1,
            min_y: 4_762_145.878_251_339,
            max_x: 482_241.220_302_562_8,
            max_y: 4_766_344.121_748_662,
        }
    );
}

#[test]
fn a_rotated_map_skips_the_polar_branches_of_the_area_asked() {
    // Verifies: ADR-0045
    // A map in a polar ortho, rotated by -45°, its view placed so that the frame's origin lands a
    // tenth of a millimetre east of the pole: msProjectRect's test of a polar projection would
    // find the pole there and take every longitude up to 90°, but it does not run where a side
    // holds a geotransform. The ring of the view, which lies between the meridians 45° and 135°
    // east, is reprojected with +over as it is.
    let polar = "+proj=ortho +lat_0=90 +lon_0=0 +datum=WGS84";
    let near = Extent {
        min_x: 1_000_000.0,
        min_y: 1_000_000.0,
        max_x: 2_000_000.0,
        max_y: 2_000_000.0,
    };
    let origin = Geotransform::rotated_map(&near, 101, 101, -45.0)
        .unwrap()
        .forward(at(0.0, 0.0));
    let shift = at(origin.x - 1e-4, origin.y);
    let view = Extent {
        min_x: near.min_x - shift.x,
        min_y: near.min_y - shift.y,
        max_x: near.max_x - shift.x,
        max_y: near.max_y - shift.y,
    };
    let gt = Geotransform::rotated_map(&view, 101, 101, -45.0).unwrap();
    let to_layer = way(LONGLAT_WGS84, polar).onto_rotated_map(gt).reversed();
    // The frame's origin lies on the pole, as the polar test finds it.
    let pole = to_layer.point(at(0.0, 0.0)).unwrap();
    assert!((pole.y - 90.0).abs() < 1e-8, "{pole:?}");
    let faked = Extent {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 101.0,
        max_y: 101.0,
    };
    let asked = to_layer.extent(&faked).unwrap();
    assert!(
        asked.min_x > 40.0 && asked.max_x < 140.0,
        "not every longitude: {asked:?}"
    );
    // Without a geotransform, a view about the pole takes every longitude up to it.
    let about_pole = Extent {
        min_x: -1_000_000.0,
        min_y: -1_000_000.0,
        max_x: 1_000_000.0,
        max_y: 1_000_000.0,
    };
    let unrotated = way(polar, LONGLAT_WGS84).extent(&about_pole).unwrap();
    assert_eq!(
        (unrotated.min_x, unrotated.max_x, unrotated.max_y),
        (-180.0, 180.0, 90.0)
    );
}

#[test]
fn the_dateline_wrap_into_a_rotated_map_is_measured_in_the_frame() {
    // Verifies: ADR-0045
    // A map in longitude and latitude, 2° a pixel. A line from 170° to 190° in a Mercator of
    // +over comes back at -170°, more than 180° from where it started. Rotated by 30°, term 2 of
    // the geotransform is not 0, so msProjectShapeLine measures the distance in the frame's
    // pixels, about 147, and wraps nothing. Rotated by 0°, it measures it through term 1, in
    // degrees, and wraps by 360° in pixels.
    let map = Extent {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 200.0,
        max_y: 100.0,
    };
    let merc_over = "+proj=merc +datum=WGS84 +over";
    let line: Vec<Coord> = [170.0, 190.0]
        .iter()
        .map(|&lon| {
            way("+proj=latlong +datum=WGS84 +over", merc_over)
                .point(at(lon, 10.0))
                .unwrap()
        })
        .collect();
    let into = |angle: f64| {
        let gt = Geotransform::rotated_map(&map, 101, 51, angle).unwrap();
        (way(merc_over, LONGLAT_WGS84).onto_rotated_map(gt), gt)
    };
    let (rotated, gt) = into(30.0);
    let shape = rotated.shape(std::slice::from_ref(&line), false);
    assert_eq!(shape.lines[0][1], gt.inverse(at(-170.0, 10.0)));
    let (unturned, gt) = into(0.0);
    let shape = unturned.shape(std::slice::from_ref(&line), false);
    let unwrapped = gt.inverse(at(-170.0, 10.0));
    assert_eq!(
        shape.lines[0][1].x,
        unwrapped.x + 360.0 * gt.inverse_term(1)
    );
}
