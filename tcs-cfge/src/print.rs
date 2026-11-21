//! Print, from the File menu: the working panes as drawn, trimmed to the
//! nodes and lines on them, made a PDF and opened in the desktop's PDF
//! viewer, whose Print chooses the printer and the paper.
//!
//! The window toolkit has no printing of its own, and drawing the sheet a
//! second time, for paper, would be a second answer to what the editor
//! draws. So the picture is the editor's own: the view is stepped across
//! the drawing at full size, a snapshot of the window taken at each step,
//! and the part of each that is the sheet -- not the selection lists beside
//! it, the bar above it or its scroll bars -- copied into one picture. The
//! view is put back where it was afterwards.
//!
//! A step is taken on a timer, so that the window has drawn the view at its
//! new place before the snapshot is taken of it.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use slint::ComponentHandle;

use crate::{Extent, MainWindow};

/// The zoom the drawing is printed at: full size, a sheet pixel to a
/// logical pixel, at which every type and name can be read.
const PRINT_ZOOM: f32 = 1.0;

/// What is left round the drawing on the paper, in the sheet's pixels.
const MARGIN: f32 = 16.0;

/// How much of the right and the bottom of the view is left out of each
/// snapshot, in the window's logical pixels: room for the scroll bars,
/// which are drawn over the sheet there.
const SCROLL_BARS: f32 = 20.0;

/// How long the window is given to draw the view at its new place before a
/// snapshot is taken of it.
const SETTLE: Duration = Duration::from_millis(60);

/// How many points, a PDF's unit, a sheet pixel is printed at: the sheet
/// drawn at 96 pixels an inch, as a screen draws it.
const POINTS_PER_PIXEL: f32 = 0.75;

thread_local! {
    /// Whether a print is being made now: a second asked for meanwhile is
    /// not started, the view being the first's until it is done.
    static PRINTING: Cell<bool> = const { Cell::new(false) };
}

/// A picture being put together, its pixels red, green and blue, a byte
/// each, row after row from the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// Its width, in pixels.
    pub width: usize,
    /// Its height, in pixels.
    pub height: usize,
    /// Its pixels.
    pub rgb: Vec<u8>,
}

impl Picture {
    /// Makes a white picture.
    ///
    /// # Arguments
    ///
    /// * `width`  - Its width, in pixels.
    /// * `height` - Its height, in pixels.
    ///
    /// # Returns
    ///
    /// The picture.
    pub fn white(width: usize, height: usize) -> Picture {
        Picture {
            width,
            height,
            rgb: vec![255; width * height * 3],
        }
    }

    /// Copies a part of a snapshot into the picture, what falls outside
    /// either left out.
    ///
    /// # Arguments
    ///
    /// * `from`      - The snapshot's pixels, red, green, blue and alpha, a
    ///                 byte each, row after row from the top.
    /// * `from_size` - The snapshot's width and height, in pixels.
    /// * `from_at`   - The top left of the part copied, in the snapshot.
    /// * `size`      - The part's width and height, in pixels.
    /// * `to_at`     - Where its top left goes, in the picture.
    pub fn copy_in(
        &mut self,
        from: &[u8],
        from_size: (usize, usize),
        from_at: (i64, i64),
        size: (i64, i64),
        to_at: (i64, i64),
    ) {
        let (fw, fh) = (from_size.0 as i64, from_size.1 as i64);
        let (tw, th) = (self.width as i64, self.height as i64);
        for row in 0..size.1.max(0) {
            let (fy, ty) = (from_at.1 + row, to_at.1 + row);
            if fy < 0 || fy >= fh || ty < 0 || ty >= th {
                continue;
            }
            for col in 0..size.0.max(0) {
                let (fx, tx) = (from_at.0 + col, to_at.0 + col);
                if fx < 0 || fx >= fw || tx < 0 || tx >= tw {
                    continue;
                }
                let f = ((fy * fw + fx) * 4) as usize;
                let t = ((ty * tw + tx) * 3) as usize;
                self.rgb[t..t + 3].copy_from_slice(&from[f..f + 3]);
            }
        }
    }
}

