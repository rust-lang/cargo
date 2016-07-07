//! Functions and types related to 2D vector graphics.

use ffi_utils::{self, Text};
use libc::{c_double, c_int};
use std::marker::PhantomData;
use std::mem;
use std::ops::Mul;
use std::ptr;
use ui_sys::{self, uiDrawBrush, uiDrawBrushType, uiDrawContext, uiDrawFontFamilies, uiDrawMatrix};
use ui_sys::{uiDrawPath, uiDrawStrokeParams};

pub use ui_sys::uiDrawBrushGradientStop as BrushGradientStop;
pub use ui_sys::uiDrawLineCap as LineCap;
pub use ui_sys::uiDrawLineJoin as LineJoin;
pub use ui_sys::uiDrawDefaultMiterLimit as DEFAULT_MITER_LIMIT;
pub use ui_sys::uiDrawFillMode as FillMode;

pub struct Context {
    ui_draw_context: *mut uiDrawContext,
}

impl Context {
    #[inline]
    pub unsafe fn from_ui_draw_context(ui_draw_context: *mut uiDrawContext) -> Context {
        ffi_utils::ensure_initialized();
        Context {
            ui_draw_context: ui_draw_context,
        }
    }

    #[inline]
    pub fn stroke(&self, path: &Path, brush: &Brush, stroke_params: &StrokeParams) {
        ffi_utils::ensure_initialized();
        unsafe {
            let brush = brush.as_ui_draw_brush_ref();
            let stroke_params = stroke_params.as_ui_draw_stroke_params_ref();
            ui_sys::uiDrawStroke(self.ui_draw_context,
                                 path.ui_draw_path,
                                 &brush.ui_draw_brush as *const uiDrawBrush as *mut uiDrawBrush,
                                 &stroke_params.ui_draw_stroke_params as *const uiDrawStrokeParams
                                 as *mut uiDrawStrokeParams)
        }
    }

    #[inline]
    pub fn fill(&self, path: &Path, brush: &Brush) {
        ffi_utils::ensure_initialized();
        unsafe {
            let brush = brush.as_ui_draw_brush_ref();
            ui_sys::uiDrawFill(self.ui_draw_context,
                               path.ui_draw_path,
                               &brush.ui_draw_brush as *const uiDrawBrush as *mut uiDrawBrush)
        }
    }

    #[inline]
    pub fn transform(&self, matrix: &Matrix) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawTransform(self.ui_draw_context,
                                    &matrix.ui_matrix as *const uiDrawMatrix as *mut uiDrawMatrix)
        }
    }

    #[inline]
    pub fn save(&self) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawSave(self.ui_draw_context)
        }
    }

    #[inline]
    pub fn restore(&self) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawRestore(self.ui_draw_context)
        }
    }

    #[inline]
    pub fn draw_text(&self, x: f64, y: f64, layout: &text::Layout) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawText(self.ui_draw_context, x, y, layout.as_ui_draw_text_layout())
        }
    }
}

#[derive(Clone, Debug)]
pub enum Brush {
    Solid(SolidBrush),
    LinearGradient(LinearGradientBrush),
    RadialGradient(RadialGradientBrush),
    Image,
}

#[derive(Clone, Debug)]
pub struct UiDrawBrushRef<'a> {
    ui_draw_brush: uiDrawBrush,
    phantom: PhantomData<&'a uiDrawBrush>,
}

