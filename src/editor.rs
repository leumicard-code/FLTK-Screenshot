use std::path::Path;

use anyhow::{Context, Result};
use gtk4::cairo;
use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Arrow,
    Rect,
    Blur,
    Crop,
}

#[derive(Debug, Clone, Copy)]
pub struct Annotation {
    pub tool: Tool,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

struct CropSnapshot {
    base_surface: cairo::ImageSurface,
    base_pixels: Vec<u8>,
    width: i32,
    height: i32,
    stride: usize,
    annotations: Vec<Annotation>,
}

pub struct EditorState {
    base_surface: cairo::ImageSurface,
    base_pixels: Vec<u8>, // עותק קריאה בלבד בפורמט BGRA לטשטוש מהיר
    width: i32,
    height: i32,
    stride: usize,
    pub current_tool: Tool,
    pub annotations: Vec<Annotation>,
    crop_history: Vec<CropSnapshot>,
    pub draft: Option<Annotation>, // הציור שנגרר כרגע עם העכבר
}

impl EditorState {
    /// טוען PNG לתוך Cairo ImageSurface (דרך gdk::Texture, בלי תלויות נוספות)
    pub fn from_file(path: &Path) -> Result<Self> {
        let texture = gdk::Texture::from_filename(path)
            .with_context(|| format!("Failed to load image: {}", path.display()))?;

        let width = texture.width();
        let height = texture.height();

        let mut surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)
            .context("Failed to create Cairo surface")?;
        let stride = surface.stride() as usize;

        let base_pixels = {
            let mut data = surface.data().context("Failed to lock Cairo surface")?;
            texture.download(&mut data, stride);
            data.to_vec()
        };
        surface.mark_dirty();

        Ok(Self {
            base_surface: surface,
            base_pixels,
            width,
            height,
            stride,
            current_tool: Tool::Arrow,
            annotations: Vec::new(),
            crop_history: Vec::new(),
            draft: None,
        })
    }

    pub fn can_undo(&self) -> bool {
        !self.annotations.is_empty() || !self.crop_history.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        if self.annotations.pop().is_some() {
            return true;
        }
        if let Some(snap) = self.crop_history.pop() {
            self.base_surface = snap.base_surface;
            self.base_pixels = snap.base_pixels;
            self.width = snap.width;
            self.height = snap.height;
            self.stride = snap.stride;
            self.annotations = snap.annotations;
            return true;
        }
        false
    }

    /// מחשב מיקום וסקייל כדי להציג את התמונה במרכז ה-DrawingArea בלי עיוות
    pub fn image_transform(&self, widget_w: f64, widget_h: f64) -> (f64, f64, f64) {
        let iw = self.width as f64;
        let ih = self.height as f64;
        if iw <= 0.0 || ih <= 0.0 || widget_w <= 0.0 || widget_h <= 0.0 {
            return (0.0, 0.0, 1.0);
        }
        let scale = (widget_w / iw).min(widget_h / ih);
        let offset_x = (widget_w - iw * scale) / 2.0;
        let offset_y = (widget_h - ih * scale) / 2.0;
        (offset_x, offset_y, scale)
    }