/// The places the view's top left is put at, one a step, so that the
/// steps together show every part of an area: across, then down.
///
/// # Arguments
///
/// * `area` - The area, in the sheet's pixels.
/// * `step` - How much of the sheet one step shows, across and down, in the
///            sheet's pixels.
///
/// # Returns
///
/// Each step's top left, in the sheet's pixels.
pub fn steps(area: &Extent, step: (f32, f32)) -> Vec<(f32, f32)> {
    let (across, down) = (step.0.max(1.0), step.1.max(1.0));
    let count = |length: f32, by: f32| (length / by).ceil().max(1.0) as usize;
    let mut out = Vec::new();
    for row in 0..count(area.height, down) {
        for col in 0..count(area.width, across) {
            out.push((
                area.x + col as f32 * across,
                area.y + row as f32 * down,
            ));
        }
    }
    out
}

/// An area grown by a margin all round.
///
/// # Arguments
///
/// * `area`   - The area.
/// * `margin` - How much.
///
/// # Returns
///
/// The larger area, or the area itself where it is empty.
pub fn grown(area: &Extent, margin: f32) -> Extent {
    if area.width <= 0.0 {
        return area.clone();
    }
    Extent {
        x: area.x - margin,
        y: area.y - margin,
        width: area.width + 2.0 * margin,
        height: area.height + 2.0 * margin,
    }
}

/// An area kept to the sheet, and taken up to its top, so that the panes'
/// names, along the top, are printed with the nodes under them.
///
/// # Arguments
///
/// * `area` - The area, in the sheet's pixels.
///
/// # Returns
///
/// The area from the sheet's top, and from its left edge rightwards.
pub fn on_the_sheet(area: &Extent) -> Extent {
    let (left, bottom) = (area.x.max(0.0), area.y + area.height);
    Extent {
        x: left,
        y: 0.0,
        width: area.x + area.width - left,
        height: bottom.max(0.0),
    }
}

/// A PDF of one page, filled by a picture.
///
/// # Arguments
///
/// * `picture` - The picture.
/// * `page`    - The page's width and height, in points.
/// * `title`   - What the document is called, for the viewer's title.
///
/// # Returns
///
/// The PDF's bytes.
pub fn pdf_of(picture: &Picture, page: (f32, f32), title: &str) -> Vec<u8> {
    let pixels = miniz_oxide::deflate::compress_to_vec_zlib(&picture.rgb, 6);
    let (w, h) = (page.0, page.1);
    let content = format!("q {w:.2} 0 0 {h:.2} 0 0 cm /Im0 Do Q\n");
    // The title, as a PDF string: its backslashes and brackets escaped, and
    // anything outside printable ASCII left out.
    let title: String = title
        .chars()
        .filter(|c| (' '..='~').contains(c))
        .flat_map(|c| match c {
            '\\' | '(' | ')' => vec!['\\', c],
            _ => vec![c],
        })
        .collect();
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    let mut object = |out: &mut Vec<u8>, body: &[u8]| {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", offsets.len()).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    object(&mut out, b"<< /Type /Catalog /Pages 2 0 R >>");
    object(&mut out, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>");
    object(
        &mut out,
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.2} {h:.2}] \
             /Resources << /XObject << /Im0 4 0 R >> >> /Contents 5 0 R >>"
        )
        .as_bytes(),
    );
    let mut image = format!(
        "<< /Type /XObject /Subtype /Image /Width {} /Height {} \
         /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode \
         /Length {} >>\nstream\n",
        picture.width,
        picture.height,
        pixels.len()
    )
    .into_bytes();
    image.extend_from_slice(&pixels);
    image.extend_from_slice(b"\nendstream");
    object(&mut out, &image);
    let mut contents =
        format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
    contents.extend_from_slice(content.as_bytes());
    contents.extend_from_slice(b"endstream");
    object(&mut out, &contents);
    object(&mut out, format!("<< /Title ({title}) >>").as_bytes());
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1)
            .as_bytes(),
    );
    for at in &offsets {
        out.extend_from_slice(format!("{at:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info {} 0 R >>\n\
             startxref\n{xref}\n%%EOF\n",
            offsets.len() + 1,
            offsets.len()
        )
        .as_bytes(),
    );
    out
}

/// Where the PDF is written: the system's temporary directory, under the
/// configuration's name.
///
/// # Arguments
///
/// * `name` - The configuration's file name, or empty.
///
/// # Returns
///
/// The path.
pub fn pdf_path(name: &str) -> PathBuf {
    let stem = Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("configuration");
    std::env::temp_dir().join(format!("tcs-cfge-print-{stem}.pdf"))
}

