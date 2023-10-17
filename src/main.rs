use std::error::Error;
use std::path::PathBuf;
use std::fs::File;
use std::io::{LineWriter,Write};
use serde::{Serialize,Deserialize};
use config::Config as CConfig;
use log::{info,debug,LevelFilter};
use log4rs::append::console::ConsoleAppender;
use log4rs::encode::pattern::PatternEncoder;
use log4rs::config::{Appender, Root};
use log4rs::Config;
use clap::{Arg, ArgAction, Command};

#[derive(Serialize, Deserialize,Debug)]
pub struct STResponse {
	#[serde(rename = "OBJECT_NAME")]
	pub object_name: Option<String>,
	#[serde(rename = "OBJECT_ID")]
	pub international_designator: Option<String>,
	#[serde(rename = "NORAD_CAT_ID")]
	pub norad_id: String,
	#[serde(rename = "EPOCH")]
	pub datetime: chrono::naive::NaiveDateTime,
	#[serde(rename = "REV_AT_EPOCH")]
	pub revolution_number: String,
	#[serde(rename = "TLE_LINE1")]
	pub line_1: String,
	#[serde(rename = "TLE_LINE2")]
	pub line_2: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
	pub username: String,
	pub password: String,
	pub norad_ids: Vec<u32>,
	pub connection_timeout: u32,
	pub connection_read_timeout: u32,
	pub connection_retries: u8,
	pub output_filename: String,
	pub output_directory: String,
}

impl Settings {
	pub fn new(config_file: &str) -> Result<Self, Box<dyn Error>> {
		let settings = CConfig::builder()
			.add_source(config::File::with_name(config_file))
			.build()?;
		Ok(settings.try_deserialize()?)
	}
}

fn main() -> Result<(),Box<dyn Error>> {
	let stdout = ConsoleAppender::builder().build();
	info!("Starting up");
	let config = Config::builder()
		.appender(Appender::builder().build("stdout", Box::new(stdout)))
		.build(Root::builder().appender("stdout").build(LevelFilter::Info))?;
	let loghandle = log4rs::init_config(config)?;
	let matches = Command::new("tle_retriever")
		.about("TLE Retriever Service")
		.version("0.0.1")
		.subcommand_required(false)
		.arg_required_else_help(true)
		.author("kristian.desjardins@nrcan-rncan.gc.ca")
				.arg(
					Arg::new("config")
						.short('c')
						.long("config")
						.action(ArgAction::Set)
						.required(true)
						.num_args(1)
					)
				.arg(
					Arg::new("loglevel")
						.short('l')
						.long("loglevel")
						.help("logging level off, error, info, debug, trace")
						.action(ArgAction::Set)
				)
	.get_matches();
	if let Some(loglevel) = matches.get_one::<String>("loglevel") {
		let lfilter = match loglevel.as_str() {
			"off"   => LevelFilter::Off,
			"error" => LevelFilter::Error,
			"warn"  => LevelFilter::Warn,
			"info"  => LevelFilter::Info,
			"debug" => LevelFilter::Debug,
			"trace" => LevelFilter::Trace,
			&_      => return Err("Invalid loglevel, needs to be one of: off,error,warn,info,debug or trace".into()),
		};
		let stdout = ConsoleAppender::builder()
			.encoder(Box::new(PatternEncoder::new("{d(%Y-%m-%d %T%.3f)(utc)} [{l}] - {m}{n}")))
			.build();
		let config = Config::builder()
			.appender(Appender::builder().build("stdout", Box::new(stdout)))
			.build(Root::builder().appender("stdout").build(lfilter))
			.unwrap();
		loghandle.set_config(config);
	}
	let config_file = matches.get_one::<String>("config").unwrap();
	let settings = Settings::new(config_file)?;
	debug!("{:#?}",settings);

	// construct output_filename
	let mut filename = PathBuf::from(settings.output_directory);
	filename.push(settings.output_filename);
	info!("Creating output file {}",filename.display());
	let file = File::create(filename)?;
    let mut file = LineWriter::new(file);

	let mut query = "https://www.space-track.org/basicspacedata/query/class/gp/NORAD_CAT_ID/".to_string();
	let nids: String = settings.norad_ids.into_iter().map(|n| n.to_string()).collect::<Vec<_>>().join(",");
	query.push_str(&nids);
	query.push_str("/orderby/TLE_LINE1%20ASC/format/json");
	let response = ureq::post("https://www.space-track.org/ajaxauth/login").send_form(&[
		("identity", &settings.username),
		("password", &settings.password),
		("query", &query),
	])?;
	let sts:Vec<STResponse> = response.into_json()?;
	for resp in sts {
		let name = resp.object_name.unwrap_or("Unknown".to_string());
		let line1 = resp.line_1;
		let line2 = resp.line_2;
		file.write_all(name.as_bytes())?;
		file.write_all(b"\n")?;
		file.write_all(line1.as_bytes())?;
		file.write_all(b"\n")?;
		file.write_all(line2.as_bytes())?;
		file.write_all(b"\n")?;
	}
	Ok(())
}
