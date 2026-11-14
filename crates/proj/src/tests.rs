//! Tests of the reprojection against the worked examples of IOGP's Guidance Note 7-2 and the
//! projections the cases in scope name.

use super::*;

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
    // NAD83's GRS80 and WGS84 part by a tenth of a millimetre at the origin, which the change of
    // ellipsoid that PROJ makes for a zero `towgs84` keeps.
    let projected = way(LONGLAT_WGS84, EPSG_3978)
        .point(at(-95.0, 49.0))
        .unwrap();
    assert!(
        projected.x.abs() < 1e-3 && projected.y.abs() < 1e-3,
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
fn an_unknown_projection_or_parameter_is_an_error_naming_it() {
    // Verifies: ADR-0037
    let error = |text: &str| Projection::parse(text).unwrap_err();
    assert_eq!(
        error("+proj=tmerc +zone=32"),
        Error::Unsupported("+proj=tmerc".to_owned())
    );
    assert_eq!(
        error("+proj=lcc +lat_1=49 +over"),
        Error::Unsupported("+over".to_owned())
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
fn an_extent_reprojects_to_the_rectangle_that_holds_its_sides() {
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
