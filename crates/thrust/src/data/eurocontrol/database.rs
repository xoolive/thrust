//! EUROCONTROL airway database handling.
//!
//! This module provides functionality to load and query an airway database

use std::hash::Hash;
use std::{collections::HashMap, path};

use geodesy::prelude::*;
use once_cell::sync::Lazy;
use serde::Serialize;

use crate::data::field15::{Connector, Field15Element, Modifier, Point};
use crate::data::{
    eurocontrol::aixm::{
        designated_point::{parse_designated_point_zip_file, DesignatedPoint},
        navaid::{parse_navaid_zip_file, Navaid},
        route::{parse_route_zip_file, Route},
        route_segment::{parse_route_segment_zip_file, PointReference, RouteSegment},
    },
    field15::{Altitude, Speed},
};

/**
 * An airway database containing navaids, designated points, route segments, and routes.
 */
pub struct AirwayDatabase {
    navaids: HashMap<String, Navaid>,
    designated_points: HashMap<String, DesignatedPoint>,
    route_segments: HashMap<String, RouteSegment>,
    routes: HashMap<String, Route>,
}

impl AirwayDatabase {
    /// Load the airway database from the specified directory path.
    pub fn new(path: &path::Path) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(AirwayDatabase {
            navaids: parse_navaid_zip_file(path.join("Navaid.BASELINE.zip"))?,
            designated_points: parse_designated_point_zip_file(path.join("DesignatedPoint.BASELINE.zip"))?,
            route_segments: parse_route_segment_zip_file(path.join("RouteSegment.BASELINE.zip"))?,
            routes: parse_route_zip_file(path.join("Route.BASELINE.zip"))?,
        })
    }
}

const VALID_ROUTE_PREFIXES: [&str; 32] = [
    "UN", "UM", "UL", "UT", "UZ", "UY", "UP", "UA", "UB", "UG", "UH", "UJ", "UQ", "UR", "UV", "UW", "L", "A", "B", "G",
    "H", "J", "Q", "R", "T", "V", "W", "Y", "Z", "M", "N", "P",
];

/// The WGS84 ellipsoid.
static WGS84: Lazy<Ellipsoid> = Lazy::new(|| Ellipsoid::named("WGS84").unwrap());

/**
 * A resolved route consisting of candidate route segments, based on their name.
 */
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedRoute {
    pub segments: Vec<ResolvedRouteSegment>,
    pub name: String,
}

/**
 * A resolved route segment consisting of start and end points.
 * Optionally, altitude and speed constraints can be included, propagated from modifiers.
 */
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedRouteSegment {
    pub start: ResolvedPoint,
    pub end: ResolvedPoint,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitude: Option<Altitude>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<Speed>,
}

/**
 * A resolved point (based on their name), which can be a navaid, designated point, coordinates, or None.
 */
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ResolvedPoint {
    Navaid(Navaid),
    DesignatedPoint(DesignatedPoint),
    Coordinates { latitude: f64, longitude: f64 },
    None,
}

impl std::fmt::Display for ResolvedPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolvedPoint::Navaid(navaid) => write!(
                f,
                "Navaid({}: {:.3},{:.3})",
                navaid.name.as_ref().unwrap(),
                navaid.latitude,
                navaid.longitude
            ),
            ResolvedPoint::DesignatedPoint(dp) => write!(
                f,
                "DesignatedPoint({}: {:.3}, {:.3})",
                dp.designator, dp.latitude, dp.longitude
            ),
            ResolvedPoint::Coordinates { latitude, longitude } => {
                write!(f, "Coordinates(lat: {}, lon: {})", latitude, longitude)
            }
            ResolvedPoint::None => write!(f, "None"),
        }
    }
}

impl From<&ResolvedPoint> for Coor2D {
    fn from(val: &ResolvedPoint) -> Self {
        match val {
            ResolvedPoint::Navaid(navaid) => Coor2D::geo(navaid.latitude, navaid.longitude),
            ResolvedPoint::DesignatedPoint(dp) => Coor2D::geo(dp.latitude, dp.longitude),
            ResolvedPoint::Coordinates { latitude, longitude } => Coor2D::geo(*latitude, *longitude),
            ResolvedPoint::None => Coor2D::default(),
        }
    }
}

