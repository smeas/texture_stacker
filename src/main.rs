use std::{error::Error, fs::File, io::BufWriter, time::Instant};

use png::{BitDepth, ColorType};

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
enum TextureType {
    Color,
    NormalMap,
    //ChannelPacked,
}

struct RawImage {
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

type Result<T> = std::result::Result<T, Box<dyn Error>>;

// enum ProcessingError {
// }

fn read_image_from_file(file_name: &str) -> Result<RawImage> {
    let infile = File::open(&file_name)?;
    let decoder = png::Decoder::new(infile);

    let mut reader = decoder.read_info()?;
    let mut buffer = vec![0; reader.output_buffer_size()];

    let info = reader.next_frame(&mut buffer)?;
    let bytes = &buffer[..info.buffer_size()];

    if reader.info().is_animated() {
        panic!("APNG is not supported");
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
        _ => panic!(),
    }
}

#[derive(PartialEq, Eq)]
struct Pixel(u8, u8, u8, u8);

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
        _ => panic!(),
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
        _ => panic!(),
    }
}





fn create_mask_from_alpha_channel(image: &RawImage) -> Vec<bool> {
    let format = &image.format;

    assert_eq!(format.color_type, ColorType::Rgba);

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


fn apply_image(
    src: &[u8],
    dst: &mut [u8],
    src_format: &ImageFormat,
    dst_format: &ImageFormat,
    tex_type: TextureType,
) {
    let src_stride = calc_pixel_stride(src_format);
    let dst_stride = calc_pixel_stride(dst_format);

    dbg!(src_stride);
    dbg!(dst_stride);

    assert_eq!(src.len() / src_stride, dst.len() / dst_stride);
    assert_eq!(tex_type, TextureType::Color);

    for i in 0..(dst.len() / dst_stride) {
        let src_pixel = bytes_to_pixel(&src[i * src_stride..], src_format);
        let dst_pixel = bytes_to_pixel(&dst[i * dst_stride..], dst_format);

        if dst_pixel == Pixel(0, 0, 0, 255) || dst_pixel == Pixel(0, 0, 0, 0) {
            pixel_to_bytes(src_pixel, dst_format, &mut dst[i * dst_stride..]);
        }
    }
}

fn stack_images(input_files: &[&str], output_file: &str, tex_type: TextureType) -> Result<()> {
    assert!(input_files.len() >= 1);

    let mut buffer;
    let buffer_format;

    // Load the first image into the buffer.
    {
        let image = read_image_from_file(input_files[0])?;

        assert!(image.format.bit_depth == BitDepth::Eight);
        assert!(
            image.format.color_type == ColorType::Rgb || image.format.color_type == ColorType::Rgba
        );

        buffer = image.data;
        buffer_format = image.format;
    }

    for infile in input_files {
        let image = read_image_from_file(infile)?;

        apply_image(
            &image.data,
            &mut buffer,
            &image.format,
            &buffer_format,
            tex_type,
        );
    }

    let output_image = RawImage {
        data: buffer,
        format: buffer_format,
    };
    write_image_to_file(output_file, &output_image)?;

    Ok(())
}


fn combine_texture_sets(input_sets: &[&[&str]], output_files: &[&str]) -> Result<()> {
    // Pixel mask for each texture set.
    let mut set_masks = vec![];
    let mut working_res = (0u32, 0u32);

    // Compute masks for each texture set.
    for input_set in input_sets {
        let image = read_image_from_file(input_set[0])?;

        assert_eq!(image.format.color_type, ColorType::Rgba); // need alpha channel for mask
        assert!(image.format.width > 0 && image.format.height > 0);

        if working_res == (0, 0) {
            working_res = (image.format.width, image.format.height);
        } else {
            assert_eq!(working_res, (image.format.width, image.format.height));
        }

        let mask = create_mask_from_alpha_channel(&image);
        set_masks.push(mask);
    }

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
            let pixel = if mask[i] { Pixel(255, 255, 255, 255) } else { Pixel(0, 0, 0, 255) };
            pixel_to_bytes(pixel, &format, &mut buffer[i * stride..]);
        }

        write_image_to_file(&format!("TEST2/Result/mask{}.png", i), &RawImage {data: buffer, format})?;
    }

    // Combine all the image sets into the output files.
    for (texture_index, output_file) in output_files.iter().enumerate() {
        let mut output_image: Option<RawImage> = None;

        for (set_index, input_set) in input_sets.iter().enumerate() {
            let image = read_image_from_file(input_set[texture_index])?;
            let format = &image.format;

            if let Some(raw_output_image) = &output_image {
                assert_eq!(&raw_output_image.format, format);
            } else {
                let buffer_size = format.width as usize * format.height as usize * calc_pixel_stride(format);
                output_image = Some(RawImage {
                    data: vec![0; buffer_size],
                    format: *format,
                });
            }

            if set_index == 0 {
                // For the first image in the set we just copy the image without masking to get a nice background color for the output image.
                copy_image(&image, output_image.as_mut().unwrap());
            } else {
                let mask = &set_masks[set_index];
                copy_image_masked(&image, output_image.as_mut().unwrap(), &mask);
            }
        }

        write_image_to_file(output_file, &output_image.expect("TODO: no image"))?;
    }

    Ok(())
}

