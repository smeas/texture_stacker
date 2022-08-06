use crate::util::{log_error, log_info, log_warn};
use crate::Result;
use png::{BitDepth, ColorType};
use std::{fs::File, io::BufWriter, path::PathBuf};

#[derive(Debug)]
pub(crate) struct InputTextureSet {
    pub name: String,
    pub textures: Vec<Option<String>>,
}

#[derive(Debug)]
pub(crate) struct ProcessConfig {
    pub keep_mask_alpha: bool,
    pub suffixes: Vec<String>,
    pub output_masks: bool,
    pub output_directory: PathBuf,
    pub output_texture_name: PathBuf,
}

pub(crate) struct RawImage {
    data: Vec<u8>,
    format: ImageFormat,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct ImageFormat {
    width: u32,
    height: u32,
    color_type: ColorType,
    bit_depth: BitDepth,
}

#[derive(PartialEq, Eq)]
struct Pixel(u8, u8, u8, u8);

fn read_image_from_file(file_name: &str) -> Result<RawImage> {
    let infile = File::open(&file_name)?;
    let decoder = png::Decoder::new(infile);

    let mut reader = decoder.read_info()?;
    let mut buffer = vec![0; reader.output_buffer_size()];

    let info = reader.next_frame(&mut buffer)?;
    let bytes = &buffer[..info.buffer_size()];

    if reader.info().is_animated() {
        log_error!(
            "The image '{}' is an animated PNG, which is not supported.",
            file_name
        );
        panic!();
    }

    Ok(RawImage {
        data: bytes.to_vec(),
        format: ImageFormat {
            width: info.width,
            height: info.height,
            bit_depth: info.bit_depth,
            color_type: info.color_type,
        },
    })
}

fn write_image_to_file(file_name: &str, image: &RawImage) -> Result<()> {
    let format = &image.format;

    let file = File::create(&file_name)?;
    let mut w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(&mut w, format.width, format.height);
    encoder.set_depth(format.bit_depth);
    encoder.set_color(format.color_type);
    //encoder.set_srgb(png::SrgbRenderingIntent::)

    let mut writer = encoder.write_header()?;
    writer.write_image_data(&image.data)?;

    Ok(())
}

///
/// Calculate the amount of bytes per pixel of the given image format.
///
/// Panics if the format is not supported.
///
fn calc_pixel_stride(format: &ImageFormat) -> usize {
    match format {
        ImageFormat {
            bit_depth: BitDepth::Eight,
            color_type: ColorType::Rgb,
            ..
        } => 3,
        ImageFormat {
            bit_depth: BitDepth::Eight,
            color_type: ColorType::Rgba,
            ..
        } => 4,
        ImageFormat {
            bit_depth: BitDepth::Sixteen,
            color_type: ColorType::Rgb,
            ..
        } => 6,
        ImageFormat {
            bit_depth: BitDepth::Sixteen,
            color_type: ColorType::Rgba,
            ..
        } => 8,
        _ => panic!("Format not supported"),
    }
}

///
/// Interprets a slice of bytes as a pixel in the given image format.
///
/// Panics if the format is not supported.
///
fn bytes_to_pixel(slice: &[u8], format: &ImageFormat) -> Pixel {
    match format {
        ImageFormat {
            bit_depth: BitDepth::Eight,
            color_type: ColorType::Rgb,
            ..
        } => Pixel(slice[0], slice[1], slice[2], 255),
        ImageFormat {
            bit_depth: BitDepth::Eight,
            color_type: ColorType::Rgba,
            ..
        } => Pixel(slice[0], slice[1], slice[2], slice[3]),
        _ => panic!("Format not supported"),
    }
}

///
/// Writes the byte representation of pixel in the given format to the given slice.
///
/// Panics if the format is not supported.
///
fn pixel_to_bytes(pixel: Pixel, format: &ImageFormat, slice: &mut [u8]) {
    match format {
        ImageFormat {
            bit_depth: BitDepth::Eight,
            color_type: ColorType::Rgb,
            ..
        } => {
            slice[0] = pixel.0;
            slice[1] = pixel.1;
            slice[2] = pixel.2;
        }
        ImageFormat {
            bit_depth: BitDepth::Eight,
            color_type: ColorType::Rgba,
            ..
        } => {
            slice[0] = pixel.0;
            slice[1] = pixel.1;
            slice[2] = pixel.2;
            slice[3] = pixel.3;
        }
        _ => panic!("Format not supported"),
    }
}

fn create_mask_from_alpha_channel(image: &RawImage) -> Vec<bool> {
    let format = &image.format;

    assert_eq!(
        format.color_type,
        ColorType::Rgba,
        "mask texture is missing alpha channel"
    );

    let pixel_stride = calc_pixel_stride(format);
    let num_pixels = format.width as usize * format.height as usize;

    assert_eq!(image.data.len(), num_pixels * pixel_stride);

    let mut pixel_mask = vec![false; num_pixels];

    for i in 0..num_pixels {
        let pixel = bytes_to_pixel(&image.data[i * pixel_stride..], format);
        pixel_mask[i] = pixel.3 != 0;
    }

    pixel_mask
}

pub(crate) fn combine_texture_sets(input_sets: &[InputTextureSet], config: &ProcessConfig) -> Result<()> {
    // Assumptions.
    for texture_set in input_sets {
        assert!(texture_set.textures.len() > 0);
        assert!(texture_set.textures[0].is_some());
    }

    // Pixel mask for each texture set.
    let mut set_masks = vec![];
    let mut working_res = (0u32, 0u32);

    // Compute masks for each texture set.
    for input_set in input_sets {
        let file_name = input_set.textures[0].as_ref().expect("the first texture of the set was not present");
        let image = read_image_from_file(&file_name)?;
        let image_format = &image.format;
        let image_size = (image_format.width, image_format.height);

        // Need alpha channel for mask
        if image_format.color_type != ColorType::Rgba {
            log_error!(
                "The image '{}' needs to have an alpha channel in order for a mask to be computed.",
                &file_name
            );
            panic!();
        }

        if image_size == (0, 0) {
            log_error!("The image '{}' is zero sized.", &file_name);
            panic!();
        }

        if working_res == (0, 0) {
            working_res = image_size;
        } else {
            if image_size != working_res {
                log_error!(
                    "The image '{}' does not have the same resolution {:?} as the previous image(s) {:?}.",
                    &file_name,
                    image_size,
                    working_res
                );
                panic!();
            }
        }

        let mask = create_mask_from_alpha_channel(&image);
        set_masks.push(mask);
    }

    if config.output_masks {
        // Write masks to files for debugging.
        for (i, mask) in set_masks.iter().enumerate() {
            let num_pixels = mask.len();
            let format = ImageFormat {
                width: working_res.0,
                height: working_res.1,
                bit_depth: BitDepth::Eight,
                color_type: ColorType::Rgba,
            };
            let stride = calc_pixel_stride(&format);
            let mut buffer = vec![0u8; num_pixels * stride];

            for i in 0..num_pixels {
                let pixel = if mask[i] {
                    Pixel(255, 255, 255, 255)
                } else {
                    Pixel(0, 0, 0, 255)
                };
                pixel_to_bytes(pixel, &format, &mut buffer[i * stride..]);
            }

            let filename = format!("{}/mask{}.png", config.output_directory.to_string_lossy(), i);
            write_image_to_file(
                &filename,
                &RawImage {
                    data: buffer,
                    format,
                },
            )
            .unwrap_or_else(|err| {
                log_error!("Failed to write mask to file '{}': {:?}", filename, err);
            });
        }
    }

    // Combine all the image sets into the output files.
    for (suffix_index, suffix) in config.suffixes.iter().enumerate() {
        let mut output_image: Option<RawImage> = None;
        let mut first = true;

        for (set_index, input_set) in input_sets.iter().enumerate() {
            // Grab the texture filename if it exists.
            let texture_filename;
            if let Some(filename) = &input_set.textures[suffix_index] {
                texture_filename = filename;
            } else {
                continue;
            }

            let image = read_image_from_file(texture_filename)?;
            let format = &image.format;
            let is_mask_source_image = suffix_index == 0;

            if let Some(raw_output_image) = &output_image {
                // Output image has already been created. Validate the current image's format
                // against the output image's one.

                let output_format = &raw_output_image.format;
                let output_size = (output_format.width, output_format.height);
                let input_size = (format.width, format.height);

                if input_size != output_size {
                    log_error!(
                        "The image '{}' does not have the same resolution {:?} as the previous image(s) {:?}.",
                        &texture_filename,
                        input_size,
                        output_size
                    );
                    panic!();
                }

                if format.bit_depth != output_format.bit_depth {
                    log_error!(
                        "The image '{}' does not have the same bit-depth ({:?}) as the previous image(s) ({:?}).",
                        &texture_filename,
                        format.bit_depth,
                        output_format.bit_depth,
                    );
                    panic!();
                }

                if format.color_type != output_format.color_type {
                    match (format.color_type, output_format.color_type) {
                        (ColorType::Rgb, ColorType::Rgba) => {} // ok
                        (ColorType::Rgba, ColorType::Rgb) => {
                            // alpha is lost
                            if is_mask_source_image && !config.keep_mask_alpha {
                                // ok: desired behaviour
                            } else {
                                log_warn!("Encountered an unexpected alpha channel in image '{}', it will be discarded as the previous texture(s) did not have one.", &texture_filename);
                            }
                        }
                        _ => {
                            log_error!(
                                "The image '{}' has an unsupported color type ({:?})",
                                &texture_filename,
                                format.color_type
                            );
                            panic!();
                        }
                    }
                }
            } else {
                // Create the output image using the format of the current image.

                let mut output_format = *format;
                if is_mask_source_image && !config.keep_mask_alpha {
                    output_format.color_type = ColorType::Rgb;
                }

                let buffer_size = format.width as usize
                    * format.height as usize
                    * calc_pixel_stride(&output_format);

                output_image = Some(RawImage {
                    data: vec![0; buffer_size],
                    format: output_format,
                });
            }

            if first {
                // For the first image in the set we just copy the image without masking to get a nice background color for the output image.
                copy_image(&image, output_image.as_mut().unwrap());
                first = false;
            } else {
                let mask = &set_masks[set_index];
                copy_image_masked(&image, output_image.as_mut().unwrap(), &mask);
            }
        }

        if let Some(image) = &output_image {
            let mut output_file_path = PathBuf::new();
            output_file_path.push(&config.output_directory);
            // NOTE: If output_texture_name contains a '/' or '\', this could lead to unexpected results.
            output_file_path.push(format!("{}{}", &config.output_texture_name.to_string_lossy(), suffix));
            output_file_path.set_extension("png");

            let output_file = output_file_path.to_str().unwrap();
            log_info!("{}", output_file);
            write_image_to_file(output_file, &image)?;
        }
    }

    Ok(())
}

fn copy_image_masked(source_image: &RawImage, dest_image: &mut RawImage, mask: &[bool]) {
    assert_eq!(
        (source_image.format.width, source_image.format.height),
        (dest_image.format.width, dest_image.format.height),
        "image dimension mismatch"
    );

    let num_pixels = source_image.format.width as usize * source_image.format.height as usize;
    let source_stride = source_image.data.len() / num_pixels;
    let dest_stride = dest_image.data.len() / num_pixels;

    assert_eq!(
        mask.len(),
        num_pixels,
        "mask size does not match image size"
    );

    for i in 0..num_pixels {
        if mask[i] {
            let pixel = bytes_to_pixel(
                &source_image.data[i * source_stride..],
                &source_image.format,
            );
            pixel_to_bytes(
                pixel,
                &dest_image.format,
                &mut dest_image.data[i * dest_stride..],
            );
        }
    }
}

fn copy_image(source_image: &RawImage, dest_image: &mut RawImage) {
    if source_image.format == dest_image.format {
        // Fast path: same format, just a memcpy.
        assert_eq!(
            source_image.data.len(),
            dest_image.data.len(),
            "image size mismatch"
        );
        dest_image.data.copy_from_slice(&source_image.data);
    } else {
        // Slow path: different format, need to convert.
        assert_eq!(
            (source_image.format.width, source_image.format.height),
            (dest_image.format.width, dest_image.format.height),
            "image dimension mismatch"
        );

        let num_pixels = source_image.format.width as usize * source_image.format.height as usize;
        let source_stride = source_image.data.len() / num_pixels;
        let dest_stride = dest_image.data.len() / num_pixels;

        for i in 0..num_pixels {
            let pixel = bytes_to_pixel(
                &source_image.data[i * source_stride..],
                &source_image.format,
            );
            pixel_to_bytes(
                pixel,
                &dest_image.format,
                &mut dest_image.data[i * dest_stride..],
            );
        }
    }
}
