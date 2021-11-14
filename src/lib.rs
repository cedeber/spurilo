use chrono::{DateTime, Utc};
use console::{style, Term};
use geo::prelude::*;
use geojson::GeoJson;
use gpx::{read, Gpx, Track};
use serde_json::json;
use std::error::Error;
use std::fs;
use std::io::BufReader;

pub async fn open(path: &str) -> Result<(), Box<dyn Error>> {
    let term = Term::stdout();
    let file = fs::File::open(path).unwrap();
    let reader = BufReader::new(file);
    let gpx: Gpx = read(reader).unwrap();

    let mut name: Option<String> = None;
    let mut description: Option<String> = None;
    let mut datetime: Option<DateTime<Utc>> = None;
    let mut location: Option<String> = None;
    let mut distance = 0.0;
    let mut uphill = 0.0;
    let mut downhill = 0.0;

    let metadata = gpx.metadata;
    //println!("Metadata {:#?}", metadata);

    if let Some(metadata) = metadata {
        name = metadata.name;
        description = metadata.description;
    }

    // The name is usually saved on the first track (if not in metadata)
    let track: &Track = &gpx.tracks[0];
    //println!("Name 1st track: {:?}", track.name);
    if name.is_none() {
        name = track.name.clone();
    }

    if description.is_none() {
        description = track.description.clone();
    }

    for track in gpx.tracks.iter() {
        for segment in track.segments.iter() {
            let mut waypoints_iter = segment.points.iter();
            let mut previous_waypoint = waypoints_iter.next().unwrap();

            if datetime.is_none() {
                datetime = previous_waypoint.time
            }

            if location.is_none() {
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

                                location = Some(
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

                // thresholds
                // TODO probably also take speed into account?
                if geodesic_distance > 3.0
                    || (elevation_diff.is_some() && elevation_diff.unwrap() > 3.0)
                {
                    // distance
                    distance += geodesic_distance;

                    // elevation
                    if previous_waypoint.elevation.is_some() && current_waypoint.elevation.is_some()
                    {
                        let previous_elevation = previous_waypoint.elevation.unwrap();
                        let current_elevation = current_waypoint.elevation.unwrap();
                        let diff = current_elevation - previous_elevation;

                        if diff >= 0. {
                            uphill += diff
                        } else {
                            downhill -= diff
                        }
                    }

                    previous_waypoint = current_waypoint;
                }
            }
        }
    }

    term.write_line(&format!(
        "{}",
        &style(name.unwrap_or_else(|| "".to_string())).blue().bold()
    ))?;
    if let Some(desc) = description {
        term.write_line(&desc)?;
    }
    if let Some(date) = datetime {
        term.write_line("")?;
        term.write_line(&format!(
            "{: <15} {:?}",
            &style("Date & Time").bold().dim(),
            date
        ))?;
    }
    if let Some(loc) = location {
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
        distance as i32
    ))?;
    term.write_line(&format!(
        "{: <15} {}m",
        &style("Uphill").bold().dim(),
        uphill as i32
    ))?;
    term.write_line(&format!(
        "{: <15} {}m",
        &style("Downhill").bold().dim(),
        downhill as i32
    ))?;

    Ok(())
}
