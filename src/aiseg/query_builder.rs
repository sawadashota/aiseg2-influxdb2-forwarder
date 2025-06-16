//! Query string builders for AiSEG2 API requests.
//!
//! This module provides unified query building functionality for different
//! types of AiSEG2 requests, reducing duplication across collectors.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{DateTime, Datelike, Local};

/// Types of queries supported by the AiSEG2 API.
#[derive(Debug, Clone)]
pub enum QueryType {
    /// Daily total query for general metrics
    DailyTotal { date: DateTime<Local> },
    /// Circuit-specific daily total query
    CircuitDailyTotal {
        date: DateTime<Local>,
        circuit_id: String,
    },
}

/// Builder for creating AiSEG2 query strings.
pub struct QueryBuilder;

impl QueryBuilder {
    /// Creates a base64-encoded query string for the given query type.
    ///
    /// # Arguments
    /// * `query_type` - The type of query to build
    ///
    /// # Returns
    /// A base64-encoded JSON query string
    pub fn build(query_type: QueryType) -> String {
        match query_type {
            QueryType::DailyTotal { date } => Self::build_daily_total_query(date),
            QueryType::CircuitDailyTotal { date, circuit_id } => {
                Self::build_circuit_daily_total_query(date, &circuit_id)
            }
        }
    }

    /// Builds a daily total query string.
    ///
    /// # Format
    /// ```json
    /// {"day":[2024,6,6],"month_compare":"mon","day_compare":"day"}
    /// ```
    fn build_daily_total_query(date: DateTime<Local>) -> String {
        let query = format!(
            r#"{{"day":[{},{},{}],"month_compare":"mon","day_compare":"day"}}"#,
            date.year(),
            date.month(),
            date.day()
        );

        STANDARD.encode(query)
    }

    /// Builds a circuit-specific daily total query string.
    ///
    /// # Format
    /// ```json
    /// {"day":[2024,6,8],"term":"2024/06/08","termStr":"day","id":"1","circuitid":"30"}
    /// ```
    fn build_circuit_daily_total_query(date: DateTime<Local>, circuit_id: &str) -> String {
        let query = format!(
            r#"{{"day":[{},{},{}],"term":"{}","termStr":"day","id":"1","circuitid":"{}"}}"#,
            date.year(),
            date.month(),
            date.day(),
            date.format("%Y/%m/%d"),
            circuit_id
        );

        STANDARD.encode(query)
    }
}

/// Helper function to create a daily total query.
/// This maintains backward compatibility with existing code.
pub fn make_daily_total_query(date: DateTime<Local>) -> String {
    QueryBuilder::build(QueryType::DailyTotal { date })
}

/// Helper function to create a circuit daily total query.
/// This maintains backward compatibility with existing code.
pub fn make_circuit_query(circuit_id: &str, date: DateTime<Local>) -> String {
    QueryBuilder::build(QueryType::CircuitDailyTotal {
        date,
        circuit_id: circuit_id.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_daily_total_query() {
        let date = Local.with_ymd_and_hms(2024, 6, 6, 10, 30, 0).unwrap();
        let query = QueryBuilder::build(QueryType::DailyTotal { date });

        // Decode and verify
        let decoded = String::from_utf8(STANDARD.decode(&query).unwrap()).unwrap();
        assert!(decoded.contains(r#""day":[2024,6,6]"#));
        assert!(decoded.contains(r#""month_compare":"mon""#));
        assert!(decoded.contains(r#""day_compare":"day""#));
    }

    #[test]
    fn test_circuit_daily_total_query() {
        let date = Local.with_ymd_and_hms(2024, 6, 8, 10, 30, 0).unwrap();
        let query = QueryBuilder::build(QueryType::CircuitDailyTotal {
            date,
            circuit_id: "30".to_string(),
        });

        // Decode and verify
        let decoded = String::from_utf8(STANDARD.decode(&query).unwrap()).unwrap();
        assert!(decoded.contains(r#""day":[2024,6,8]"#));
        assert!(decoded.contains(r#""term":"2024/06/08""#));
        assert!(decoded.contains(r#""termStr":"day""#));
        assert!(decoded.contains(r#""id":"1""#));
        assert!(decoded.contains(r#""circuitid":"30""#));
    }

    #[test]
    fn test_backward_compatibility_daily() {
        let date = Local.with_ymd_and_hms(2024, 2, 29, 0, 0, 0).unwrap();
        let query = make_daily_total_query(date);

        let decoded = String::from_utf8(STANDARD.decode(&query).unwrap()).unwrap();
        assert!(decoded.contains(r#""day":[2024,2,29]"#));
    }

    #[test]
    fn test_backward_compatibility_circuit() {
        let date = Local.with_ymd_and_hms(2023, 12, 31, 23, 59, 59).unwrap();
        let query = make_circuit_query("25", date);

        let decoded = String::from_utf8(STANDARD.decode(&query).unwrap()).unwrap();
        assert!(decoded.contains(r#""day":[2023,12,31]"#));
        assert!(decoded.contains(r#""term":"2023/12/31""#));
        assert!(decoded.contains(r#""circuitid":"25""#));
    }

    #[test]
    fn test_different_circuit_ids() {
        let date = Local::now();
        let circuits = vec!["25", "26", "27", "30"];

        for circuit_id in circuits {
            let query = make_circuit_query(circuit_id, date);
            let decoded = String::from_utf8(STANDARD.decode(&query).unwrap()).unwrap();
            assert!(decoded.contains(&format!(r#""circuitid":"{}""#, circuit_id)));
        }
    }
}
