use std::mem::ManuallyDrop;
use std::slice;

use napi::*;

use crate::sk::Bitmap;

#[derive(Debug, Clone)]
pub struct ImageData {
  pub(crate) width: usize,
  pub(crate) height: usize,
  pub(crate) data: *const u8,
}

impl Drop for ImageData {
  fn drop(&mut self) {
    let len = (self.width * self.height * 4) as usize;
    unsafe { Vec::from_raw_parts(self.data as *mut u8, len, len) };
  }
}

impl ImageData {
  pub fn create_js_class(env: &Env) -> Result<JsFunction> {
    env.define_class("ImageData", image_data_constructor, &[])
  }
}

#[js_function(3)]
fn image_data_constructor(ctx: CallContext) -> Result<JsUndefined> {
  let first_arg = ctx.get::<JsUnknown>(0)?;
  let first_arg_type = first_arg.get_type()?;
  let ((js_width, width), (js_height, height), arraybuffer_length, mut initial_data) =
    match first_arg_type {
      ValueType::Number => {
        let js_width = unsafe { first_arg.cast::<JsNumber>() };
        let js_height = ctx.get::<JsNumber>(1)?;
        let width = js_width.get_uint32()?;
        let height = js_height.get_uint32()?;
        let arraybuffer_length = (width * height * 4) as usize;
        Ok((
          (js_width, width),
          (js_height, height),
          arraybuffer_length,
          ManuallyDrop::new(vec![0u8; arraybuffer_length]),
        ))
      }
      ValueType::Object => {
        let image_data_ab = unsafe { first_arg.cast::<JsTypedArray>() }.into_value()?;
        if image_data_ab.typedarray_type != TypedArrayType::Uint8Clamped {
          return Err(Error::new(
            Status::InvalidArg,
            "ImageData constructor: Argument 1 does not implement interface Uint8ClampedArray."
              .to_owned(),
          ));
        }
        let arraybuffer: &[u8] = image_data_ab.as_ref();
        let arraybuffer_length = arraybuffer.len();
        let js_width = ctx.get::<JsNumber>(1)?;
        let width = js_width.get_uint32()?;
        let (js_height, height) = if ctx.length == 3 {
          let js_height = ctx.get::<JsNumber>(2)?;
          let height = js_height.get_uint32()?;
          if height * width * 4 != arraybuffer_length as u32 {
            return Err(Error::new(
              Status::InvalidArg,
              "Index or size is negative or greater than the allowed amount".to_owned(),
            ));
          }
          (js_height, height)
        } else {
          let height = arraybuffer_length as u32 / width / 4u32;
          (ctx.env.create_uint32(height)?, height)
        };
        Ok((
          (js_width, width),
          (js_height, height),
          arraybuffer_length,
          ManuallyDrop::new(unsafe {
            slice::from_raw_parts(arraybuffer.as_ptr() as *const u8, arraybuffer_length).to_owned()
          }),
        ))
      }
      _ => Err(Error::new(
        Status::InvalidArg,
        format!(
          "Invalid type of first argument of ImageData constructor [{:?}]",
          first_arg_type
        ),
      )),
    }?;
  let data_ptr = initial_data.as_mut_ptr();
  let image_data = ImageData {
    width: width as usize,
    height: height as usize,
    data: data_ptr,
  };
  let arraybuffer = unsafe {
    ctx
      .env
      .create_arraybuffer_with_borrowed_data(data_ptr, arraybuffer_length, 0, noop_finalize)
  }?;
  let typed_array =
    arraybuffer
      .into_raw()
      .into_typedarray(TypedArrayType::Uint8Clamped, arraybuffer_length, 0)?;

  let mut this = ctx.this_unchecked::<JsObject>();
  ctx.env.wrap(&mut this, image_data)?;
  this.define_properties(&[
    Property::new(ctx.env, "data")?
      .with_value(typed_array)
      .with_property_attributes(PropertyAttributes::Enumerable),
    Property::new(ctx.env, "width")?
      .with_value(js_width)
      .with_property_attributes(PropertyAttributes::Enumerable),
    Property::new(ctx.env, "height")?
      .with_value(js_height)
      .with_property_attributes(PropertyAttributes::Enumerable),
  ])?;
  ctx.env.get_undefined()
}