impl Brush {
    pub fn as_ui_draw_brush_ref(&self) -> UiDrawBrushRef {
        ffi_utils::ensure_initialized();
        match *self {
            Brush::Solid(ref solid_brush) => {
                UiDrawBrushRef {
                    ui_draw_brush: uiDrawBrush {
                        Type: uiDrawBrushType::Solid,

                        R: solid_brush.r,
                        G: solid_brush.g,
                        B: solid_brush.b,
                        A: solid_brush.a,

                        X0: 0.0,
                        Y0: 0.0,
                        X1: 0.0,
                        Y1: 0.0,
                        OuterRadius: 0.0,
                        Stops: ptr::null_mut(),
                        NumStops: 0,
                    },
                    phantom: PhantomData,
                }
            }
            Brush::LinearGradient(ref linear_gradient_brush) => {
                UiDrawBrushRef {
                    ui_draw_brush: uiDrawBrush {
                        Type: uiDrawBrushType::LinearGradient,

                        R: 0.0,
                        G: 0.0,
                        B: 0.0,
                        A: 0.0,

                        X0: linear_gradient_brush.start_x,
                        Y0: linear_gradient_brush.start_y,
                        X1: linear_gradient_brush.end_x,
                        Y1: linear_gradient_brush.end_y,
                        OuterRadius: 0.0,
                        Stops: linear_gradient_brush.stops.as_ptr() as *mut BrushGradientStop,
                        NumStops: linear_gradient_brush.stops.len(),
                    },
                    phantom: PhantomData,
                }
            }
            Brush::RadialGradient(ref radial_gradient_brush) => {
                UiDrawBrushRef {
                    ui_draw_brush: uiDrawBrush {
                        Type: uiDrawBrushType::RadialGradient,

                        R: 0.0,
                        G: 0.0,
                        B: 0.0,
                        A: 0.0,

                        X0: radial_gradient_brush.start_x,
                        Y0: radial_gradient_brush.start_y,
                        X1: radial_gradient_brush.outer_circle_center_x,
                        Y1: radial_gradient_brush.outer_circle_center_y,
                        OuterRadius: radial_gradient_brush.outer_radius,
                        Stops: radial_gradient_brush.stops.as_ptr() as *mut BrushGradientStop,
                        NumStops: radial_gradient_brush.stops.len(),
                    },
                    phantom: PhantomData,
                }
            }
            Brush::Image => {
                // These don't work yet in `libui`, but just for completeness' sake…
                UiDrawBrushRef {
                    ui_draw_brush: uiDrawBrush {
                        Type: uiDrawBrushType::Image,

                        R: 0.0,
                        G: 0.0,
                        B: 0.0,
                        A: 0.0,

                        X0: 0.0,
                        Y0: 0.0,
                        X1: 0.0,
                        Y1: 0.0,
                        OuterRadius: 0.0,
                        Stops: ptr::null_mut(),
                        NumStops: 0,
                    },
                    phantom: PhantomData,
                }
            }
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct SolidBrush {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

#[derive(Clone, Debug)]
pub struct LinearGradientBrush {
    pub start_x: f64,
    pub start_y: f64,
    pub end_x: f64,
    pub end_y: f64,
    pub stops: Vec<BrushGradientStop>,
}

#[derive(Clone, Debug)]
pub struct RadialGradientBrush {
    pub start_x: f64,
    pub start_y: f64,
    pub outer_circle_center_x: f64,
    pub outer_circle_center_y: f64,
    pub outer_radius: f64,
    pub stops: Vec<BrushGradientStop>,
}

#[derive(Clone, Debug)]
pub struct StrokeParams {
    pub cap: LineCap,
    pub join: LineJoin,
    pub thickness: f64,
    pub miter_limit: f64,
    pub dashes: Vec<f64>,
    pub dash_phase: f64,
}

#[derive(Clone, Debug)]
pub struct UiDrawStrokeParamsRef<'a> {
    ui_draw_stroke_params: uiDrawStrokeParams,
    phantom: PhantomData<&'a uiDrawStrokeParams>,
}

impl StrokeParams {
    pub fn as_ui_draw_stroke_params_ref(&self) -> UiDrawStrokeParamsRef {
        ffi_utils::ensure_initialized();
        UiDrawStrokeParamsRef {
            ui_draw_stroke_params: uiDrawStrokeParams {
                Cap: self.cap,
                Join: self.join,
                Thickness: self.thickness,
                MiterLimit: self.miter_limit,
                Dashes: self.dashes.as_ptr() as *mut c_double,
                NumDashes: self.dashes.len(),
                DashPhase: self.dash_phase,
            },
            phantom: PhantomData,
        }
    }
}

pub struct Path {
    ui_draw_path: *mut uiDrawPath,
}

impl Drop for Path {
    #[inline]
    fn drop(&mut self) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawFreePath(self.ui_draw_path)
        }
    }
}

impl Path {
    #[inline]
    pub fn new(fill_mode: FillMode) -> Path {
        ffi_utils::ensure_initialized();
        unsafe {
            Path {
                ui_draw_path: ui_sys::uiDrawNewPath(fill_mode),
            }
        }
    }

