/*
 * Copyright 2021 Aon Cyber Solutions
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

pub mod tsv_writer;
pub mod xlsx_writer;

use clap::{arg_enum, value_t, App, Arg};
use notatin::{
    cli_util::parse_paths,
    err::Error,
    filter::FilterBuilder,
    parser::ParserIterator,
    parser_builder::ParserBuilder,
    progress, util,
};
use std::{
    fs::File,
    io::{BufWriter, Write},
};
use tsv_writer::WriteTsv;
use xlsx_writer::WriteXlsx;

fn main() -> Result<(), Error> {
    let matches = App::new("Notatin Registry Dump")
        .version("0.2")
        .arg(Arg::from_usage(
            "-r --recover 'Recover deleted and versioned keys and values'",
        ))
        .arg(Arg::from_usage(
            "--recovered-only 'Only export recovered items (applicable for tsv and xlsx output only)'",
        ))
        .arg(Arg::from_usage(
            "--full-field-info 'Get the offset and length for each key/value field (applicable for jsonl output only)'",
        ))
        .arg(Arg::from_usage(
            "-f --filter=[STRING] 'Key path for filter (ex: \'ControlSet001\\Services\')'",
        ))
        .arg(
            Arg::with_name("input")
                .short("i")
                .long("input")
                .value_name("FILE(S)")
                .help("Base registry file with optional transaction log(s) (Comma separated list)")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output")
                .value_name("FILE")
                .help("Output file")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::from_usage("<TYPE> 'output type'")
                .short("t")
                .possible_values(&OutputType::variants())
                .case_insensitive(true)
                .default_value("jsonl"),
        )
        .get_matches();

    let (input, logs) = parse_paths(matches.value_of("input").expect("Required value"));
    let output = matches.value_of("output").expect("Required value");
    let recover = matches.is_present("recover");
    let recovered_only = matches.is_present("recovered-only");
    let get_full_field_info = matches.is_present("full-field-info");
    let output_type = value_t!(matches, "TYPE", OutputType).unwrap_or_else(|e| e.exit());

    let mut parser_builder = ParserBuilder::from_path(input);
    parser_builder.update_console(true);
    parser_builder.recover_deleted(recover);
    parser_builder.get_full_field_info(get_full_field_info);
    for log in logs.unwrap_or_default() {
        parser_builder.with_transaction_log(log);
    }
    let parser = parser_builder.build()?;

    let filter = match matches.value_of("filter") {
        Some(f) => Some(
            FilterBuilder::new()
                .add_key_path(f)
                .return_child_keys(true)
                .build()?,
        ),
        None => None,
    };

    let mut console = progress::new(true);
    console.write("Writing file")?;
    if output_type == OutputType::Xlsx {
        WriteXlsx::new(output, recovered_only).write(&parser, filter)?;
    } else if output_type == OutputType::Tsv {
        WriteTsv::new(output, recovered_only)?.write(&parser, filter)?;
    } else {
        let write_file = File::create(output)?;
        if output_type == OutputType::Common {
            util::write_common_export_format(&parser, filter, write_file)?;
        } else {
            let mut iter = ParserIterator::new(&parser);
            if let Some(filter) = filter {
                iter.with_filter(filter);
            }
            let mut writer = BufWriter::new(write_file);
            for (index, key) in iter.iter().enumerate() {
                console.update_progress(index)?;
                writeln!(&mut writer, "{}", serde_json::to_string(&key).unwrap())?;
            }
        }
    }
    console.write(&format!("\nFinished writing {}\n", output))?;
    Ok(())
}

arg_enum! {
    #[derive(PartialEq, Debug)]
    pub enum OutputType {
        Jsonl,
        Common,
        Tsv,
        Xlsx
    }
}
