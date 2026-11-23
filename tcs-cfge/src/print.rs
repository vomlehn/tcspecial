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

use std::collections::BTreeSet;

use crate::document::Document;
use crate::subdiagram::Home;
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

/// How tall the line at the head of each page is, in points: where the
/// page's name is written, above its picture.
const HEADING: f32 = 20.0;

/// One page of a print, made: the picture of a page of the configuration,
/// the part of the sheet it shows, and what it is headed.
#[derive(Debug, Clone, PartialEq)]
pub struct Printed {
    /// The picture.
    pub picture: Picture,
    /// The part of the sheet it shows, in the sheet's pixels: what says
    /// how large it is printed.
    pub area: Extent,
    /// What is written above it: which page of the configuration it is.
    pub heading: String,
}

/// Makes text something a PDF string can hold: its backslashes and
/// brackets escaped, and anything outside printable ASCII left out, the
/// font being one every viewer has and holding no more.
///
/// # Arguments
///
/// * `text` - The text.
///
/// # Returns
///
/// What goes between the string's brackets.
fn pdf_text(text: &str) -> String {
    text.chars()
        .filter(|c| (' '..='~').contains(c))
        .flat_map(|c| match c {
            '\\' | '(' | ')' => vec!['\\', c],
            _ => vec![c],
        })
        .collect()
}

/// A PDF of pages, one for each picture, each headed by its name.
///
/// Each page is its picture's shape, at 96 pixels an inch, with the line
/// of its heading above it, written in Helvetica, which every PDF viewer
/// has without the file carrying it.
///
/// The PDF records, where it is given one, the digest of the files it was
/// printed from: see [`sources_digest`] and [`recorded_sources`].
///
/// # Arguments
///
/// * `pages`   - The pages, in order.
/// * `title`   - What the document is called, for the viewer's title.
/// * `sources` - The digest of the files printed, if it is to be kept.
///
/// # Returns
///
/// The PDF's bytes.
pub fn pdf_of(
    pages: &[Printed],
    title: &str,
    sources: Option<&str>,
) -> Vec<u8> {
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    let mut object = |out: &mut Vec<u8>, body: &[u8]| {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", offsets.len()).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    };
    // The catalogue, the list of pages and the font, then three objects a
    // page -- the page, its picture and what draws them -- numbered from
    // four.
    let page_of = |n: usize| 4 + 3 * n;
    let kids: Vec<String> = (0..pages.len())
        .map(|n| format!("{} 0 R", page_of(n)))
        .collect();
    object(&mut out, b"<< /Type /Catalog /Pages 2 0 R >>");
    object(
        &mut out,
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        )
        .as_bytes(),
    );
    object(
        &mut out,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    );
    for (n, page) in pages.iter().enumerate() {
        let w = page.area.width * POINTS_PER_PIXEL;
        let drawn = page.area.height * POINTS_PER_PIXEL;
        let h = drawn + HEADING;
        object(
            &mut out,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.2} {h:.2}] \
                 /Resources << /XObject << /Im0 {} 0 R >> \
                 /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
                page_of(n) + 1,
                page_of(n) + 2,
            )
            .as_bytes(),
        );
        let pixels =
            miniz_oxide::deflate::compress_to_vec_zlib(&page.picture.rgb, 6);
        let mut image = format!(
            "<< /Type /XObject /Subtype /Image /Width {} /Height {} \
             /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode \
             /Length {} >>\nstream\n",
            page.picture.width,
            page.picture.height,
            pixels.len()
        )
        .into_bytes();
        image.extend_from_slice(&pixels);
        image.extend_from_slice(b"\nendstream");
        object(&mut out, &image);
        let content = format!(
            "q {w:.2} 0 0 {drawn:.2} 0 0 cm /Im0 Do Q\n\
             BT /F1 11 Tf 6 {:.2} Td ({}) Tj ET\n",
            drawn + 6.0,
            pdf_text(&page.heading)
        );
        let mut contents =
            format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        contents.extend_from_slice(content.as_bytes());
        contents.extend_from_slice(b"endstream");
        object(&mut out, &contents);
    }
    let sources = sources
        .map(|digest| format!(" {SOURCES_KEY}{})", pdf_text(digest)))
        .unwrap_or_default();
    object(
        &mut out,
        format!("<< /Title ({}){sources} >>", pdf_text(title)).as_bytes(),
    );
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

/// One page of the configuration to print: which, and what it is headed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// The page: `None` for the top, or a subdiagram's name.
    pub home: Home,
    /// What is written above its picture.
    pub heading: String,
}

