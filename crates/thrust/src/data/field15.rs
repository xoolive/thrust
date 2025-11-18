//! ## ICAO Field 15 Parser
//!
//! This module parses ICAO Field 15 entries from flight plans.
//! Field 15 contains the route description including speed, altitude, waypoints,
//! airways, and various modifiers.
//!
//! Based on ICAO DOC 4444 specifications and implements the three basic token types:
//! - Points: Published Route Points (PRP), lat/lon coordinates, Point/Bearing/Distance, Aerodrome
//! - Connectors: ATS routes, SID, STAR, DCT, VFR/IFR, OAT/GAT
//! - Modifiers: Speed and Level changes
//!
//! The parser tokenizes the input string, identifies each token type,
//! and constructs a structured representation using Rust enums and structs.
//!
//! # Acknowledgement
//!
//! This parser is inspired by and adapted from the Python implementation
//! available at <https://github.com/pventon/ICAO-F15-Parser/>
//!
//! ## Example
//!
//! The following field 15 entry:
//!
//! `N0490F360 ELCOB6B ELCOB UT300 SENLO UN502 JSY DCT LIZAD DCT MOPAT DCT LUNIG DCT MOMIN DCT PIKIL/M084F380 NATD HOIST/N0490F380 N756C ANATI/N0441F340 DCT MIVAX DCT OBTEK DCT XORLO ROCKT2`
//!
//! becomes a structured sequence which serializes to JSON as follows:
//!
//!   ```json
//!   [
//!     {"speed": {"kts": 490}, "altitude": {"FL": 360}},
//!     {"SID": "ELCOB6B"},
//!     {"waypoint": "ELCOB"},
//!     {"airway": "UT300"},
//!     {"waypoint": "SENLO"},
//!     {"airway": "UN502"},
//!     {"waypoint": "JSY"},
//!     "DCT",
//!     {"waypoint": "LIZAD"},
//!     // (truncated)
//!     {"waypoint": "PIKIL"},
//!     {"speed": {"Mach": 0.84}, "altitude": {"FL": 380}},
//!     {"NAT": "NATD"},
//!     {"waypoint": "HOIST"},
//!     // (truncated)
//!     {"waypoint": "XORLO"},
//!     {"STAR": "ROCKT2"}
//!   ]
//!   ```
//!
//! ## Usage
//!
//! ```rust
//! use thrust::data::field15::{Field15Element, Field15Parser};
//!
//! let elements: Vec<Field15Element> = Field15Parser::parse(&line);
//! match serde_json::to_string(&elements) {
//!     Ok(json) => println!("{}", json),
//!     Err(e) => eprintln!("JSON serialization error: {}", e),
//! }
//! ```
//!

use serde::{Deserialize, Serialize};
use std::fmt;

/// A single element in a Field 15 ICAO route
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Field15Element {
    /// A point in the route (waypoint, coordinate, or navaid)
    Point(Point),
    /// A connector between points (airway or direct)
    Connector(Connector),
    /// A modifier that changes speed, altitude, or other parameters
    Modifier(Modifier),
}

/// A point in the route (waypoint, coordinate, or navaid)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Point {
    /// Named waypoint or navaid (Published Route Point)  
    /// - Basic point: `[A-Z]{1,3}`  
    /// - Point with 5 characters: `[A-Z]{5}`
    #[serde(rename = "waypoint")]
    Waypoint(String),
    /// latitude/longitude coordinates in degrees, e.g., (52.5, 13.4)
    /// - lat/lon in degrees: `[0-9]{2}[NS][0-9]{3}[EW]`
    /// - lat/lon in DM: `[0-9]{4}[NS][0-9]{5}[EW]`
    #[serde(rename = "coords")]
    Coordinates((f64, f64)),
    /// Point/Bearing/Distance format, e.g., "POINT180060" or "5430N01020E180060"
    /// - From PRP: `[A-Z]{1,5}[0-9]{6}`
    /// - From lat/lon in degrees: `[0-9]{2}[NS][0-9]{3}[EW][0-9]{6}`
    /// - From lat/lon in DM: `[0-9]{4}[NS][0-9]{5}[EW][0-9]{6}`
    #[serde(rename = "point_bearing_distance")]
    BearingDistance {
        /// Base point (waypoint or coordinate)
        point: Box<Point>,
        /// Bearing in degrees (000-360)
        bearing: u16,
        /// Distance in nautical miles (001-999)
        distance: u16,
    },
    /// Aerodrome (ICAO location indicator) - 4 letter code
    #[serde(rename = "aerodrome")]
    Aerodrome(String),
}

/// A connector between points (airway or direct)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Connector {
    /// ATS routes (e.g., "UM184", "L738", "A308")
    ///
    /// They include A, B, G, H, J, L, M, N, P, Q, R, T, U routes
    /// and upper airways like UN, UM, UL, UY, etc.
    /// - `[A-Z]{1,2}[0-9]`
    /// - `[A-Z][0-9]{1,3}[A-Z]`
    /// - `[A-Z][0-9]{2,3}`
    /// - `[A-Z]{3}[0-9]{3}` (Turkish type)
    /// - `[A-Z]{3}[0-9]{1,2}`
    /// - `[A-Z]{2}[0-9][A-Z]`
    /// - `[A-Z]{2}[0-9]{2,3}`
    /// - `[A-Z]{4}[0-9]{1,2}` (Russian air corridors)
    /// - `[A-Z]{2}[0-9]{2,3}[A-Z]`
    /// - `[A-Z][0-9]{3}[A-Z]`
    #[serde(rename = "airway")]
    Airway(String),
    /// Direct routing (DCT)
    #[serde(rename = "DCT")]
    Direct,
    /// SID (Standard Instrument Departure), can be literal "SID" or a named SID
    /// - `[A-Z]{3}[0-9]{1,2}[A-Z]`
    /// - `[A-Z]{5}[0-9]{1,2}`
    /// - `[A-Z]{4,6}[0-9][A-Z]`
    /// - `[A-Z]{5}[0-9]{2}[A-Z]`
    #[serde(rename = "SID")]
    Sid(String),
    /// STAR (Standard Arrival Route), can be literal "STAR" or a named STAR
    /// - `[A-Z]{3}[0-9]{1,2}[A-Z]`
    /// - `[A-Z]{5}[0-9]{1,2}`
    /// - `[A-Z]{4,6}[0-9][A-Z]`
    /// - `[A-Z]{5}[0-9]{2}[A-Z]`
    #[serde(rename = "STAR")]
    Star(String),
    /// VFR indicator: change to Visual Flight Rules
    #[serde(rename = "VFR")]
    Vfr,
    /// IFR indicator: change to Instrument Flight Rules
    #[serde(rename = "IFR")]
    Ifr,
    /// OAT indicator: change to Operational Air Traffic (military)
    #[serde(rename = "OAT")]
    Oat,
    /// GAT indicator: change to General Air Traffic
    #[serde(rename = "GAT")]
    Gat,
    /// IFPSTOP: CFMU IFPS special, stop IFR handling
    #[serde(rename = "IFPSTOP")]
    IfpStop,
    /// IFPSTART: CFMU IFPS special, start IFR handling
    #[serde(rename = "IFPSTART")]
    IfpStart,
    /// Stay at current position with optional time
    #[serde(rename = "STAY")]
    StayTime { minutes: Option<u16> },
    /// North Atlantic Track (NAT), e.g. NATA-NATZ, NAT1-NAT9, NATX, etc.
    /// - `NAT[A-Z]`
    /// - `NAT[A-Z][0-9]`
    #[serde(rename = "NAT")]
    Nat(String),
    /// Polar Track Structure (PTS) track, e.g. PTS0-PTS9, PTSA-PTSZ
    /// - `PTS[0-9]` or `PTS[A-Z]`
    #[serde(rename = "PTS")]
    Pts(String),
}

