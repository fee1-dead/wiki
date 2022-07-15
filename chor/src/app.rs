use std::{sync::Arc, future::Future};

use egui::{CentralPanel, ScrollArea, mutex::Mutex};

use crate::worker;

pub struct Page {

}

#[derive(Default)]
pub struct Ctxt {
    pub ctx: egui::Context,
}

impl Ctxt {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self { ctx: cc.egui_ctx.clone() }
    }
}

#[derive(Default)]
pub struct Chor {
    pub ctx: Arc<Ctxt>,
}

impl Chor {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> (Self, impl Future<Output = ()>) {
        // This is also where you can customized the look at feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.
        let ctx = Arc::new(Ctxt::new(cc));
        let c = ctx.clone();
        (Self { ctx }, worker(c))
    }
}

impl eframe::App for Chor {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                for s in "test se a te tea esea se se te teet fee teefi fii tm".split(' ') {
                    ui.label(s);
                }
            });
        });
    }
}