    /// מבצע חיתוך אמיתי על התמונה והציורים הקיימים (מחזיר true אם האזור תקין ונחתך)
    pub fn apply_crop(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) -> Result<bool> {
        let cx = (x1.min(x2).floor() as i32).clamp(0, self.width);
        let cy = (y1.min(y2).floor() as i32).clamp(0, self.height);
        let cx2 = (x1.max(x2).ceil() as i32).clamp(0, self.width);
        let cy2 = (y1.max(y2).ceil() as i32).clamp(0, self.height);

        let cw = cx2 - cx;
        let ch = cy2 - cy;
        if cw < 10 || ch < 10 {
            return Ok(false); // התעלמות מלחיצה אקראית קטנה מדי
        }

        let mut new_surface = cairo::ImageSurface::create(cairo::Format::ARgb32, cw, ch)
            .context("Failed to create cropped surface")?;
        {
            let cr = cairo::Context::new(&new_surface).context("Failed to create Cairo context")?;
            cr.translate(-(cx as f64), -(cy as f64));
            self.render_image_space(&cr);
        }
        new_surface.flush();

        let stride = new_surface.stride() as usize;
        let base_pixels = new_surface
            .data()
            .context("Failed to lock cropped surface")?
            .to_vec();

        let prev_surface = std::mem::replace(&mut self.base_surface, new_surface);
        let prev_pixels = std::mem::replace(&mut self.base_pixels, base_pixels);
        let prev_annotations = std::mem::take(&mut self.annotations);
        self.crop_history.push(CropSnapshot {
            base_surface: prev_surface,
            base_pixels: prev_pixels,
            width: self.width,
            height: self.height,
            stride: self.stride,
            annotations: prev_annotations,
        });

        self.width = cw;
        self.height = ch;
        self.stride = stride;
        Ok(true)
    }

    /// ממיר קואורדינטות עכבר בחלון לקואורדינטות פיקסלים בתמונה המקורית
    pub fn widget_to_image(&self, wx: f64, wy: f64, widget_w: f64, widget_h: f64) -> (f64, f64) {
        let (ox, oy, scale) = self.image_transform(widget_w, widget_h);
        let ix = ((wx - ox) / scale).clamp(0.0, self.width as f64);
        let iy = ((wy - oy) / scale).clamp(0.0, self.height as f64);
        (ix, iy)
    }

    /// מצייר על ה-DrawingArea (כולל מרכוז וסקייל לפי גודל החלון)
    pub fn draw_on_widget(&self, cr: &cairo::Context, widget_w: f64, widget_h: f64) {
        let (ox, oy, scale) = self.image_transform(widget_w, widget_h);
        let _ = cr.save();
        cr.translate(ox, oy);
        cr.scale(scale, scale);
        self.render_image_space(cr);
        let _ = cr.restore();
    }

    /// מרנדר את התמונה + כל הציורים ברזולוציה המקורית ל-gdk::Texture (לשמירה ול-Clipboard)
    pub fn export_texture(&self) -> Result<gdk::Texture> {
        let mut out = cairo::ImageSurface::create(cairo::Format::ARgb32, self.width, self.height)
            .context("Failed to create export surface")?;
        {
            let cr = cairo::Context::new(&out).context("Failed to create Cairo context")?;
            self.render_image_space(&cr);
        }
        out.flush();

        let stride = out.stride() as usize;
        let data = out.data().context("Failed to borrow export surface")?;
        let bytes = glib::Bytes::from(&*data);

        let mem_tex = gdk::MemoryTexture::new(
            self.width,
            self.height,
            gdk::MemoryFormat::B8g8r8a8Premultiplied,
            &bytes,
            stride,
        );
        Ok(mem_tex.upcast())
    }

    fn render_image_space(&self, cr: &cairo::Context) {
        let _ = cr.set_source_surface(&self.base_surface, 0.0, 0.0);
        let _ = cr.paint();

        for ann in &self.annotations {
            self.draw_annotation(cr, ann);
        }
        if let Some(draft) = &self.draft {
            self.draw_annotation(cr, draft);
        }
    }

