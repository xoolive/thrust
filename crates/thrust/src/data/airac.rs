use chrono::{Datelike, Duration, NaiveDate};

const AIRAC_EPOCH: (i32, u32, u32) = (1998, 1, 29);

pub fn airac_code_from_date(date: NaiveDate) -> String {
    let epoch = airac_epoch();
    let delta_days = (date - epoch).num_days();
    let serial = delta_days.div_euclid(28);
    let effective = epoch + Duration::days(serial * 28);
    let ordinal = (effective.ordinal0() / 28) + 1;
    format!("{:02}{:02}", effective.year() % 100, ordinal)
}

pub fn airac_year_epoch(year: i32) -> Result<NaiveDate, Box<dyn std::error::Error>> {
    let beg = NaiveDate::from_ymd_opt(year, 1, 1).ok_or("Invalid year")?;
    let extra_days = (beg - airac_epoch()).num_days().rem_euclid(28);
    Ok(beg - Duration::days(extra_days - 28))
}

pub fn effective_date_from_airac_code(airac_code: &str) -> Result<NaiveDate, Box<dyn std::error::Error>> {
    let (year, cycle) = parse_airac_code(airac_code)?;
    let year_epoch = airac_year_epoch(year)?;
    let effective = year_epoch + Duration::days((cycle as i64 - 1) * 28);

    if airac_code_from_date(effective) != airac_code {
        return Err(format!("AIRAC mismatch for calculated start date: {effective}").into());
    }
    Ok(effective)
}

pub fn airac_interval(airac_code: &str) -> Result<(NaiveDate, NaiveDate), Box<dyn std::error::Error>> {
    let begin = effective_date_from_airac_code(airac_code)?;
    Ok((begin, begin + Duration::days(28)))
}

fn parse_airac_code(airac_code: &str) -> Result<(i32, u32), Box<dyn std::error::Error>> {
    if airac_code.len() != 4 || !airac_code.chars().all(|c| c.is_ascii_digit()) {
        return Err("AIRAC code must be in YYCC format, e.g. 2508".into());
    }

    let yy = airac_code[0..2].parse::<i32>()?;
    let cc = airac_code[2..4].parse::<u32>()?;
    if !(1..=14).contains(&cc) {
        return Err("AIRAC cycle number must be between 01 and 14".into());
    }

    Ok((2000 + yy, cc))
}

fn airac_epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(AIRAC_EPOCH.0, AIRAC_EPOCH.1, AIRAC_EPOCH.2).expect("Invalid AIRAC epoch")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_code_and_date_consistency() {
        let date = NaiveDate::from_ymd_opt(2025, 8, 15).expect("valid date");
        let code = airac_code_from_date(date);
        assert_eq!(code.len(), 4);

        let effective = effective_date_from_airac_code(&code).expect("valid effective date");
        assert!(effective <= date);
        assert!(date < effective + Duration::days(28));

        let (begin, end) = airac_interval(&code).expect("valid interval");
        assert_eq!(begin, effective);
        assert_eq!(end, begin + Duration::days(28));
    }

    #[test]
    fn rejects_invalid_airac_codes() {
        assert!(effective_date_from_airac_code("ABCD").is_err());
        assert!(effective_date_from_airac_code("2515").is_err());
        assert!(effective_date_from_airac_code("250").is_err());
    }
}