///  A modifier that changes flight parameters
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Modifier {
    /// Speed (e.g., "N0456" for 456 knots, "M079" for Mach 0.79, "K0893" for 893 km/h)
    pub speed: Option<Speed>,
    /// Flight level or altitude (e.g., "F340" for FL340, "S1130" for 11,300 meters)
    pub altitude: Option<Altitude>,
    /// Cruise climb end altitude (for cruise climb)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitude_cruise_to: Option<Altitude>,
    /// Cruise climb indicator (e.g., "PLUS" after speed/altitude)
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub cruise_climb: bool,
}

/// Speed representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Speed {
    /// Knots (N followed by 4 digits)
    #[serde(rename = "kts")]
    Knots(u16),
    /// Mach number as a float (e.g., 0.79)
    #[serde(rename = "Mach")]
    Mach(f32),
    /// Kilometers per hour (K followed by 4 digits)
    #[serde(rename = "km/h")]
    KilometersPerHour(u16),
}

/// Altitude representation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Altitude {
    /// Flight level (F followed by 3 digits)
    #[serde(rename = "FL")]
    FlightLevel(u16),
    /// Standard metric level (S followed by 4 digits, in tens of meters)
    #[serde(rename = "S")]
    MetricLevel(u16),
    /// Altitude in feet (A followed by 4 digits, in hundreds of feet)
    #[serde(rename = "ft")]
    Altitude(u16),
    /// Metric altitude (M followed by 4 digits, in tens of meters)
    #[serde(rename = "m")]
    MetricAltitude(u16),
    /// VFR altitude
    #[serde(rename = "VFR")]
    Vfr,
}

impl fmt::Display for Field15Element {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Field15Element::Point(p) => write!(f, "Point({})", p),
            Field15Element::Connector(c) => write!(f, "Connector({})", c),
            Field15Element::Modifier(m) => write!(f, "Modifier({})", m),
        }
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Point::Waypoint(s) => write!(f, "Waypoint({})", s),
            Point::Coordinates((lat, lon)) => write!(f, "Coordinate({:.5},{:.5})", lat, lon),
            Point::BearingDistance {
                point,
                bearing,
                distance,
            } => {
                write!(f, "BearingDistance({}/{:03}/{:03})", point, bearing, distance)
            }
            Point::Aerodrome(s) => write!(f, "Aerodrome({})", s),
        }
    }
}

impl fmt::Display for Connector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Connector::Airway(s) => write!(f, "Airway({})", s),
            Connector::Direct => write!(f, "DCT"),
            Connector::Vfr => write!(f, "VFR"),
            Connector::Ifr => write!(f, "IFR"),
            Connector::Oat => write!(f, "OAT"),
            Connector::Gat => write!(f, "GAT"),
            Connector::IfpStop => write!(f, "IFPSTOP"),
            Connector::IfpStart => write!(f, "IFPSTART"),
            Connector::StayTime { minutes } => {
                if let Some(m) = minutes {
                    write!(f, "STAY({})", m)
                } else {
                    write!(f, "STAY")
                }
            }
            Connector::Sid(s) => write!(f, "SID({})", s),
            Connector::Star(s) => write!(f, "STAR({})", s),
            Connector::Nat(s) => write!(f, "NAT({})", s),
            Connector::Pts(s) => write!(f, "PTS({})", s),
        }
    }
}

impl fmt::Display for Modifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.speed, &self.altitude, self.cruise_climb) {
            (Some(s), Some(a), true) => write!(f, "{}{}PLUS", s, a),
            (Some(s), Some(a), false) => write!(f, "{}{}", s, a),
            (Some(s), None, true) => write!(f, "{}PLUS", s),
            (Some(s), None, false) => write!(f, "{}", s),
            (None, Some(a), true) => write!(f, "{}PLUS", a),
            (None, Some(a), false) => write!(f, "{}", a),
            (None, None, _) => write!(f, ""),
        }
    }
}

impl fmt::Display for Speed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Speed::Knots(n) => write!(f, "N{:04}", n),
            Speed::Mach(m) => write!(f, "M{:0>5.2}", m),
            Speed::KilometersPerHour(k) => write!(f, "K{:04}", k),
        }
    }
}

impl fmt::Display for Altitude {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Altitude::FlightLevel(fl) => write!(f, "F{:03}", fl),
            Altitude::MetricLevel(s) => write!(f, "S{:04}", s),
            Altitude::Altitude(a) => write!(f, "A{:04}", a),
            Altitude::MetricAltitude(m) => write!(f, "M{:04}", m),
            Altitude::Vfr => write!(f, "VFR"),
        }
    }
}

/// A parser for ICAO Field 15 route strings
pub struct Field15Parser;