impl PartialEq for ResolvedPoint {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ResolvedPoint::Navaid(a), ResolvedPoint::Navaid(b)) => a.identifier == b.identifier,
            (ResolvedPoint::DesignatedPoint(a), ResolvedPoint::DesignatedPoint(b)) => a.identifier == b.identifier,
            (
                ResolvedPoint::Coordinates {
                    latitude: a_lat,
                    longitude: a_lon,
                },
                ResolvedPoint::Coordinates {
                    latitude: b_lat,
                    longitude: b_lon,
                },
            ) => (a_lat - b_lat).abs() < f64::EPSILON && (a_lon - b_lon).abs() < f64::EPSILON,
            (ResolvedPoint::None, ResolvedPoint::None) => true,
            _ => false,
        }
    }
}

impl Eq for ResolvedPoint {}

// The hash trait implementation is needed for the DFS algorithm to reconstruct paths
impl Hash for ResolvedPoint {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            ResolvedPoint::Navaid(navaid) => {
                navaid.identifier.hash(state);
            }
            ResolvedPoint::DesignatedPoint(dp) => {
                dp.identifier.hash(state);
            }
            ResolvedPoint::Coordinates { latitude, longitude } => {
                latitude.to_bits().hash(state);
                longitude.to_bits().hash(state);
            }
            ResolvedPoint::None => {
                0.hash(state);
            }
        }
    }
}

impl ResolvedPoint {
    /// Resolve a point from the database.
    pub fn from_db(point: &PointReference, db: &AirwayDatabase) -> Self {
        match point {
            PointReference::Navaid(id) => {
                if let Some(navaid) = db.navaids.get(id) {
                    ResolvedPoint::Navaid(navaid.clone())
                } else {
                    ResolvedPoint::None
                }
            }
            PointReference::DesignatedPoint(id) => {
                if let Some(dp) = db.designated_points.get(id) {
                    ResolvedPoint::DesignatedPoint(dp.clone())
                } else {
                    ResolvedPoint::None
                }
            }
            PointReference::None => ResolvedPoint::None,
        }
    }
    /// Resolve a point by its name from the database.
    pub fn lookup(name: &str, db: &AirwayDatabase) -> Vec<Self> {
        let candidates = db
            .navaids
            .values()
            .filter(|n| {
                n.name
                    .as_deref()
                    .is_some_and(|n_name| n_name.trim().eq_ignore_ascii_case(name))
            })
            .collect::<Vec<_>>();
        if !candidates.is_empty() {
            return candidates.iter().map(|n| ResolvedPoint::Navaid((*n).clone())).collect();
        }
        let candidates = db
            .designated_points
            .values()
            .filter(|dp| dp.designator.trim().eq_ignore_ascii_case(name))
            .collect::<Vec<_>>();
        if !candidates.is_empty() {
            return candidates
                .iter()
                .map(|dp| ResolvedPoint::DesignatedPoint((*dp).clone()))
                .collect();
        }
        vec![]
    }
}

impl ResolvedRouteSegment {
    /// Resolve a route segment from the database.
    pub fn from_db(segment: &RouteSegment, db: &AirwayDatabase) -> Self {
        ResolvedRouteSegment {
            start: ResolvedPoint::from_db(&segment.start, db),
            end: ResolvedPoint::from_db(&segment.end, db),
            name: None,
            altitude: None,
            speed: None,
        }
    }
}

impl ResolvedRoute {
    /// Resolve a route from the database.
    pub fn from_db(route: &Route, db: &AirwayDatabase) -> Self {
        let segments = db
            .route_segments
            .values()
            .filter(|segment| segment.route_formed.as_deref() == Some(&route.identifier))
            .map(|segment| ResolvedRouteSegment::from_db(segment, db))
            .collect::<Vec<_>>();
        ResolvedRoute {
            segments,
            name: format!(
                "{}{}{}",
                route.prefix.as_deref().unwrap_or(""),
                route.second_letter.as_deref().unwrap_or(""),
                route.number.as_deref().unwrap_or("")
            ),
        }
    }