/// Draws a page of the configuration in the window, and says where on the
/// sheet its drawing is, every node and line, in the sheet's pixels: a
/// width of nought for a page with nothing on it.
pub type ShowPage = dyn Fn(&MainWindow, &Home) -> Extent;

/// One page's picture being put together: the part of the sheet printed,
/// the places the view is put to show all of it, and the picture.
struct Plan {
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
}

/// A print being made: the pages, where it has got to, and what is made.
struct Job {
    /// The pages, in the order they are printed.
    pages: Vec<Page>,
    /// The page being printed.
    at: usize,
    /// Its picture, once its drawing has been found.
    plan: Option<Plan>,
    /// The pages made so far.
    printed: Vec<Printed>,
    /// Where the view was, and the zoom chosen, to be put back.
    back: (f32, f32, f32),
    /// The configuration's file name, which the PDF is named after.
    name: String,
    /// Draws a page in the window: see [`ShowPage`].
    show: Rc<ShowPage>,
    /// Draws the window again as it was before the print.
    restore: Rc<dyn Fn(&MainWindow)>,
}

/// Prints every page of the configuration, the top page and then each
/// subdiagram's, as pages of one PDF: see the module.
///
/// The pages are drawn in turn in this window, which takes no presses
/// meanwhile, and drawn again as they were afterwards.
///
/// # Arguments
///
/// * `window`  - The editor's main window.
/// * `pages`   - The pages, in order: see [`pages_of`].
/// * `show`    - Draws a page in the window and says where its drawing is.
/// * `restore` - Draws the window again as it was.
/// * `name`    - The configuration's file name, which the PDF is named
///               after.
pub fn print(
    window: &MainWindow,
    pages: Vec<Page>,
    show: Rc<ShowPage>,
    restore: Rc<dyn Fn(&MainWindow)>,
    name: &str,
) {
    if PRINTING.with(Cell::get) {
        return;
    }
    PRINTING.with(|p| p.set(true));
    window.set_printing(true);
    let job = Job {
        pages,
        at: 0,
        plan: None,
        printed: Vec::new(),
        back: (
            window.get_view_left(),
            window.get_view_top(),
            window.get_zoom_chosen(),
        ),
        name: name.to_string(),
        show,
        restore,
    };
    take_step(window.as_weak(), Rc::new(RefCell::new(job)));
}

/// The plan for printing an area of the sheet: its steps planned and its
/// picture white.
///
/// # Arguments
///
/// * `window` - The editor's window.
/// * `area`   - Where on the sheet the drawing is, in the sheet's pixels.
///
/// # Returns
///
/// The plan, or none for an empty area or a view too small to step by.
fn plan_for(window: &MainWindow, area: Extent) -> Option<Plan> {
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
    Some(Plan {
        steps: steps(&area, step),
        next: 0,
        picture: Picture::white(size(area.width), size(area.height)),
        scale,
        area,
    })
}

/// Makes every page's picture at once, without a timer between the steps,
/// for a window drawn by hand rather than by an event loop: `tcs-cfge
/// --print`'s and the screenshots' headless window. A page with nothing on
/// it is left out. The window is drawn again as it was afterwards.
///
/// # Arguments
///
/// * `window`  - The editor's window.
/// * `pages`   - The pages, in order.
/// * `show`    - Draws a page in the window and says where its drawing is.
/// * `restore` - Draws the window again as it was.
/// * `draw`    - Draws the window, after the view is moved and before the
///               snapshot is taken.
///
/// # Returns
///
/// The pages made.
///
/// # Errors
///
/// Where the window cannot be snapshotted.
pub fn made_now(
    window: &MainWindow,
    pages: &[Page],
    show: &dyn Fn(&MainWindow, &Home) -> Extent,
    restore: &dyn Fn(&MainWindow),
    draw: &dyn Fn(&MainWindow),
) -> Result<Vec<Printed>, String> {
    let back = (
        window.get_view_left(),
        window.get_view_top(),
        window.get_zoom_chosen(),
    );
    window.set_printing(true);
    let mut printed = Vec::new();
    let mut made = || -> Result<(), String> {
        for page in pages {
            let area = show(window, &page.home);
            draw(window);
            let Some(mut plan) = plan_for(window, area) else {
                continue;
            };
            for &(left, top) in &plan.steps.clone() {
                window.invoke_show_at(left, top, PRINT_ZOOM);
                draw(window);
                capture(window, &mut plan)?;
            }
            printed.push(Printed {
                picture: plan.picture,
                area: plan.area,
                heading: page.heading.clone(),
            });
        }
        Ok(())
    };
    let outcome = made();
    window.invoke_show_at(back.0, back.1, back.2);
    window.set_printing(false);
    restore(window);
    outcome.map(|()| printed)
}