    #[inline]
    pub fn new_figure(&self, x: f64, y: f64) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawPathNewFigure(self.ui_draw_path, x, y)
        }
    }

    #[inline]
    pub fn new_figure_with_arc(&self,
                               x_center: f64,
                               y_center: f64,
                               radius: f64,
                               start_angle: f64,
                               sweep: f64,
                               negative: bool) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawPathNewFigureWithArc(self.ui_draw_path,
                                               x_center,
                                               y_center,
                                               radius,
                                               start_angle,
                                               sweep,
                                               negative as c_int)
        }
    }

    #[inline]
    pub fn line_to(&self, x: f64, y: f64) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawPathLineTo(self.ui_draw_path, x, y)
        }
    }

    #[inline]
    pub fn arc_to(&self,
                  x_center: f64,
                  y_center: f64,
                  radius: f64,
                  start_angle: f64,
                  sweep: f64,
                  negative: bool) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawPathArcTo(self.ui_draw_path,
                                    x_center,
                                    y_center,
                                    radius,
                                    start_angle,
                                    sweep,
                                    negative as c_int)
        }
    }

    #[inline]
    pub fn bezier_to(&self, c1x: f64, c1y: f64, c2x: f64, c2y: f64, end_x: f64, end_y: f64) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawPathBezierTo(self.ui_draw_path, c1x, c1y, c2x, c2y, end_x, end_y)
        }
    }

    #[inline]
    pub fn close_figure(&self) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawPathCloseFigure(self.ui_draw_path)
        }
    }

    #[inline]
    pub fn add_rectangle(&self, x: f64, y: f64, width: f64, height: f64) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawPathAddRectangle(self.ui_draw_path, x, y, width, height)
        }
    }

    #[inline]
    pub fn end(&self) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawPathEnd(self.ui_draw_path)
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Matrix {
    pub ui_matrix: uiDrawMatrix,
}

impl Matrix {
    #[inline]
    pub fn from_ui_matrix(ui_matrix: &uiDrawMatrix) -> Matrix {
        Matrix {
            ui_matrix: *ui_matrix,
        }
    }

    #[inline]
    pub fn identity() -> Matrix {
        unsafe {
            let mut matrix = mem::uninitialized();
            ui_sys::uiDrawMatrixSetIdentity(&mut matrix);
            Matrix::from_ui_matrix(&matrix)
        }
    }

    #[inline]
    pub fn translate(&mut self, x: f64, y: f64) {
        unsafe {
            ui_sys::uiDrawMatrixTranslate(&mut self.ui_matrix, x, y)
        }
    }

    #[inline]
    pub fn scale(&mut self, x_center: f64, y_center: f64, x: f64, y: f64) {
        unsafe {
            ui_sys::uiDrawMatrixScale(&mut self.ui_matrix, x_center, y_center, x, y)
        }
    }

    #[inline]
    pub fn rotate(&mut self, x: f64, y: f64, angle: f64) {
        unsafe {
            ui_sys::uiDrawMatrixRotate(&mut self.ui_matrix, x, y, angle)
        }
    }

    #[inline]
    pub fn skew(&mut self, x: f64, y: f64, xamount: f64, yamount: f64) {
        unsafe {
            ui_sys::uiDrawMatrixSkew(&mut self.ui_matrix, x, y, xamount, yamount)
        }
    }

    #[inline]
    pub fn multiply(&mut self, src: &Matrix) {
        unsafe {
            ui_sys::uiDrawMatrixMultiply(&mut self.ui_matrix,
                                         &src.ui_matrix as *const uiDrawMatrix
                                         as *mut uiDrawMatrix)
        }
    }