impl Field15Parser {
    /// Parse a Field 15 route string into a list of elements
    ///
    /// The parser treats forward slash (/) as both whitespace and a token separator,
    /// similar to the reference Python implementation's tokenization approach.
    pub fn parse(route: &str) -> Vec<Field15Element> {
        let mut elements = Vec::new();
        let tokens = Self::tokenize(route);
        let mut i = 0;
        let mut first_point_parsed = false;

        while i < tokens.len() {
            let token = tokens[i];

            // Handle truncate indicator 'T' - must be last token
            if token == "T" {
                // Truncate indicator - no more tokens should follow
                if i + 1 < tokens.len() {
                    // Error: tokens after truncate, but continue parsing
                }
                break;
            }

            // Handle forward slash - it signals a speed/altitude change is coming
            if token == "/" {
                i += 1;
                continue;
            }

            // Check for STAY element (STAY followed by digit)
            if Self::is_stay(token) {
                // Look for optional /HHMM
                let stay_minutes = if i + 2 < tokens.len() && tokens[i + 1] == "/" {
                    if let Some(mins) = Self::parse_stay_time(tokens[i + 2]) {
                        i += 2; // Skip slash and time
                        Some(mins)
                    } else {
                        None
                    }
                } else {
                    None
                };
                elements.push(Field15Element::Connector(Connector::StayTime { minutes: stay_minutes }));
                i += 1;
                continue;
            }

            // Check for "C" cruise climb indicator
            if token == "C" && i + 1 < tokens.len() && tokens[i + 1] == "/" {
                // This is a cruise climb indicator, skip it (handled by modifier)
                i += 1;
                continue;
            }

            // Check for modifiers first (this handles post-slash modifiers too)
            if let Some(modifier) = Self::parse_modifier(token) {
                elements.push(Field15Element::Modifier(modifier));
            }
            // Check for keywords
            else if token == "DCT" {
                elements.push(Field15Element::Connector(Connector::Direct));
            } else if token == "VFR" {
                elements.push(Field15Element::Connector(Connector::Vfr));
            } else if token == "IFR" {
                elements.push(Field15Element::Connector(Connector::Ifr));
            } else if token == "OAT" {
                elements.push(Field15Element::Connector(Connector::Oat));
            } else if token == "GAT" {
                elements.push(Field15Element::Connector(Connector::Gat));
            } else if token == "IFPSTOP" {
                elements.push(Field15Element::Connector(Connector::IfpStop));
            } else if token == "IFPSTART" {
                elements.push(Field15Element::Connector(Connector::IfpStart));
            } else if token == "SID" {
                // Literal "SID" keyword
                elements.push(Field15Element::Connector(Connector::Sid("SID".to_string())));
                first_point_parsed = true;
            } else if token == "STAR" {
                // Literal "STAR" keyword
                elements.push(Field15Element::Connector(Connector::Star("STAR".to_string())));
                first_point_parsed = true;
            }
            // Check for SID/STAR procedures BEFORE checking airways and waypoints
            else if !first_point_parsed && Self::is_procedure(token) {
                // First procedure is a SID
                elements.push(Field15Element::Connector(Connector::Sid(token.to_string())));
                first_point_parsed = true;
            } else if Self::is_procedure(token) && i == tokens.len() - 1 {
                // Last procedure-like item is a STAR (only if it's the last token)
                elements.push(Field15Element::Connector(Connector::Star(token.to_string())));
                first_point_parsed = true;
            }
            // After DCT, check if last element was DCT
            else if !elements.is_empty()
                && matches!(elements.last(), Some(Field15Element::Connector(Connector::Direct)))
            {
                // Force parsing as a point after DCT - skip airway check
                if let Some(point) = Self::parse_point(token) {
                    elements.push(Field15Element::Point(point));
                    first_point_parsed = true;
                }
            }
            // NAT/PTS connectors
            else if Self::is_nat(token) {
                elements.push(Field15Element::Connector(Connector::Nat(token.to_string())));
            } else if Self::is_pts(token) {
                elements.push(Field15Element::Connector(Connector::Pts(token.to_string())));
            }
            // Check for airways (only if not after DCT)
            else if Self::is_airway(token) {
                // If this is the last token and matches SID/STAR, treat as STAR not airway
                if i == tokens.len() - 1 && Self::is_procedure(token) {
                    elements.push(Field15Element::Connector(Connector::Star(token.to_string())));
                    first_point_parsed = true;
                } else {
                    elements.push(Field15Element::Connector(Connector::Airway(token.to_string())));
                }
            }
            // Finally, check for points (this includes waypoints as fallback)
            else if let Some(point) = Self::parse_point(token) {
                elements.push(Field15Element::Point(point));
                first_point_parsed = true;
            }

            i += 1;
        }

        elements
    }

    /// Tokenize the route string
    ///
    /// Treats whitespace (space, newline, tab, carriage return) and forward slash
    /// as delimiters. The forward slash is also returned as a separate token.
    fn tokenize(route: &str) -> Vec<&str> {
        let mut tokens = Vec::new();
        let mut current_token_start = 0;
        let mut in_token = false;

        for (i, ch) in route.char_indices() {
            let is_whitespace = ch == ' ' || ch == '\n' || ch == '\t' || ch == '\r';
            let is_slash = ch == '/';

            if is_whitespace || is_slash {
                // End current token if we're in one
                if in_token {
                    tokens.push(&route[current_token_start..i]);
                    in_token = false;
                }

                // Add slash as a separate token
                if is_slash {
                    tokens.push("/");
                }
            } else if !in_token {
                // Start a new token
                current_token_start = i;
                in_token = true;
            }
        }

        // Add final token if we ended while in a token
        if in_token {
            tokens.push(&route[current_token_start..]);
        }

        tokens
    }

    /// Try to parse a token as a speed/altitude modifier
    fn parse_modifier(token: &str) -> Option<Modifier> {
        if token.len() < 4 {
            return None;
        }

        // Check for PLUS suffix (cruise climb)
        let (base_token, cruise_climb) = if let Some(stripped) = token.strip_suffix("PLUS") {
            (stripped, true)
        } else {
            (token, false)
        };

        if base_token.len() < 4 {
            return None;
        }

        // Determine speed length based on first character
        // M (Mach) = 4 chars (M + 3 digits), N/K (Knots/KPH) = 5 chars (N/K + 4 digits)
        let speed_len = if base_token.starts_with('M') { 4 } else { 5 };

        if base_token.len() < speed_len {
            return None;
        }

        // Try to parse speed
        let speed = Self::parse_speed(&base_token[..speed_len]);

        // Parse altitude(s) from remaining characters
        let (altitude, altitude_cruise_to) = if speed.is_some() && base_token.len() > speed_len {
            let remaining = &base_token[speed_len..];
            // Check for two altitudes (cruise climb: speed/alt1/alt2)
            if remaining.len() >= 7 {
                // Try to parse two altitudes
                let first_alt_len = if remaining.starts_with('F') || remaining.starts_with('A') {
                    4
                } else {
                    5
                };
                if remaining.len() >= first_alt_len + 3 {
                    let alt1 = Self::parse_altitude(&remaining[..first_alt_len]);
                    let alt2 = Self::parse_altitude(&remaining[first_alt_len..]);
                    if alt1.is_some() && alt2.is_some() {
                        (alt1, alt2)
                    } else {
                        (Self::parse_altitude(remaining), None)
                    }
                } else {
                    (Self::parse_altitude(remaining), None)
                }
            } else {
                (Self::parse_altitude(remaining), None)
            }
        } else if speed.is_none() && base_token.len() >= 3 {
            (Self::parse_altitude(base_token), None)
        } else {
            (None, None)
        };

        // Only treat as modifier if altitude is present
        if altitude.is_some() {
            Some(Modifier {
                speed,
                altitude,
                altitude_cruise_to,
                cruise_climb,
            })
        } else {
            None
        }
    }