/// Opens a file in the desktop's own program for it, without waiting for
/// that program.
///
/// # Arguments
///
/// * `path` - The file.
///
/// # Errors
///
/// Where the opener cannot be started.
fn open_in_viewer(path: &Path) -> std::io::Result<()> {
    let mut command = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    command.arg(path).spawn().map(|_| ())
}

/// A print being made: where it has got to, and what it is made into.
struct Job {
    /// The area printed, in the sheet's pixels.
    area: Extent,
    /// Each step's top left, in the sheet's pixels.
    steps: Vec<(f32, f32)>,
    /// The next step taken.
    next: usize,
    /// The picture, as far as it has got.
    picture: Picture,
    /// The window's scale: physical pixels to a logical one.
    scale: f32,
    /// Where the view was, and the zoom chosen, to be put back.
    back: (f32, f32, f32),
    /// The configuration's file name, which the PDF is named after.
    name: String,
}

/// Prints the working panes: see the module.
///
/// # Arguments
///
/// * `window` - The editor's window.
/// * `area`   - Where on the sheet the drawing is, every node and line, in
///              the sheet's pixels; nothing is printed for an empty one.
/// * `name`   - The configuration's file name, which the PDF is named
///              after.
pub fn print(window: &MainWindow, area: Extent, name: &str) {
    if PRINTING.with(Cell::get) {
        return;
    }
    let Some(job) = job_for(window, area, name) else {
        return;
    };
    PRINTING.with(|p| p.set(true));
    window.set_printing(true);
    take_step(window.as_weak(), Rc::new(RefCell::new(job)));
}

/// A print of an area of the sheet, its steps planned and its picture
/// white.
///
/// # Arguments
///
/// * `window` - The editor's window.
/// * `area`   - Where on the sheet the drawing is, in the sheet's pixels.
/// * `name`   - The configuration's file name.
///
/// # Returns
///
/// The print, or none for an empty area or a view too small to step by.
fn job_for(window: &MainWindow, area: Extent, name: &str) -> Option<Job> {
    if area.width <= 0.0 {
        return None;
    }
    let area = on_the_sheet(&grown(&area, MARGIN));
    let scale = window.window().scale_factor();
    let step = (
        (window.get_view_width() - SCROLL_BARS) / PRINT_ZOOM,
        (window.get_view_height() - SCROLL_BARS) / PRINT_ZOOM,
    );
    if step.0 <= 0.0 || step.1 <= 0.0 {
        return None;
    }
    let size = |length: f32| (length * PRINT_ZOOM * scale).ceil() as usize;
    Some(Job {
        steps: steps(&area, step),
        next: 0,
        picture: Picture::white(size(area.width), size(area.height)),
        scale,
        back: (
            window.get_view_left(),
            window.get_view_top(),
            window.get_zoom_chosen(),
        ),
        area,
        name: name.to_string(),
    })
}

/// Makes a print's picture at once, without a timer between the steps,
/// for a window drawn by hand rather than by an event loop: the
/// screenshots' headless window. The view is put back afterwards.
///
/// # Arguments
///
/// * `window` - The editor's window.
/// * `area`   - Where on the sheet the drawing is, in the sheet's pixels.
/// * `draw`   - Draws the window, after the view is moved and before the
///              snapshot is taken.
///
/// # Returns
///
/// The picture, and the PDF made of it; none for an empty area.
///
/// # Errors
///
/// Where the window cannot be snapshotted.
#[cfg(test)]
pub fn made_now(
    window: &MainWindow,
    area: Extent,
    draw: &dyn Fn(&MainWindow),
) -> Result<Option<(Picture, Vec<u8>)>, String> {
    let Some(mut job) = job_for(window, area, "configuration") else {
        return Ok(None);
    };
    window.set_printing(true);
    for &(left, top) in &job.steps.clone() {
        window.invoke_show_at(left, top, PRINT_ZOOM);
        draw(window);
        capture(window, &mut job)?;
    }
    window.invoke_show_at(job.back.0, job.back.1, job.back.2);
    window.set_printing(false);
    let page = (
        job.area.width * POINTS_PER_PIXEL,
        job.area.height * POINTS_PER_PIXEL,
    );
    let pdf = pdf_of(&job.picture, page, "configuration");
    Ok(Some((job.picture, pdf)))
}

