use chrono::{DateTime, Utc};
use console::{style, Term};
use geo::prelude::*;
use geo::simplifyvw::SimplifyVWPreserve;
use geo::{Coordinate, LineString};
use geojson::GeoJson;
use gpx::{read, Gpx, Track};
use piet_common::kurbo::{Line, Rect};
use piet_common::{Color, Device, RenderContext};
use serde_json::json;
use std::error::Error;
use std::fs;
use std::io::BufReader;
use std::vec::IntoIter;

const HEIGHT: usize = 1000; // uphill & downhill
const DISTANCE_BASE: f64 = 5.0;
const DRAW_RATIO: f64 = 3.0;

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

pub async fn open(path: &str) -> Result<Gpx, Box<dyn Error>> {
    let gpx: Gpx = {
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        read(reader)?
    };

    Ok(gpx)
}

pub async fn parse(gpx: &Gpx) -> Result<(GpxInfo, LineString<f64>), Box<dyn Error>> {
    let mut info = GpxInfo::new();

    {
        let Track {
            name, description, ..
        } = &gpx.tracks[0];

        info.name = match &gpx.metadata {
            Some(metadata) if metadata.name.is_some() => metadata.name.clone(),
            _ => name.clone(),
        };

        info.description = match &gpx.metadata {
            Some(metadata) if metadata.description.is_some() => metadata.description.clone(),
            _ => description.clone(),
        };
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
                    previous_waypoint.point().x(),
                    previous_waypoint.point().y(),
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
                                        .replace('\"', ""),
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
                info.distance += geodesic_distance;

                if let Some(elevation) = current_waypoint.elevation {
                    elevation_shape.push(Coordinate {
                        x: info.distance,
                        y: elevation,
                    })
                }
                previous_waypoint = current_waypoint;
            }
        }
    }

    // Simplified representation of the elevation, using the Visvalingam-Whyatt algorithm.
    // This is the triangle area = 0.5 * b * h
    // Length: 100m?
    // Uphill: uphill / (distance / 2) * 100
    // epsilon = 0.5 * length * uphill
    // ? TODO Dynamic uphill according to the past distance?
    let epsilon = 0.5 * DISTANCE_BASE * (info.uphill / (info.distance / 2.) * DISTANCE_BASE);
    let line_string: LineString<f64> = elevation_shape.into();
    let simplified = line_string.simplifyvw_preserve(&epsilon);

    let mut simplifier_iter: IntoIter<Coordinate<f64>> = simplified.clone().into_iter();
    let mut previous = simplifier_iter.next().unwrap();

    for current_simpl in simplifier_iter {
        let diff = current_simpl.y - previous.y;
        if diff >= 0.0 {
            info.uphill += diff;
        } else {
            info.downhill -= diff;
        }

        previous = current_simpl;
    }

    Ok((info, simplified))
}

pub async fn draw(line: &LineString<f64>, info: &GpxInfo) -> Result<(), Box<dyn Error>> {
    // Graphics
    let mut device = Device::new().unwrap();
    let mut bitmap = device
        .bitmap_target((info.distance / DRAW_RATIO) as usize, HEIGHT, 1.0)
        .unwrap();
    let mut ctx = bitmap.render_context();

    ctx.fill(
        Rect::new(0., 0., (info.distance / DRAW_RATIO) as f64, HEIGHT as f64),
        &Color::WHITE,
    );

    let mut simplifier_iter: IntoIter<Coordinate<f64>> = line.clone().into_iter();
    let mut previous = simplifier_iter.next().unwrap();

    for current_simpl in simplifier_iter {
        ctx.stroke(
            Line::new(
                (previous.x / DRAW_RATIO, HEIGHT as f64 - previous.y),
                (
                    current_simpl.x / DRAW_RATIO,
                    HEIGHT as f64 - current_simpl.y,
                ),
            ),
            &Color::FUCHSIA,
            1.0,
        );

        previous = current_simpl;
    }

    ctx.finish().unwrap();
    std::mem::drop(ctx);

    bitmap
        .save_to_file("temp-image.png")
        .expect("file save error");

    Ok(())
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
