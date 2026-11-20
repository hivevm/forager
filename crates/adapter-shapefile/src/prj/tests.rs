use super::*;

const GCS_WGS84: &str = r#"GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]]"#;
const GCS_NAD83: &str = r#"GEOGCS["GCS_North_American_1983",DATUM["D_North_American_1983",SPHEROID["GRS_1980",6378137.0,298.257222101]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]]"#;

fn read(text: &str) -> DeclaredProjection {
    declared("a.prj", text.as_bytes())
}

fn proj4(text: &str) -> String {
    match read(text) {
        DeclaredProjection::Proj4(p) => p,
        other => panic!("{text}: {other:?}"),
    }
}

fn why(text: &str) -> String {
    match read(text) {
        DeclaredProjection::Unreadable { file, why } => {
            assert_eq!(file, "a.prj");
            why
        }
        other => panic!("{text}: {other:?}"),
    }
}

fn projcs(geogcs: &str, projection: &str, parameters: &[(&str, f64)]) -> String {
    let parameters: String = parameters
        .iter()
        .map(|(name, value)| format!(r#",PARAMETER["{name}",{value:?}]"#))
        .collect();
    format!(r#"PROJCS["p",{geogcs},PROJECTION["{projection}"]{parameters},UNIT["Meter",1.0]]"#)
}

#[test]
fn a_prj_in_longitude_and_latitude_utm_or_a_conic_declares_its_proj4_string() {
    // Verifies: API-source-declares-projection
    assert_eq!(proj4(GCS_WGS84), "+proj=longlat +datum=WGS84 +no_defs");
    assert_eq!(proj4(GCS_NAD83), "+proj=longlat +datum=NAD83 +no_defs");
    let utm = |y0: f64| {
        projcs(
            GCS_WGS84,
            "Transverse_Mercator",
            &[
                ("False_Easting", 500_000.0),
                ("False_Northing", y0),
                ("Central_Meridian", 9.0),
                ("Scale_Factor", 0.9996),
                ("Latitude_Of_Origin", 0.0),
            ],
        )
    };
    assert_eq!(
        proj4(&utm(0.0)),
        "+proj=utm +zone=32 +datum=WGS84 +units=m +no_defs"
    );
    assert_eq!(
        proj4(&utm(10_000_000.0)),
        "+proj=utm +zone=32 +south +datum=WGS84 +units=m +no_defs"
    );
    let lcc = projcs(
        GCS_NAD83,
        "Lambert_Conformal_Conic",
        &[
            ("False_Easting", 0.0),
            ("False_Northing", 0.0),
            ("Central_Meridian", -95.0),
            ("Standard_Parallel_1", 49.0),
            ("Standard_Parallel_2", 77.0),
            ("Latitude_Of_Origin", 49.0),
        ],
    );
    assert_eq!(
        proj4(&lcc),
        "+proj=lcc +lat_0=49 +lon_0=-95 +lat_1=49 +lat_2=77 +x_0=0 +y_0=0 +datum=NAD83 \
         +units=m +no_defs"
    );
    // A transverse Mercator that is no UTM zone keeps its parameters.
    let tmerc = projcs(
        GCS_WGS84,
        "Transverse_Mercator",
        &[("Central_Meridian", 10.5)],
    );
    assert_eq!(
        proj4(&tmerc),
        "+proj=tmerc +lat_0=0 +lon_0=10.5 +k=1 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs"
    );
    // An authority is not followed: the string comes from the parameters.
    let with_authority = format!(
        r#"{},AUTHORITY["EPSG","32632"]]"#,
        utm(0.0).strip_suffix(']').unwrap()
    );
    assert_eq!(
        proj4(&with_authority),
        "+proj=utm +zone=32 +datum=WGS84 +units=m +no_defs"
    );
}

#[test]
fn a_local_cs_declares_no_projection_and_what_does_not_read_says_why() {
    // Verifies: API-source-declares-projection
    assert_eq!(
        read(r#"LOCAL_CS["engineering",UNIT["Meter",1.0]]"#),
        DeclaredProjection::NoProjection
    );
    assert!(why("").contains("keyword"));
    assert!(why("GEOGCS[").contains("GEOGCS"));
    assert!(why(&format!("{GCS_WGS84} x")).contains("follows"));
    let datum = GCS_WGS84.replace("D_WGS_1984", "D_European_1950");
    assert!(why(&datum).contains("D_European_1950"), "{}", why(&datum));
    let pm = GCS_WGS84.replace(
        r#"PRIMEM["Greenwich",0.0]"#,
        r#"PRIMEM["Paris",2.33722917]"#,
    );
    assert!(why(&pm).contains("prime meridian"));
    let feet =
        projcs(GCS_WGS84, "Mercator_1SP", &[]).replace(r#"["Meter",1.0]"#, r#"["Foot",0.3048]"#);
    assert!(why(&feet).contains("linear unit"));
    assert!(why(&projcs(GCS_WGS84, "Albers", &[])).contains("Albers"));
    let deep = "A[".repeat(40) + &"]".repeat(40);
    assert!(why(&deep).contains("deeper"));
    assert!(why("GEOGCS[\"\u{fffd}").contains("quote"));
    assert!(matches!(
        declared("a.prj", &[0xff, 0xfe]),
        DeclaredProjection::Unreadable { .. }
    ));
}

#[test]
fn only_the_first_2047_bytes_of_a_prj_are_read() {
    // Verifies: API-source-declares-projection
    // Padded past what MapServer reads, the WKT is cut short and does not read.
    let padded = format!("{}{GCS_WGS84}", " ".repeat(MAX_PRJ_BYTES));
    assert!(matches!(
        read(&padded),
        DeclaredProjection::Unreadable { .. }
    ));
    let fits = format!("{}{GCS_WGS84}", " ".repeat(MAX_PRJ_BYTES - GCS_WGS84.len()));
    assert_eq!(proj4(&fits), "+proj=longlat +datum=WGS84 +no_defs");
    // A NUL ends the text, as it ends MapServer's C string.
    assert_eq!(
        proj4(&format!("{GCS_WGS84}\0garbage")),
        "+proj=longlat +datum=WGS84 +no_defs"
    );
}

#[test]
fn an_ogc_prj_with_axes_a_scaled_conic_and_esri_s_orthographic_read_as_gdal_reads_them() {
    // Verifies: API-source-declares-projection
    // An OGC WKT, as PostGIS writes one: its AXIS values are bare words.
    let ogc = r#"PROJCS["WGS 84 / UTM zone 32N",GEOGCS["WGS 84",DATUM["WGS_1984",SPHEROID["WGS 84",6378137,298.257223563,AUTHORITY["EPSG","7030"]],AUTHORITY["EPSG","6326"]],PRIMEM["Greenwich",0],UNIT["degree",0.0174532925199433],AXIS["Latitude",NORTH],AXIS["Longitude",EAST]],PROJECTION["Transverse_Mercator"],PARAMETER["latitude_of_origin",0],PARAMETER["central_meridian",9],PARAMETER["scale_factor",0.9996],PARAMETER["false_easting",500000],PARAMETER["false_northing",0],UNIT["metre",1],AXIS["Easting",EAST],AXIS["Northing",NORTH]]"#;
    assert_eq!(
        proj4(ogc),
        "+proj=utm +zone=32 +datum=WGS84 +units=m +no_defs"
    );
    // ESRI's two-parallel conic with a scale factor keeps it.
    let michigan = projcs(
        GCS_NAD83,
        "Lambert_Conformal_Conic",
        &[
            ("False_Easting", 8_000_000.0),
            ("Central_Meridian", -84.36666666666666),
            ("Standard_Parallel_1", 44.18333333333333),
            ("Standard_Parallel_2", 45.7),
            ("Scale_Factor", 1.000_038_2),
            ("Latitude_Of_Origin", 43.31666666666667),
        ],
    );
    assert!(
        proj4(&michigan).contains(" +k_0=1.0000382 "),
        "{}",
        proj4(&michigan)
    );
    // Whether ESRI's spherical Orthographic comes out spherical is not settled: it does not read.
    assert!(why(&projcs(GCS_WGS84, "Orthographic", &[])).contains("Orthographic"));
}