/// Takes the print's next step: the page drawn and its drawing found, and
/// the view put at the page's next step and, once the window has drawn it,
/// the sheet copied from a snapshot into the page's picture. After the last
/// page, puts the window back and makes the PDF.
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
    let at = job.borrow().at;
    let Some(page) = job.borrow().pages.get(at).cloned() else {
        finish(&window, &job.borrow());
        return;
    };
    // The page is drawn at every step, not only its first: the window is
    // drawn again from the document by any edit made in another window
    // meanwhile, and that draws the top page.
    let show = job.borrow().show.clone();
    let area = show(&window, &page.home);
    if job.borrow().plan.is_none() {
        match plan_for(&window, area) {
            Some(plan) => job.borrow_mut().plan = Some(plan),
            // A page with nothing on it is left out.
            None => {
                job.borrow_mut().at += 1;
                take_step(weak, job);
                return;
            }
        }
    }
    let step = job
        .borrow()
        .plan
        .as_ref()
        .and_then(|plan| plan.steps.get(plan.next).copied());
    let Some((left, top)) = step else {
        // The page is done: on to the next.
        {
            let mut job = job.borrow_mut();
            if let Some(plan) = job.plan.take() {
                job.printed.push(Printed {
                    picture: plan.picture,
                    area: plan.area,
                    heading: page.heading,
                });
            }
            job.at += 1;
        }
        take_step(weak, job);
        return;
    };
    window.invoke_show_at(left, top, PRINT_ZOOM);
    slint::Timer::single_shot(SETTLE, move || {
        if let Some(window) = weak.upgrade() {
            let taken = match job.borrow_mut().plan.as_mut() {
                Some(plan) => capture(&window, plan),
                None => Ok(()),
            };
            if let Err(why) = taken {
                eprintln!("tcs-cfge: the print could not be made: {why}");
                let (back, restore) = {
                    let job = job.borrow();
                    (job.back, job.restore.clone())
                };
                window.invoke_show_at(back.0, back.1, back.2);
                window.set_printing(false);
                restore(&window);
                PRINTING.with(|p| p.set(false));
                return;
            }
            if let Some(plan) = job.borrow_mut().plan.as_mut() {
                plan.next += 1;
            }
        }
        take_step(weak, job);
    });
}

/// Copies the part of the window's snapshot that is the sheet, where it is
/// in the area printed, into the page's picture.
///
/// # Arguments
///
/// * `window` - The editor's window, its view at a step.
/// * `plan`   - The page's picture being made.
///
/// # Errors
///
/// Where the window cannot be snapshotted.
fn capture(window: &MainWindow, plan: &mut Plan) -> Result<(), String> {
    let shot = window.window().take_snapshot().map_err(|e| e.to_string())?;
    let (left, top) = (window.get_view_left(), window.get_view_top());
    let (vx, vy) = (window.get_view_x(), window.get_view_y());
    let shown = (
        (window.get_view_width() - SCROLL_BARS) / PRINT_ZOOM,
        (window.get_view_height() - SCROLL_BARS) / PRINT_ZOOM,
    );
    let area = &plan.area;
    // What of the area this step shows, in the sheet's pixels.
    let x0 = left.max(area.x);
    let y0 = top.max(area.y);
    let x1 = (left + shown.0).min(area.x + area.width);
    let y1 = (top + shown.1).min(area.y + area.height);
    if x1 <= x0 || y1 <= y0 {
        return Ok(());
    }
    let px = |v: f32| (v * PRINT_ZOOM * plan.scale).round() as i64;
    let at_window = |v: f32| (v * plan.scale).round() as i64;
    let (ax, ay) = (area.x, area.y);
    plan.picture.copy_in(
        shot.as_bytes(),
        (shot.width() as usize, shot.height() as usize),
        (at_window(vx) + px(x0 - left), at_window(vy) + px(y0 - top)),
        (px(x1 - x0) + 1, px(y1 - y0) + 1),
        (px(x0 - ax), px(y0 - ay)),
    );
    Ok(())
}