fn copy_image_masked(source_image: &RawImage, dest_image: &mut RawImage, mask: &[bool]) {
    assert_eq!(source_image.format.width, dest_image.format.width);
    assert_eq!(source_image.format.height, dest_image.format.height);

    let num_pixels = source_image.format.width as usize * source_image.format.height as usize;
    let source_stride = source_image.data.len() / num_pixels;
    let dest_stride = dest_image.data.len() / num_pixels;

    assert_eq!(mask.len(), num_pixels);

    for i in 0..num_pixels {
        if mask[i] {
            let pixel = bytes_to_pixel(&source_image.data[i * source_stride..], &source_image.format);
            pixel_to_bytes(pixel, &dest_image.format, &mut dest_image.data[i * dest_stride..]);
        }
    }
}

fn copy_image(source_image: &RawImage, dest_image: &mut RawImage) {
    if source_image.format == dest_image.format {
        // Fast path: same format, just a memcpy.
        assert_eq!(source_image.data.len(), dest_image.data.len());
        dest_image.data.copy_from_slice(&source_image.data);
    } else {
        // Slow path: different format, need to convert.
        assert_eq!(source_image.format.width, dest_image.format.width);
        assert_eq!(source_image.format.height, dest_image.format.height);

        let num_pixels = source_image.format.width as usize * source_image.format.height as usize;
        let source_stride = source_image.data.len() / num_pixels;
        let dest_stride = dest_image.data.len() / num_pixels;

        for i in 0..num_pixels {
            let pixel = bytes_to_pixel(&source_image.data[i * source_stride..], &source_image.format);
            pixel_to_bytes(pixel, &dest_image.format, &mut dest_image.data[i * dest_stride..]);
        }
    }
}


fn main() {
    println!("Hello, world!");

    // let infile = File::open("TEST/InputFiles/T_CarPlayerBody_D.png").unwrap();
    // let decoder = png::Decoder::new(infile);

    // let mut reader = decoder.read_info().unwrap();
    // let mut buf = vec![0; reader.output_buffer_size()];

    // let info = reader.next_frame(&mut buf).unwrap();
    // dbg!(info.width);
    // dbg!(info.height);
    // dbg!(info.bit_depth);
    // dbg!(info.color_type);

    // let bytes = &buf[..info.buffer_size()];
    //dbg!(bytes);

    // let image = read_image_from_file("TEST/InputFiles/T_CarPlayerBody_N.png");
    // dbg!(&image.format);
    // dbg!(&image.data[0..16]);

    // write_image_to_file("TEST/test.png", &image).unwrap();

    let start_time = Instant::now();

    // stack_images(&[
    //     "TEST/InputFiles/T_CarPlayerBody_D.png",
    //     "TEST/InputFiles/T_CarPlayerDoors_D.png",
    //     "TEST/InputFiles/T_CarPlayerTires_D.png",
    //     "TEST/InputFiles/T_CarPlayerWings_D.png",
    // ], "TEST/T_CarPlayer_D.png", TextureType::Color).unwrap();

    combine_texture_sets(
        &[
            &[
                "TEST2/T_SM_CarPlayer_v03_CarBody_D.png",
                "TEST2/T_SM_CarPlayer_v03_CarBody_N.png",
                "TEST2/T_SM_CarPlayer_v03_CarBody_E.png",
                "TEST2/T_SM_CarPlayer_v03_CarBody_M.png",
            ],
            &[
                "TEST2/T_SM_CarPlayer_v03_Doors_D.png",
                "TEST2/T_SM_CarPlayer_v03_Doors_N.png",
                "TEST2/T_SM_CarPlayer_v03_Doors_E.png",
                "TEST2/T_SM_CarPlayer_v03_Doors_M.png",
            ],
            &[
                "TEST2/T_SM_CarPlayer_v03_Tires_D.png",
                "TEST2/T_SM_CarPlayer_v03_Tires_N.png",
                "TEST2/T_SM_CarPlayer_v03_Tires_E.png",
                "TEST2/T_SM_CarPlayer_v03_Tires_M.png",
            ],
            &[
                "TEST2/T_SM_CarPlayer_v03_Wings_D.png",
                "TEST2/T_SM_CarPlayer_v03_Wings_N.png",
                "TEST2/T_SM_CarPlayer_v03_Wings_E.png",
                "TEST2/T_SM_CarPlayer_v03_Wings_M.png",
            ],
        ],
        &[
            "TEST2/Result/T_CarPlayer_D.png",
            "TEST2/Result/T_CarPlayer_N.png",
            "TEST2/Result/T_CarPlayer_E.png",
            "TEST2/Result/T_CarPlayer_M.png",
        ],
    ).unwrap();

    println!("Finished in {} s", start_time.elapsed().as_secs_f32());
}

#[cfg(test)]
mod tests {
    use crate::{
        apply_image, read_image_from_file, write_image_to_file, ImageFormat, RawImage, TextureType,
    };

    #[test]
    fn asd() {
        let image = read_image_from_file("TEST/InputFiles/T_CarPlayerBody_D.png").unwrap();

        let mut dst_buffer =
            vec![0; image.format.width as usize * image.format.height as usize * 4];
        let dst_format = ImageFormat {
            width: image.format.width,
            height: image.format.height,
            bit_depth: image.format.bit_depth,
            color_type: png::ColorType::Rgba,
        };

        apply_image(
            &image.data,
            &mut dst_buffer,
            &image.format,
            &dst_format,
            TextureType::Color,
        );

        //assert_eq!(&image.data, &dst_buffer);

        let dst_image = RawImage {
            data: dst_buffer,
            format: dst_format,
        };
        write_image_to_file("TEST/result.png", &dst_image).unwrap();
    }
}