    /// Lookup routes by their name from the database.
    pub fn lookup(name: &str, db: &AirwayDatabase) -> Vec<Self> {
        if VALID_ROUTE_PREFIXES.iter().any(|prefix| name.starts_with(prefix)) {
            // First decompose the name into its components
            // Another approach would be to make a single string match,
            // but this serves as sanity check as well.
            let last = name.chars().last().unwrap();
            let (name, multiple) = if last.is_alphabetic() {
                (&name[..name.len() - 1], Some(last.to_string()))
            } else {
                (name, None)
            };
            let (prefix, second_letter, number) = if name.starts_with('U') && name.len() >= 3 {
                (
                    Some("U".to_string()),
                    name.chars().nth(1).unwrap().to_string(),
                    name[2..].to_string(),
                )
            } else if name.len() >= 2 {
                (None, name.chars().next().unwrap().to_string(), name[1..].to_string())
            } else {
                (None, "".to_string(), "".to_string())
            };
            let candidates = db
                .routes
                .values()
                .filter(|route| {
                    route.prefix.as_deref() == prefix.as_deref()
                        && route.second_letter.as_deref() == Some(&second_letter)
                        && route.number.as_deref() == Some(&number)
                        && route.multiple_identifier.as_deref() == multiple.as_deref()
                })
                .collect::<Vec<_>>();
            return candidates
                .iter()
                .map(|route| ResolvedRoute::from_db(route, db))
                .collect();
        }
        vec![]
    }

    /// Check if the route contains the specified point.
    pub fn contains(&self, point: &ResolvedPoint) -> bool {
        self.segments
            .iter()
            .any(|segment| &segment.start == point || &segment.end == point)
    }

    /// Find a sub-route between two points, if it exists.
    /// The implementation uses a depth-first search (DFS) algorithm to find a path
    /// between the start and end points within the route segments.
    pub fn between(&self, start: &ResolvedPoint, end: &ResolvedPoint) -> Option<ResolvedRoute> {
        // Build adjacency map: point -> list of (next_point, segment_index, is_forward)
        let mut graph: HashMap<&ResolvedPoint, Vec<(&ResolvedPoint, usize, bool)>> = HashMap::new();

        for (i, segment) in self.segments.iter().enumerate() {
            // Forward direction: start -> end
            graph.entry(&segment.start).or_default().push((&segment.end, i, true));

            // Backward direction: end -> start
            graph.entry(&segment.end).or_default().push((&segment.start, i, false));
        }

        // Try DFS from start to end
        if let Some(path) = Self::dfs_helper(
            &graph,
            start,
            end,
            &mut Vec::new(),
            &mut std::collections::HashSet::new(),
        ) {
            return Some(self.build_route_from_path(path));
        }

        None
    }

    fn dfs_helper<'a>(
        graph: &HashMap<&'a ResolvedPoint, Vec<(&'a ResolvedPoint, usize, bool)>>,
        current: &'a ResolvedPoint,
        target: &'a ResolvedPoint,
        path: &mut Vec<(usize, bool)>,
        visited: &mut std::collections::HashSet<usize>,
    ) -> Option<Vec<(usize, bool)>> {
        if current == target {
            return Some(path.clone());
        }

        if let Some(neighbors) = graph.get(current) {
            for (next_point, segment_idx, is_forward) in neighbors {
                if !visited.contains(segment_idx) {
                    visited.insert(*segment_idx);
                    path.push((*segment_idx, *is_forward));

                    if let Some(result) = Self::dfs_helper(graph, next_point, target, path, visited) {
                        return Some(result);
                    }

                    path.pop();
                    visited.remove(segment_idx);
                }
            }
        }

        None
    }

    fn build_route_from_path(&self, path: Vec<(usize, bool)>) -> ResolvedRoute {
        let mut segments = Vec::new();

        for (segment_idx, is_forward) in path {
            let segment = &self.segments[segment_idx];
            if is_forward {
                segments.push(segment.clone());
            } else {
                // Reverse the segment
                segments.push(ResolvedRouteSegment {
                    start: segment.end.clone(),
                    end: segment.start.clone(),
                    name: Some(self.name.clone()),
                    altitude: segment.altitude.clone(),
                    speed: segment.speed.clone(),
                });
            }
        }

        ResolvedRoute {
            segments,
            name: self.name.clone(),
        }
    }
}

