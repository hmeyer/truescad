use super::Float;
use gtk::prelude::*;
use crate::mesh_view;
use nalgebra as na;
use crate::object_widget;
use crate::settings;
use sourceview4::prelude::*;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter};
use tessellation::{ImplicitFunction, ManifoldDualContouring, Mesh};
use truescad_luascad;
use truescad_luascad::implicit3d;

#[derive(Clone)]
pub struct Editor {
    pub widget: gtk::ScrolledWindow,
    source_view: sourceview4::View,
    buffer: Option<sourceview4::Buffer>,
}

struct ObjectAdaptor {
    implicit: Box<dyn implicit3d::Object<Float>>,
    resolution: Float,
}

impl ImplicitFunction<Float> for ObjectAdaptor {
    fn bbox(&self) -> &implicit3d::BoundingBox<Float> {
        self.implicit.bbox()
    }
    fn value(&self, p: &na::Point3<Float>) -> Float {
        self.implicit.approx_value(p, self.resolution)
    }
    fn normal(&self, p: &na::Point3<Float>) -> na::Vector3<Float> {
        self.implicit.normal(p)
    }
}

impl Editor {
    pub fn new(xw: &object_widget::ObjectWidget, debug_buffer: &gtk::TextBuffer) -> Editor {
        let widget = gtk::ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
        let mut buffer = None;
        let mut src_view = sourceview4::View::new();
        if let Some(lang_mgr) = sourceview4::LanguageManager::default() {
            let lang_search_paths = lang_mgr.search_path();
            let mut lang_search_paths_str: Vec<&str> =
                lang_search_paths.iter().map(|s| s.as_str()).collect();
            lang_search_paths_str.push("./language-specs/");
            lang_mgr.set_search_path(&lang_search_paths_str);
            if let Some(lua) = lang_mgr.language("truescad-lua") {
                if let Some(style_mgr) = sourceview4::StyleSchemeManager::default() {
                    style_mgr.append_search_path("./styles/");
                    if let Some(scheme) = style_mgr.scheme("build") {
                        let b = sourceview4::Buffer::with_language(&lua);
                        b.set_highlight_syntax(true);
                        b.set_style_scheme(Some(&scheme));
                        src_view = sourceview4::View::with_buffer(&b);
                        buffer = Some(b);
                    } else {
                        println!("failed to get scheme.");
                    }
                } else {
                    println!("failed to get default StyleSchemeManager.");
                }
            } else {
                println!("failed to get lang.");
            }
        } else {
            println!("failed to get default LanguageManager.");
        }
        src_view.set_monospace(true);
        widget.add(&src_view);
        let renderer = xw.renderer.clone();
        let drawing_area = xw.drawing_area.clone();
        let debug_buffer_clone = debug_buffer.clone();
        let editor = Editor {
            widget,
            source_view: src_view,
            buffer,
        };
        let editor_clone = editor.clone();

        editor.source_view.connect_key_release_event(
            move |_: &sourceview4::View, key: &gdk::EventKey| -> glib::Propagation {
                if key.keyval() == gdk::keys::constants::F5 {
                    // compile
                    let mut output = Vec::new();
                    let obj = editor_clone.get_object(&mut output);
                    debug_buffer_clone.set_text(&String::from_utf8(output).unwrap());
                    renderer.borrow_mut().set_object(obj);
                    drawing_area.queue_draw();
                }
                glib::Propagation::Proceed
            },
        );
        editor
    }
    fn get_object(&self, msg: &mut dyn Write) -> Option<Box<dyn implicit3d::Object<Float>>> {
        let code_buffer = self.source_view.buffer().unwrap();
        let code_text = code_buffer
            .text(
                &code_buffer.start_iter(),
                &code_buffer.end_iter(),
                true,
            )
            .unwrap();
        match truescad_luascad::eval(&code_text) {
            Ok((print_result, maybe_object)) => {
                writeln!(msg, "{}", print_result).unwrap();
                match maybe_object {
                    Some(mut o) => {
                        let s = settings::SettingsData::default();
                        o.set_parameters(&implicit3d::PrimitiveParameters {
                            fade_range: s.fade_range,
                            r_multiplier: s.r_multiplier,
                        });
                        Some(o)
                    }
                    None => {
                        writeln!(msg, "\nwarning : no object - did you call build()?").unwrap();
                        None
                    }
                }
            }
            Err(x) => {
                writeln!(msg, "\nerror : {:?}", x).unwrap();
                None
            }
        }
    }
    pub fn open(&self, filename: &str) {
        let open_result = File::open(filename);
        if let Ok(f) = open_result {
            let reader = BufReader::new(f);
            let mut buffer = String::new();
            for line in reader.lines() {
                if let Ok(line) = line {
                    buffer.push_str(&line);
                    buffer.push('\n');
                }
            }
            self.source_view.buffer().unwrap().set_text(&buffer);
        } else {
            println!("could not open {:?}: {:?}", &filename, open_result);
        }
    }
    pub fn save(&self, filename: &str) {
        save_from_sourceview(&self.source_view, filename);
    }
    pub fn tessellate(&self) -> Option<Mesh<Float>> {
        let maybe_obj = self.get_object(&mut ::std::io::stdout());
        if let Some(obj) = maybe_obj {
            let s = settings::SettingsData::default();
            let adaptor = ObjectAdaptor {
                implicit: obj,
                resolution: s.tessellation_resolution,
            };

            let mesh = ManifoldDualContouring::new(
                &adaptor,
                s.tessellation_resolution,
                s.tessellation_error,
            )
            .tessellate();
            if let Some(ref mesh) = mesh {
                mesh_view::show_mesh(mesh);
            }
            return mesh;
        }
        None
    }
}

fn save_from_sourceview(source_view: &sourceview4::View, filename: &str) {
    let open_result = File::create(filename);
    if let Ok(f) = open_result {
        let code_buffer = source_view.buffer().unwrap();
        let code_text = code_buffer
            .text(
                &code_buffer.start_iter(),
                &code_buffer.end_iter(),
                true,
            )
            .unwrap();
        let mut writer = BufWriter::new(f);
        let write_result = writer.write(code_text.as_bytes());
        println!("writing {:?}: {:?}", &filename, write_result);
    } else {
        println!(
            "opening for write {:?} failed: {:?}",
            &filename, open_result
        );
    }
}