    fn draw_annotation(&self, cr: &cairo::Context, ann: &Annotation) {
        let stroke = ((self.width.max(self.height) as f64) * 0.003).clamp(3.0, 8.0);

        match ann.tool {
            Tool::Rect => {
                let x = ann.x1.min(ann.x2);
                let y = ann.y1.min(ann.y2);
                let w = (ann.x2 - ann.x1).abs();
                let h = (ann.y2 - ann.y1).abs();

                cr.set_source_rgb(0.95, 0.2, 0.2);
                cr.set_line_width(stroke);
                cr.rectangle(x, y, w, h);
                let _ = cr.stroke();
            }
            Tool::Crop => {
                let x = ann.x1.min(ann.x2);
                let y = ann.y1.min(ann.y2);
                let w = (ann.x2 - ann.x1).abs();
                let h = (ann.y2 - ann.y1).abs();

                let _ = cr.save();
                // החשכה של כל מה שמחוץ לאזור החיתוך
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.55);
                cr.set_fill_rule(cairo::FillRule::EvenOdd);
                cr.rectangle(0.0, 0.0, self.width as f64, self.height as f64);
                cr.rectangle(x, y, w, h);
                let _ = cr.fill();

                // מסגרת לבנה מקווקוות סביב הבחירה
                cr.set_source_rgb(1.0, 1.0, 1.0);
                cr.set_line_width((stroke * 0.6).max(2.0));
                cr.set_dash(&[8.0, 6.0], 0.0);
                cr.rectangle(x, y, w, h);
                let _ = cr.stroke();
                let _ = cr.restore();
            }
            Tool::Arrow => {
                let dx = ann.x2 - ann.x1;
                let dy = ann.y2 - ann.y1;
                if dx.hypot(dy) < 2.0 {
                    return;
                }

                let angle = dy.atan2(dx);
                let head_len = stroke * 4.5;
                let head_angle = 0.45_f64;

                cr.set_source_rgb(0.95, 0.2, 0.2);
                cr.set_line_width(stroke);
                cr.set_line_cap(cairo::LineCap::Round);

                // קו החץ (נעצר קצת לפני השפיץ כדי שיראה חד)
                let shaft_x = ann.x2 - (head_len * 0.7) * angle.cos();
                let shaft_y = ann.y2 - (head_len * 0.7) * angle.sin();
                cr.move_to(ann.x1, ann.y1);
                cr.line_to(shaft_x, shaft_y);
                let _ = cr.stroke();

                // ראש החץ (משולש מלא)
                let p1_x = ann.x2 - head_len * (angle - head_angle).cos();
                let p1_y = ann.y2 - head_len * (angle - head_angle).sin();
                let p2_x = ann.x2 - head_len * (angle + head_angle).cos();
                let p2_y = ann.y2 - head_len * (angle + head_angle).sin();

                cr.move_to(ann.x2, ann.y2);
                cr.line_to(p1_x, p1_y);
                cr.line_to(p2_x, p2_y);
                cr.close_path();
                let _ = cr.fill();
            }
            Tool::Blur => {
                self.draw_pixelate(cr, ann);
            }
        }
    }

    /// טשטוש פיקסלים (פסיפס) — מהיר ובלתי הפיך להסתרת סיסמאות / מידע רגיש
    fn draw_pixelate(&self, cr: &cairo::Context, ann: &Annotation) {
        let x0 = (ann.x1.min(ann.x2) as i32).clamp(0, self.width);
        let y0 = (ann.y1.min(ann.y2) as i32).clamp(0, self.height);
        let x1 = (ann.x1.max(ann.x2) as i32).clamp(0, self.width);
        let y1 = (ann.y1.max(ann.y2) as i32).clamp(0, self.height);

        let block = (self.width.max(self.height) / 130).clamp(12, 28);
        let mut by = y0;
        while by < y1 {
            let bh = (y1 - by).min(block);
            let mut bx = x0;
            while bx < x1 {
                let bw = (x1 - bx).min(block);
                let (mut r, mut g, mut b, mut count) = (0u64, 0u64, 0u64, 0u64);

                for py in by..(by + bh) {
                    let row = py as usize * self.stride;
                    for px in bx..(bx + bw) {
                        let idx = row + px as usize * 4;
                        if idx + 2 < self.base_pixels.len() {
                            b += self.base_pixels[idx] as u64;
                            g += self.base_pixels[idx + 1] as u64;
                            r += self.base_pixels[idx + 2] as u64;
                            count += 1;
                        }
                    }
                }

                if count > 0 {
                    cr.set_source_rgb(
                        (r / count) as f64 / 255.0,
                        (g / count) as f64 / 255.0,
                        (b / count) as f64 / 255.0,
                    );
                    cr.rectangle(bx as f64, by as f64, bw as f64, bh as f64);
                    let _ = cr.fill();
                }

                bx += block;
            }
            by += block;
        }
    }
}