    #[inline]
    pub fn invertible(&self) -> bool {
        unsafe {
            ui_sys::uiDrawMatrixInvertible(&self.ui_matrix as *const uiDrawMatrix
                                           as *mut uiDrawMatrix) != 0
        }
    }

    #[inline]
    pub fn invert(&mut self) -> bool {
        unsafe {
            ui_sys::uiDrawMatrixInvert(&mut self.ui_matrix) != 0
        }
    }

    #[inline]
    pub fn transform_point(&self, mut point: (f64, f64)) -> (f64, f64) {
        unsafe {
            ui_sys::uiDrawMatrixTransformPoint(&self.ui_matrix as *const uiDrawMatrix as
                                               *mut uiDrawMatrix,
                                               &mut point.0,
                                               &mut point.1);
            point
        }
    }

    #[inline]
    pub fn transform_size(&self, mut size: (f64, f64)) -> (f64, f64) {
        unsafe {
            ui_sys::uiDrawMatrixTransformSize(&self.ui_matrix as *const uiDrawMatrix as
                                              *mut uiDrawMatrix,
                                              &mut size.0,
                                              &mut size.1);
            size
        }
    }
}

impl Mul<Matrix> for Matrix {
    type Output = Matrix;

    fn mul(mut self, other: Matrix) -> Matrix {
        self.multiply(&other);
        self
    }
}

pub struct FontFamilies {
    ui_draw_font_families: *mut uiDrawFontFamilies,
}

impl Drop for FontFamilies {
    #[inline]
    fn drop(&mut self) {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawFreeFontFamilies(self.ui_draw_font_families)
        }
    }
}

impl FontFamilies {
    #[inline]
    pub fn list() -> FontFamilies {
        ffi_utils::ensure_initialized();
        unsafe {
            FontFamilies {
                ui_draw_font_families: ui_sys::uiDrawListFontFamilies(),
            }
        }
    }

    #[inline]
    pub fn len(&self) -> u64 {
        ffi_utils::ensure_initialized();
        unsafe {
            ui_sys::uiDrawFontFamiliesNumFamilies(self.ui_draw_font_families)
        }
    }

    #[inline]
    pub fn family(&self, index: u64) -> Text {
        ffi_utils::ensure_initialized();
        assert!(index < self.len());
        unsafe {
            Text::new(ui_sys::uiDrawFontFamiliesFamily(self.ui_draw_font_families, index))
        }
    }
}

pub mod text {
    use ffi_utils;
    use libc::c_char;
    use std::ffi::{CStr, CString};
    use std::mem;
    use ui_sys::{self, uiDrawTextFont, uiDrawTextFontDescriptor, uiDrawTextLayout};

    pub use ui_sys::uiDrawTextWeight as Weight;
    pub use ui_sys::uiDrawTextItalic as Italic;
    pub use ui_sys::uiDrawTextStretch as Stretch;
    pub use ui_sys::uiDrawTextFontMetrics as FontMetrics;

    pub struct FontDescriptor {
        family: CString,
        pub size: f64,
        pub weight: Weight,
        pub italic: Italic,
        pub stretch: Stretch,
    }

    impl FontDescriptor {
        #[inline]
        pub fn new(family: &str, size: f64, weight: Weight, italic: Italic, stretch: Stretch)
                   -> FontDescriptor {
            ffi_utils::ensure_initialized();
            FontDescriptor {
                family: CString::new(family.as_bytes().to_vec()).unwrap(),
                size: size,
                weight: weight,
                italic: italic,
                stretch: stretch,
            }
        }

        /// FIXME(pcwalton): Should this return an Option?
        #[inline]
        pub fn load_closest_font(&self) -> Font {
            ffi_utils::ensure_initialized();
            unsafe {
                let font_descriptor = uiDrawTextFontDescriptor {
                    Family: self.family.as_ptr(),
                    Size: self.size,
                    Weight: self.weight,
                    Italic: self.italic,
                    Stretch: self.stretch,
                };
                Font {
                    ui_draw_text_font: ui_sys::uiDrawLoadClosestFont(&font_descriptor),
                }
            }
        }

        #[inline]
        pub fn family(&self) -> &str {
            self.family.to_str().unwrap()
        }
    }

