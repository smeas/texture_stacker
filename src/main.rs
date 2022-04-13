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

#[derive(Debug)]
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

fn calc_stride(format: &ImageFormat) -> usize {
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

fn apply_image(
    src: &[u8],
    dst: &mut [u8],
    src_format: &ImageFormat,
    dst_format: &ImageFormat,
    tex_type: TextureType,
) {
    let src_stride = calc_stride(src_format);
    let dst_stride = calc_stride(dst_format);

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

    stack_images(&[
        "TEST/InputFiles/T_CarPlayerBody_D.png",
        "TEST/InputFiles/T_CarPlayerDoors_D.png",
        "TEST/InputFiles/T_CarPlayerTires_D.png",
        "TEST/InputFiles/T_CarPlayerWings_D.png",
    ], "TEST/T_CarPlayer_D.png", TextureType::Color).unwrap();

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
