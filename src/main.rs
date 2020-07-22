//use ConfigParser::
//

use std::{env, fs, thread};
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::str;
use std::sync::Arc;
use std::vec::Vec;

use nix::{
    unistd,
    sys::stat::Mode
};

use ini::{
    Ini,
    ini::Properties
};

use serenity::{
    prelude::*,
    model::id::ChannelId
};

struct FifoConfig {
    section: String,       // Name of the config section
    fifo: String,          // File name of the FIFO created
    channel: u64           // Channel id where the message will be sent
}

struct DispipeConfig {
    token: String,
    root: String,
    fifo_configs: Vec<FifoConfig>
}

static MAIN_CONFIG_SECTION : &'static str = "Dispipe";

fn get_required_property(properties: &Properties, property: &str, section_name: &str) -> String {
    return properties
        .get(property)
        .expect(format!("Missing required property '{}' from section '{}'", property, section_name).as_str())
        .to_string();
}

/*
fn get_optional_property(properties: &Properties, property: &str) -> Option<String> {
    return properties
        .get(property)
        .map_or(None, |t| Some(t.to_string()));
}
*/

fn path_is_fifo(path: &str) -> bool {
    let metadata = fs::metadata(path)
        .expect(format!("Cannot read metadata at path: {}", path).as_str());

    return metadata.file_type().is_fifo();
}

fn load_config(configuration_path: String) -> DispipeConfig
{
    /* Config Structure:
     *
     * 1. The 'Dispipe' section is mandatory
     * 2. All other sections are optional "fifo configs"
     *
     * Example:
     *
     * [Dispipe]
     * token = ...                     ; Discord bot token
     * root = /var/dispipe             ; Directory containing the fifo files
     *
     *                                 ; A "fifo config" section
     * [Example]                       ; Section name for easy identification, printed on stdout
     * fifo = example.fifo             ; Name of the fifo file to listen on
     * channel = 99999999999999999     ; Channel ID to send messages to
     *
     * ...                             ; Any number of additional fifo configs can be created
     *
     */


    // Load INI
    let ini = Ini::load_from_file(configuration_path)
        .expect("INI not found");

    // Extract section with main options
    let main_section = ini
        .section(Some(MAIN_CONFIG_SECTION))
        .expect(format!("Missing main INI section: '{}'", MAIN_CONFIG_SECTION).as_str());

    // Extract required properties from main section
    let token = get_required_property(main_section, "token", MAIN_CONFIG_SECTION);
    let root = get_required_property(main_section, "root", MAIN_CONFIG_SECTION);

    // Load fifo configurations from INI
    let mut fifo_configs : Vec<FifoConfig> = Vec::new();
    for (section_name, properties) in ini.iter() {
        let section_name = section_name.unwrap();
        if section_name != MAIN_CONFIG_SECTION {

            // Get "fifo" and "channel" values
            let fifo = get_required_property(properties, "fifo", section_name);
            let channel = get_required_property(properties, "channel", section_name)
                .parse::<u64>()
                .expect("Channel should be an integer value");

            // Add config to struct
            fifo_configs.push(FifoConfig {
                fifo: fifo,
                channel: channel,
                section: section_name.to_string()
            });
        }
    }

    // Return the whole config
    return DispipeConfig {
        token: token,
        root: root,
        fifo_configs: fifo_configs
    }
}

fn validate_config(conf: &DispipeConfig) {
    let root = Path::new(&conf.root);
    assert!(root.is_dir(), "root must point to a directory");
    assert!(root.is_absolute(), "root path must be absolute");

    for fifo_config in &conf.fifo_configs {
        let path = Path::new(&conf.root).join(&fifo_config.fifo);
        let path_str = path.to_str().unwrap();
        if path.exists() {
            assert!(path_is_fifo(path_str), "File already exists at \"{}\". Please delete or move it.", path_str);
        }
    }
}

fn read_bytes_until_newline(fifo_path: &PathBuf) -> String {
    // Open the fifo at path. This will be closed when the function returns and re-opened when this
    // function is called again, which seems to work most reliably
    let file = File::open(&fifo_path)
        .expect(format!("Could not open fifo: {}", fifo_path.to_str().unwrap()).as_str());

    // Discord messages are 2000 characters (bytes) max
    let capacity = 2000;
    let mut buffer = Vec::with_capacity(capacity);

    // Read until input is newline or we reach capacity
    for byte in file.bytes() {
        let byte = byte.unwrap();
        buffer.push(byte);
        if byte == b'\n' || buffer.len() == capacity {
            break;
        }
    }
    return String::from_utf8(buffer).unwrap();
}

struct Handler;
impl EventHandler for Handler { }

fn main() {
    /* Algorithm:
     * - Translate INI file into DispipeConfig structure
     * - Create discord authenticated client using token from config
     * - For every fifo section in dispipe config:
     *   - Create the FIFO if it doesn't already exist
     *   - Spawn a thread reading a line from the the FIFO
     *      - When a line is found, send a message to the specified channel
     */

    // Get configuration path
    let configuration_path = env::args().nth(1)
        .expect("USAGE: ...");

    println!("Loading configuration '{}'", configuration_path);

    let conf = load_config(configuration_path);
    validate_config(&conf);

    println!("Initializing Discord client");

    let mut client = Client::new(&conf.token, Handler).expect("Error creating client");
    let http = &client.cache_and_http.http;
    let mut threads = vec![];

    for fifo_config in conf.fifo_configs
    {
        let fifo_name = fifo_config.fifo.clone();
        let section_name = fifo_config.section.clone();
        let fifo_path = Path::new(&conf.root).join(&fifo_name);
        let channel = fifo_config.channel;
        let my_http = Arc::clone(http);
        let fifo_path_str = fifo_path.to_str().unwrap();

        // If the file does not exist, make the fifo. We assume if a file exists it
        // must be a fifo, otherwise validate step would have failed.
        if !fifo_path.exists() {
            unistd::mkfifo(&fifo_path, Mode::S_IRUSR | Mode::S_IWUSR)
                .expect(format!("Error creating fifo at \"{}\"", &fifo_path_str).as_str());
        }

        println!("FIFO: {} -> #{}", fifo_path_str, channel);

        // Spawn a thread to read from the fifo
        let thread = thread::Builder::new()
            .name(fifo_config.section)
            .spawn(move || {

            // Read line-by-line from the from the fifo in loop
            loop {
                let message = read_bytes_until_newline(&fifo_path);
                let result = ChannelId(channel).say(&my_http, &message);
                match result {
                    Ok(_) => print!("{}|{}", &section_name, &message),
                    Err(why) => println!("Error: {}", why)
                }
            }
        });
        threads.push(thread);
    }

    client.start()
        .expect("The discord client could not start.");

    for thread in threads {
        match thread {
            Ok(th) => th.join().expect("Could not join thread."),
            Err(err) => println!("{}", err)
        }
    }
}
