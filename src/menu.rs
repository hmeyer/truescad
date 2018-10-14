extern crate gtk;

use gtk::traits::*;

pub fn create_menu<
    FT: Fn() + 'static,
    FO: Fn() + 'static,
    FS: Fn() + 'static,
    FSA: Fn() + 'static,
    FSET: Fn() + 'static,
    FEX: Fn() + 'static,
    FQ: Fn() + 'static,
>(
    tessellate_action: FT,
    open_action: FO,
    save_action: FS,
    save_as_action: FSA,
    settings_action: FSET,
    export_action: FEX,
    quit_action: FQ,
) -> gtk::MenuBar {
    let menu_bar = gtk::MenuBar::new();
    let file = gtk::MenuItem::new_with_mnemonic("_File");
    let f_menu = gtk::Menu::new();
    let f_new = gtk::MenuItem::new_with_mnemonic("_New");
    let f_open = gtk::MenuItem::new_with_mnemonic("_Open");
    let f_save = gtk::MenuItem::new_with_mnemonic("_Save");
    let f_save_as = gtk::MenuItem::new_with_mnemonic("Save _as");
    let f_tessellate = gtk::MenuItem::new_with_mnemonic("_Tessellate");
    let f_export_stl = gtk::MenuItem::new_with_mnemonic("_Export STL");
    let f_settings = gtk::MenuItem::new_with_mnemonic("_Settings");
    let f_quit = gtk::MenuItem::new_with_mnemonic("_Quit");

    f_open.connect_activate(move |_| {
        open_action();
    });
    f_save.connect_activate(move |_| {
        save_action();
    });
    f_save_as.connect_activate(move |_| {
        save_as_action();
    });
    f_tessellate.connect_activate(move |_| {
        tessellate_action();
    });
    f_export_stl.connect_activate(move |_| {
        export_action();
    });
    f_settings.connect_activate(move |_| {
        settings_action();
    });
    f_quit.connect_activate(move |_| {
        quit_action();
    });
    let help = gtk::MenuItem::new_with_mnemonic("_Help");
    let h_menu = gtk::Menu::new();
    let h_about = gtk::MenuItem::new_with_mnemonic("A_bout");

    f_menu.append(&f_new);
    f_menu.append(&f_open);
    f_menu.append(&f_save);
    f_menu.append(&f_save_as);
    f_menu.append(&f_tessellate);
    f_menu.append(&f_export_stl);
    f_menu.append(&f_settings);
    f_menu.append(&f_quit);
    file.set_submenu(Some(&f_menu));
    menu_bar.append(&file);

    h_menu.append(&h_about);
    help.set_submenu(Some(&h_menu));
    menu_bar.append(&help);
    menu_bar
}