    /// Parse speed component
    fn parse_speed(s: &str) -> Option<Speed> {
        if s.len() < 4 {
            return None;
        }

        let speed_type = s.chars().next()?;
        let value_str = &s[1..];

        match speed_type {
            'N' if value_str.len() == 4 => value_str.parse::<u16>().ok().map(Speed::Knots),
            'M' if value_str.len() == 3 => {
                // Mach is given as 3 digits, e.g., "079" => 0.79
                value_str.parse::<u16>().ok().map(|v| Speed::Mach((v as f32) / 100.0))
            }
            'K' if value_str.len() == 4 => value_str.parse::<u16>().ok().map(Speed::KilometersPerHour),
            _ => None,
        }
    }

    /// Parse altitude component
    fn parse_altitude(s: &str) -> Option<Altitude> {
        if s == "VFR" {
            return Some(Altitude::Vfr);
        }

        if s.len() < 4 {
            return None;
        }

        let alt_type = s.chars().next()?;
        let value_str = &s[1..];

        match alt_type {
            'F' if value_str.len() == 3 => value_str.parse::<u16>().ok().map(Altitude::FlightLevel),
            'S' if value_str.len() == 4 => value_str.parse::<u16>().ok().map(Altitude::MetricLevel),
            'A' if value_str.len() == 4 => value_str.parse::<u16>().ok().map(Altitude::Altitude),
            'M' if value_str.len() == 4 => value_str.parse::<u16>().ok().map(Altitude::MetricAltitude),
            _ => None,
        }
    }

    /// Check if a token is a NAT track (NATA-NATZ, NAT1-NAT9, NATX, etc.)
    fn is_nat(token: &str) -> bool {
        // NAT[A-Z] or NAT[A-Z][0-9]
        if token.len() == 4 && token.starts_with("NAT") {
            let c = token.chars().nth(3).unwrap();
            c.is_ascii_uppercase()
        } else if token.len() == 5 && token.starts_with("NAT") {
            let c = token.chars().nth(3).unwrap();
            let d = token.chars().nth(4).unwrap();
            c.is_ascii_uppercase() && d.is_ascii_digit()
        } else {
            false
        }
    }

    /// Check if a token is a PTS track (PTS0-PTS9, PTSA-PTSZ)
    fn is_pts(token: &str) -> bool {
        // PTS[0-9] or PTS[A-Z]
        if token.len() == 4 && token.starts_with("PTS") {
            let c = token.chars().nth(3).unwrap();
            c.is_ascii_digit() || c.is_ascii_uppercase()
        } else {
            false
        }
    }

    /// Check if a token is an airway designation (excluding NAT/PTS)
    fn is_airway(token: &str) -> bool {
        if token.len() < 2 || token.len() > 7 {
            return false;
        }

        // Exclude NAT/PTS tracks
        if Self::is_nat(token) || Self::is_pts(token) {
            return false;
        }

        let first_char = token.chars().next().unwrap();
        if !first_char.is_alphabetic() {
            return false;
        }

        // Airways must contain at least one digit
        let has_digit = token.chars().any(|c| c.is_ascii_digit());
        if !has_digit {
            return false;
        }

        let valid_prefixes = [
            "UN", "UM", "UL", "UT", "UZ", "UY", "UP", "UA", "UB", "UG", "UH", "UJ", "UQ", "UR", "UV", "UW", "L", "A",
            "B", "G", "H", "J", "Q", "R", "T", "V", "W", "Y", "Z", "M", "N", "P",
        ];

        valid_prefixes.iter().any(|&p| token.starts_with(p))
    }

    /// Parse a point (waypoint, coordinate, bearing/distance, or aerodrome)
    fn parse_point(token: &str) -> Option<Point> {
        if token.is_empty() {
            return None;
        }

        // Check if it's a coordinate - must be checked before bearing/distance
        if Self::is_coordinate(token) {
            if let Some(coord) = Self::parse_coordinate(token) {
                return Some(Point::Coordinates(coord));
            } else {
                return None;
            }
        }

        // Check for bearing/distance format (e.g., POINT180060 or 02S001W180060)
        // Format: point name followed by exactly 6 digits (bearing 3 digits, distance 3 digits)
        if token.len() > 6 {
            let potential_digits = &token[token.len() - 6..];
            if potential_digits.chars().all(|c| c.is_ascii_digit()) {
                let point_name = &token[..token.len() - 6];

                // Try as coordinate first
                if Self::is_coordinate(point_name) {
                    if let Some(coord) = Self::parse_coordinate(point_name) {
                        if let (Ok(bearing), Ok(distance)) = (
                            potential_digits[..3].parse::<u16>(),
                            potential_digits[3..].parse::<u16>(),
                        ) {
                            if bearing <= 360 && distance <= 999 {
                                return Some(Point::BearingDistance {
                                    point: Box::new(Point::Coordinates(coord)),
                                    bearing,
                                    distance,
                                });
                            }
                        }
                    }
                } else if !point_name.is_empty() && point_name.chars().all(|c| c.is_ascii_alphabetic()) {
                    // Only allow Waypoint for non-coordinate
                    if let (Ok(bearing), Ok(distance)) = (
                        potential_digits[..3].parse::<u16>(),
                        potential_digits[3..].parse::<u16>(),
                    ) {
                        if bearing <= 360 && distance <= 999 {
                            return Some(Point::BearingDistance {
                                point: Box::new(Point::Waypoint(point_name.to_string())),
                                bearing,
                                distance,
                            });
                        }
                    }
                }
            }
        }

        // Check if it's a 4-letter aerodrome code (all uppercase letters, no digits)
        if token.len() == 4
            && token.chars().all(|c| c.is_ascii_uppercase())
            && !token.chars().any(|c| c.is_ascii_digit())
        {
            return Some(Point::Aerodrome(token.to_string()));
        }

        // Otherwise, it's a waypoint (PRP)
        Some(Point::Waypoint(token.to_string()))
    }

    /// Parse ICAO coordinate string into (lat, lon) in degrees.
    /// Supports formats like 5430N01020E, 54N010E, 5430N, 01020E, etc.
    fn parse_coordinate(token: &str) -> Option<(f64, f64)> {
        // Find N/S and E/W
        let n_idx = token.find('N');
        let s_idx = token.find('S');
        let e_idx = token.find('E');
        let w_idx = token.find('W');

        // Latitude
        let (lat_val, lat_sign, lat_end) = match (n_idx, s_idx) {
            (Some(idx), _) => (&token[..idx], 1.0, idx + 1),
            (_, Some(idx)) => (&token[..idx], -1.0, idx + 1),
            _ => return None,
        };
        let lat = match lat_val.len() {
            2 => lat_val.parse::<f64>().ok()? * lat_sign,
            4 => {
                let deg = lat_val[..2].parse::<f64>().ok()?;
                let min = lat_val[2..4].parse::<f64>().ok()?;
                (deg + min / 60.0) * lat_sign
            }
            _ => return None,
        };

        // Longitude
        let (lon_val, lon_sign) = match (e_idx, w_idx) {
            (Some(idx), _) => (&token[lat_end..idx], 1.0),
            (_, Some(idx)) => (&token[lat_end..idx], -1.0),
            _ => return None,
        };
        let lon = match lon_val.len() {
            3 => lon_val.parse::<f64>().ok()? * lon_sign,
            5 => {
                let deg = lon_val[..3].parse::<f64>().ok()?;
                let min = lon_val[3..5].parse::<f64>().ok()?;
                (deg + min / 60.0) * lon_sign
            }
            _ => return None,
        };

        Some((lat, lon))
    }