/// Puts the view at the job's next step and, once the window has drawn it,
/// copies the sheet from a snapshot into the picture; after the last step,
/// puts the view back and makes the PDF.
///
/// # Arguments
///
/// * `weak` - The editor's window.
/// * `job`  - The print.
fn take_step(weak: slint::Weak<MainWindow>, job: Rc<RefCell<Job>>) {
    let Some(window) = weak.upgrade() else {
        PRINTING.with(|p| p.set(false));
        return;
    };
    let next = job.borrow().next;
    let Some(&(left, top)) = job.borrow().steps.get(next) else {
        finish(&window, &job.borrow());
        return;
    };
    window.invoke_show_at(left, top, PRINT_ZOOM);
    slint::Timer::single_shot(SETTLE, move || {
        if let Some(window) = weak.upgrade() {
            if let Err(why) = capture(&window, &mut job.borrow_mut()) {
                eprintln!("tcs-cfge: the print could not be made: {why}");
                let back = job.borrow().back;
                window.invoke_show_at(back.0, back.1, back.2);
                window.set_printing(false);
                PRINTING.with(|p| p.set(false));
                return;
            }
            job.borrow_mut().next += 1;
        }
        take_step(weak, job);
    });
}

/// Copies the part of the window's snapshot that is the sheet, where it is
/// in the area printed, into the picture.
///
/// # Arguments
///
/// * `window` - The editor's window, its view at a step.
/// * `job`    - The print.
///
/// # Errors
///
/// Where the window cannot be snapshotted.
fn capture(window: &MainWindow, job: &mut Job) -> Result<(), String> {
    let shot = window.window().take_snapshot().map_err(|e| e.to_string())?;
    let (left, top) = (window.get_view_left(), window.get_view_top());
    let (vx, vy) = (window.get_view_x(), window.get_view_y());
    let shown = (
        (window.get_view_width() - SCROLL_BARS) / PRINT_ZOOM,
        (window.get_view_height() - SCROLL_BARS) / PRINT_ZOOM,
    );
    let area = &job.area;
    // What of the area this step shows, in the sheet's pixels.
    let x0 = left.max(area.x);
    let y0 = top.max(area.y);
    let x1 = (left + shown.0).min(area.x + area.width);
    let y1 = (top + shown.1).min(area.y + area.height);
    if x1 <= x0 || y1 <= y0 {
        return Ok(());
    }
    let px = |v: f32| (v * PRINT_ZOOM * job.scale).round() as i64;
    let at_window = |v: f32| (v * job.scale).round() as i64;
    job.picture.copy_in(
        shot.as_bytes(),
        (shot.width() as usize, shot.height() as usize),
        (at_window(vx) + px(x0 - left), at_window(vy) + px(y0 - top)),
        (px(x1 - x0) + 1, px(y1 - y0) + 1),
        (px(x0 - area.x), px(y0 - area.y)),
    );
    Ok(())
}