/// Puts the window back as it was, writes the PDF and opens it.
///
/// # Arguments
///
/// * `window` - The editor's window.
/// * `job`    - The print, every page made.
fn finish(window: &MainWindow, job: &Job) {
    window.invoke_show_at(job.back.0, job.back.1, job.back.2);
    window.set_printing(false);
    (job.restore)(window);
    PRINTING.with(|p| p.set(false));
    if job.printed.is_empty() {
        return;
    }
    let path = pdf_path(&job.name);
    let title = Path::new(&job.name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("tcs-cfge");
    let pdf = pdf_of(&job.printed, title, None);
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

/// The key a PDF records the digest of its sources under, in its
/// document information: see [`sources_digest`].
const SOURCES_KEY: &str = "/TcsSources (";

/// A digest of every file a configuration is printed from: the top file and
/// its other spelling, each subdiagram's file, and the simulator's file
/// where one was given -- each as it is on the disk now.
///
/// What a print records, and what says whether it is out of date: two
/// digests differ when any of the files does, and are the same on every
/// machine, unlike the pictures, which depend on its fonts. Each file is
/// taken by its name from the configuration's directory, so that where the
/// checkout is does not change the digest, and in the order of those names.
///
/// # Arguments
///
/// * `document` - The configuration, read from `top`.
/// * `top`      - The configuration's file.
///
/// # Returns
///
/// The digest, in lower-case hexadecimal.
///
/// # Errors
///
/// A sentence naming a file that cannot be read.
pub fn sources_digest(
    document: &Document,
    top: &Path,
) -> Result<String, String> {
    let here = top.parent().unwrap_or(Path::new(""));
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for (path, _) in document.files(top)? {
        // The other spelling is listed only where it is there; any other
        // file that is not there yet is one nothing was printed from.
        if !tcsos::fs::path_exists(&path) {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|why| {
            format!("{} cannot be read: {why}", path.display())
        })?;
        let name = path.strip_prefix(here).unwrap_or(&path);
        files.push((name.to_string_lossy().into_owned(), bytes));
    }
    files.sort();
    let mut all = Vec::new();
    for (name, bytes) in &files {
        all.extend_from_slice(format!("{name}\n{}\n", bytes.len()).as_bytes());
        all.extend_from_slice(bytes);
    }
    Ok(crate::history::digest_of(&all))
}

/// The digest of its sources a PDF recorded when it was printed.
///
/// # Arguments
///
/// * `pdf` - The PDF's bytes.
///
/// # Returns
///
/// The digest; none for a PDF that recorded none.
pub fn recorded_sources(pdf: &[u8]) -> Option<String> {
    let key = SOURCES_KEY.as_bytes();
    let at = pdf.windows(key.len()).position(|w| w == key)? + key.len();
    let end = pdf[at..].iter().position(|&b| b == b')')? + at;
    String::from_utf8(pdf[at..end].to_vec()).ok()
}

/// The pages of a configuration, in the order they are printed: the top
/// page, then each subdiagram its box stands for, in the order the boxes
/// are held, each once.
///
/// A page is reached by its box, wherever the box is drawn, so a page
/// whose box is on another subdiagram's page is printed after that page.
/// The reader allows no such thing -- a file it includes may include no
/// other -- but a walk of pages that relied on that would print a page for
/// ever were it ever otherwise, so each page is printed only the first
/// time it is reached.
///
/// # Arguments
///
/// * `document` - The configuration.
/// * `file`     - The configuration's file name, which heads the top page.
///
/// # Returns
///
/// The pages, each with its heading.
pub fn pages_of(document: &Document, file: &str) -> Vec<Page> {
    let mut pages = vec![Page {
        home: None,
        heading: file.to_string(),
    }];
    let mut seen: BTreeSet<Home> = BTreeSet::from([None]);
    let mut at = 0;
    while at < pages.len() {
        let here = pages[at].home.clone();
        for node in &document.nodes {
            if node.home != here || node.node_type != "subdiagram" {
                continue;
            }
            let home = Some(node.name.clone());
            if !seen.insert(home.clone()) {
                continue;
            }
            pages.push(Page {
                heading: format!(
                    "{file}: subdiagram {}, from {}",
                    node.name,
                    document.include_path(&node.name)
                ),
                home,
            });
        }
        at += 1;
    }
    pages
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

    /// The PDF has a page for each picture, each the picture's shape at 96
    /// pixels an inch with its heading's line above it, its heading written
    /// there; its cross-reference table points at each of its objects; and
    /// each page's pixels unpacked are its picture's.
    #[test]
    fn the_pdf_holds_a_page_for_each_picture() {
        let printed = |rgb: Vec<u8>, width: f32, heading: &str| Printed {
            picture: Picture {
                width: 2,
                height: 1,
                rgb,
            },
            area: Extent {
                x: 0.0,
                y: 0.0,
                width,
                height: 100.0,
            },
            heading: heading.to_string(),
        };
        let pages = [
            printed(vec![10, 20, 30, 40, 50, 60], 200.0, "top.ytcs"),
            printed(vec![1, 2, 3, 4, 5, 6], 400.0, "top.ytcs: zone (a)"),
        ];
        let pdf = pdf_of(&pages, "a (b).ytcs", Some("0123abcd"));
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
        assert!(has("/Kids [4 0 R 7 0 R] /Count 2"));
        // 200 by 100 pixels is 150 by 75 points, and the heading's line.
        assert!(has("/MediaBox [0 0 150.00 95.00]"));
        assert!(has("/MediaBox [0 0 300.00 95.00]"));
        assert!(has("(top.ytcs) Tj"));
        assert!(has("(top.ytcs: zone \\(a\\)) Tj"));
        assert!(has("/BaseFont /Helvetica"));
        assert!(has("/Title (a \\(b\\).ytcs)"));
        assert_eq!(recorded_sources(&pdf).as_deref(), Some("0123abcd"));
        assert_eq!(
            recorded_sources(&pdf_of(&pages, "a", None)),
            None,
            "a print from the menu records none"
        );
        assert!(pdf.ends_with(b"%%EOF\n"));
        // Each offset in the table is the start of its object: three for
        // the document, three a page, and the title's.
        let xref = find(b"xref\n", 0).expect("a table");
        let tail = String::from_utf8_lossy(&pdf[xref..]).into_owned();
        let start: usize = tail
            .rsplit("startxref\n")
            .next()
            .and_then(|t| t.lines().next())
            .and_then(|n| n.parse().ok())
            .expect("its offset");
        assert_eq!(start, xref);
        let objects = 3 + 3 * pages.len() + 1;
        assert!(has(&format!("xref\n0 {}\n", objects + 1)));
        for (n, line) in tail.lines().skip(3).take(objects).enumerate() {
            let at: usize = line[..10].parse().expect("an offset");
            let said = format!("{} 0 obj", n + 1);
            assert!(pdf[at..].starts_with(said.as_bytes()), "object {}", n + 1);
        }
        // The pixels of each page, unpacked, in order.
        let mut from = 0;
        for page in &pages {
            let image = find(b"/Subtype /Image", from).expect("the image");
            let length: usize =
                String::from_utf8_lossy(&pdf[image..image + 200])
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
            assert_eq!(unpacked, page.picture.rgb);
            from = begin + length;
        }
    }

    /// A configuration with these boxes, each on its page: `(page, box)`,
    /// a page of `None` being the top.
    ///
    /// # Arguments
    ///
    /// * `boxes` - Each box's page and name.
    ///
    /// # Returns
    ///
    /// The configuration.
    fn with_boxes(boxes: &[(Option<&str>, &str)]) -> Document {
        let mut document = Document::new();
        for (page, name) in boxes {
            document.apply(&crate::journal::Edit::AddNode {
                name: name.to_string(),
                node_type: "subdiagram".to_string(),
                home: page.map(str::to_string),
            });
        }
        document
    }

    /// The top page first, then each subdiagram in the order its box is
    /// held, headed by the file and the subdiagram.
    #[test]
    fn every_page_is_printed_the_top_first() {
        let document = with_boxes(&[(None, "zone"), (None, "deck")]);
        let pages = pages_of(&document, "top.ytcs");
        let homes: Vec<Option<&str>> =
            pages.iter().map(|page| page.home.as_deref()).collect();
        assert_eq!(homes, [None, Some("zone"), Some("deck")]);
        assert_eq!(pages[0].heading, "top.ytcs");
        assert!(
            pages[1].heading.starts_with("top.ytcs: subdiagram zone"),
            "{}",
            pages[1].heading
        );
    }

    /// Each page is printed once, however it is reached: by two boxes, by a
    /// box on a page under it, and by a box that leads back to the top or to
    /// its own page.
    #[test]
    fn a_page_reached_twice_or_in_a_loop_is_printed_once() {
        let document = with_boxes(&[
            (None, "zone"),
            (None, "zone"),
            (Some("zone"), "deeper"),
            (Some("deeper"), "zone"),
            (Some("deeper"), "deeper"),
        ]);
        let pages = pages_of(&document, "top.ytcs");
        let homes: Vec<Option<&str>> =
            pages.iter().map(|page| page.home.as_deref()).collect();
        assert_eq!(homes, [None, Some("zone"), Some("deeper")]);
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