    /// Check if a token is a coordinate
    ///
    /// Coordinates can be in various formats:
    /// - 5020N (degrees/minutes latitude)
    /// - 5020N00130W (degrees/minutes lat/lon)
    /// - 50N005W (degrees only)
    /// - 5020N00130W (full format)
    fn is_coordinate(token: &str) -> bool {
        if token.len() < 4 {
            return false;
        }

        // Must contain N/S for latitude or E/W for longitude
        let has_lat = token.contains('N') || token.contains('S');
        let has_lon = token.contains('E') || token.contains('W');

        if !has_lat && !has_lon {
            return false;
        }

        // Find positions of direction indicators
        let lat_pos = token.find('N').or_else(|| token.find('S'));
        let lon_pos = token.find('E').or_else(|| token.find('W'));

        // Check format validity
        match (lat_pos, lon_pos) {
            (Some(lat_idx), Some(lon_idx)) => {
                // Both lat and lon present - lat must come first
                if lat_idx >= lon_idx {
                    return false;
                }

                // Characters before lat indicator must be digits
                let lat_part = &token[..lat_idx];
                if lat_part.is_empty() || !lat_part.chars().all(|c| c.is_ascii_digit()) {
                    return false;
                }

                // Characters between lat and lon indicators must be digits
                let lon_part = &token[lat_idx + 1..lon_idx];
                if lon_part.is_empty() || !lon_part.chars().all(|c| c.is_ascii_digit()) {
                    return false;
                }

                // Nothing should follow the longitude indicator
                lon_idx == token.len() - 1
            }
            (Some(lat_idx), None) => {
                // Only latitude present
                let lat_part = &token[..lat_idx];
                !lat_part.is_empty() && lat_part.chars().all(|c| c.is_ascii_digit()) && lat_idx == token.len() - 1
            }
            (None, Some(lon_idx)) => {
                // Only longitude present (unusual but valid)
                let lon_part = &token[..lon_idx];
                !lon_part.is_empty() && lon_part.chars().all(|c| c.is_ascii_digit()) && lon_idx == token.len() - 1
            }
            (None, None) => false,
        }
    }

    /// Check if a token is a procedure (SID/STAR)
    fn is_procedure(token: &str) -> bool {
        // [A-Z]{3}[0-9]{1,2}[A-Z]
        if token.len() >= 5 && token.len() <= 7 {
            let bytes = token.as_bytes();
            if bytes.len() >= 5 && bytes[0..3].iter().all(|b| b.is_ascii_uppercase()) && (bytes[3].is_ascii_digit()) {
                // [A-Z]{3}[0-9]{1,2}[A-Z]
                if bytes.len() == 5 && bytes[4].is_ascii_uppercase() {
                    return true;
                }
                if bytes.len() == 6 && bytes[4].is_ascii_digit() && bytes[5].is_ascii_uppercase() {
                    return true;
                }
            }
        }
        // [A-Z]{5}[0-9]{1,2}
        if token.len() == 6 || token.len() == 7 {
            let bytes = token.as_bytes();
            if bytes[0..5].iter().all(|b| b.is_ascii_uppercase())
                && bytes[5].is_ascii_digit()
                && (token.len() == 6 || (token.len() == 7 && bytes[6].is_ascii_digit()))
            {
                return true;
            }
        }
        // [A-Z]{4,6}[0-9][A-Z]
        if token.len() >= 6 && token.len() <= 8 {
            let bytes = token.as_bytes();
            let prefix_len = token.len() - 2;
            if (4..=6).contains(&prefix_len)
                && bytes[0..prefix_len].iter().all(|b| b.is_ascii_uppercase())
                && bytes[prefix_len].is_ascii_digit()
                && bytes[prefix_len + 1].is_ascii_uppercase()
            {
                return true;
            }
        }
        // [A-Z]{5}[0-9]{2}[A-Z]
        if token.len() == 8 {
            let bytes = token.as_bytes();
            if bytes[0..5].iter().all(|b| b.is_ascii_uppercase())
                && bytes[5].is_ascii_digit()
                && bytes[6].is_ascii_digit()
                && bytes[7].is_ascii_uppercase()
            {
                return true;
            }
        }
        false
    }

    /// Check if a token is a STAY element (STAY followed by digit)
    fn is_stay(token: &str) -> bool {
        token.len() == 5 && token.starts_with("STAY") && token.chars().nth(4).unwrap().is_ascii_digit()
    }