#[derive(Debug)]
enum EnrichedCandidates {
    Point((Vec<ResolvedPoint>, Option<Altitude>, Option<Speed>)),
    PointCoords((ResolvedPoint, Option<Altitude>, Option<Speed>)),
    Airway((Vec<ResolvedRoute>, String, Option<Altitude>, Option<Speed>)),
    Direct(),
}

impl AirwayDatabase {
    /// Enrich a sequence of Field15Elements into resolved route segments.
    /// A resolved route segment consists of start and end points,
    /// along with optional altitude and speed constraints.
    /// All points and airways are resolved against the database.
    pub fn enrich_route(&self, elements: Vec<Field15Element>) -> Vec<ResolvedRouteSegment> {
        let mut altitude = None;
        let mut speed = None;

        // First, resolve all candidates
        let mut resolved: Vec<EnrichedCandidates> = Vec::new();
        for element in &elements {
            match element {
                Field15Element::Modifier(m) => {
                    let Modifier {
                        speed: s, altitude: a, ..
                    } = m;
                    altitude = a.clone();
                    speed = s.clone();
                }
                Field15Element::Point(Point::Waypoint(name)) => {
                    let lookup = ResolvedPoint::lookup(name, self);
                    if lookup.is_empty() {
                        tracing::warn!("No point found for identifier '{}'", name);
                    }
                    resolved.push(EnrichedCandidates::Point((
                        ResolvedPoint::lookup(name, self),
                        altitude.clone(),
                        speed.clone(),
                    )));
                }
                Field15Element::Point(Point::Coordinates((lat, lon))) => {
                    resolved.push(EnrichedCandidates::PointCoords((
                        ResolvedPoint::Coordinates {
                            latitude: *lat,
                            longitude: *lon,
                        },
                        altitude.clone(),
                        speed.clone(),
                    )));
                }
                Field15Element::Connector(Connector::Airway(name)) => {
                    let lookup = ResolvedRoute::lookup(name, self);
                    if lookup.is_empty() {
                        tracing::warn!("No airway found for identifier '{}'", name);
                        resolved.push(EnrichedCandidates::Direct());
                    } else {
                        resolved.push(EnrichedCandidates::Airway((
                            lookup,
                            name.to_string(),
                            altitude.clone(),
                            speed.clone(),
                        )));
                    }
                }
                Field15Element::Connector(Connector::Direct) => {
                    resolved.push(EnrichedCandidates::Direct());
                }
                Field15Element::Connector(Connector::Nat(_)) | Field15Element::Connector(Connector::Pts(_)) => {
                    // NAT and PTS are not handled yet
                    resolved.push(EnrichedCandidates::Direct());
                }
                _ => {}
            }
        }

        // 1. For each candidate airway, retain only those that contain both the previous and next point.
        for i in 1..resolved.len() - 1 {
            let (before_i, i_and_after) = &mut resolved.split_at_mut(i);
            if let (EnrichedCandidates::Airway((routes, _, _, _)), after_i) = i_and_after.split_first_mut().unwrap() {
                tracing::debug!("Filtering airway candidates: {:?}", routes);
                if let Some(EnrichedCandidates::Point((points, _, _))) = before_i.last() {
                    routes.retain(|r| points.iter().any(|p| r.contains(p)));
                    tracing::debug!("Filtering airway candidates with point {:?}: {:?}", points, routes);
                }
                if let Some(EnrichedCandidates::Point((points, _, _))) = after_i.first() {
                    routes.retain(|r| points.iter().any(|p| r.contains(p)));
                    tracing::debug!("Filtering airway candidates with point {:?}: {:?}", points, routes);
                }
            }
        }

        // 2. Transform now empty airway candidates to Direct
        for candidate in resolved.iter_mut() {
            if let EnrichedCandidates::Airway((routes, name, _, _)) = candidate {
                if routes.is_empty() {
                    tracing::warn!("No valid airway remaining for '{}'", name);
                    *candidate = EnrichedCandidates::Direct();
                }
            }
        }

        // 3. For each point, retain only those that are present in the adjacent airway segments.
        for i in 0..resolved.len() {
            let (before_i, i_and_after) = &mut resolved.split_at_mut(i);
            if let (EnrichedCandidates::Point((points, _, _)), after_i) = i_and_after.split_first_mut().unwrap() {
                tracing::debug!("Filtering point candidates: {:?}", points);
                if let Some(EnrichedCandidates::Airway((routes, _, _, _))) = before_i.last() {
                    points.retain(|p| routes.iter().any(|r| r.contains(p)));
                    tracing::debug!("Filtering point candidates with airway {:?}: {:?}", routes, points);
                }
                if let Some(EnrichedCandidates::Airway((routes, _, _, _))) = after_i.first() {
                    points.retain(|p| routes.iter().any(|r| r.contains(p)));
                    tracing::debug!("Filtering point candidates with airway {:?}: {:?}", routes, points);
                }
            }
        }

        // 4. Trim airways to the segments between the before and after points.
        for i in 1..resolved.len() - 1 {
            let (before_i, i_and_after) = &mut resolved.split_at_mut(i);
            if let (EnrichedCandidates::Airway((routes, _, _, _)), after_i) = i_and_after.split_first_mut().unwrap() {
                if let Some(EnrichedCandidates::Point((before, _, _))) = before_i.last() {
                    if let Some(EnrichedCandidates::Point((after, _, _))) = after_i.first() {
                        if let Some(before) = before.first() {
                            if let Some(after) = after.first() {
                                for route in routes.iter_mut() {
                                    if let Some(trimmed) = route.between(before, after) {
                                        *route = trimmed;
                                        tracing::debug!(
                                            "Trimmed airway '{}' between points {} and {}: {:?}",
                                            route.name,
                                            before,
                                            after,
                                            *route
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 5. Replace empty routes with Direct
        for candidate in resolved.iter_mut() {
            if let EnrichedCandidates::Airway((routes, name, _, _)) = candidate {
                if routes.iter().all(|r| r.segments.is_empty()) {
                    tracing::warn!("No valid segments remaining for airway '{}'", name);
                    *candidate = EnrichedCandidates::Direct();
                }
            }
        }

        // 6. Break the tie for remaining multiple candidate points
        let mut last_known: Option<ResolvedPoint> = None;

        for i in 0..resolved.len() {
            if let EnrichedCandidates::Point((points, _, _)) = &resolved[i] {
                if points.len() > 1 {
                    // Find the next definitive point ahead
                    let mut next_definitive: Option<&ResolvedPoint> = None;
                    for candidate in resolved[i..].iter() {
                        match candidate {
                            EnrichedCandidates::Point((pts, _, _)) if pts.len() == 1 => {
                                next_definitive = pts.first();
                                break;
                            }
                            EnrichedCandidates::PointCoords((pt, _, _)) => {
                                next_definitive = Some(pt);
                                break;
                            }
                            _ => {}
                        }
                    }

                    match (&last_known, next_definitive) {
                        (None, None) => {
                            tracing::warn!("Cannot disambiguate point {:?}: no reference points available", points);
                        }
                        (None, Some(_)) => {
                            tracing::info!("Disambiguating point {:?} using only next definitive point", points);
                        }
                        (Some(a), None) => {
                            tracing::info!("Disambiguating point {:?} using only last known point", points);

                            // Only last known point is available, pick the closest candidate
                            let mut best_idx = 0;
                            let mut best_distance = f64::INFINITY;

                            for (idx, candidate) in points.iter().enumerate() {
                                let distance =
                                    WGS84.distance(&Into::<Coor2D>::into(a), &Into::<Coor2D>::into(candidate));
                                if distance < best_distance {
                                    best_distance = distance;
                                    best_idx = idx;
                                }
                            }

                            // Keep only the best candidate
                            if let EnrichedCandidates::Point((points, _, _)) = &mut resolved[i] {
                                let best = points[best_idx].clone();
                                points.clear();
                                points.push(best);
                            }
                        }
                        (Some(a), Some(b)) => {
                            tracing::info!("Disambiguating point {:?} using both reference points", points);

                            let mut best_idx = 0;
                            let mut best_score = f64::INFINITY;

                            for (idx, candidate) in points.iter().enumerate() {
                                tracing::info!("Scoring candidate {}: {} ({}-{})", idx, candidate, a, b);
                                let score = score_hybrid(&a.into(), &b.into(), &candidate.into());
                                if score < best_score {
                                    best_score = score;
                                    best_idx = idx;
                                }
                            }

                            // Keep only the best candidate
                            if let EnrichedCandidates::Point((points, _, _)) = &mut resolved[i] {
                                let best = points[best_idx].clone();
                                points.clear();
                                points.push(best);
                            }
                        }
                    }
                }

                // Update last_known point
                if let EnrichedCandidates::Point((pts, _, _)) = &resolved[i] {
                    if let Some(pt) = pts.first() {
                        last_known = Some(pt.clone());
                    }
                }
            } else if let EnrichedCandidates::PointCoords((pt, _, _)) = &resolved[i] {
                last_known = Some(pt.clone());
            }
        }

        // 7. Build the final sequence of resolved route segments.
        let mut segments = Vec::new();
        let mut previous_point: Option<ResolvedPoint> = None;

        for enriched in resolved {
            match enriched {
                EnrichedCandidates::Point((points, alt, spd)) => {
                    if let Some(point) = points.first() {
                        if let Some(prev) = &previous_point {
                            if prev == point {
                                continue;
                            }
                            segments.push(ResolvedRouteSegment {
                                start: prev.clone(),
                                end: point.clone(),
                                name: None,
                                altitude: alt,
                                speed: spd,
                            });
                        }
                        previous_point = Some(point.clone());
                    }
                }
                EnrichedCandidates::PointCoords((point, alt, spd)) => {
                    if let Some(prev) = previous_point {
                        segments.push(ResolvedRouteSegment {
                            start: prev,
                            end: point.clone(),
                            name: None,
                            altitude: alt,
                            speed: spd,
                        });
                    }
                    previous_point = Some(point.clone());
                }
                EnrichedCandidates::Airway((routes, name, alt, spd)) => {
                    if let Some(route) = routes.first() {
                        for segment in &route.segments {
                            segments.push(ResolvedRouteSegment {
                                start: segment.start.clone(),
                                end: segment.end.clone(),
                                name: Some(name.clone()),
                                altitude: alt.clone(),
                                speed: spd.clone(),
                            });
                        }
                        previous_point = Some(route.segments.last().unwrap().end.clone());
                    }
                }
                EnrichedCandidates::Direct() => {
                    // Direct segments are handled by just carrying forward the previous point
                }
            }
        }
        segments
    }
}

fn score_hybrid(a: &Coor2D, b: &Coor2D, x: &Coor2D) -> f64 {
    // Ideally gap_ration is close to 1.0 and the bearing difference close to 0.0
    let ab = WGS84.geodesic_inv(a, b).to_degrees();
    let ax = WGS84.geodesic_inv(a, x).to_degrees();
    let xb = WGS84.geodesic_inv(x, b).to_degrees();

    // Think about triangular inequality, we want x to be "between" a and b
    let gap_ratio = (ax[2] + xb[2]) / ab[2].max(1e-9);

    let delta_a = (ax[0] - ab[0]).abs().min(360.0 - (ax[0] - ab[0]).abs());
    let delta_b = (xb[0] - ab[0]).abs().min(360.0 - (xb[0] - ab[0]).abs());
    let bearing_diff = (delta_a + delta_b) / 2.0; // Normalize to [0,1]

    tracing::info!(
        "Scoring point: {} = {} + {}; bearing_diff = {:.3}, gap_ratio = {:.3}",
        ab[2],
        ax[2],
        xb[2],
        bearing_diff,
        gap_ratio
    );
    // Combine the two metrics into a score
    bearing_diff / 180. + (gap_ratio - 1.0).max(0.)
}
