use gtk::prelude::*;
use gtk::SpinButton;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::rc::Rc;

const SETTINGS_FILENAME: &str = ".truescad";

macro_rules! add_setting {
    ($field :ident, $data :expr) => {{
        let data_clone = $data.clone();
        let h_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let label = gtk::Label::with_mnemonic(stringify!($field));
        let setting = SpinButton::with_range(0.0001, 1000., 0.01);
        setting.set_value($data.borrow().$field);
        setting.connect_value_changed(move |f: &SpinButton| {
            data_clone.borrow_mut().$field = f.value();
        });
        h_box.pack_start(&label, true, false, 5);
        h_box.pack_start(&setting, true, false, 5);
        h_box
    }};
}

pub fn show_settings_dialog<T: gtk::prelude::IsA<gtk::Window>>(parent: Option<&T>) {
    let data = Rc::new(RefCell::new(SettingsData::default()));

    let dialog = gtk::Dialog::with_buttons(
        Some("Settings"),
        parent,
        gtk::DialogFlags::MODAL,
        &[
            ("OK", gtk::ResponseType::Ok),
            ("Cancel", gtk::ResponseType::Cancel),
        ],
    );
    dialog
        .content_area()
        .add(&add_setting!(tessellation_resolution, &data));
    dialog
        .content_area()
        .add(&add_setting!(tessellation_error, &data));
    dialog
        .content_area()
        .add(&add_setting!(fade_range, &data));
    dialog
        .content_area()
        .add(&add_setting!(r_multiplier, &data));

    dialog.show_all();
    let ret = dialog.run();

    if ret == gtk::ResponseType::Ok {
        data.borrow().save();
    }
    dialog.close();
}

#[derive(Serialize, Deserialize)]
pub struct SettingsData {
    pub tessellation_resolution: f64,
    pub tessellation_error: f64,
    pub fade_range: f64,
    pub r_multiplier: f64,
}

#[derive(Debug)]
enum SettingsError {
    Io(std::io::Error),
    Dec(toml::de::Error),
    Enc(toml::ser::Error),
}

impl SettingsData {
    fn path() -> Result<std::path::PathBuf, SettingsError> {
        let mut path = match dirs::home_dir() {
            Some(p) => p,
            None => std::env::current_dir().map_err(SettingsError::Io)?,
        };
        path.push(SETTINGS_FILENAME);
        Ok(path)
    }
    fn get_toml() -> Result<Self, SettingsError> {
        let path = SettingsData::path()?;
        let f = File::open(path).map_err(SettingsError::Io)?;
        let mut reader = BufReader::new(f);
        let mut buffer = String::new();
        reader
            .read_to_string(&mut buffer)
            .map_err(SettingsError::Io)?;
        toml::from_str(&buffer).map_err(SettingsError::Dec)
    }

    fn put_toml(&self) -> Result<(), SettingsError> {
        let toml_str = toml::to_string(self).map_err(SettingsError::Enc)?;
        let path = SettingsData::path()?;
        let file = File::create(path).map_err(SettingsError::Io)?;
        let mut writer = BufWriter::new(file);
        writer.write(toml_str.as_bytes()).map_err(SettingsError::Io)?;
        Ok(())
    }

    pub fn save(&self) {
        match self.put_toml() {
            Ok(_) => {}
            Err(e) => println!("error writing settings: {:?}", e),
        }
    }
}

impl Default for SettingsData {
    fn default() -> SettingsData {
        match SettingsData::get_toml() {
            Ok(c) => c,
            Err(e) => {
                println!("error reading settings: {:?}", e);
                SettingsData {
                    tessellation_resolution: 0.12,
                    tessellation_error: 2.,
                    fade_range: 0.1,
                    r_multiplier: 1.0,
                }
            }
        }
    }
}