pub(crate) struct Image {
  pub(crate) bitmap: Option<Bitmap>,
  pub(crate) complete: bool,
  pub(crate) alt: String,
  width: f64,
  height: f64,
  pub(crate) need_regenerate_bitmap: bool,
  pub(crate) is_svg: bool,
}

impl Image {
  #[inline(always)]
  pub(crate) fn regenerate_bitmap_if_need<D>(&mut self, data: D)
  where
    D: AsRef<[u8]>,
  {
    if !self.need_regenerate_bitmap || !self.is_svg {
      return;
    }
    self.bitmap = Bitmap::from_svg_data_with_custom_size(
      data.as_ref().as_ptr(),
      data.as_ref().len(),
      self.width as f32,
      self.height as f32,
    );
  }
}

impl Image {
  pub fn create_js_class(env: &Env) -> Result<JsFunction> {
    env.define_class(
      "Image",
      image_constructor,
      &vec![
        Property::new(env, "width")?
          .with_getter(get_width)
          .with_setter(set_width),
        Property::new(env, "height")?
          .with_getter(get_height)
          .with_setter(set_height),
        Property::new(env, "naturalWidth")?
          .with_getter(get_natural_width)
          .with_property_attributes(PropertyAttributes::Enumerable),
        Property::new(env, "naturalHeight")?
          .with_getter(get_natural_height)
          .with_property_attributes(PropertyAttributes::Enumerable),
        Property::new(env, "complete")?
          .with_getter(get_complete)
          .with_property_attributes(PropertyAttributes::Enumerable),
        Property::new(env, "alt")?
          .with_setter(set_alt)
          .with_getter(get_alt),
        Property::new(env, "src")?
          .with_setter(set_src)
          .with_getter(get_src),
      ],
    )
  }
}

#[js_function]
fn image_constructor(ctx: CallContext) -> Result<JsUndefined> {
  let js_image = Image {
    complete: false,
    bitmap: None,
    alt: "".to_string(),
    width: -1.0,
    height: -1.0,
    need_regenerate_bitmap: false,
    is_svg: false,
  };
  let mut this = ctx.this_unchecked::<JsObject>();
  this.set_named_property("_src", ctx.env.get_undefined()?)?;
  ctx.env.wrap(&mut this, js_image)?;
  ctx.env.get_undefined()
}

#[js_function]
fn get_width(ctx: CallContext) -> Result<JsNumber> {
  let this = ctx.this_unchecked::<JsObject>();
  let image = ctx.env.unwrap::<Image>(&this)?;

  ctx
    .env
    .create_double(if image.width <= 0.0 { 0.0 } else { image.width })
}

#[js_function]
fn get_natural_width(ctx: CallContext) -> Result<JsNumber> {
  let this = ctx.this_unchecked::<JsObject>();
  let image = ctx.env.unwrap::<Image>(&this)?;

  ctx
    .env
    .create_double(image.bitmap.as_ref().map(|b| b.width).unwrap_or(0) as f64)
}

#[js_function(1)]
fn set_width(ctx: CallContext) -> Result<JsUndefined> {
  let width = ctx.get::<JsNumber>(0)?.get_double()?;
  let this = ctx.this_unchecked::<JsObject>();
  let image = ctx.env.unwrap::<Image>(&this)?;
  if (width - image.width).abs() > f64::EPSILON {
    image.width = width;
    image.need_regenerate_bitmap = true;
  }
  ctx.env.get_undefined()
}