    pub struct Font {
        ui_draw_text_font: *mut uiDrawTextFont,
    }

    impl Drop for Font {
        #[inline]
        fn drop(&mut self) {
            ffi_utils::ensure_initialized();
            unsafe {
                ui_sys::uiDrawFreeTextFont(self.ui_draw_text_font)
            }
        }
    }

    impl Font {
        #[inline]
        pub unsafe fn from_ui_draw_text_font(ui_draw_text_font: *mut uiDrawTextFont) -> Font {
            Font {
                ui_draw_text_font: ui_draw_text_font,
            }
        }

        #[inline]
        pub fn handle(&self) -> usize {
            ffi_utils::ensure_initialized();
            unsafe {
                ui_sys::uiDrawTextFontHandle(self.ui_draw_text_font)
            }
        }

        #[inline]
        pub fn describe(&self) -> FontDescriptor {
            ffi_utils::ensure_initialized();
            unsafe {
                let mut ui_draw_text_font_descriptor = mem::uninitialized();
                ui_sys::uiDrawTextFontDescribe(self.ui_draw_text_font,
                                               &mut ui_draw_text_font_descriptor);
                let family = CStr::from_ptr(ui_draw_text_font_descriptor.Family).to_bytes()
                                                                                .to_vec();
                let font_descriptor = FontDescriptor {
                    family: CString::new(family).unwrap(),
                    size: ui_draw_text_font_descriptor.Size,
                    weight: ui_draw_text_font_descriptor.Weight,
                    italic: ui_draw_text_font_descriptor.Italic,
                    stretch: ui_draw_text_font_descriptor.Stretch,
                };
                ui_sys::uiFreeText(ui_draw_text_font_descriptor.Family as *mut c_char);
                font_descriptor
            }
        }

        #[inline]
        pub fn metrics(&self) -> FontMetrics {
            ffi_utils::ensure_initialized();
            unsafe {
                let mut metrics = mem::uninitialized();
                ui_sys::uiDrawTextFontGetMetrics(self.ui_draw_text_font, &mut metrics);
                metrics
            }
        }
    }

    pub struct Layout {
        ui_draw_text_layout: *mut uiDrawTextLayout,
    }

    impl Drop for Layout {
        #[inline]
        fn drop(&mut self) {
            ffi_utils::ensure_initialized();
            unsafe {
                ui_sys::uiDrawFreeTextLayout(self.ui_draw_text_layout)
            }
        }
    }

    impl Layout {
        #[inline]
        pub fn new(text: &str, default_font: &Font, width: f64) -> Layout {
            ffi_utils::ensure_initialized();
            unsafe {
                let c_string = CString::new(text.as_bytes().to_vec()).unwrap();
                Layout {
                    ui_draw_text_layout:
                        ui_sys::uiDrawNewTextLayout(c_string.as_ptr(),
                                                    default_font.ui_draw_text_font,
                                                    width),
                }
            }
        }

        #[inline]
        pub fn as_ui_draw_text_layout(&self) -> *mut uiDrawTextLayout {
            self.ui_draw_text_layout
        }

        #[inline]
        pub fn set_width(&self, width: f64) {
            ffi_utils::ensure_initialized();
            unsafe {
                ui_sys::uiDrawTextLayoutSetWidth(self.ui_draw_text_layout, width)
            }
        }

        #[inline]
        pub fn extents(&self) -> (f64, f64) {
            ffi_utils::ensure_initialized();
            unsafe {
                let mut extents = (0.0, 0.0);
                ui_sys::uiDrawTextLayoutExtents(self.ui_draw_text_layout,
                                                &mut extents.0,
                                                &mut extents.1);
                extents
            }
        }

        #[inline]
        pub fn set_color(&self, start_char: i64, end_char: i64, r: f64, g: f64, b: f64, a: f64) {
            ffi_utils::ensure_initialized();
            unsafe {
                ui_sys::uiDrawTextLayoutSetColor(self.ui_draw_text_layout,
                                                 start_char,
                                                 end_char,
                                                 r,
                                                 g,
                                                 b,
                                                 a)
            }
        }
    }
}

