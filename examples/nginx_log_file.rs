use std::{
    fs::File,
    io::{BufRead, BufReader},
    net::IpAddr,
    str::FromStr,
};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use polars::{
    io::SerReader,
    prelude::{CsvReadOptions, ParquetWriter},
};
use regex::Regex;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct NginxLog {
    addr: IpAddr,
    datetime: DateTime<Utc>,
    method: String,
    url: String,
    protocol: String,
    status: u16,
    body_bytes: u64,
    referer: String,
    user_agent: String,
}

fn main() -> Result<()> {
    let file = "./fixtures/nginx_access.log";
    let file = File::open(file)?;
    let reader = BufReader::new(file);

    let csv_file = std::fs::File::create("fixtures/nginx_access.csv")?;
    let mut csv_writer = csv::Writer::from_writer(csv_file);
    for line in reader.lines().map_while(Result::ok) {
        let log = parse_nginx_log(&line).map_err(|e| anyhow!("Failed to parse log: {:?}", e))?;
        csv_writer.serialize(log)?;
        csv_writer.flush()?;
    }

    let mut df = CsvReadOptions::default()
        .try_into_reader_with_file_path(Some("fixtures/nginx_access.csv".into()))?
        .finish()?;

    let mut file = std::fs::File::create("./fixtures/nginx_access.parquet")?;
    ParquetWriter::new(&mut file).finish(&mut df).unwrap();

    Ok(())
}

fn parse_nginx_log(s: &str) -> Result<NginxLog> {
    let re = Regex::new(
        r#"^(?<ip>\S+)\s+\S+\s+\S+\s+\[(?<date>[^\]]+)\]\s+"(?<method>\S+)\s+(?<url>\S+)\s+(?<proto>[^"]+)"\s+(?<status>\d+)\s+(?<bytes>\d+)\s+"(?<referer>[^"]+)"\s+"(?<ua>[^"]+)"$"#,
    )?;
    let cap = re.captures(s).ok_or(anyhow!("parse error"))?;

    let addr = cap
        .name("ip")
        .map(|m| m.as_str())
        .ok_or(anyhow!("parse ip error"))?;
    let addr = IpAddr::from_str(addr)?;
    let datetime = cap
        .name("date")
        .map(|m| m.as_str())
        .ok_or(anyhow!("parse date error"))?;
    let datetime = DateTime::parse_from_str(datetime, "%d/%b/%Y:%H:%M:%S %z")
        .map(|dt| dt.with_timezone(&Utc))?;
    let method = cap
        .name("method")
        .map(|m| m.as_str().to_string())
        .ok_or(anyhow!("parse method error"))?;
    let url = cap
        .name("url")
        .map(|m| m.as_str().to_string())
        .ok_or(anyhow!("parse url error"))?;
    let protocol = cap
        .name("proto")
        .map(|m| m.as_str().to_string())
        .ok_or(anyhow!("parse proto error"))?;
    let status = cap
        .name("status")
        .map(|m| m.as_str().parse())
        .ok_or(anyhow!("parse status error"))??;
    let body_bytes = cap
        .name("bytes")
        .map(|m| m.as_str().parse())
        .ok_or(anyhow!("parse bytes error"))??;
    let referer = cap
        .name("referer")
        .map(|m| m.as_str().to_string())
        .ok_or(anyhow!("parse referer error"))?;
    let user_agent = cap
        .name("ua")
        .map(|m| m.as_str().to_string())
        .ok_or(anyhow!("parse ua error"))?;

    Ok(NginxLog {
        addr,
        datetime,
        method,
        url,
        protocol,
        status,
        body_bytes,
        referer,
        user_agent,
    })
}