#[js_function]
fn get_height(ctx: CallContext) -> Result<JsNumber> {
  let this = ctx.this_unchecked::<JsObject>();
  let image = ctx.env.unwrap::<Image>(&this)?;

  ctx.env.create_double(if image.height <= 0.0 {
    0.0
  } else {
    image.height
  })
}

#[js_function]
fn get_natural_height(ctx: CallContext) -> Result<JsNumber> {
  let this = ctx.this_unchecked::<JsObject>();
  let image = ctx.env.unwrap::<Image>(&this)?;

  ctx
    .env
    .create_double(image.bitmap.as_ref().map(|b| b.height).unwrap_or(0) as f64)
}

#[js_function(1)]
fn set_height(ctx: CallContext) -> Result<JsUndefined> {
  let height = ctx.get::<JsNumber>(0)?.get_double()?;
  let this = ctx.this_unchecked::<JsObject>();
  let image = ctx.env.unwrap::<Image>(&this)?;
  if (image.height - height).abs() > f64::EPSILON {
    image.height = height;
    image.need_regenerate_bitmap = true;
  }
  ctx.env.get_undefined()
}

#[js_function]
fn get_complete(ctx: CallContext) -> Result<JsBoolean> {
  let this = ctx.this_unchecked::<JsObject>();
  let image = ctx.env.unwrap::<Image>(&this)?;

  ctx.env.get_boolean(image.complete)
}

#[js_function]
fn get_alt(ctx: CallContext) -> Result<JsString> {
  let this = ctx.this_unchecked::<JsObject>();
  let image = ctx.env.unwrap::<Image>(&this)?;

  ctx.env.create_string(image.alt.as_str())
}

#[js_function(1)]
fn set_alt(ctx: CallContext) -> Result<JsUndefined> {
  let this = ctx.this_unchecked::<JsObject>();
  let mut image = ctx.env.unwrap::<Image>(&this)?;
  let arg = ctx.get::<JsString>(0)?.into_utf8()?;
  image.alt = arg.as_str()?.to_string();

  ctx.env.get_undefined()
}

#[js_function]
fn get_src(ctx: CallContext) -> Result<JsUnknown> {
  let this = ctx.this_unchecked::<JsObject>();
  this.get_named_property("_src")
}

#[js_function(1)]
fn set_src(ctx: CallContext) -> Result<JsUndefined> {
  let mut this = ctx.this_unchecked::<JsObject>();
  let src_arg = ctx.get::<JsBuffer>(0)?;
  let src_data = src_arg.into_value()?;
  let image = ctx.env.unwrap::<Image>(&this)?;

  let length = (&src_data).len();
  let data_ref: &[u8] = &src_data;
  let mut is_svg = false;
  for i in 3..length {
    if '<' == data_ref[i - 3] as char {
      match data_ref[i - 2] as char {
        '?' | '!' => continue,
        's' => {
          is_svg = 'v' == data_ref[i - 1] as char && 'g' == data_ref[i] as char;
          break;
        }
        _ => {
          is_svg = false;
        }
      }
    }
  }
  image.complete = true;
  image.is_svg = is_svg;
  if is_svg {
    let bitmap = Bitmap::from_svg_data(src_data.as_ptr(), length);
    if let Some(b) = bitmap.as_ref() {
      if (image.width - -1.0).abs() < f64::EPSILON {
        image.width = b.width as f64;
      }
      if (image.height - -1.0).abs() < f64::EPSILON {
        image.height = b.height as f64;
      }
    }
    image.bitmap = bitmap;
  } else {
    let bitmap = Bitmap::from_buffer(src_data.as_ptr() as *mut u8, length);
    if (image.width - -1.0).abs() < f64::EPSILON {
      image.width = bitmap.width as f64;
    }
    if (image.height - -1.0).abs() < f64::EPSILON {
      image.height = bitmap.height as f64;
    }
    image.bitmap = Some(bitmap)
  }

  this.set_named_property("_src", src_data.into_raw())?;
  ctx.env.get_undefined()
}