/// Puts the view back where it was, writes the PDF and opens it.
///
/// # Arguments
///
/// * `window` - The editor's window.
/// * `job`    - The print, its picture whole.
fn finish(window: &MainWindow, job: &Job) {
    window.invoke_show_at(job.back.0, job.back.1, job.back.2);
    window.set_printing(false);
    PRINTING.with(|p| p.set(false));
    let page = (
        job.area.width * POINTS_PER_PIXEL,
        job.area.height * POINTS_PER_PIXEL,
    );
    let path = pdf_path(&job.name);
    let title = Path::new(&job.name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("tcs-cfge");
    let pdf = pdf_of(&job.picture, page, title);
    if let Err(why) = std::fs::write(&path, pdf) {
        eprintln!("tcs-cfge: {} cannot be written: {why}", path.display());
        return;
    }
    if let Err(why) = open_in_viewer(&path) {
        eprintln!(
            "tcs-cfge: {} was written, but cannot be opened: {why}",
            path.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The steps cover the whole area, across then down, and an area
    /// smaller than a step is one step.
    #[test]
    fn the_steps_cover_the_area() {
        let area = Extent {
            x: 10.0,
            y: 20.0,
            width: 250.0,
            height: 90.0,
        };
        let s = steps(&area, (100.0, 50.0));
        assert_eq!(
            s,
            vec![
                (10.0, 20.0),
                (110.0, 20.0),
                (210.0, 20.0),
                (10.0, 70.0),
                (110.0, 70.0),
                (210.0, 70.0)
            ]
        );
        assert_eq!(steps(&area, (500.0, 500.0)), vec![(10.0, 20.0)]);
    }

    /// A margin grows an area all round; an empty area stays empty.
    #[test]
    fn a_margin_grows_the_area() {
        let area = Extent {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
        };
        let g = grown(&area, 5.0);
        assert_eq!((g.x, g.y, g.width, g.height), (5.0, 15.0, 40.0, 50.0));
        assert_eq!(grown(&Extent::default(), 5.0).width, 0.0);
    }

    /// A part of a snapshot lands where it is put, its alpha dropped, and
    /// what falls outside either picture is left out.
    #[test]
    fn a_part_is_copied_where_it_goes() {
        // A 3 x 2 snapshot, each pixel its own value.
        let from: Vec<u8> = (0..6u8).flat_map(|n| [n, n, n, 255]).collect();
        let mut picture = Picture::white(2, 2);
        picture.copy_in(&from, (3, 2), (1, 0), (5, 5), (0, 0));
        assert_eq!(picture.rgb, vec![1, 1, 1, 2, 2, 2, 4, 4, 4, 5, 5, 5]);
        let mut picture = Picture::white(2, 2);
        picture.copy_in(&from, (3, 2), (0, 0), (1, 1), (1, 1));
        assert_eq!(&picture.rgb[9..12], &[0, 0, 0]);
        assert_eq!(&picture.rgb[0..3], &[255, 255, 255]);
    }

    /// The PDF is one page the picture's shape, its cross-reference table
    /// pointing at each of its objects, and its pixels unpacked are the
    /// picture's.
    #[test]
    fn the_pdf_holds_the_picture() {
        let picture = Picture {
            width: 2,
            height: 1,
            rgb: vec![10, 20, 30, 40, 50, 60],
        };
        let pdf = pdf_of(&picture, (150.0, 75.0), "a (b).ytcs");
        // Where some bytes first are, from a place on: the PDF is bytes,
        // its pixels packed, so it is searched as bytes, not as text.
        let find = |what: &[u8], from: usize| {
            pdf[from..]
                .windows(what.len())
                .position(|w| w == what)
                .map(|at| at + from)
        };
        let has = |what: &str| find(what.as_bytes(), 0).is_some();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        assert!(has("/MediaBox [0 0 150.00 75.00]"));
        assert!(has("/Width 2 /Height 1"));
        assert!(has("/Title (a \\(b\\).ytcs)"));
        assert!(pdf.ends_with(b"%%EOF\n"));
        // Each offset in the table is the start of its object.
        let xref = find(b"xref\n", 0).expect("a table");
        let tail = String::from_utf8_lossy(&pdf[xref..]).into_owned();
        let start: usize = tail
            .rsplit("startxref\n")
            .next()
            .and_then(|t| t.lines().next())
            .and_then(|n| n.parse().ok())
            .expect("its offset");
        assert_eq!(start, xref);
        for (n, line) in tail.lines().skip(3).take(6).enumerate() {
            let at: usize = line[..10].parse().expect("an offset");
            let said = format!("{} 0 obj", n + 1);
            assert!(pdf[at..].starts_with(said.as_bytes()), "object {}", n + 1);
        }
        // The pixels, unpacked.
        let image = find(b"/Subtype /Image", 0).expect("the image");
        let length: usize = String::from_utf8_lossy(&pdf[image..image + 200])
            .split("/Length ")
            .nth(1)
            .and_then(|t| t.split(' ').next())
            .and_then(|n| n.parse().ok())
            .expect("its length");
        let begin = find(b"stream\n", image).expect("its stream") + 7;
        let unpacked = miniz_oxide::inflate::decompress_to_vec_zlib(
            &pdf[begin..begin + length],
        )
        .expect("it unpacks");
        assert_eq!(unpacked, picture.rgb);
    }

    /// An area is kept to the sheet's left edge and taken up to its top.
    #[test]
    fn the_area_is_kept_to_the_sheet() {
        let a = on_the_sheet(&Extent {
            x: -10.0,
            y: 30.0,
            width: 100.0,
            height: 50.0,
        });
        assert_eq!((a.x, a.y, a.width, a.height), (0.0, 0.0, 90.0, 80.0));
    }

    /// The PDF is named after the configuration, in the temporary
    /// directory.
    #[test]
    fn the_pdf_is_named_after_the_configuration() {
        let path = pdf_path("/somewhere/thermostat.ytcs");
        assert_eq!(
            path.file_name().and_then(|n| n.to_str()),
            Some("tcs-cfge-print-thermostat.pdf")
        );
        assert!(path.starts_with(std::env::temp_dir()));
        assert!(pdf_path("").ends_with("tcs-cfge-print-configuration.pdf"));
    }
}
