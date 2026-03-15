pub mod settings;
pub mod send;

pub fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // Font definitions
    fonts.font_data.insert(
        "GoogleSansFlex-Regular".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../fonts/gsans_regular.ttf"))),
    );

    fonts.font_data.insert(
        "GoogleSansFlex-Bold".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../fonts/gsans_bold.ttf"))),
    );

    fonts.font_data.insert(
        "GoogleSansCode".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!("../fonts/gsans_code.ttf"))),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "GoogleSansFlex-Regular".to_owned());

    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("GoogleSansCode".to_owned());

    // Create a dedicated font family for Headers (used for titles)
    fonts.families.insert(
        egui::FontFamily::Name("Heading".into()),
        vec!["GoogleSansFlex-Bold".to_owned()],
    );

    ctx.set_fonts(fonts);
}

pub fn configure_styles(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    
    // Larger basic font size
    for (_text_style, font_id) in style.text_styles.iter_mut() {
        font_id.size *= 1.1; 
    }
    
    // More spacing
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.window_margin = egui::Margin::same(20);
    
    ctx.set_style(style);
}