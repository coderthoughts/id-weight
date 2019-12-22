extern crate clap;
extern crate chrono;
#[macro_use]
extern crate lazy_static;
extern crate regex;
extern crate serial;

use clap::{Arg, App};
use chrono::offset::Utc;
use chrono::DateTime;
use regex::{Captures, Regex};
use serial::prelude::*;
use std::io::prelude::*;
use std::process;

use std::fs::File;
use std::io;
use std::io::{Error, ErrorKind, Result};
use std::path::Path;
use std::time::Duration;

use std::io::BufReader;

fn main() {
    let matches = App::new("Weight Reader")
        .version("0.1")
        .arg(Arg::with_name("scale").short("s").long("scale").value_name("DEVICE")
            .help("The weight scale device to use, e.g. COM4 or /dev/ttyUSB0")
            .takes_value(true))
        .arg(Arg::with_name("dir").short("d").long("dir").value_name("DIR")
            .help("The output directory for written files")
            .takes_value(true))
        .arg(Arg::with_name("file").short("f").long("file-name").value_name("NAME")
            .help("The name of the output file")
            .takes_value(true))
        .arg(Arg::with_name("extension").short("e").long("ext").value_name("EXT")
            .help("The extenion of the output file, without preceding dot")
            .takes_value(true))
        .arg(Arg::with_name("test").short("t").long("test")
            .help("Use test data instead of reading from scale device")
            .takes_value(false))
        .get_matches();

    let device;
    let test = matches.is_present("test");
    if !test {
        if let Some(d) = matches.value_of("scale") {
            device = d;
        } else {
            println!("If not using test data you must specify a scale device\n{}\n\nUse -h to get help.", matches.usage());
            process::exit(1);
        }
    } else {
        device = "unused";
    }

    let dir = matches.value_of("dir").unwrap_or(".");
    let file = matches.value_of("file").unwrap_or("read_weight");
    let ext = matches.value_of("extension").unwrap_or("csv");
    let test = matches.is_present("test");

    println!("Args: w: {} d: {} f: {} e: {} t: {}", device, dir, file, ext, test);

    let weight;
    if test {
        println!("Using test data, not reading from device");
        weight = 423;
    } else {
        let mut port = serial::open(&device).unwrap();
        weight = read_from_scale(&mut port, &device).unwrap();
    }

    write_weight_to_file(format!("{}/{}.{}", dir, file, ext).as_str(), weight).unwrap();
}

fn read_from_scale<T: SerialPort>(port: &mut T, device_name: &str) -> io::Result<u32> {
    port.reconfigure(&|settings| {
        settings.set_baud_rate(serial::Baud9600)?;
        settings.set_char_size(serial::Bits8);
        settings.set_parity(serial::ParityNone);
        settings.set_stop_bits(serial::Stop1);
        settings.set_flow_control(serial::FlowNone);
        Ok(())
    })?;

    port.set_timeout(Duration::from_millis(1000))?;

    let reader = BufReader::new(port);
    let mut s = String::new();
    for line in reader.lines() {
        if line.is_ok() {
            // let read = line.unwrap_or("Reading failed".into());
            let read = line?;
            println!("{:?}", &read);
            s.push_str(&read);
            s.push_str("\n");

            if let Some((w, _z)) = parse_scale_data(&s) {
                println!("Found the weight: {}", w);
                return Ok(w);
            }
        }
    }
    Err(Error::new(ErrorKind::InvalidInput, 
        format!("Unable to read from device {}", device_name)))
}

fn parse_scale_data(data: &str) -> Option<(u32,u32)> {
    lazy_static! {
        static ref RE: Regex = Regex::new(r#"(?x)
            \s+Date:\s+\d\d[.]\d\d[.]\d\d\n
            \s+Time:\s+\d\d[:]\d\d[:]\d\d\n
            \s+Gross\s+(?P<w>\d+)kg"#).unwrap();
    }
    match RE.captures(data) {
        Some(c) => Some((
            read_match_as_u32(&c, "w"), 0)),
        None => None
    }
}

fn read_match_as_u32(c: &Captures, n: &str) -> u32 {
    // TODO can this be cleaner without the unwraps?
    c.name(n).unwrap().as_str().parse::<u32>().unwrap()
}

fn write_weight_to_file(filepath: &str, weight: u32) -> Result<()> {
    let path = Path::new(filepath);
    let mut file = File::create(&path)?;

    let datetime: DateTime<Utc> = Utc::now();
    let dt = datetime.format("%d-%m-%Y, %T, ");
    let s = format!("Date, Time, Weight\n{}{}\n", dt, weight);
    file.write_all(s.as_bytes())?;
    println!("Written: {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_scale_data() {
        assert_eq!(None, parse_scale_data(""));
        assert_eq!(None, parse_scale_data(r#"  Date:   09.07.06
  Time:   01:13:39
"#));
        assert_eq!(Some((24,0)), parse_scale_data(r#"  Date:   09.07.06
  Time:   01:13:39
  Gross       24kg
"#));
        assert_eq!(Some((0,0)), parse_scale_data(r#"  Date:   09.07.13
  Time:   07:54:36
  Gross        0kg

"#));
    }
}