    /// Parse STAY time in HHMM format
    fn parse_stay_time(time_str: &str) -> Option<u16> {
        if time_str.len() != 4 {
            return None;
        }
        // Validate HHMM format: HH must be 00-23, MM must be 00-59
        let hh = time_str[..2].parse::<u16>().ok()?;
        let mm = time_str[2..].parse::<u16>().ok()?;
        if hh <= 23 && mm <= 59 {
            Some(hh * 60 + mm)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_speed_parsing() {
        assert_eq!(Field15Parser::parse_speed("N0456"), Some(Speed::Knots(456)));
        assert_eq!(Field15Parser::parse_speed("M079"), Some(Speed::Mach(0.79)));
        assert_eq!(Field15Parser::parse_speed("K0893"), Some(Speed::KilometersPerHour(893)));
    }

    #[test]
    fn test_altitude_parsing() {
        assert_eq!(Field15Parser::parse_altitude("F340"), Some(Altitude::FlightLevel(340)));
        assert_eq!(
            Field15Parser::parse_altitude("S1130"),
            Some(Altitude::MetricLevel(1130))
        );
    }

    #[test]
    fn test_coordinate_detection() {
        assert!(Field15Parser::is_coordinate("62N010W"));
        assert!(Field15Parser::is_coordinate("5430N"));
        assert!(Field15Parser::is_coordinate("53N100W"));
        assert!(!Field15Parser::is_coordinate("LACOU"));
    }

    #[test]
    fn test_procedure_detection() {
        assert!(Field15Parser::is_procedure("LACOU5A"));
        assert!(Field15Parser::is_procedure("ROXOG1H"));
        assert!(Field15Parser::is_procedure("RANUX3D"));
        assert!(!Field15Parser::is_procedure("LACOU"));
        assert!(!Field15Parser::is_procedure("CNA"));
    }

    #[test]
    fn test_airway_detection() {
        assert!(Field15Parser::is_airway("UM184"));
        assert!(Field15Parser::is_airway("UN863"));
        assert!(Field15Parser::is_airway("L738"));
        assert!(Field15Parser::is_airway("A308"));
        assert!(!Field15Parser::is_airway("DCT"));
        assert!(!Field15Parser::is_airway("LACOU"));
    }

    #[test]
    fn test_tokenization() {
        let tokens = Field15Parser::tokenize("N0450F100 POINT/M079F200 DCT");
        assert_eq!(tokens, vec!["N0450F100", "POINT", "/", "M079F200", "DCT"]);
    }

    #[test]
    fn test_tokenization_multiple_whitespace() {
        let tokens = Field15Parser::tokenize("A  B\tC\nD\rE");
        assert_eq!(tokens, vec!["A", "B", "C", "D", "E"]);
    }

    #[test]
    fn test_slash_handling() {
        let route = "N0450F100 POINT/M079F200";
        let elements = Field15Parser::parse(route);

        // Should have initial modifier, point, and then modified speed/altitude
        assert!(elements.len() >= 3);

        let modifiers: Vec<_> = elements
            .iter()
            .filter(|e| matches!(e, Field15Element::Modifier(_)))
            .collect();
        assert_eq!(modifiers.len(), 2);
    }

    #[test]
    fn test_coordinate_validation() {
        assert!(Field15Parser::is_coordinate("5020N"));
        assert!(Field15Parser::is_coordinate("5020N00130W"));
        assert!(Field15Parser::is_coordinate("50N005W"));
        assert!(Field15Parser::is_coordinate("00N000E"));

        assert!(!Field15Parser::is_coordinate("N5020")); // Wrong order
        assert!(!Field15Parser::is_coordinate("5020W00130N")); // Lon before lat
        assert!(!Field15Parser::is_coordinate("ABC")); // No direction
        assert!(!Field15Parser::is_coordinate("50N")); // Too short
    }

    #[test]
    fn test_bearing_distance_with_coordinate() {
        let route = "N0450F100 02S001W180060";
        let elements = Field15Parser::parse(route);

        let bearing_dist = elements
            .iter()
            .find(|e| matches!(e, Field15Element::Point(Point::BearingDistance { .. })));

        assert!(bearing_dist.is_some());
        if let Some(Field15Element::Point(Point::BearingDistance {
            point,
            bearing,
            distance,
        })) = bearing_dist
        {
            assert_eq!(**point, Point::Coordinates((-2.0, -1.0)));
            assert_eq!(*bearing, 180);
            assert_eq!(*distance, 60);
        }
    }

    #[test]
    fn test_simple_route() {
        let route = "N0456F340 LACOU5A LACOU UM184 CNA UN863 MANAK UY110 REVTU UP87 ROXOG ROXOG1H";
        let elements = Field15Parser::parse(route);

        assert_eq!(elements.len(), 12);
        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(456)),
                    altitude: Some(Altitude::FlightLevel(340)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Sid("LACOU5A".to_string())),
                Field15Element::Point(Point::Waypoint("LACOU".to_string())),
                Field15Element::Connector(Connector::Airway("UM184".to_string())),
                Field15Element::Point(Point::Waypoint("CNA".to_string())),
                Field15Element::Connector(Connector::Airway("UN863".to_string())),
                Field15Element::Point(Point::Waypoint("MANAK".to_string())),
                Field15Element::Connector(Connector::Airway("UY110".to_string())),
                Field15Element::Point(Point::Waypoint("REVTU".to_string())),
                Field15Element::Connector(Connector::Airway("UP87".to_string())),
                Field15Element::Point(Point::Waypoint("ROXOG".to_string())),
                Field15Element::Connector(Connector::Star("ROXOG1H".to_string())),
            ]
        );
    }

    #[test]
    fn test_readme_example() {
        // Example from the reference README

        let route = "N0450M0825 00N000E B9 00N001E VFR IFR 00N001W/N0350F100 01N001W 01S001W 02S001W180060";
        let elements = Field15Parser::parse(route);

        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(450)),
                    altitude: Some(Altitude::MetricAltitude(825)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Point(Point::Coordinates((0., 0.))),
                Field15Element::Connector(Connector::Airway("B9".to_string())),
                Field15Element::Point(Point::Coordinates((0., 1.))),
                Field15Element::Connector(Connector::Vfr),
                Field15Element::Connector(Connector::Ifr),
                Field15Element::Point(Point::Coordinates((0., -1.))),
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(350)),
                    altitude: Some(Altitude::FlightLevel(100)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Point(Point::Coordinates((1., -1.))),
                Field15Element::Point(Point::Coordinates((-1., -1.))),
                Field15Element::Point(Point::BearingDistance {
                    point: Box::new(Point::Coordinates((-2., -1.))),
                    bearing: 180,
                    distance: 60,
                }),
            ]
        );
    }

    #[test]
    fn test_oat_gat_connectors() {
        let route = "N0450F100 POINT OAT POINT GAT POINT";
        let elements = Field15Parser::parse(route);

        assert_eq!(elements.len(), 6);
        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(450)),
                    altitude: Some(Altitude::FlightLevel(100)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Point(Point::Waypoint("POINT".to_string())),
                Field15Element::Connector(Connector::Oat),
                Field15Element::Point(Point::Waypoint("POINT".to_string())),
                Field15Element::Connector(Connector::Gat),
                Field15Element::Point(Point::Waypoint("POINT".to_string())),
            ]
        );
    }

    #[test]
    fn test_ifp_stop_start() {
        let route = "N0450F100 POINT IFPSTOP POINT IFPSTART POINT";
        let elements = Field15Parser::parse(route);
        assert_eq!(elements.len(), 6);
        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(450)),
                    altitude: Some(Altitude::FlightLevel(100)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Point(Point::Waypoint("POINT".to_string())),
                Field15Element::Connector(Connector::IfpStop),
                Field15Element::Point(Point::Waypoint("POINT".to_string())),
                Field15Element::Connector(Connector::IfpStart),
                Field15Element::Point(Point::Waypoint("POINT".to_string())),
            ]
        );
    }

    #[test]
    fn test_bearing_distance_format() {
        let route = "N0450F100 POINT WAYPOINT180060";
        let elements = Field15Parser::parse(route);

        assert_eq!(elements.len(), 3);
        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(450)),
                    altitude: Some(Altitude::FlightLevel(100)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Point(Point::Waypoint("POINT".to_string())),
                Field15Element::Point(Point::BearingDistance {
                    point: Box::new(Point::Waypoint("WAYPOINT".to_string())),
                    bearing: 180,
                    distance: 60,
                }),
            ]
        );
    }

    #[test]
    fn test_aerodrome_detection() {
        let route = "N0450F100 LFPG DCT EGLL";
        let elements = Field15Parser::parse(route);
        assert_eq!(elements.len(), 4);
        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(450)),
                    altitude: Some(Altitude::FlightLevel(100)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Point(Point::Aerodrome("LFPG".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Aerodrome("EGLL".to_string())),
            ]
        );
    }

    #[test]
    fn test_truncate_indicator() {
        let route = "N0450F100 POINT DCT POINT2 T";
        let elements = Field15Parser::parse(route);
        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(450)),
                    altitude: Some(Altitude::FlightLevel(100)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Point(Point::Waypoint("POINT".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("POINT2".to_string())),
            ]
        );
    }

    #[test]
    fn test_literal_sid_star() {
        let route = "N0450F100 SID POINT DCT POINT2 STAR";
        let elements = Field15Parser::parse(route);

        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(450)),
                    altitude: Some(Altitude::FlightLevel(100)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Sid("SID".to_string())),
                Field15Element::Point(Point::Waypoint("POINT".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("POINT2".to_string())),
                Field15Element::Connector(Connector::Star("STAR".to_string())),
            ]
        );
    }

    #[test]
    fn test_aerodrome_no_digits() {
        let route = "N0450F100 LFPG DCT EGLL";
        let elements = Field15Parser::parse(route);

        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(450)),
                    altitude: Some(Altitude::FlightLevel(100)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Point(Point::Aerodrome("LFPG".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Aerodrome("EGLL".to_string())),
            ]
        );
    }

    #[test]
    fn test_star_must_be_last() {
        // STAR-like pattern in middle should be waypoint or airway
        let route = "N0450F100 POINT1A POINT DCT POINT";
        let elements = Field15Parser::parse(route);

        // POINT1A should not be STAR since it's not last
        assert!(!elements
            .iter()
            .any(|e| { matches!(e, Field15Element::Connector(Connector::Star(s)) if s == "POINT1A") }));
    }

    #[test]
    fn test_single_char_point() {
        // Single character "C" should be a waypoint, not cruise climb indicator
        let route = "N0450F100 POINT DCT C DCT POINT";
        let elements = Field15Parser::parse(route);

        // C should appear as a waypoint
        assert!(elements
            .iter()
            .any(|e| { matches!(e, Field15Element::Point(Point::Waypoint(s)) if s == "C") }));
    }

    #[test]
    fn test_readme_tokenization() {
        // From README: whitespace is space, newline, tab, carriage return, and forward slash
        let route = "N0450M0825\n00N000E\tB9 00N001E/VFR";
        let elements = Field15Parser::parse(route);

        assert!(elements.len() >= 5);
        assert!(elements
            .iter()
            .any(|e| matches!(e, Field15Element::Connector(Connector::Vfr))));
    }

    #[test]
    fn test_bearing_distance_validation() {
        // Test bearing > 360 (should still parse but with validation in real impl)
        let route = "N0450F100 POINT999999";
        let elements = Field15Parser::parse(route);

        // Should not parse as bearing/distance if bearing > 360
        assert!(!elements
            .iter()
            .any(|e| { matches!(e, Field15Element::Point(Point::BearingDistance { bearing, .. }) if *bearing > 360) }));
    }

    #[test]
    fn test_route_with_speed_changes() {
        let route = "N0495F320 RANUX3D RANUX UN858 VALEK/N0491F330 UM163 DIK UN853 ARCKY DCT NVO DCT BERIM DCT BIKRU/N0482F350 DCT VEDEN";
        let elements = Field15Parser::parse(route);

        assert_eq!(elements.len(), 19);
        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(495)),
                    altitude: Some(Altitude::FlightLevel(320)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Sid("RANUX3D".to_string())),
                Field15Element::Point(Point::Waypoint("RANUX".to_string())),
                Field15Element::Connector(Connector::Airway("UN858".to_string())),
                Field15Element::Point(Point::Waypoint("VALEK".to_string())),
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(491)),
                    altitude: Some(Altitude::FlightLevel(330)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Airway("UM163".to_string())),
                Field15Element::Point(Point::Waypoint("DIK".to_string())),
                Field15Element::Connector(Connector::Airway("UN853".to_string())),
                Field15Element::Point(Point::Waypoint("ARCKY".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("NVO".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("BERIM".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("BIKRU".to_string())),
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(482)),
                    altitude: Some(Altitude::FlightLevel(350)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("VEDEN".to_string())),
            ]
        );
    }

    #[test]
    fn test_route_with_coordinates() {
        let route = "N0458F320 BERGI UL602 SUPUR UP1 GODOS P1 ROLUM P13 ASKAM L7 SUM DCT PEMOS/M079F320 DCT 62N010W 63N020W 63N030W 64N040W 64N050W";
        let elements = Field15Parser::parse(route);
        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(458)),
                    altitude: Some(Altitude::FlightLevel(320)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Point(Point::Waypoint("BERGI".to_string())),
                Field15Element::Connector(Connector::Airway("UL602".to_string())),
                Field15Element::Point(Point::Waypoint("SUPUR".to_string())),
                Field15Element::Connector(Connector::Airway("UP1".to_string())),
                Field15Element::Point(Point::Waypoint("GODOS".to_string())),
                Field15Element::Connector(Connector::Airway("P1".to_string())),
                Field15Element::Point(Point::Waypoint("ROLUM".to_string())),
                Field15Element::Connector(Connector::Airway("P13".to_string())),
                Field15Element::Point(Point::Waypoint("ASKAM".to_string())),
                Field15Element::Connector(Connector::Airway("L7".to_string())),
                Field15Element::Point(Point::Waypoint("SUM".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("PEMOS".to_string())),
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Mach(0.79)),
                    altitude: Some(Altitude::FlightLevel(320)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Coordinates((62., -10.))),
                Field15Element::Point(Point::Coordinates((63., -20.))),
                Field15Element::Point(Point::Coordinates((63., -30.))),
                Field15Element::Point(Point::Coordinates((64., -40.))),
                Field15Element::Point(Point::Coordinates((64., -50.))),
            ]
        );
    }

    #[test]
    fn test_route_with_modifiers() {
        let route = "N0427F230 DET1J DET L6 DVR L9 KONAN/N0470F350 UL607 MATUG";
        let elements = Field15Parser::parse(route);

        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(427)),
                    altitude: Some(Altitude::FlightLevel(230)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Sid("DET1J".to_string())),
                Field15Element::Point(Point::Waypoint("DET".to_string())),
                Field15Element::Connector(Connector::Airway("L6".to_string())),
                Field15Element::Point(Point::Waypoint("DVR".to_string())),
                Field15Element::Connector(Connector::Airway("L9".to_string())),
                Field15Element::Point(Point::Waypoint("KONAN".to_string())),
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(470)),
                    altitude: Some(Altitude::FlightLevel(350)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Airway("UL607".to_string())),
                Field15Element::Point(Point::Waypoint("MATUG".to_string())),
            ]
        );
    }

    #[test]
    fn test_multiple_airways() {
        let route =
            "N0463F350 ERIXU3B ERIXU UN860 ETAMO UZ271 ADEKA UT18 AMLIR/N0461F370 UT18 BADAM UZ151 FJR UM731 DIVKO";
        let elements = Field15Parser::parse(route);

        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(463)),
                    altitude: Some(Altitude::FlightLevel(350)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Sid("ERIXU3B".to_string())),
                Field15Element::Point(Point::Waypoint("ERIXU".to_string())),
                Field15Element::Connector(Connector::Airway("UN860".to_string())),
                Field15Element::Point(Point::Waypoint("ETAMO".to_string())),
                Field15Element::Connector(Connector::Airway("UZ271".to_string())),
                Field15Element::Point(Point::Waypoint("ADEKA".to_string())),
                Field15Element::Connector(Connector::Airway("UT18".to_string())),
                Field15Element::Point(Point::Waypoint("AMLIR".to_string())),
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(461)),
                    altitude: Some(Altitude::FlightLevel(370)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Airway("UT18".to_string())),
                Field15Element::Point(Point::Waypoint("BADAM".to_string())),
                Field15Element::Connector(Connector::Airway("UZ151".to_string())),
                Field15Element::Point(Point::Waypoint("FJR".to_string())),
                Field15Element::Connector(Connector::Airway("UM731".to_string())),
                Field15Element::Point(Point::Waypoint("DIVKO".to_string())),
            ]
        );
    }

    #[test]
    fn test_long_complex_route() {
        let route = "N0459F320 OBOKA UZ29 TORNU DCT RAVLO Y70 OTBED L60 PENIL M144 BAGSO DCT RINUS DCT GISTI/M079F330 DCT MALOT/M079F340 DCT 54N020W 55N030W 54N040W 51N050W DCT ALLRY/N0463F360 DCT YQX";
        let elements = Field15Parser::parse(route);
        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(459)),
                    altitude: Some(Altitude::FlightLevel(320)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Point(Point::Waypoint("OBOKA".to_string())),
                Field15Element::Connector(Connector::Airway("UZ29".to_string())),
                Field15Element::Point(Point::Waypoint("TORNU".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("RAVLO".to_string())),
                Field15Element::Connector(Connector::Airway("Y70".to_string())),
                Field15Element::Point(Point::Waypoint("OTBED".to_string())),
                Field15Element::Connector(Connector::Airway("L60".to_string())),
                Field15Element::Point(Point::Waypoint("PENIL".to_string())),
                Field15Element::Connector(Connector::Airway("M144".to_string())),
                Field15Element::Point(Point::Waypoint("BAGSO".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("RINUS".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("GISTI".to_string())),
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Mach(0.79)),
                    altitude: Some(Altitude::FlightLevel(330)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("MALOT".to_string())),
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Mach(0.79)),
                    altitude: Some(Altitude::FlightLevel(340)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Direct),
                // Fix for the $PLACEHOLDER$ in test_long_complex_route
                Field15Element::Point(Point::Coordinates((54., -20.))),
                Field15Element::Point(Point::Coordinates((55., -30.))),
                Field15Element::Point(Point::Coordinates((54., -40.))),
                Field15Element::Point(Point::Coordinates((51., -50.))),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("ALLRY".to_string())),
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(463)),
                    altitude: Some(Altitude::FlightLevel(360)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("YQX".to_string())),
            ]
        );
    }

    #[test]
    fn test_complex_route_for_tokenization() {
        // Full example from README with proper tokenization
        let route = "N0450M0825 00N000E B9 00N001E VFR IFR 00N001W/N0350F100 01N001W 01S001W 02S001W180060";
        let elements = Field15Parser::parse(route);
        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(450)),
                    altitude: Some(Altitude::MetricAltitude(825)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Point(Point::Coordinates((0., 0.))),
                Field15Element::Connector(Connector::Airway("B9".to_string())),
                Field15Element::Point(Point::Coordinates((0., 1.))),
                Field15Element::Connector(Connector::Vfr),
                Field15Element::Connector(Connector::Ifr),
                Field15Element::Point(Point::Coordinates((0., -1.))),
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(350)),
                    altitude: Some(Altitude::FlightLevel(100)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Point(Point::Coordinates((1., -1.))),
                Field15Element::Point(Point::Coordinates((-1., -1.))),
                Field15Element::Point(Point::BearingDistance {
                    point: Box::new(Point::Coordinates((-2., -1.))),
                    bearing: 180,
                    distance: 60,
                }),
            ]
        );
    }

    #[test]
    fn test_nat_track_is_nat() {
        // NAT tracks should be parsed as airways, not aerodromes
        assert!(Field15Parser::is_nat("NATD"));
        assert!(Field15Parser::is_nat("NATA"));
        assert!(Field15Parser::is_nat("NATZ"));
        assert!(Field15Parser::is_nat("NATZ1"));
        assert!(!Field15Parser::is_nat("NAT1")); // Not a valid NAT track
        assert!(!Field15Parser::is_nat("NAT")); // Too short
    }

    #[test]
    fn test_nat_track_in_route() {
        let route = "N0490F360 ELCOB6B ELCOB UT300 SENLO UN502 JSY DCT LIZAD DCT MOPAT DCT LUNIG DCT MOMIN DCT PIKIL/M084F380 NATD HOIST/N0490F380 N756C ANATI/N0441F340 DCT MIVAX DCT OBTEK DCT XORLO ROCKT2";
        let elements = Field15Parser::parse(route);

        assert_eq!(
            elements,
            vec![
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(490)),
                    altitude: Some(Altitude::FlightLevel(360)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Sid("ELCOB6B".to_string())),
                Field15Element::Point(Point::Waypoint("ELCOB".to_string())),
                Field15Element::Connector(Connector::Airway("UT300".to_string())),
                Field15Element::Point(Point::Waypoint("SENLO".to_string())),
                Field15Element::Connector(Connector::Airway("UN502".to_string())),
                Field15Element::Point(Point::Waypoint("JSY".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("LIZAD".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("MOPAT".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("LUNIG".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("MOMIN".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("PIKIL".to_string())),
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Mach(0.84)),
                    altitude: Some(Altitude::FlightLevel(380)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Nat("NATD".to_string())),
                Field15Element::Point(Point::Waypoint("HOIST".to_string())),
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(490)),
                    altitude: Some(Altitude::FlightLevel(380)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Airway("N756C".to_string())),
                Field15Element::Point(Point::Waypoint("ANATI".to_string())),
                Field15Element::Modifier(Modifier {
                    speed: Some(Speed::Knots(441)),
                    altitude: Some(Altitude::FlightLevel(340)),
                    cruise_climb: false,
                    altitude_cruise_to: None
                }),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("MIVAX".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("OBTEK".to_string())),
                Field15Element::Connector(Connector::Direct),
                Field15Element::Point(Point::Waypoint("XORLO".to_string())),
                Field15Element::Connector(Connector::Star("ROCKT2".to_string())),
            ]
        );
    }
}
