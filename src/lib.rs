use chrono::{DateTime, Utc};
use console::{style, Term};
use geo::prelude::*;
use geo::{Coordinate, LineString};
use geojson::GeoJson;
use gpx::{read, Gpx, Track};
use serde_json::json;
use std::error::Error;
use std::fs;
use std::io::BufReader;
use std::vec::IntoIter;

pub struct GpxInfo {
    name: Option<String>,
    description: Option<String>,
    datetime: Option<DateTime<Utc>>,
    location: Option<String>,
    distance: f64,
    uphill: f64,
    downhill: f64,
}

impl GpxInfo {
    fn new() -> Self {
        Self {
            name: None,
            description: None,
            datetime: None,
            location: None,
            distance: 0.0,
            uphill: 0.0,
            downhill: 0.0,
        }
    }
}

pub async fn open(path: &str) -> Result<GpxInfo, Box<dyn Error>> {
    let mut info = GpxInfo::new();
    let file = fs::File::open(path).unwrap();
    let reader = BufReader::new(file);
    let gpx: Gpx = read(reader).unwrap();
    let metadata = gpx.metadata;

    if let Some(metadata) = metadata {
        info.name = metadata.name;
        info.description = metadata.description;
    }

    // The name is usually saved on the first track (if not in metadata)
    let track: &Track = &gpx.tracks[0];
    //println!("Name 1st track: {:?}", track.name);
    if info.name.is_none() {
        info.name = track.name.clone();
    }

    if info.description.is_none() {
        info.description = track.description.clone();
    }

    let mut elevation_shape: Vec<Coordinate<f64>> = Vec::new();

    for track in gpx.tracks.iter() {
        for segment in track.segments.iter() {
            let mut waypoints_iter = segment.points.iter();
            let mut previous_waypoint = waypoints_iter.next().unwrap();

            if info.datetime.is_none() {
                info.datetime = previous_waypoint.time
            }

            if info.location.is_none() {
                if let Ok(req) = reqwest::get(&format!(
                    "https://photon.komoot.io/reverse?lon={}&lat={}&limit=1&lang=fr",
                    previous_waypoint.point().lng(),
                    previous_waypoint.point().lat(),
                ))
                .await
                {
                    if let Ok(GeoJson::FeatureCollection(ref ctn)) = req.json::<GeoJson>().await {
                        for feature in &ctn.features {
                            if let Some(ref props) = feature.properties {
                                let default = json!("");
                                let name = props.get("name").unwrap_or(&default);
                                let street = props.get("street").unwrap_or(&default);
                                let city = props.get("city").unwrap_or(&default);
                                let country = props.get("country").unwrap_or(&default);

                                info.location = Some(
                                    format!("{}, {}, {}, {}", name, street, city, country)
                                        .trim()
                                        .to_string()
                                        .replace("\"", ""),
                                );
                            }
                        }
                    }
                }
            }

            for current_waypoint in waypoints_iter {
                let geodesic_distance = previous_waypoint
                    .point()
                    .geodesic_distance(&current_waypoint.point());

                let mut elevation_diff: Option<f64> = None;
                if previous_waypoint.elevation.is_some() && current_waypoint.elevation.is_some() {
                    let previous_elevation = previous_waypoint.elevation.unwrap();
                    let current_elevation = current_waypoint.elevation.unwrap();
                    elevation_diff = Some(current_elevation - previous_elevation);
                }

                if let Some(elevation) = current_waypoint.elevation {
                    elevation_shape.push(Coordinate {
                        x: info.distance,
                        y: elevation,
                    })
                }

                // thresholds
                // TODO probably also take speed into account?
                //      @see https://docs.rs/geo/0.18.0/geo/#simplification
                if geodesic_distance > 3.0
                    || (elevation_diff.is_some() && elevation_diff.unwrap() > 3.0)
                {
                    // distance
                    info.distance += geodesic_distance;

                    // elevation
                    if previous_waypoint.elevation.is_some() && current_waypoint.elevation.is_some()
                    {
                        let previous_elevation = previous_waypoint.elevation.unwrap();
                        let current_elevation = current_waypoint.elevation.unwrap();
                        let diff = current_elevation - previous_elevation;

                        if diff >= 0. {
                            info.uphill += diff
                        } else {
                            info.downhill -= diff
                        }
                    }

                    previous_waypoint = current_waypoint;
                }
            }
        }
    }

    let line_string: LineString<f64> = elevation_shape.clone().into();
    let simplified = line_string.simplify(&1.0);

    let mut simplifier_iter: IntoIter<Coordinate<f64>> = simplified.clone().into_iter();
    let mut simpl_up: f64 = 0.0;
    let mut simpl_down: f64 = 0.0;
    let mut previous = simplifier_iter.next().unwrap();

    for current_simpl in simplifier_iter {
        let diff = current_simpl.y - previous.y;
        if diff >= 0.0 {
            simpl_up += diff
        } else {
            simpl_down -= diff
        }

        previous = current_simpl;
    }

    println!("{:?}", &elevation_shape);
    println!("{:?}", &simplified);
    println!("{:?}", &info.uphill);
    println!("{:?}", &simpl_up);
    println!("{:?}", &simpl_down);

    Ok(info)
}

pub fn print(info: &GpxInfo) -> Result<(), Box<dyn Error>> {
    let term = Term::stdout();
    let default_value = String::from("");

    term.write_line(&format!(
        "{}",
        &style(info.name.as_ref().unwrap_or(&default_value))
            .blue()
            .bold()
    ))?;

    if let Some(desc) = &info.description {
        term.write_line(desc)?;
    }

    if let Some(date) = &info.datetime {
        term.write_line("")?;
        term.write_line(&format!(
            "{: <15} {:?}",
            &style("Date & Time").bold().dim(),
            date
        ))?;
    }

    if let Some(loc) = &info.location {
        term.write_line(&format!(
            "{: <15} {:?}",
            &style("Location").bold().dim(),
            loc
        ))?;
    }

    term.write_line("")?;
    term.write_line(&format!(
        "{: <15} {}m",
        &style("Distance").bold().dim(),
        info.distance as i32
    ))?;
    term.write_line(&format!(
        "{: <15} {}m",
        &style("Uphill").bold().dim(),
        info.uphill as i32
    ))?;
    term.write_line(&format!(
        "{: <15} {}m",
        &style("Downhill").bold().dim(),
        info.downhill as i32
    ))?;

    Ok(())
}
